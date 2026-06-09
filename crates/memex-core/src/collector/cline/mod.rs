//! Cline adapter —— 解析 Cline VS Code / Cursor / CLI 扩展写到磁盘的
//! `tasks/<task_id>/` 目录里的 `api_conversation_history.json`。
//!
//! 拆分：把 `#[cfg(test)] mod tests` 抽到 sibling `tests.rs`，主文件只保留
//! 扩展安装位置探测 + `Adapter` 实现。

#[cfg(test)]
mod tests;

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
                    item.get("text")
                        .and_then(|t| t.as_str())
                        .or_else(|| item.get("content").and_then(|c| c.as_str()))
                        .map(str::to_string)
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }

    fn read_task_prompt(task_dir: &Path) -> Option<String> {
        let meta_path = task_dir.join("task_metadata.json");
        if meta_path.exists()
            && let Ok(content) = fs::read_to_string(&meta_path)
            && let Ok(meta) = serde_json::from_str::<TaskMetadata>(&content)
        {
            return meta.task;
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
