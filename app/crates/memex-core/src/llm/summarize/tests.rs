use super::parse::{build_prompt, extract_first_sentence, parse_summary};
use super::period::{PeriodBudget, PeriodKind, classify_period, condense_for_period};
use super::{
    MAX_INPUT_CHARS, SessionSummary, summarize_chunk, summarize_period, summarize_project,
};
use crate::llm::provider::{LlmProvider, LlmRequest, LlmResponse};
use crate::locale::PromptLocale;
use anyhow::Result;

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
    let prompt = build_prompt(&messages, None, PromptLocale::Zh);
    assert!(prompt.len() <= MAX_INPUT_CHARS + 100);
}

/// 当 collector 给出 current_project_path 时，build_prompt 应把它嵌进 prompt，
/// 让 LLM 知道这是一个待校验的路径而非完全凭空推断。
#[test]
fn test_build_prompt_embeds_current_project_path_when_present() {
    let messages = vec![("user".into(), "hello".into())];
    let with_path = build_prompt(
        &messages,
        Some("/Users/me/Documents/tt-demo/src"),
        PromptLocale::Zh,
    );
    assert!(with_path.contains("/Users/me/Documents/tt-demo/src"));
    assert!(with_path.contains("corrected_project_path"));
}

/// 空字符串 path 应当被视作"没有给"，不污染 prompt（避免占用 token）。
#[test]
fn test_build_prompt_skips_empty_current_project_path() {
    let messages = vec![("user".into(), "hello".into())];
    let without_path = build_prompt(&messages, Some(""), PromptLocale::Zh);
    assert!(!without_path.contains("corrected_project_path"));
    assert!(!without_path.contains("当前 collector"));
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
            corrected_project_path: None,
            intent: None,
        },
        SessionSummary {
            title: "Add adapters".into(),
            summary: "Added Claude and Cursor adapters.".into(),
            topics: vec!["adapters".into()],
            decisions: vec![],
            project_name: None,
            corrected_project_path: None,
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
        corrected_project_path: None,
        intent: None,
    }];
    let result = summarize_period(&provider, "2026-06-01", &sessions).unwrap();
    assert!(result.title.contains("Daily Report"));
}

#[test]
fn test_classify_period_kinds() {
    assert_eq!(classify_period("Monthly 2026-06"), PeriodKind::Monthly);
    assert_eq!(classify_period("Weekly 2026-W23"), PeriodKind::Weekly);
    assert_eq!(classify_period("Week 2026-W23"), PeriodKind::Weekly);
    assert_eq!(classify_period("Daily 2026-06-08"), PeriodKind::Daily);
    assert_eq!(classify_period("2026-06-08"), PeriodKind::Daily);
    // monthly 必须先于 daily/weekly 判断
    assert_eq!(classify_period("Monthly Weekly"), PeriodKind::Monthly);
}

#[test]
fn test_period_budget_monthly_is_largest() {
    let m = PeriodBudget::for_kind(PeriodKind::Monthly);
    let w = PeriodBudget::for_kind(PeriodKind::Weekly);
    let d = PeriodBudget::for_kind(PeriodKind::Daily);
    assert!(m.sessions_per_group > w.sessions_per_group);
    assert!(w.sessions_per_group >= d.sessions_per_group);
    assert!(m.max_summary_chars > w.max_summary_chars);
    assert!(m.min_words >= 1500);
    assert!(m.max_tokens >= w.max_tokens);
}

/// 验证 condense 在月度预算下能保留更多 session 内容、按 (project, topic) 二级分组。
#[test]
fn test_condense_for_period_monthly_uses_project_grouping() {
    let mk = |title: &str, project: Option<&str>, topic: &str| SessionSummary {
        title: title.into(),
        summary: format!("详细工作 ({})。", title).repeat(50),
        topics: vec![topic.into()],
        decisions: vec![format!("decision for {}", title)],
        project_name: project.map(String::from),
        corrected_project_path: None,
        intent: None,
    };
    // 两个 project 共用 "bug" 这个 topic 应该被分到两个组里
    let summaries = vec![
        mk("memex 修复 popup", Some("memex"), "bug"),
        mk("memex 修复 sidebar", Some("memex"), "bug"),
        mk("paikebao 修复登录", Some("tt-paikebao-mp"), "bug"),
        mk("无项目随手改", None, "misc"),
    ];
    let budget = PeriodBudget::for_kind(PeriodKind::Monthly);
    let condensed = condense_for_period(&summaries, &budget, PromptLocale::Zh);
    // 应至少 3 组：memex · bug / tt-paikebao-mp · bug / misc
    assert!(condensed.len() >= 3, "got {} groups", condensed.len());
    let joined = condensed.join("\n");
    assert!(joined.contains("memex · bug"));
    assert!(joined.contains("tt-paikebao-mp · bug"));
    // monthly 预算下单组 summary 上限 1500，应该容得下叠加文本（50× ≈ 800-1200 字符）
    assert!(joined.len() > 1000);
}
