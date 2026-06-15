use std::path::PathBuf;

use tempfile::TempDir;

use super::*;

fn create_test_db(dir: &std::path::Path) -> PathBuf {
    let db_path = dir.join("opencode.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE project (id TEXT PRIMARY KEY, name TEXT);
         INSERT INTO project VALUES ('proj1', 'test-project');
         CREATE TABLE session (
             id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT,
             slug TEXT NOT NULL, directory TEXT NOT NULL, title TEXT NOT NULL,
             version TEXT NOT NULL, share_url TEXT, summary_additions INTEGER,
             summary_deletions INTEGER, summary_files INTEGER, summary_diffs TEXT,
             revert TEXT, permission TEXT,
             time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
             time_compacting INTEGER, time_archived INTEGER
         );
         INSERT INTO session VALUES ('ses_001', 'proj1', NULL, 'test', '/tmp/proj', 'Test Session', '1', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 1000000, 2000000, NULL, NULL);
         CREATE TABLE message (
             id TEXT PRIMARY KEY, session_id TEXT NOT NULL,
             time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
             data TEXT NOT NULL
         );
         INSERT INTO message VALUES ('msg_001', 'ses_001', 1000000, 1000000, '{\"role\":\"user\"}');
         INSERT INTO message VALUES ('msg_002', 'ses_001', 1001000, 1001000, '{\"role\":\"assistant\"}');
         CREATE TABLE part (
             id TEXT PRIMARY KEY, message_id TEXT NOT NULL, session_id TEXT NOT NULL,
             time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
             data TEXT NOT NULL
         );
         INSERT INTO part VALUES ('prt_001', 'msg_001', 'ses_001', 1000000, 1000000, '{\"type\":\"text\",\"text\":\"hello opencode\"}');
         INSERT INTO part VALUES ('prt_002', 'msg_002', 'ses_001', 1001000, 1001000, '{\"type\":\"text\",\"text\":\"response from opencode\"}');",
    )
    .unwrap();
    db_path
}

#[test]
fn test_scan_opencode_sessions() {
    let tmp = TempDir::new().unwrap();
    let db_path = create_test_db(tmp.path());
    let adapter = OpenCodeAdapter::with_db_path(db_path);
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "ses_001");
    assert_eq!(sessions[0].project_path, Some("/tmp/proj".to_string()));
}

#[test]
fn test_opencode_placeholder_title_classification() {
    assert!(is_opencode_placeholder_title(
        "New session - 2026-01-23T08:45:35.508Z"
    ));
    assert!(is_opencode_placeholder_title(
        "New session - 2025-12-31T23:59:59.000Z"
    ));
    assert!(is_opencode_placeholder_title(
        "new session - 2026-06-01T08:33:59.831Z"
    ));
    assert!(is_opencode_placeholder_title("New session"));
    assert!(!is_opencode_placeholder_title("Greeting tone check-in"));
    assert!(!is_opencode_placeholder_title("Vue 3 简单按钮弹窗组件"));
    assert!(!is_opencode_placeholder_title("New session about Redis"));
}

#[test]
fn test_scan_filters_placeholder_titles() {
    let tmp = TempDir::new().unwrap();
    let db_path = tmp.path().join("opencode.db");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch(
        "CREATE TABLE project (id TEXT PRIMARY KEY, name TEXT);
         INSERT INTO project VALUES ('proj1', 'test-project');
         CREATE TABLE session (
             id TEXT PRIMARY KEY, project_id TEXT, parent_id TEXT,
             slug TEXT NOT NULL, directory TEXT NOT NULL, title TEXT NOT NULL,
             version TEXT NOT NULL, share_url TEXT, summary_additions INTEGER,
             summary_deletions INTEGER, summary_files INTEGER, summary_diffs TEXT,
             revert TEXT, permission TEXT,
             time_created INTEGER NOT NULL, time_updated INTEGER NOT NULL,
             time_compacting INTEGER, time_archived INTEGER
         );
         INSERT INTO session VALUES
             ('s_placeholder', 'proj1', NULL, 'x', '/tmp/p', 'New session - 2026-01-23T08:45:35.508Z', '1', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 1000000, 2000000, NULL, NULL),
             ('s_real', 'proj1', NULL, 'y', '/tmp/p', 'Vue 3 简单按钮弹窗组件', '1', NULL, NULL, NULL, NULL, NULL, NULL, NULL, 1000000, 2000000, NULL, NULL);",
    )
    .unwrap();
    let adapter = OpenCodeAdapter::with_db_path(db_path);
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 2);
    let placeholder = sessions.iter().find(|s| s.id == "s_placeholder").unwrap();
    let real = sessions.iter().find(|s| s.id == "s_real").unwrap();
    assert!(
        placeholder.title.is_none(),
        "opencode 自带占位 title 应当被过滤为 None，实际：{:?}",
        placeholder.title
    );
    assert_eq!(real.title.as_deref(), Some("Vue 3 简单按钮弹窗组件"));
}

#[test]
fn test_collect_opencode_messages() {
    let tmp = TempDir::new().unwrap();
    let db_path = create_test_db(tmp.path());
    let adapter = OpenCodeAdapter::with_db_path(db_path);
    let sessions = adapter.scan().unwrap();
    let messages = adapter.collect(&sessions[0]).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].content, "hello opencode");
    assert_eq!(messages[1].content, "response from opencode");
}

#[test]
fn test_missing_db() {
    let adapter = OpenCodeAdapter::with_db_path(PathBuf::from("/nonexistent/opencode.db"));
    let sessions = adapter.scan().unwrap();
    assert!(sessions.is_empty());
}

/// Regression：collect() open db 失败时，错误链必须保留 path —— 否则上层只能
/// 看到裸 SQLite 错误（"unable to open database file"），无法判断是哪个 db。
/// 这跟 scan() 路径上已有的 with_context 行为对齐。
#[test]
fn test_collect_open_failure_includes_path_in_error_chain() {
    let tmp = TempDir::new().unwrap();
    // db_path 是个目录而不是 SQLite 文件 → SQLite 必定 open 失败，且 path.exists()
    // 返回 true 绕过 collect 内部的 "missing db → 返回空" 早返路径。
    let bad_path = tmp.path().join("opencode.db");
    std::fs::create_dir_all(&bad_path).unwrap();

    let adapter = OpenCodeAdapter::with_db_path(bad_path.clone());
    let session = SessionMeta {
        id: "ses_x".to_string(),
        source: "opencode".to_string(),
        project_path: Some("/tmp/proj".to_string()),
        file_path: bad_path.to_string_lossy().to_string(),
        last_offset: 0,
        mtime: 0,
        created_secs: 0,
        title: None,
    };
    let err = adapter
        .collect(&session)
        .expect_err("expected open failure");
    let chain = format!("{err:#}");

    assert!(
        chain.contains("opencode.db"),
        "error chain must contain db path; got: {chain}"
    );
    assert!(
        chain.contains("failed to open opencode db"),
        "error chain must include with_context message; got: {chain}"
    );
}
