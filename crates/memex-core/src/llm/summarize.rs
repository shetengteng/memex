use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::provider::{LlmProvider, LlmRequest};

const SUMMARY_SYSTEM: &str = "\
你是一位面向技术开发场景的会话摘要助手。输入是用户与 AI 助手的一段编程对话。\
请生成结构化摘要。

输出严格合法的 JSON（不带 ```json 标记），包含以下字段：
- title (string): 一行标题，不超过 60 字符，概括核心工作
- summary (string): 2-4 句话，说明完成了什么、解决了什么问题、做了哪些关键决策
- intent (string|null): 用一句不超过 60 字符的中文，概括用户在本次会话中**真正想达成的目标**\
  （不是助手的执行过程，也不是表面问题）。如：\"修复 popup 列表中 intent 字段不显示\"、\
  \"调研为什么周报里出现了 Gemini 字样\"。无法确定时输出 null。
- topics (string[]): 1-5 个主题关键词
- decisions (string[]): 0-3 个关键技术决策，每条是纯字符串
- project_name (string|null): 从对话中推断出的项目名称。根据文件路径、代码仓库、\
  package.json/Cargo.toml 中出现的项目标识来判断。输出最后一级有意义的目录名即可\
  （如 \"memex\"、\"tt-paikebao-mp\"）。如果无法确定则输出 null

语言：所有自然语言使用简体中文。技术标识保持原样（文件路径、命令名、函数名、缩写）。";

const CHUNK_SUMMARY_SYSTEM: &str = "\
你是一个面向技术开发场景的文本摘要助手。输入是编程会话中的一段文本，\
请用一句话（简体中文，不超过 120 字符）抓住核心信息。\
保持技术标识原样：文件路径、命令、代码符号不要翻译。\
只输出这一句话，不要带引号、markdown 或任何额外格式。";

const PROJECT_SUMMARY_SYSTEM: &str = "\
你是一个项目进展摘要助手。输入是同一个项目内多个会话的摘要，\
请生成项目级别的总览。输出 JSON，字段如下：
- title: 项目名或一行标题，不超过 60 个字符
- summary: 用 3-5 句话概括项目当前的进展、关键状态
- topics: 1-8 个覆盖所有会话的主题关键词数组
- decisions: 0-5 个关键架构/技术决策数组
所有自然语言字段都必须使用简体中文，无论输入语言是什么。\
保持技术标识原样：文件路径、命令名、函数名、英文缩写（SQL/HTTP/API 等）不要翻译。\
只输出合法 JSON，不要带 markdown 代码块标记。";

const PERIODIC_SUMMARY_SYSTEM: &str = "\
你是一位资深工程师的工作报告撰写助手。你的任务是把输入的多个会话摘要合并成一份详细报告。

输出要求：一个合法 JSON 对象（不要 ```json 标记），包含 3 个字段：

{
  \"summary\": \"【Memex 桌面应用】完成了 popup UI 的 shadcn 风格重构，替换了所有非 shadcn-vue 组件，修复了 searchInputRef 绑定到 Vue 组件实例而非原生 DOM 元素导致的 focus 报错。通过 $el 访问底层 HTMLInputElement 解决了问题。\\n\\n【LLM 集成】将 max_tokens 从 512 提升到 2048/4096，解决了 DeepSeek V4 Flash 因 reasoning chain 耗尽 token 导致 content 为空的问题。添加了空响应检测和 parse_summary 的 fallback 保护。\\n\\n【Bug 修复】排查了 Dashboard 白屏问题，根因是已安装的 Memex.app 与 dev server 端口冲突，通过关闭旧进程解决。\",
  \"topics\": [\"Memex\", \"LLM\", \"UI重构\", \"Bug修复\"],
  \"decisions\": [\"选择 $el 方式访问原生 DOM 而非重写组件\", \"max_tokens 按场景分档：默认 2048，报告 4096\"]
}

summary 字段的硬性要求（非常重要，必须遵守）：
1. 按【主题名】分段，每段之间用 \\n\\n 分隔
2. 每个主题写 3-5 句话：做了什么 + 为什么这样做 + 达成什么结果
3. 日报 summary 最少 200 字，周报 summary 最少 500 字
4. Bug 修复要写根因和解决方案
5. 要具体到文件名、函数名、技术细节，不要笼统概括

topics: 5-10 个关键词
decisions: 3-8 条技术决策，每条是一句完整中文
不要输出 title 字段，title 由系统自动生成。

语言：中文。技术标识保持原样（路径、命令、函数名）。";

const MAX_INPUT_CHARS: usize = 8000;
const MAX_PERIOD_INPUT_CHARS: usize = 16000;
const MIN_CHUNK_CHARS_FOR_SUMMARY: usize = 200;
const L1_BATCH_SIZE: usize = 10;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub title: String,
    pub summary: String,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    #[serde(default)]
    pub project_name: Option<String>,
    /// L2 摘要新增字段：用户在本次会话中真正想达成的目标，一句话。
    /// L3 / L4 项目级与周期级摘要不强制要求 LLM 输出此字段，因此默认 None。
    #[serde(default)]
    pub intent: Option<String>,
}

pub fn summarize_session(
    provider: &dyn LlmProvider,
    messages: &[(String, String)],
) -> Result<SessionSummary> {
    let prompt = build_prompt(messages);
    let request = LlmRequest::with_prompt(prompt).with_system(SUMMARY_SYSTEM);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub fn summarize_chunk(provider: &dyn LlmProvider, content: &str) -> Result<String> {
    if content.len() < MIN_CHUNK_CHARS_FOR_SUMMARY {
        return Ok(extract_first_sentence(content, 120));
    }
    let truncated = if content.len() > 2000 {
        format!("{}... (truncated)", &content[..content.floor_char_boundary(2000)])
    } else {
        content.to_string()
    };
    let prompt = format!("请用一句话总结以下内容：\n\n{}", truncated);
    let request = LlmRequest::with_prompt(prompt).with_system(CHUNK_SUMMARY_SYSTEM);
    match provider.generate(&request) {
        Ok(response) => {
            let s = response.text.trim().trim_matches('"').to_string();
            Ok(if s.len() > 120 {
                s.chars().take(120).collect()
            } else {
                s
            })
        }
        Err(_) => Ok(extract_first_sentence(content, 120)),
    }
}

pub fn summarize_project(
    provider: &dyn LlmProvider,
    session_summaries: &[SessionSummary],
) -> Result<SessionSummary> {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str("以下是同一个项目的多个会话摘要：\n\n");
    for (i, s) in session_summaries.iter().enumerate() {
        let entry = format!(
            "会话 {}：{}\n  摘要：{}\n  主题：{}\n\n",
            i + 1,
            s.title,
            s.summary,
            s.topics.join("、")
        );
        if prompt.len() + entry.len() > MAX_INPUT_CHARS {
            break;
        }
        prompt.push_str(&entry);
    }
    prompt.push_str("请输出一个项目级总览的 JSON。");
    let request = LlmRequest::with_prompt(prompt).with_system(PROJECT_SUMMARY_SYSTEM);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

pub fn summarize_period(
    provider: &dyn LlmProvider,
    period_label: &str,
    session_summaries: &[SessionSummary],
) -> Result<SessionSummary> {
    let condensed = condense_for_period(session_summaries);

    let mut prompt = String::with_capacity(MAX_PERIOD_INPUT_CHARS);
    prompt.push_str(&format!(
        "以下是 {} 期间的工作会话摘要（共 {} 个会话）：\n\n",
        period_label,
        session_summaries.len()
    ));
    let mut included = 0usize;
    for entry in &condensed {
        if prompt.len() + entry.len() > MAX_PERIOD_INPUT_CHARS {
            break;
        }
        prompt.push_str(entry);
        included += 1;
    }
    if included < condensed.len() {
        prompt.push_str(&format!(
            "（还有 {} 组工作因篇幅限制未列出）\n\n",
            condensed.len() - included
        ));
    }
    let is_weekly = period_label.contains("Week") || period_label.contains("weekly");
    let min_words = if is_weekly { 500 } else { 200 };
    prompt.push_str(&format!(
        "\n请综合以上 {} 个会话，生成一个 JSON 对象。\
         summary 必须按【主题名】分段，每段 3-5 句话，总长度不少于 {} 字。\
         涵盖所有主要主题，写出具体技术细节，不要笼统概括。",
        session_summaries.len(),
        min_words
    ));
    let request = LlmRequest::with_prompt(prompt)
        .with_system(PERIODIC_SUMMARY_SYSTEM)
        .with_max_tokens(4096);
    let response = provider.generate(&request)?;
    parse_summary(&response.text)
}

fn condense_for_period(summaries: &[SessionSummary]) -> Vec<String> {
    use std::collections::BTreeMap;
    let mut by_topic: BTreeMap<String, Vec<&SessionSummary>> = BTreeMap::new();

    for s in summaries {
        let key = s.topics.first().cloned().unwrap_or_else(|| "其他".to_string());
        by_topic.entry(key).or_default().push(s);
    }

    let mut entries = Vec::new();
    for (topic, group) in &by_topic {
        let titles: Vec<&str> = group.iter().take(10).map(|s| s.title.as_str()).collect();
        let all_decisions: Vec<&str> = group
            .iter()
            .flat_map(|s| s.decisions.iter().map(|d| d.as_str()))
            .take(8)
            .collect();
        let summaries_text: String = group
            .iter()
            .take(8)
            .map(|s| s.summary.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let max_summary_len = 500;
        let mut entry = format!(
            "【{}】（{} 个会话）\n  代表性工作：{}\n  详细内容：{}\n",
            topic,
            group.len(),
            titles.join("、"),
            if summaries_text.len() > max_summary_len {
                format!("{}...", &summaries_text[..summaries_text.floor_char_boundary(max_summary_len)])
            } else {
                summaries_text
            }
        );
        if !all_decisions.is_empty() {
            entry.push_str(&format!("  技术决策：{}\n", all_decisions.join("；")));
        }
        entry.push('\n');
        entries.push(entry);
    }
    entries
}

pub const fn min_chunk_chars() -> usize {
    MIN_CHUNK_CHARS_FOR_SUMMARY
}

pub const fn l1_batch_size() -> usize {
    L1_BATCH_SIZE
}

fn build_prompt(messages: &[(String, String)]) -> String {
    let mut prompt = String::with_capacity(MAX_INPUT_CHARS);
    prompt.push_str("以下是一段对话：\n\n");

    let mut total_len = prompt.len();
    for (role, content) in messages {
        let header = format!("[{}]：", role);
        let truncated = if content.len() > 1000 {
            let end = content.char_indices()
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

fn parse_summary(text: &str) -> Result<SessionSummary> {
    if text.trim().len() < 10 {
        anyhow::bail!("LLM returned too-short response ({} chars), cannot parse summary", text.len());
    }

    let cleaned = strip_code_fences(text);

    if let Ok(mut summary) = serde_json::from_str::<SessionSummary>(&cleaned) {
        if !summary.summary.is_empty() {
            // 即便走快速分支，也把 intent 的空白 / 空字符串规范化成 None，
            // 与 extract_summary_from_value 的行为保持一致 —— 否则
            // UI 会出现 intent === "" 这种意义不明的脏数据。
            summary.intent = summary
                .intent
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            return Ok(summary);
        }
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
                serde_json::Value::Object(obj) => {
                    obj.get("decision")
                        .or_else(|| obj.get("content"))
                        .or_else(|| obj.get("desc"))
                        .and_then(|v| v.as_str())
                        .map(String::from)
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    };

    let project_name = val.get("project_name")
        .and_then(|v| v.as_str())
        .map(String::from)
        .filter(|s| !s.is_empty());

    let intent = val.get("intent")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    SessionSummary { title, summary, topics, decisions, project_name, intent }
}

fn extract_first_sentence(text: &str, max_len: usize) -> String {
    let end = text.find('.').map(|i| i + 1).unwrap_or(text.len());
    let sentence: String = text.chars().take(end.min(max_len)).collect();
    sentence.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse};

    struct MockProvider {
        response: String,
    }

    impl MockProvider {
        fn new(response: &str) -> Self {
            Self {
                response: response.to_string(),
            }
        }
    }

    impl LlmProvider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }
        fn is_available(&self) -> bool {
            true
        }
        fn generate(&self, _request: &LlmRequest) -> Result<LlmResponse> {
            Ok(LlmResponse {
                text: self.response.clone(),
                model: "mock".into(),
                tokens_used: 10,
            })
        }
    }

    #[test]
    fn test_parse_summary_valid_json() {
        let json = r#"{"title":"Fix auth bug","summary":"Fixed JWT token parsing.","topics":["auth","jwt"],"decisions":["use RS256"]}"#;
        let s = parse_summary(json).unwrap();
        assert_eq!(s.title, "Fix auth bug");
        assert_eq!(s.topics.len(), 2);
    }

    #[test]
    fn test_parse_summary_with_fences() {
        let text =
            "```json\n{\"title\":\"Test\",\"summary\":\"S\",\"topics\":[],\"decisions\":[]}\n```";
        let s = parse_summary(text).unwrap();
        assert_eq!(s.title, "Test");
    }

    #[test]
    fn test_parse_summary_extracts_intent() {
        let json = r#"{
            "title": "Fix",
            "summary": "Fixed it.",
            "topics": ["bug"],
            "decisions": [],
            "intent": "修复登录失败的问题"
        }"#;
        let s = parse_summary(json).unwrap();
        assert_eq!(s.intent.as_deref(), Some("修复登录失败的问题"));
    }

    #[test]
    fn test_parse_summary_intent_missing_is_none() {
        let json = r#"{"title":"X","summary":"y","topics":[],"decisions":[]}"#;
        let s = parse_summary(json).unwrap();
        assert!(s.intent.is_none());
    }

    #[test]
    fn test_parse_summary_intent_empty_string_is_none() {
        let json = r#"{"title":"X","summary":"y","topics":[],"decisions":[],"intent":"  "}"#;
        let s = parse_summary(json).unwrap();
        assert!(s.intent.is_none(), "空字符串/纯空白应当视为 None");
    }

    #[test]
    fn test_parse_summary_fallback() {
        let text = "This is not valid JSON but a plain text response.";
        let s = parse_summary(text).unwrap();
        assert!(!s.title.is_empty());
    }

    #[test]
    fn test_parse_summary_object_decisions() {
        let json = r#"```json
{
    "title": "日报 2026-06-04",
    "summary": "完成了多项优化工作。",
    "topics": ["优化", "重构"],
    "decisions": [
        {"chapter": "架构", "decision": "选择 SQLite FTS5"},
        {"chapter": "性能", "decision": "用 sqlite_sequence 替代 COUNT(*)"}
    ]
}
```"#;
        let s = parse_summary(json).unwrap();
        assert_eq!(s.title, "日报 2026-06-04");
        assert_eq!(s.decisions.len(), 2);
        assert_eq!(s.decisions[0], "选择 SQLite FTS5");
        assert_eq!(s.decisions[1], "用 sqlite_sequence 替代 COUNT(*)");
        assert_eq!(s.topics.len(), 2);
    }

    #[test]
    fn test_build_prompt_truncation() {
        let messages: Vec<(String, String)> = (0..100)
            .map(|i| ("user".to_string(), format!("Message {} with content", i)))
            .collect();
        let prompt = build_prompt(&messages);
        assert!(prompt.len() <= MAX_INPUT_CHARS + 100);
    }

    #[test]
    fn test_extract_first_sentence() {
        assert_eq!(
            extract_first_sentence("Hello world. More stuff.", 60),
            "Hello world."
        );
        assert_eq!(extract_first_sentence("Short", 60), "Short");
    }

    #[test]
    fn test_summarize_chunk_short_content_no_llm() {
        let provider = MockProvider::new("should not be called");
        let short = "Quick fix applied.";
        let result = summarize_chunk(&provider, short).unwrap();
        assert_eq!(result, "Quick fix applied.");
    }

    #[test]
    fn test_summarize_chunk_uses_llm_for_long_content() {
        let provider = MockProvider::new("Implemented Redis caching layer for session data.");
        let long_content = "a]".repeat(200);
        let result = summarize_chunk(&provider, &long_content).unwrap();
        assert_eq!(result, "Implemented Redis caching layer for session data.");
    }

    #[test]
    fn test_summarize_chunk_truncates_long_response() {
        let provider = MockProvider::new(&"x".repeat(200));
        let content = "a".repeat(300);
        let result = summarize_chunk(&provider, &content).unwrap();
        assert!(result.len() <= 120);
    }

    #[test]
    fn test_summarize_project() {
        let provider = MockProvider::new(
            r#"{"title":"Memex Core","summary":"Built core features.","topics":["rust","search"],"decisions":["use FTS5"]}"#,
        );
        let sessions = vec![
            SessionSummary {
                title: "Add search".into(),
                summary: "Implemented FTS5 search.".into(),
                topics: vec!["search".into()],
                decisions: vec![],
                project_name: None,
                intent: None,
            },
            SessionSummary {
                title: "Add adapters".into(),
                summary: "Added Claude and Cursor adapters.".into(),
                topics: vec!["adapters".into()],
                decisions: vec![],
                project_name: None,
                intent: None,
            },
        ];
        let result = summarize_project(&provider, &sessions).unwrap();
        assert_eq!(result.title, "Memex Core");
        assert!(!result.topics.is_empty());
    }

    #[test]
    fn test_summarize_period() {
        let provider = MockProvider::new(
            r#"{"title":"Daily Report 2026-06-01","summary":"Worked on features.","topics":["dev"],"decisions":[]}"#,
        );
        let sessions = vec![SessionSummary {
            title: "Feature work".into(),
            summary: "Built new feature.".into(),
            topics: vec!["feature".into()],
            decisions: vec!["use trait pattern".into()],
            project_name: None,
            intent: None,
        }];
        let result = summarize_period(&provider, "2026-06-01", &sessions).unwrap();
        assert!(result.title.contains("Daily Report"));
    }
}
