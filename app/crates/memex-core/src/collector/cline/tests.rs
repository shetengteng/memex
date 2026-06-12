use std::fs;

use tempfile::TempDir;

use super::*;
use crate::storage::models::Role;

#[test]
fn test_parse_cline_task() {
    let tmp = TempDir::new().unwrap();
    let task_dir = tmp.path().join("task-abc123");
    fs::create_dir_all(&task_dir).unwrap();

    let conv = r#"[
        {"role": "user", "content": [{"type": "text", "text": "Fix the bug in auth.rs"}]},
        {"role": "assistant", "content": "I'll look at the auth module and fix the issue."},
        {"role": "user", "content": "Thanks, now deploy it"},
        {"role": "assistant", "content": "Done, deployed successfully."}
    ]"#;
    fs::write(task_dir.join("api_conversation_history.json"), conv).unwrap();

    let meta = r#"{"task": "Fix auth bug"}"#;
    fs::write(task_dir.join("task_metadata.json"), meta).unwrap();

    let adapter = ClineAdapter::with_task_dirs(vec![tmp.path().to_path_buf()]);
    let sessions = adapter.scan().unwrap();
    assert_eq!(sessions.len(), 1);
    // Cline 没有暴露 cwd，project_path 留空；task 描述当对话标题。
    assert_eq!(sessions[0].project_path, None);
    assert_eq!(sessions[0].title.as_deref(), Some("Fix auth bug"));

    let messages = adapter.collect(&sessions[0]).unwrap();
    assert_eq!(messages.len(), 4);
    assert_eq!(messages[0].role, Role::User);
    assert!(messages[0].content.contains("auth.rs"));
    assert_eq!(messages[1].role, Role::Assistant);
}

#[test]
fn test_empty_tasks_dir() {
    let tmp = TempDir::new().unwrap();
    let adapter = ClineAdapter::with_task_dirs(vec![tmp.path().to_path_buf()]);
    let sessions = adapter.scan().unwrap();
    assert!(sessions.is_empty());
}

#[test]
fn test_task_without_conversation_file_skipped() {
    let tmp = TempDir::new().unwrap();
    let task_dir = tmp.path().join("task-no-conv");
    fs::create_dir_all(&task_dir).unwrap();
    fs::write(task_dir.join("task_metadata.json"), "{}").unwrap();

    let adapter = ClineAdapter::with_task_dirs(vec![tmp.path().to_path_buf()]);
    let sessions = adapter.scan().unwrap();
    assert!(sessions.is_empty());
}
