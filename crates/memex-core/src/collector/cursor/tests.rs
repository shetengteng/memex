use super::*;
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
