use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::ClaudeCodeAdapter;
use crate::collector::Adapter;
use crate::storage::models::{Role, SessionMeta};

fn write_fixture(dir: &Path, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

fn make_session(file_path: &Path, id: &str, last_offset: u64) -> SessionMeta {
    SessionMeta {
        id: id.to_string(),
        source: "claude_code".to_string(),
        project_path: None,
        file_path: file_path.to_string_lossy().to_string(),
        last_offset,
        mtime: 0,
    }
}

#[test]
fn test_parse_normal_jsonl() {
    let tmp = TempDir::new().unwrap();
    let jsonl = r#"{"type":"human","uuid":"m1","role":"human","message":{"role":"human","content":"hello"},"timestamp":"2026-05-01T10:00:00Z"}
{"type":"assistant","uuid":"m2","role":"assistant","message":{"role":"assistant","content":"hi there"},"timestamp":"2026-05-01T10:00:01Z"}
"#;
    let file_path = write_fixture(tmp.path(), "project/session1.jsonl", jsonl);
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());

    let messages = adapter.collect(&make_session(&file_path, "session1", 0)).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].content, "hello");
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[1].content, "hi there");
}

#[test]
fn test_incremental_read() {
    let tmp = TempDir::new().unwrap();
    let line1 = r#"{"type":"human","uuid":"m1","role":"human","message":{"role":"human","content":"first"},"timestamp":"2026-05-01T10:00:00Z"}"#;
    let line2 = r#"{"type":"assistant","uuid":"m2","role":"assistant","message":{"role":"assistant","content":"second"},"timestamp":"2026-05-01T10:00:01Z"}"#;
    let content = format!("{}\n{}\n", line1, line2);
    let file_path = write_fixture(tmp.path(), "proj/s1.jsonl", &content);

    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let offset = (line1.len() + 1) as u64;
    let messages = adapter.collect(&make_session(&file_path, "s1", offset)).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "second");
}

#[test]
fn test_malformed_json_skipped() {
    let tmp = TempDir::new().unwrap();
    let content = r#"{"type":"human","uuid":"m1","role":"human","message":{"role":"human","content":"good"}}
NOT VALID JSON
{"type":"assistant","uuid":"m2","role":"assistant","message":{"role":"assistant","content":"also good"}}
"#;
    let file_path = write_fixture(tmp.path(), "proj/s2.jsonl", content);
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let messages = adapter.collect(&make_session(&file_path, "s2", 0)).unwrap();
    assert_eq!(messages.len(), 2);
}

#[test]
fn test_empty_file() {
    let tmp = TempDir::new().unwrap();
    let file_path = write_fixture(tmp.path(), "proj/empty.jsonl", "");
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let messages = adapter.collect(&make_session(&file_path, "empty", 0)).unwrap();
    assert!(messages.is_empty());
}

#[test]
fn test_scan_discovers_files() {
    let tmp = TempDir::new().unwrap();
    write_fixture(tmp.path(), "proj_a/session1.jsonl", "{}");
    write_fixture(tmp.path(), "proj_b/session2.jsonl", "{}");
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 2);
}
