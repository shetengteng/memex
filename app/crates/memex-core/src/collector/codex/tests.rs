use std::fs;
use std::path::{Path, PathBuf};

use chrono::DateTime;
use tempfile::TempDir;

use super::CodexAdapter;
use super::discover::DateAccessors;
use crate::collector::Adapter;
use crate::storage::models::Role;

fn write_index(base: &Path, entries: &[(&str, &str, &str)]) {
    let path = base.join("session_index.jsonl");
    let mut content = String::new();
    for (id, ts, name) in entries {
        content.push_str(
            &serde_json::json!({
                "id": id,
                "updated_at": ts,
                "thread_name": name,
            })
            .to_string(),
        );
        content.push('\n');
    }
    fs::write(path, content).unwrap();
}

fn write_session(base: &Path, ts: &str, session_id: &str, body: &str) -> PathBuf {
    let dt = DateTime::parse_from_rfc3339(&ts.replace('Z', "+00:00")).unwrap();
    let dir = base
        .join("sessions")
        .join(format!("{:04}", dt.year()))
        .join(format!("{:02}", dt.month()))
        .join(format!("{:02}", dt.day()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!(
        "rollout-{}-{}.jsonl",
        "20260301T000000", session_id
    ));
    fs::write(&file, body).unwrap();
    file
}

#[test]
fn test_scan_uses_session_index_with_thread_name_as_title() {
    // 空 body：拿不到 session_meta.cwd，project_path 应为 None；
    // 而 index.thread_name = "demo thread" 应被识别为对话标题写到 title。
    let tmp = TempDir::new().unwrap();
    write_index(
        tmp.path(),
        &[("sess-1", "2026-03-01T10:00:00Z", "demo thread")],
    );
    write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-1", "");

    let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, "sess-1");
    assert_eq!(sessions[0].project_path, None);
    assert_eq!(sessions[0].title.as_deref(), Some("demo thread"));
}

#[test]
fn test_scan_uses_session_meta_cwd_for_project_path() {
    // session_meta.payload.cwd 是真实工作目录，必须 win over index 字段。
    let tmp = TempDir::new().unwrap();
    let body = r#"{"type":"session_meta","timestamp":"2026-03-01T10:00:00Z","payload":{"cwd":"/Users/x/proj"}}
"#;
    write_index(
        tmp.path(),
        &[("sess-cwd", "2026-03-01T10:00:00Z", "thread-label")],
    );
    write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-cwd", body);

    let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions[0].project_path.as_deref(), Some("/Users/x/proj"));
    assert_eq!(sessions[0].title.as_deref(), Some("thread-label"));
}

#[test]
fn test_collect_parses_response_items() {
    let tmp = TempDir::new().unwrap();
    let body = r#"{"type":"session_meta","timestamp":"2026-03-01T10:00:00Z","payload":{"cwd":"/Users/x/proj"}}
{"type":"response_item","timestamp":"2026-03-01T10:00:01Z","payload":{"role":"user","content":[{"type":"input_text","text":"hello codex"}]}}
{"type":"response_item","timestamp":"2026-03-01T10:00:02Z","payload":{"role":"assistant","content":[{"type":"output_text","text":"hi from assistant"}]}}
"#;
    write_index(
        tmp.path(),
        &[("sess-collect", "2026-03-01T10:00:00Z", "thread")],
    );
    write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-collect", body);

    let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    let messages = adapter.collect(&sessions[0]).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(messages[0].content, "hello codex");
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(messages[1].content, "hi from assistant");
}

#[test]
fn test_environment_context_filtered() {
    let tmp = TempDir::new().unwrap();
    let body = r#"{"type":"response_item","timestamp":"2026-03-01T10:00:01Z","payload":{"role":"user","content":[{"type":"input_text","text":"<environment_context>OS=mac</environment_context>"},{"type":"input_text","text":"actual user prompt"}]}}
"#;
    write_index(tmp.path(), &[("sess-env", "2026-03-01T10:00:00Z", "t")]);
    write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-env", body);

    let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    let messages = adapter.collect(&sessions[0]).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].content, "actual user prompt");
}

#[test]
fn test_event_msg_last_agent_message_captured() {
    let tmp = TempDir::new().unwrap();
    let body = r#"{"type":"event_msg","timestamp":"2026-03-01T10:01:00Z","payload":{"type":"last_agent_message","last_agent_message":"Done. Next: deploy"}}
"#;
    write_index(tmp.path(), &[("sess-evt", "2026-03-01T10:00:00Z", "t")]);
    write_session(tmp.path(), "2026-03-01T10:00:00Z", "sess-evt", body);

    let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    let messages = adapter.collect(&sessions[0]).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].role, Role::Assistant);
    assert!(messages[0].content.contains("deploy"));
}

#[test]
fn test_missing_session_index_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let adapter = CodexAdapter::with_base_dir(tmp.path().to_path_buf());
    let sessions = adapter.scan().unwrap();
    assert!(sessions.is_empty());
}
