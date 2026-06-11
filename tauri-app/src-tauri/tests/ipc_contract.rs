//! IPC contract tests.
//!
//! Pins the JSON field shape exposed to the Vue frontend so we catch
//! breaking renames before they ship. Keep these assertions in lock-step
//! with `tauri-app/src/types/index.ts`.

use memex_core::storage::db::SessionListFilter;
use memex_core::storage::queries::{ProjectSummary, StatsBreakdown, TimelineEntry};
use memex_menubar_lib::commands::daemon::DaemonStatus;
use memex_menubar_lib::commands::sessions::SummaryProgress;
use memex_menubar_lib::commands::stats::Stats;

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
        llm_model: Some("qwen2.5:7b".to_string()),
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
            "llm_model",
        ],
    );
    assert_eq!(v["sessions"], 10);
    assert_eq!(v["llm_provider"], "ollama");
    assert_eq!(v["llm_model"], "qwen2.5:7b");
}

/// ProjectSummary IPC 形态：所有多词字段都是 camelCase
/// （`projectPath` / `sessionCount` / `messageCount` / `lastTitle` /
/// `lastUpdated` / `byAdapter`），snake_case 不再出现。前端
/// `tauri-app/src/types/index.ts::ProjectSummary` 必须与此保持一致。
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
            "projectPath",
            "name",
            "sessionCount",
            "messageCount",
            "lastTitle",
            "lastUpdated",
            "byAdapter",
        ],
    );
    let obj = v.as_object().unwrap();
    for legacy in &[
        "project_path",
        "session_count",
        "message_count",
        "last_title",
        "last_updated",
        "by_adapter",
    ] {
        assert!(
            !obj.contains_key(*legacy),
            "ProjectSummary leaked snake_case key: {}",
            legacy
        );
    }
    assert_eq!(v["byAdapter"]["cursor"], 4);
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
fn session_list_filter_camel_or_snake_roundtrip() {
    // 前端只提供已知字段。`deny_unknown_fields` 让任何拼写错误（如
    // `adapter`、`projct`、`tiem`）在反序列化阶段就抛错，避免后端静默
    // 当作 `None` 然后返回全表把 UI 撑爆。
    let json = serde_json::json!({
        "adapters": ["cursor", "claude_code"],
        "projects": ["memex"],
        "time": "7d",
        "summary": "done",
        "query": "redis",
        "sort": "messages"
    });
    let filter: SessionListFilter = serde_json::from_value(json).unwrap();
    assert_eq!(
        filter.adapters.as_deref(),
        Some(&["cursor".to_string(), "claude_code".to_string()][..])
    );
    assert_eq!(filter.projects.as_deref(), Some(&["memex".to_string()][..]));
    assert_eq!(filter.time.as_deref(), Some("7d"));
    assert_eq!(filter.summary.as_deref(), Some("done"));
    assert_eq!(filter.query.as_deref(), Some("redis"));
    assert_eq!(filter.sort.as_deref(), Some("messages"));
}

#[test]
fn session_list_filter_all_optional() {
    // 前端不勾任何 facet 时整个 JSON 是空对象，等价于"不过滤"。
    let filter: SessionListFilter = serde_json::from_value(serde_json::json!({})).unwrap();
    assert!(filter.adapters.is_none());
    assert!(filter.projects.is_none());
    assert!(filter.time.is_none());
    assert!(filter.summary.is_none());
    assert!(filter.query.is_none());
    assert!(filter.sort.is_none());
}

#[test]
fn session_list_filter_rejects_unknown_field() {
    let err = serde_json::from_value::<SessionListFilter>(serde_json::json!({
        "adapter": ["typo"],
    }))
    .expect_err("typo `adapter` should be rejected by deny_unknown_fields");
    assert!(
        err.to_string().contains("adapter"),
        "error should mention the offending field, got: {err}"
    );
}

// Phase 4：原 `lock_info_roundtrip` 测试已删 —— `LockInfo` 类型不再暴露给
// menubar crate（lock 文件由 `memex_daemon::lockfile` 写、`memex-cli` 通过
// `daemon_client` 读，两边各自有 round-trip 测试）。
