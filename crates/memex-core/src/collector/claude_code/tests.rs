use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

use super::*;
use crate::storage::models::Role;

fn write_fixture(dir: &Path, filename: &str, content: &str) -> PathBuf {
    let path = dir.join(filename);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

#[test]
fn test_parse_normal_jsonl() {
    let tmp = TempDir::new().unwrap();
    let jsonl = r#"{"type":"human","uuid":"m1","role":"human","message":{"role":"human","content":"hello"},"timestamp":"2026-05-01T10:00:00Z"}
{"type":"assistant","uuid":"m2","role":"assistant","message":{"role":"assistant","content":"hi there"},"timestamp":"2026-05-01T10:00:01Z"}
"#;
    let file_path = write_fixture(tmp.path(), "project/session1.jsonl", jsonl);
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let session = SessionMeta {
        id: "session1".into(),
        source: "claude_code".into(),
        project_path: Some("project".into()),
        file_path: file_path.to_string_lossy().to_string(),
        last_offset: 0,
        mtime: 0,
        created_secs: 0,
        title: None,
    };
    let messages = adapter.collect(&session).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].content, "hello");
    assert_eq!(messages[1].role, Role::Assistant);
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

#[test]
fn test_empty_file() {
    let tmp = TempDir::new().unwrap();
    let file_path = write_fixture(tmp.path(), "proj/empty.jsonl", "");
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let session = SessionMeta {
        id: "empty".into(),
        source: "claude_code".into(),
        project_path: None,
        file_path: file_path.to_string_lossy().to_string(),
        last_offset: 0,
        mtime: 0,
        created_secs: 0,
        title: None,
    };
    let messages = adapter.collect(&session).unwrap();
    assert!(messages.is_empty());
}

#[test]
fn test_extract_project_path_prefers_cwd_in_jsonl() {
    let tmp = TempDir::new().unwrap();
    let jsonl = r#"{"type":"user","cwd":"/Users/foo/work/my-proj","message":{"role":"human","content":"hi"}}
"#;
    let file_path = write_fixture(tmp.path(), "-Users-foo-work-my--proj/session1.jsonl", jsonl);
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let got = adapter.extract_project_path(&file_path);
    assert_eq!(got.as_deref(), Some("/Users/foo/work/my-proj"));
}

#[test]
fn test_extract_project_path_falls_back_to_dash_decode() {
    let tmp = TempDir::new().unwrap();
    // 没有 cwd 字段的损坏行：必须退到 dash-decode。
    let file_path = write_fixture(
        tmp.path(),
        "-Users-foo-Documents-bar/session1.jsonl",
        "{}\n",
    );
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let got = adapter.extract_project_path(&file_path);
    assert_eq!(got.as_deref(), Some("/Users/foo/Documents/bar"));
}

#[test]
fn test_extract_project_path_for_subagent_returns_parent_project() {
    let tmp = TempDir::new().unwrap();
    // subagent 文件路径形如 `<encoded-proj>/<uuid>/subagents/agent-x.jsonl`
    // 它的首行 cwd 通常已经是父项目的绝对路径。
    let jsonl = r#"{"type":"assistant","cwd":"/Users/foo/Documents/bar","message":{"role":"assistant","content":"ok"}}
"#;
    let file_path = write_fixture(
        tmp.path(),
        "-Users-foo-Documents-bar/abc-uuid/subagents/agent-x.jsonl",
        jsonl,
    );
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let got = adapter.extract_project_path(&file_path);
    assert_eq!(got.as_deref(), Some("/Users/foo/Documents/bar"));
}

#[test]
fn test_extract_project_path_subagent_dash_decode_fallback() {
    let tmp = TempDir::new().unwrap();
    // subagent 文件没 cwd 时：fallback 必须落到 `<encoded-proj>` 那一层，
    // 而不是 `subagents` / `<uuid>` / `agent-x`。
    let file_path = write_fixture(
        tmp.path(),
        "-Users-foo-Documents-bar/abc-uuid/subagents/agent-y.jsonl",
        "{}\n",
    );
    let adapter = ClaudeCodeAdapter::with_base_dir(tmp.path().to_path_buf());
    let got = adapter.extract_project_path(&file_path);
    assert_eq!(got.as_deref(), Some("/Users/foo/Documents/bar"));
}
