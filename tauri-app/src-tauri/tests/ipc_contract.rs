//! IPC contract tests.
//!
//! Pins the JSON field shape exposed to the Vue frontend so we catch
//! breaking renames before they ship. Keep these assertions in lock-step
//! with `tauri-app/src/types/index.ts`.

use memex_core::storage::queries::{ProjectSummary, StatsBreakdown, TimelineEntry};
use memex_menubar_lib::commands::{DaemonStatus, LockInfo, Stats, SummaryProgress};

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
        sessions_eligible_for_summary: 8,
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
            "sessions_eligible_for_summary",
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
        aborted: false,
    })
    .unwrap();
    assert_object_keys(
        &v,
        &[
            "current",
            "total",
            "session_id",
            "success",
            "done",
            "aborted",
        ],
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

#[test]
fn daemon_status_contract() {
    let v = serde_json::to_value(DaemonStatus {
        running: true,
        pid: Some(12345),
        port: Some(45291),
        http_ok: true,
        started_at: Some("2026-06-07T03:00:00+00:00".into()),
    })
    .unwrap();
    assert_object_keys(&v, &["running", "pid", "port", "http_ok", "started_at"]);
    assert_eq!(v["pid"], 12345);
    assert_eq!(v["http_ok"], true);
}

#[test]
fn daemon_status_handles_null_fields() {
    // 没在跑的 daemon：pid / port / started_at 都该是 null，而不是 0 或空字符串
    let v = serde_json::to_value(DaemonStatus {
        running: false,
        pid: None,
        port: None,
        http_ok: false,
        started_at: None,
    })
    .unwrap();
    assert!(v["pid"].is_null());
    assert!(v["port"].is_null());
    assert!(v["started_at"].is_null());
}

#[test]
fn lock_info_roundtrip() {
    // LockInfo 既要 Serialize 也要 Deserialize（从 daemon.lock 文件读回来）
    let original = LockInfo {
        pid: 9999,
        port: 45291,
        started_at: "2026-06-07T03:00:00+00:00".into(),
    };
    let s = serde_json::to_string(&original).unwrap();
    let back: LockInfo = serde_json::from_str(&s).unwrap();
    assert_eq!(back.pid, 9999);
    assert_eq!(back.port, 45291);
    assert_eq!(back.started_at, "2026-06-07T03:00:00+00:00");
}
