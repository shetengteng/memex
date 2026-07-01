//! Kiro IDE adapter.
//!
//! Kiro 是 Continue.dev 的 fork，`history[].message.{role,content,id}` schema
//! 与 Continue 完全一致 —— 差异只在**数据落盘的目录结构**：
//!
//! ```text
//! ~/Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/
//! └── workspace-sessions/
//!     └── <base64url(workspace_abs_path)>/
//!         ├── sessions.json          # 该 workspace 的会话索引
//!         └── <uuid>.json            # 每条 session 的完整 history
//! ```
//!
//! 索引条目里的 `workspaceDirectory` 是**纯路径**（无 `file://` 前缀）、
//! `dateCreated` 是**毫秒** epoch 字符串（不是 Continue 的秒 / ISO）。
//! 消息解析逻辑与 ContinueAdapter 相同，用同一份 serde struct，只是走
//! 「先遍历 workspace 子目录，再读每个目录里的 sessions.json」两层扫描。
//!
//! 仅 macOS 有官方发行；Linux/Windows 上 `base_dir` 会指向不存在的路径，
//! `scan()` 直接返回空。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::debug;

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct KiroAdapter {
    base_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct SessionIndex {
    #[serde(rename = "sessionId")]
    session_id: String,
    #[serde(rename = "workspaceDirectory")]
    workspace_directory: Option<String>,
    /// 毫秒 epoch，字符串形式。可能缺失 —— 老会话没这个字段。
    #[serde(rename = "dateCreated")]
    date_created: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SessionFile {
    #[serde(default)]
    history: Vec<HistoryItem>,
}

#[derive(Debug, Deserialize)]
struct HistoryItem {
    message: Option<KiroMessage>,
}

#[derive(Debug, Deserialize)]
struct KiroMessage {
    role: Option<String>,
    content: Option<serde_json::Value>,
    id: Option<String>,
}

impl Default for KiroAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl KiroAdapter {
    pub fn new() -> Self {
        // Kiro 目前只在 macOS 有官方发行；其他平台 base_dir 不会存在，scan() 直接返回空。
        let base_dir = dirs::home_dir()
            .expect("INVARIANT: home directory must be resolvable")
            .join("Library/Application Support/Kiro/User/globalStorage/kiro.kiroagent/workspace-sessions");
        Self { base_dir }
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn extract_text(content: &serde_json::Value) -> String {
        match content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(arr) => arr
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .and_then(|t| t.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<_>>()
                .join("\n"),
            _ => String::new(),
        }
    }
}

/// Kiro `dateCreated` 是毫秒 epoch 字符串（如 `"1782375182036"`）。
/// 解析失败或缺失时返回 0 —— 让下游 `mtime` fallback 处理。
fn parse_ms_epoch(s: Option<&str>) -> u64 {
    s.and_then(|v| v.parse::<u64>().ok())
        .map(|ms| ms / 1000)
        .unwrap_or(0)
}

impl Adapter for KiroAdapter {
    fn name(&self) -> &str {
        "kiro"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut sessions = Vec::new();
        let read_root = match fs::read_dir(&self.base_dir) {
            Ok(rd) => rd,
            Err(e) => {
                debug!("kiro: failed to read {}: {}", self.base_dir.display(), e);
                return Ok(Vec::new());
            }
        };

        for ws_entry in read_root.flatten() {
            let ws_dir = ws_entry.path();
            if !ws_dir.is_dir() {
                continue;
            }
            let idx_path = ws_dir.join("sessions.json");
            if !idx_path.exists() {
                continue;
            }

            let idx_content = match fs::read_to_string(&idx_path)
                .with_context(|| format!("failed to read {}", idx_path.display()))
            {
                Ok(c) => c,
                Err(e) => {
                    debug!("kiro: {}", e);
                    continue;
                }
            };

            let entries: Vec<SessionIndex> = match serde_json::from_str(&idx_content) {
                Ok(e) => e,
                Err(e) => {
                    debug!("kiro: failed to parse {}: {}", idx_path.display(), e);
                    continue;
                }
            };

            for entry in entries {
                let file_path = ws_dir.join(format!("{}.json", entry.session_id));
                if !file_path.exists() {
                    continue;
                }

                let mtime = fs::metadata(&file_path)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // 优先用索引里的 dateCreated（Kiro 自己写的会话创建时间），
                // 缺失时退到文件系统 created()。
                let created_from_idx = parse_ms_epoch(entry.date_created.as_deref());
                let created_secs = if created_from_idx > 0 {
                    created_from_idx
                } else {
                    fs::metadata(&file_path)
                        .ok()
                        .and_then(|m| m.created().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0)
                };

                sessions.push(SessionMeta {
                    id: entry.session_id.clone(),
                    source: "kiro".to_string(),
                    project_path: entry.workspace_directory.filter(|s| !s.is_empty()),
                    file_path: file_path.to_string_lossy().to_string(),
                    last_offset: 0,
                    mtime,
                    created_secs,
                    title: None,
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

        let parsed: SessionFile = match serde_json::from_str(&content) {
            Ok(s) => s,
            Err(e) => {
                debug!("kiro: failed to parse {}: {}", session.file_path, e);
                return Ok(Vec::new());
            }
        };

        let mut messages = Vec::new();

        for (i, item) in parsed.history.iter().enumerate() {
            let msg = match &item.message {
                Some(m) => m,
                None => continue,
            };

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

            let id = msg.id.clone().unwrap_or_else(|| {
                blake3::hash(
                    format!("{}{}{}", session.id, i, super::safe_prefix(&text, 100)).as_bytes(),
                )
                .to_hex()
                .to_string()
            });

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

    fn ws_key(path: &str) -> String {
        // 测试只需要一个跟 Kiro 目录结构同构、且文件系统安全的名字。
        // Kiro 真实编码是 base64url，但 scan() 逻辑并不解码这个 key，
        // 只是遍历子目录，所以测试里用 `_` 替 `/` 就够了。
        path.replace('/', "_")
    }

    #[test]
    fn test_parse_kiro_session() {
        let tmp = TempDir::new().unwrap();
        let ws_dir = tmp.path().join(ws_key("/tmp/proj"));
        fs::create_dir_all(&ws_dir).unwrap();

        let index = r#"[{
            "sessionId": "sess-001",
            "title": "hello kiro",
            "workspaceDirectory": "/tmp/proj",
            "dateCreated": "1782375182036"
        }]"#;
        fs::write(ws_dir.join("sessions.json"), index).unwrap();

        let session_data = r#"{
            "sessionId": "sess-001",
            "history": [
                { "message": { "role": "user",      "content": [{"type":"text","text":"hi kiro"}], "id": "m1" } },
                { "message": { "role": "assistant", "content": "On it.",                            "id": "m2" } }
            ]
        }"#;
        fs::write(ws_dir.join("sess-001.json"), session_data).unwrap();

        let adapter = KiroAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].source, "kiro");
        assert_eq!(sessions[0].project_path.as_deref(), Some("/tmp/proj"));
        // 1782375182036 ms → 1782375182 s
        assert_eq!(sessions[0].created_secs, 1_782_375_182);

        let messages = adapter.collect(&sessions[0]).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert!(messages[0].content.contains("hi kiro"));
        assert_eq!(messages[1].role, Role::Assistant);
        assert!(messages[1].content.contains("On it."));
    }

    #[test]
    fn test_multi_workspace_scan() {
        // 两个不同 workspace 目录，各自独立的 sessions.json —— 都要被扫到。
        let tmp = TempDir::new().unwrap();
        for (path, sid) in [("/a", "s-a"), ("/b", "s-b")] {
            let ws = tmp.path().join(ws_key(path));
            fs::create_dir_all(&ws).unwrap();
            fs::write(
                ws.join("sessions.json"),
                format!(
                    r#"[{{"sessionId":"{sid}","workspaceDirectory":"{path}","dateCreated":"1000000"}}]"#
                ),
            )
            .unwrap();
            fs::write(
                ws.join(format!("{sid}.json")),
                r#"{"history":[{"message":{"role":"user","content":"x","id":"i"}}]}"#,
            )
            .unwrap();
        }
        let adapter = KiroAdapter::with_base_dir(tmp.path().to_path_buf());
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_missing_base_dir_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = KiroAdapter::with_base_dir(tmp.path().join("does-not-exist"));
        assert!(adapter.scan().unwrap().is_empty());
    }

    #[test]
    fn test_ignores_workspace_dir_without_index() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("empty_ws")).unwrap();
        let adapter = KiroAdapter::with_base_dir(tmp.path().to_path_buf());
        assert!(adapter.scan().unwrap().is_empty());
    }
}
