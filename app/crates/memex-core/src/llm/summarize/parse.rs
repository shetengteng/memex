//! LLM 响应解析 + 用户消息 prompt 构造。
//!
//! 把 raw provider 输出转成结构化 [`SessionSummary`] 的逻辑都集中在这里，
//! 便于在不触碰编排代码的情况下，单独迭代解析容错（fence 剥离、对象/字符串
//! 二态 decisions、空白 intent 归一化等）。

use anyhow::Result;

use super::{MAX_INPUT_CHARS, SessionSummary};

pub(super) fn build_prompt(
    messages: &[(String, String)],
    current_project_path: Option<&str>,
) -> String {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);

    if let Some(path) = current_project_path.filter(|p| !p.is_empty()) {
        prompt.push_str(&format!(
            "当前 collector 推断的项目路径：{}\n\n请判断该路径是否漂移到了子目录\
             （如末段是 src / views / components / utils 等），若是则在 \
             corrected_project_path 字段输出修正后的完整路径；若路径已合理则输出 null。\n\n",
            path
        ));
    }

    prompt.push_str("以下是一段对话：\n\n");

    let mut total_len = prompt.len();
    for (role, content) in messages {
        let header = format!("[{}]：", role);
        let truncated = if content.len() > 1000 {
            let end = content
                .char_indices()
                .take_while(|(i, _)| *i < 1000)
                .last()
                .map(|(i, c)| i + c.len_utf8())
                .unwrap_or(content.len().min(1000));
            format!("{}…（已截断）", &content[..end])
        } else {
            content.clone()
        };
        let entry = format!("{}{}\n\n", header, truncated);

        if total_len + entry.len() > MAX_INPUT_CHARS {
            prompt.push_str("…（为节省篇幅省略了较早的消息）\n");
            break;
        }
        prompt.push_str(&entry);
        total_len += entry.len();
    }

    prompt.push_str("\n请把这段对话总结为 JSON。");
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

    Ok(SessionSummary {
        title: extract_first_sentence(text, 60),
        summary: text.chars().take(500).collect(),
        topics: Vec::new(),
        decisions: Vec::new(),
        project_name: None,
        corrected_project_path: None,
        intent: None,
    })
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
