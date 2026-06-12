use super::cluster::{build_clustering_prompt, map_to_drafts, parse_thread_response};
use super::fallback::{fallback_cluster, fallback_query_match};
use crate::llm::summarize::SessionSummary;

fn s(title: &str, summary: &str, topics: &[&str]) -> SessionSummary {
    SessionSummary {
        title: title.into(),
        summary: summary.into(),
        topics: topics.iter().map(|t| (*t).into()).collect(),
        decisions: vec![],
        project_name: None,
        corrected_project_path: None,
        intent: None,
    }
}

fn sp(title: &str, summary: &str, topics: &[&str], project: &str) -> SessionSummary {
    SessionSummary {
        title: title.into(),
        summary: summary.into(),
        topics: topics.iter().map(|t| (*t).into()).collect(),
        decisions: vec![],
        project_name: Some(project.into()),
        corrected_project_path: None,
        intent: None,
    }
}

#[test]
fn parse_well_formed_json_into_drafts() {
    let batch = vec![
        ("s1".into(), s("a", "x", &["t"])),
        ("s2".into(), s("b", "y", &["t"])),
        ("s3".into(), s("c", "z", &["u"])),
    ];
    let llm_out = r#"{
        "threads": [
          {"name": "桌面化", "summary": "整体迁移", "session_indices": [1, 2]},
          {"name": "其他", "summary": "杂项", "session_indices": [3]}
        ]
    }"#;
    let parsed = parse_thread_response(llm_out).unwrap();
    let drafts = map_to_drafts(&parsed.threads, &batch);
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].name, "桌面化");
    assert_eq!(drafts[0].session_ids, vec!["s1", "s2"]);
    assert_eq!(drafts[1].name, "其他");
    assert_eq!(drafts[1].session_ids, vec!["s3"]);
}

#[test]
fn out_of_range_indices_are_dropped() {
    let batch = vec![("s1".into(), s("a", "x", &[]))];
    let llm_out = r#"{"threads":[{"name":"x","summary":"","session_indices":[0, 1, 2, 99]}]}"#;
    let parsed = parse_thread_response(llm_out).unwrap();
    let drafts = map_to_drafts(&parsed.threads, &batch);
    // 0 和 2/99 越界都被丢，只留 1 → s1
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].session_ids, vec!["s1"]);
}

#[test]
fn duplicate_indices_are_deduplicated() {
    let batch = vec![
        ("s1".into(), s("a", "x", &[])),
        ("s2".into(), s("b", "y", &[])),
    ];
    let llm_out = r#"{"threads":[{"name":"x","summary":"","session_indices":[1, 1, 2, 2]}]}"#;
    let parsed = parse_thread_response(llm_out).unwrap();
    let drafts = map_to_drafts(&parsed.threads, &batch);
    assert_eq!(drafts[0].session_ids, vec!["s1", "s2"]);
}

#[test]
fn empty_threads_array_returns_no_drafts() {
    let llm_out = r#"{"threads":[]}"#;
    let parsed = parse_thread_response(llm_out).unwrap();
    let drafts = map_to_drafts(&parsed.threads, &[]);
    assert!(drafts.is_empty());
}

#[test]
fn code_fence_wrapping_is_stripped() {
    let llm_out = "```json\n{\"threads\":[]}\n```";
    let parsed = parse_thread_response(llm_out).unwrap();
    assert_eq!(parsed.threads.len(), 0);
}

#[test]
fn threads_with_empty_name_are_filtered() {
    let batch = vec![("s1".into(), s("a", "x", &[]))];
    let llm_out = r#"{"threads":[{"name":"  ","summary":"","session_indices":[1]},
                         {"name":"x","summary":"","session_indices":[1]}]}"#;
    let parsed = parse_thread_response(llm_out).unwrap();
    let drafts = map_to_drafts(&parsed.threads, &batch);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].name, "x");
}

#[test]
fn fallback_clusters_by_topic_minimum_two_sessions() {
    let sessions = vec![
        ("s1".into(), s("a", "x", &["桌面化"])),
        ("s2".into(), s("b", "y", &["桌面化"])),
        ("s3".into(), s("c", "z", &["独立主题"])),
    ];
    let drafts = fallback_cluster(&sessions);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].name, "未知项目 · 桌面化");
    assert_eq!(drafts[0].session_ids.len(), 2);
}

#[test]
fn fallback_handles_empty_topics_as_uncategorized() {
    let sessions = vec![
        ("s1".into(), s("a", "x", &[])),
        ("s2".into(), s("b", "y", &[])),
    ];
    let drafts = fallback_cluster(&sessions);
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].name, "未知项目 · 未分类");
}

/// 同样的 topic 来自不同项目，fallback 必须分成两条独立线索
/// （这是用户反馈的 tt-qimen 误聚类问题的回归用例）。
#[test]
fn fallback_does_not_merge_across_projects() {
    let sessions = vec![
        ("s1".into(), sp("a", "x", &["prompts.txt"], "memex")),
        ("s2".into(), sp("b", "y", &["prompts.txt"], "memex")),
        ("s3".into(), sp("c", "z", &["prompts.txt"], "tt-qimen")),
        ("s4".into(), sp("d", "w", &["prompts.txt"], "tt-qimen")),
    ];
    let drafts = fallback_cluster(&sessions);
    assert_eq!(drafts.len(), 2);
    let names: Vec<_> = drafts.iter().map(|d| d.name.clone()).collect();
    assert!(names.iter().any(|n| n.contains("memex")));
    assert!(names.iter().any(|n| n.contains("tt-qimen")));
    let memex_draft = drafts.iter().find(|d| d.name.contains("memex")).unwrap();
    assert_eq!(
        memex_draft.session_ids,
        vec!["s1".to_string(), "s2".to_string()]
    );
}

#[test]
fn build_prompt_includes_project_signal() {
    let batch = vec![
        (
            "s1".into(),
            sp("写文档", "做文档相关工作", &["docs"], "memex"),
        ),
        ("s2".into(), sp("跑命盘", "排八字", &["命理"], "tt-qimen")),
    ];
    let prompt = build_clustering_prompt(&batch);
    assert!(
        prompt.contains("project=memex"),
        "应包含 project=memex 信号:\n{}",
        prompt
    );
    assert!(
        prompt.contains("project=tt-qimen"),
        "应包含 project=tt-qimen 信号:\n{}",
        prompt
    );
}

/// 关键词字面 fallback：在 title / topics / summary / decisions 任一命中即收录。
#[test]
fn fallback_query_match_hits_title_topics_summary_decisions() {
    let a = sp("修 Tauri 多窗口", "无关内容", &[], "memex");
    let b = sp("写 Markdown", "讨论 Tauri 事件循环", &[], "memex");
    let c = sp("修 bug", "不沾边", &["tauri"], "memex");
    let mut d = sp("改 schema", "不沾边", &[], "memex");
    d.decisions = vec!["Tauri 升级 v2".into()];
    let e = sp("命理预测", "不沾边", &[], "tt-qimen");
    let sessions = vec![
        ("s_a".into(), a),
        ("s_b".into(), b),
        ("s_c".into(), c),
        ("s_d".into(), d),
        ("s_e".into(), e),
    ];
    let draft = fallback_query_match(&sessions, "Tauri");
    assert_eq!(draft.name, "Tauri");
    assert_eq!(draft.session_ids.len(), 4);
    assert!(!draft.session_ids.contains(&"s_e".to_string()));
}

#[test]
fn fallback_query_match_empty_query_returns_empty() {
    let sessions = vec![("s1".into(), s("a", "x", &["topic"]))];
    let draft = fallback_query_match(&sessions, "   ");
    assert_eq!(draft.session_ids.len(), 0);
}
