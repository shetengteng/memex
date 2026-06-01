//! IPC contract tests.
//!
//! Pins the JSON field shape exposed to the Vue frontend so we catch
//! breaking renames before they ship. Keep these assertions in lock-step
//! with `tauri-app/src/types/index.ts`.

use memex_core::storage::queries::{ProjectSummary, StatsBreakdown, TimelineEntry};
use memex_menubar_lib::commands::{Stats, SummaryProgress};

fn assert_object_keys(value: &serde_json::Value, expected: &[&str]) {
    let obj = value
        .as_object()
        .unwrap_or_else(|| panic!("expected object, got {}", value));
    for key in expected {
        assert!(obj.contains_key(*key), "missing field `{key}` in {value}");
    }
}

#[test]
fn stats_contract() {
    let v = serde_json::to_value(Stats {
        sessions: 10,
        messages: 50,
        chunks: 200,
        db_exists: true,
        summaries: 5,
        chunks_summarized: 30,
        llm_provider: Some("ollama".to_string()),
    })
    .unwrap();

    assert_object_keys(
        &v,
        &[
            "sessions",
            "messages",
            "chunks",
            "db_exists",
            "summaries",
            "chunks_summarized",
            "llm_provider",
        ],
    );
    assert_eq!(v["sessions"], 10);
    assert_eq!(v["llm_provider"], "ollama");
}

#[test]
fn project_summary_contract() {
    let mut by_adapter = std::collections::BTreeMap::new();
    by_adapter.insert("cursor".to_string(), 4);
    by_adapter.insert("claude_code".to_string(), 2);

    let v = serde_json::to_value(ProjectSummary {
        project_path: "/Users/me/repo".into(),
        name: "repo".into(),
        session_count: 6,
        message_count: 120,
        last_title: Some("hello".into()),
        last_updated: "2026-06-01T00:00:00+00:00".into(),
        by_adapter,
    })
    .unwrap();

    assert_object_keys(
        &v,
        &[
            "project_path",
            "name",
            "session_count",
            "message_count",
            "last_title",
            "last_updated",
            "by_adapter",
        ],
    );
    assert_eq!(v["by_adapter"]["cursor"], 4);
}

#[test]
fn summary_progress_contract() {
    let v = serde_json::to_value(SummaryProgress {
        current: 3,
        total: 10,
        session_id: "sess-001".into(),
        success: true,
        done: false,
    })
    .unwrap();
    assert_object_keys(
        &v,
        &["current", "total", "session_id", "success", "done"],
    );
}

#[test]
fn timeline_entry_contract() {
    let v = serde_json::to_value(TimelineEntry {
        date: "2026-06-01".into(),
        adapter: "cursor".into(),
        sessions: 3,
        messages: 12,
    })
    .unwrap();
    assert_object_keys(&v, &["date", "adapter", "sessions", "messages"]);
}

#[test]
fn breakdown_contract() {
    let mut by_adapter = std::collections::BTreeMap::new();
    by_adapter.insert("cursor".to_string(), 4_i64);
    let mut by_project = std::collections::BTreeMap::new();
    by_project.insert("/repo".to_string(), 4_i64);

    let v = serde_json::to_value(StatsBreakdown {
        by_adapter,
        by_project,
        recent_7d_sessions: 1,
        recent_7d_messages: 2,
        recent_30d_sessions: 3,
        recent_30d_messages: 4,
    })
    .unwrap();
    assert_object_keys(
        &v,
        &[
            "by_adapter",
            "by_project",
            "recent_7d_sessions",
            "recent_7d_messages",
            "recent_30d_sessions",
            "recent_30d_messages",
        ],
    );
    assert_eq!(v["by_adapter"]["cursor"], 4);
    assert_eq!(v["by_project"]["/repo"], 4);
}
