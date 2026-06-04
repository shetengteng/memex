use super::*;
use crate::collector::Adapter;
use crate::storage::models::{Role, SessionMeta};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_parse_cursor_jsonl() {
    let tmp = TempDir::new().unwrap();
    let content = r#"{"role":"user","message":{"content":[{"type":"text","text":"hello cursor"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"hi from cursor assistant"}]}}
"#;
    let dir = tmp.path().join("proj/agent-transcripts/uuid1");
    fs::create_dir_all(&dir).unwrap();
    let file_path = dir.join("uuid1.jsonl");
    fs::write(&file_path, content).unwrap();

    let adapter = CursorAdapter::with_base_dir(tmp.path().to_path_buf());
    let session = SessionMeta {
        id: "uuid1".to_string(),
        source: "cursor".to_string(),
        project_path: Some("proj".to_string()),
        file_path: file_path.to_string_lossy().to_string(),
        last_offset: 0,
        mtime: 0,
        created_secs: 0,
        title: None,
    };

    let messages = adapter.collect(&session).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert!(messages[0].content.contains("hello cursor"));
    assert_eq!(messages[1].role, Role::Assistant);
}

#[test]
fn test_scan_discovers_transcripts() {
    let tmp = TempDir::new().unwrap();
    let dir1 = tmp.path().join("proj-a/agent-transcripts/s1");
    let dir2 = tmp.path().join("proj-b/agent-transcripts/s2");
    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();
    fs::write(
        dir1.join("s1.jsonl"),
        r#"{"role":"user","message":{"content":"hi"}}"#,
    )
    .unwrap();
    fs::write(
        dir2.join("s2.jsonl"),
        r#"{"role":"user","message":{"content":"hey"}}"#,
    )
    .unwrap();

    let adapter = CursorAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[cfg(unix)]
#[test]
fn test_scan_returns_empty_on_permission_denied() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    fs::create_dir_all(tmp.path().join("proj-a/agent-transcripts")).unwrap();
    let restricted_root = tmp.path().to_path_buf();
    let mut perms = fs::metadata(&restricted_root).unwrap().permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&restricted_root, perms).unwrap();

    let adapter = CursorAdapter::with_base_dir(restricted_root.clone());
    let sessions = adapter.scan().unwrap_or_default();
    assert!(
        sessions.is_empty(),
        "permission denied should yield empty scan, not panic / propagate"
    );

    let mut restore = fs::metadata(&restricted_root).unwrap().permissions();
    restore.set_mode(0o755);
    fs::set_permissions(&restricted_root, restore).unwrap();
}

#[test]
fn test_normalize_workspace_name() {
    assert_eq!(
        normalize_workspace_name("Users-Alice-Documents-personal-project"),
        "personal-project"
    );
    assert_eq!(
        normalize_workspace_name("Users-Bob-Library-Application-Support-Cursor-Workspaces-abc"),
        "ws:abc"
    );
    assert_eq!(
        normalize_workspace_name("simple-workspace"),
        "simple-workspace"
    );
}

mod sqlite_backend {
    use super::*;
    use rusqlite::Connection;

    /// 在 fixture DB 上插入一个 composer，可选附加 composerHeaders 用于
    /// 测试新版 Cursor 的 workspaceIdentifier enrichment。
    fn build_fixture_db(path: &std::path::Path) {
        build_fixture_db_with_headers(path, None);
    }

    fn build_fixture_db_with_headers(path: &std::path::Path, headers_json: Option<&str>) {
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "CREATE TABLE cursorDiskKV (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE ItemTable (key TEXT UNIQUE ON CONFLICT REPLACE, value BLOB)",
            [],
        )
        .unwrap();

        let composer_id = "11111111-2222-3333-4444-555555555555";
        let composer_value = serde_json::json!({
            "composerId": composer_id,
            "name": "fix-bug-tab",
            "createdAt": 1_700_000_000_000_i64,
            "lastUpdatedAt": 1_700_000_300_000_i64,
            "fullConversationHeadersOnly": [
                {"bubbleId": "b1", "type": 1},
                {"bubbleId": "b2", "type": 2},
                {"bubbleId": "b3", "type": 2},
            ],
        });
        conn.execute(
            "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                format!("composerData:{composer_id}"),
                composer_value.to_string().as_bytes()
            ],
        )
        .unwrap();

        if let Some(hdr) = headers_json {
            conn.execute(
                "INSERT INTO ItemTable (key, value) VALUES (?1, ?2)",
                rusqlite::params!["composer.composerHeaders", hdr.as_bytes()],
            )
            .unwrap();
        }

        let bubbles = [
            (
                "b1",
                serde_json::json!({"type": 1, "text": "请帮我修这个 bug", "richText": ""}),
            ),
            (
                "b2",
                serde_json::json!({"type": 2, "text": "好的，我来看一下问题", "richText": ""}),
            ),
            (
                "b3",
                serde_json::json!({
                    "type": 2,
                    "text": "",
                    "richText": "",
                    "toolFormerData": {
                        "name": "read_file",
                        "rawArgs": "{\"path\":\"src/lib.rs\"}",
                        "result": "fn main()"
                    }
                }),
            ),
        ];
        for (bid, val) in bubbles {
            conn.execute(
                "INSERT INTO cursorDiskKV (key, value) VALUES (?1, ?2)",
                rusqlite::params![
                    format!("bubbleId:{composer_id}:{bid}"),
                    val.to_string().as_bytes()
                ],
            )
            .unwrap();
        }
    }

    #[test]
    fn test_sqlite_scan_lists_composer_sessions() {
        // 旧版 fixture：composerData 里有 name="fix-bug-tab"，但没有 composerHeaders。
        // 新行为：name 应被识别为"对话标题"写到 title，而不是 project_path。
        // project_path 没有可信来源，应为 None。
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("state.vscdb");
        build_fixture_db(&db_path);

        let adapter = CursorAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(s.source, "cursor");
        assert!(s.id.starts_with("cursor-"));
        assert_eq!(s.project_path, None);
        assert_eq!(s.title.as_deref(), Some("fix-bug-tab"));
        assert!(s.mtime > 0);
    }

    #[test]
    fn test_sqlite_scan_uses_composer_headers_for_project_and_title() {
        // 新版 Cursor：name + workspaceIdentifier 都在 composerHeaders。
        // enrichment 应填好 project_path（来自 workspaceIdentifier.uri.path）
        // 和 title（来自 header.name，优先级高于 composerData.name）。
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("state.vscdb");
        let headers = serde_json::json!({
            "allComposers": [{
                "composerId": "11111111-2222-3333-4444-555555555555",
                "name": "审 dashboard 项目列",
                "workspaceIdentifier": {
                    "id": "wsid",
                    "uri": {
                        "fsPath": "/Users/me/Documents/tt-projects/memex",
                        "path": "/Users/me/Documents/tt-projects/memex",
                        "scheme": "file"
                    }
                }
            }]
        }).to_string();
        build_fixture_db_with_headers(&db_path, Some(&headers));

        let adapter = CursorAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);
        let s = &sessions[0];
        assert_eq!(
            s.project_path.as_deref(),
            Some("/Users/me/Documents/tt-projects/memex")
        );
        assert_eq!(s.title.as_deref(), Some("审 dashboard 项目列"));
    }

    #[test]
    fn test_sqlite_scan_multifolder_workspace_yields_no_project_path() {
        // workspaceIdentifier 只带 configPath（.code-workspace 多文件夹）时，
        // 无法还原唯一 cwd —— project_path 必须留空，避免把 configPath 当 cwd。
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("state.vscdb");
        let headers = serde_json::json!({
            "allComposers": [{
                "composerId": "11111111-2222-3333-4444-555555555555",
                "name": "multi-folder chat",
                "workspaceIdentifier": {
                    "id": "wsid",
                    "configPath": {"path": "/Users/me/my.code-workspace"}
                }
            }]
        }).to_string();
        build_fixture_db_with_headers(&db_path, Some(&headers));

        let adapter = CursorAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions[0].project_path, None);
        assert_eq!(sessions[0].title.as_deref(), Some("multi-folder chat"));
    }

    #[test]
    fn test_sqlite_collect_extracts_user_assistant_and_tool() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("state.vscdb");
        build_fixture_db(&db_path);

        let adapter = CursorAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        let msgs = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0].role, Role::User);
        assert!(msgs[0].content.contains("修这个 bug"));
        assert_eq!(msgs[1].role, Role::Assistant);
        assert!(msgs[1].content.contains("我来看一下"));
        assert_eq!(msgs[2].role, Role::Assistant);
        assert!(msgs[2].content.contains("[tool: read_file]"));
        assert!(msgs[2].content.contains("src/lib.rs"));
    }

    #[test]
    fn test_sqlite_collect_respects_last_offset() {
        let tmp = TempDir::new().unwrap();
        let db_path = tmp.path().join("state.vscdb");
        build_fixture_db(&db_path);

        let adapter = CursorAdapter::with_db_path(db_path);
        let sessions = adapter.scan().unwrap();
        let mut seed = sessions[0].clone();
        seed.last_offset = 2;
        let msgs = adapter.collect(&seed).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].content.contains("[tool: read_file]"));
    }

    #[test]
    fn test_sqlite_scan_returns_empty_when_db_missing() {
        let tmp = TempDir::new().unwrap();
        let adapter = CursorAdapter::with_db_path(tmp.path().join("absent.vscdb"));
        let sessions = adapter.scan().unwrap();
        assert!(sessions.is_empty());
    }
}
