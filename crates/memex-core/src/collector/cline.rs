use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct ClineAdapter {
    task_dirs: Vec<PathBuf>,
}

const EXTENSION_ID: &str = "saoudrizwan.claude-dev";

#[derive(Debug, Deserialize)]
struct ClineMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct TaskMetadata {
    task: Option<String>,
    #[serde(rename = "tokensIn")]
    _tokens_in: Option<u64>,
    #[serde(rename = "tokensOut")]
    _tokens_out: Option<u64>,
}

impl Default for ClineAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClineAdapter {
    pub fn new() -> Self {
        let mut task_dirs = Vec::new();

        #[cfg(target_os = "macos")]
        {
            if let Some(support) = dirs::data_dir() {
                // VS Code 下的 Cline 任务目录
                let vsc = support
                    .join("Code/User/globalStorage")
                    .join(EXTENSION_ID)
                    .join("tasks");
                if vsc.exists() {
                    task_dirs.push(vsc);
                }
                // Cursor 下的 Cline 任务目录
                let cursor = support
                    .join("Cursor/User/globalStorage")
                    .join(EXTENSION_ID)
                    .join("tasks");
                if cursor.exists() {
                    task_dirs.push(cursor);
                }
            }
        }

        #[cfg(target_os = "linux")]
        {
            if let Some(config) = dirs::config_dir() {
                let vsc = config
                    .join("Code/User/globalStorage")
                    .join(EXTENSION_ID)
                    .join("tasks");
                if vsc.exists() {
                    task_dirs.push(vsc);
                }
            }
        }

        // CLI 模式下的 Cline 任务目录
        if let Some(home) = dirs::home_dir() {
            let cli_dir = home.join(".cline").join("data").join("tasks");
            if cli_dir.exists() {
                task_dirs.push(cli_dir);
            }
        }

        Self { task_dirs }
    }

    #[cfg(test)]
    pub fn with_task_dirs(task_dirs: Vec<PathBuf>) -> Self {
        Self { task_dirs }
    }

    fn extract_text(content: &serde_json::Value) -> String {
        match content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|item| {
                    if let Some(t) = item.get("text").and_then(|t| t.as_str()) {
                        Some(t.to_string())
                    } else if let Some(t) = item.get("content").and_then(|c| c.as_str()) {
                        Some(t.to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }

    fn read_task_prompt(task_dir: &Path) -> Option<String> {
        let meta_path = task_dir.join("task_metadata.json");
        if meta_path.exists() {
            if let Ok(content) = fs::read_to_string(&meta_path) {
                if let Ok(meta) = serde_json::from_str::<TaskMetadata>(&content) {
                    return meta.task;
                }
            }
        }
        None
    }
}

impl Adapter for ClineAdapter {
    fn name(&self) -> &str {
        "cline"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let mut sessions = Vec::new();

        for tasks_root in &self.task_dirs {
            let entries = match fs::read_dir(tasks_root) {
                Ok(e) => e,
                Err(_) => continue,
            };

            for entry in entries.filter_map(|e| e.ok()) {
                let task_dir = entry.path();
                if !task_dir.is_dir() {
                    continue;
                }

                let conv_file = task_dir.join("api_conversation_history.json");
                if !conv_file.exists() {
                    continue;
                }

                let task_id = task_dir
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                let mtime = fs::metadata(&conv_file)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // task_metadata.task 是用户初始 prompt（任务描述），
                // 并不是 cwd —— 当对话标题更合适。Cline 当前没有暴露 cwd 字段，
                // project_path 留空，等真有 cwd 来源再补。
                let title = Self::read_task_prompt(&task_dir);

                let created_secs = fs::metadata(&conv_file)
                    .ok()
                    .and_then(|m| m.created().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                sessions.push(SessionMeta {
                    id: format!("cline-{}", task_id),
                    source: "cline".to_string(),
                    project_path: None,
                    file_path: conv_file.to_string_lossy().to_string(),
                    last_offset: 0,
                    mtime,
                    created_secs,
                    title,
                });
            }
        }

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", session.file_path))?;

        let parsed: Vec<ClineMessage> = match serde_json::from_str(&content) {
            Ok(msgs) => msgs,
            Err(e) => {
                debug!("cline: failed to parse {}: {}", session.file_path, e);
                return Ok(Vec::new());
            }
        };

        let mut messages = Vec::new();

        for (i, msg) in parsed.iter().enumerate() {
            let role_str = match msg.role.as_deref() {
                Some(r) => r,
                None => continue,
            };
            let role = match role_str {
                "user" | "human" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "tool" => Role::Tool,
                _ => continue,
            };

            let text = match &msg.content {
                Some(c) => Self::extract_text(c),
                None => continue,
            };

            if text.trim().is_empty() {
                continue;
            }

            let id = blake3::hash(
                format!("{}{}{}", session.id, i, super::safe_prefix(&text, 100)).as_bytes(),
            )
            .to_hex()
            .to_string();

            messages.push(RawMessage {
                id,
                session_id: session.id.clone(),
                role,
                content: text,
                timestamp: None,
                source_offset: i as u64,
            });
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

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
}
