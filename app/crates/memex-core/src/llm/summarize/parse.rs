//! LLM 响应解析 + 用户消息 prompt 构造。
//!
//! 把 raw provider 输出转成结构化 [`SessionSummary`] 的逻辑都集中在这里，
//! 便于在不触碰编排代码的情况下，单独迭代解析容错（fence 剥离、对象/字符串
//! 二态 decisions、空白 intent 归一化等）。

use anyhow::Result;
use tracing::warn;

use super::{MAX_INPUT_CHARS, SessionSummary};
use crate::locale::PromptLocale;

pub(super) fn build_prompt(
    messages: &[(String, String)],
    current_project_path: Option<&str>,
    loc: PromptLocale,
) -> String {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);

    let (path_intro, dialogue_intro, role_sep, truncated_mark, omitted_mark, footer) = match loc {
        PromptLocale::Zh => (
            "当前 collector 推断的项目路径：{path}\n\n请判断该路径是否漂移到了子目录\
             （如末段是 src / views / components / utils 等），若是则在 \
             corrected_project_path 字段输出修正后的完整路径；若路径已合理则输出 null。\n\n",
            "以下是一段对话：\n\n",
            "：",
            "…（已截断）",
            "…（为节省篇幅省略了较早的消息）\n",
            "\n请把这段对话总结为 JSON。",
        ),
        PromptLocale::En => (
            "Project path inferred by the collector: {path}\n\nCheck whether this \
             path drifted into a subdirectory (last segment looks like src / views / \
             components / utils etc.). If so, output the corrected full path in \
             `corrected_project_path`; otherwise output null.\n\n",
            "Below is a conversation:\n\n",
            ": ",
            "… (truncated)",
            "… (earlier messages omitted for brevity)\n",
            "\nSummarize this conversation as JSON.",
        ),
    };

    if let Some(path) = current_project_path.filter(|p| !p.is_empty()) {
        prompt.push_str(&path_intro.replace("{path}", path));
    }

    prompt.push_str(dialogue_intro);

    let mut total_len = prompt.len();
    for (role, content) in messages {
        let header = format!("[{}]{}", role, role_sep);
        let truncated = if content.len() > 1000 {
            let end = content
                .char_indices()
                .take_while(|(i, _)| *i < 1000)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(content.len().min(1000));
            format!("{}{}", &content[..end], truncated_mark)
        } else {
            content.clone()
        };
        let entry = format!("{}{}\n\n", header, truncated);

        if total_len + entry.len() > MAX_INPUT_CHARS {
            prompt.push_str(omitted_mark);
            break;
        }
        prompt.push_str(&entry);
        total_len += entry.len();
    }

    prompt.push_str(footer);
    prompt
}

pub(super) fn parse_summary(text: &str) -> Result<SessionSummary> {
    if text.trim().len() < 10 {
        anyhow::bail!(
            "LLM returned too-short response ({} chars), cannot parse summary",
            text.len()
        );
    }

    let cleaned = strip_code_fences(text);

    if let Ok(mut summary) = serde_json::from_str::<SessionSummary>(&cleaned)
        && !summary.summary.is_empty()
    {
        // 即便走快速分支，也把 intent 的空白 / 空字符串规范化成 None，
        // 与 extract_summary_from_value 的行为保持一致 —— 否则
        // UI 会出现 intent === "" 这种意义不明的脏数据。
        summary.intent = summary
            .intent
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        // corrected_project_path 在快速分支也要做绝对路径校验，避免 LLM 直接给短名。
        summary.corrected_project_path = summary
            .corrected_project_path
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty() && (s.starts_with('/') || s.starts_with("~/")));
        return Ok(summary);
    }

    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&cleaned) {
        let extracted = extract_summary_from_value(&val);
        if !extracted.summary.is_empty() {
            return Ok(extracted);
        }
    }

    // partial-JSON 救援：LLM 输出 token 用尽时整段 JSON 不闭合，前两个分支都失败。
    // 但 token 用完前 LLM 通常已经写完了 `"summary": "..."` 大半内容，从 cleaned
    // 文本里手工扫出这段 value 即可避免把整页用户工作丢掉。details 见
    // try_recover_summary_from_partial_json 的注释。
    if let Some(rescued) = try_recover_summary_from_partial_json(&cleaned)
        && rescued.trim().chars().count() >= 50
    {
        warn!(
            raw_chars = text.chars().count(),
            rescued_chars = rescued.chars().count(),
            "summary parse: recovered from partial JSON (likely max_tokens exhausted)"
        );
        return Ok(SessionSummary {
            title: extract_first_sentence(&rescued, 60),
            summary: rescued,
            topics: Vec::new(),
            decisions: Vec::new(),
            project_name: None,
            corrected_project_path: None,
            intent: None,
        });
    }

    // 最终 fallback：partial 救援也失败（LLM 输出连 "summary": 都没写完）。
    // 截到 800 字符（之前是 500，太短）+ 显式省略号，至少让 UI 能看出截断。
    warn!(
        raw_chars = text.chars().count(),
        "summary parse fell back to raw text — likely max_tokens exhausted and recovery failed"
    );
    let truncated_marker = match PromptLocale::current() {
        PromptLocale::Zh => "…（生成被截断）",
        PromptLocale::En => "… (generation truncated)",
    };
    let body: String = text.chars().take(800).collect();
    Ok(SessionSummary {
        title: extract_first_sentence(text, 60),
        summary: format!("{}{}", body, truncated_marker),
        topics: Vec::new(),
        decisions: Vec::new(),
        project_name: None,
        corrected_project_path: None,
        intent: None,
    })
}

/// 从 partial / 不闭合的 JSON 文本中救出 `summary` 字段值。
///
/// 用于 LLM 输出 token 用尽时的兜底：此时整段 JSON 的最后一个 `}` 通常缺失，
/// `serde_json::from_str` 与 `from_str::<Value>` 都会失败，但 LLM 在 token
/// 耗尽前往往已经写完了 `"summary": "..."` 的大半内容，把这段 value 抢救
/// 出来就能保住用户绝大部分工作记忆，而不是被 fallback 砍到 500 字符。
///
/// 实现策略（不引入第三方 partial-JSON parser，避免依赖膨胀）：
/// 1. 找到 `"summary"` key 的位置
/// 2. 跳过 `:` 与首个 `"`，定位 value 起点
/// 3. 从起点扫到下一个未转义的 `"` 作为 value 终点；若整段直到 EOF 都没有
///    收尾 `"`（最常见的 partial 形态：value 中间被截断），则把直到 EOF
///    的全部内容当作 value
/// 4. 反转义 JSON string 转义序列（`\"` / `\n` / `\t` / `\\`）
///
/// 任何一步定位失败都返回 None，让调用方回到最终 fallback 分支。
fn try_recover_summary_from_partial_json(cleaned: &str) -> Option<String> {
    let key_pos = cleaned.find("\"summary\"")?;
    let after_key = &cleaned[key_pos + "\"summary\"".len()..];
    let colon_pos = after_key.find(':')?;
    let after_colon = &after_key[colon_pos + 1..];
    let quote_open = after_colon.find('"')?;
    let body = &after_colon[quote_open + 1..];

    let bytes = body.as_bytes();
    let mut i = 0usize;
    let end = loop {
        if i >= bytes.len() {
            break bytes.len();
        }
        // 未转义的 close quote 才是真正的 value 结束。`\"` 是 JSON string
        // 转义，要继续扫；判断是否转义需要数前面连续 `\` 的奇偶性。
        if bytes[i] == b'"' && !is_escaped(bytes, i) {
            break i;
        }
        i += 1;
    };

    let raw = &body[..end];
    let unescaped = raw
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\");
    Some(unescaped)
}

/// 判断 bytes[idx] 处的字符是否被前置反斜杠转义。
/// JSON 中只有奇数个连续反斜杠才意味着转义（偶数个是字面量 `\\`）。
fn is_escaped(bytes: &[u8], idx: usize) -> bool {
    let mut count = 0usize;
    let mut j = idx;
    while j > 0 && bytes[j - 1] == b'\\' {
        count += 1;
        j -= 1;
    }
    count % 2 == 1
}

fn strip_code_fences(text: &str) -> String {
    let s = text.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        rest.trim_end_matches("```").trim().to_string()
    } else if let Some(rest) = s.strip_prefix("```") {
        rest.trim_end_matches("```").trim().to_string()
    } else {
        s.to_string()
    }
}

fn extract_summary_from_value(val: &serde_json::Value) -> SessionSummary {
    let title = val["title"].as_str().unwrap_or("").to_string();
    let summary = val["summary"].as_str().unwrap_or("").to_string();

    let topics = match val.get("topics") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    };

    let decisions = match val.get("decisions") {
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|v| match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Object(obj) => obj
                    .get("decision")
                    .or_else(|| obj.get("content"))
                    .or_else(|| obj.get("desc"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    let project_name = val
        .get("project_name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .filter(|s| !s.is_empty());

    // corrected_project_path 必须是绝对路径（防 LLM 误给短名）；空串 / 短名一律视为 None。
    let corrected_project_path = val
        .get("corrected_project_path")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && (s.starts_with('/') || s.starts_with("~/")));

    let intent = val
        .get("intent")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    SessionSummary {
        title,
        summary,
        topics,
        decisions,
        project_name,
        corrected_project_path,
        intent,
    }
}

pub(super) fn extract_first_sentence(text: &str, max_len: usize) -> String {
    let end = text.find('.').map(|i| i + 1).unwrap_or(text.len());
    let sentence: String = text.chars().take(end.min(max_len)).collect();
    sentence.trim().to_string()
}
