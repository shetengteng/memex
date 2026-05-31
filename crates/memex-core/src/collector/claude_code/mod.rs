mod parser;

use std::fs;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, warn};

use super::Adapter;
use crate::storage::models::{RawMessage, SessionMeta};
use parser::{convert_claude_message, ClaudeMessage};

pub struct ClaudeCodeAdapter {
    base_dir: PathBuf,
}

impl ClaudeCodeAdapter {
    pub fn new() -> Self {
        let base_dir = dirs::home_dir()
            .expect("cannot determine home directory")
            .join(".claude")
            .join("projects");
        Self { base_dir }
    }

    #[cfg(test)]
    pub fn with_base_dir(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    fn discover_session_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        if !self.base_dir.exists() {
            return Ok(files);
        }
        for entry in walkdir::WalkDir::new(&self.base_dir)
            .min_depth(1)
            .max_depth(4)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                files.push(path.to_path_buf());
            }
        }
        Ok(files)
    }

    fn extract_project_path(&self, file_path: &Path) -> Option<String> {
        file_path
            .parent()
            .and_then(|p| p.strip_prefix(&self.base_dir).ok())
            .map(|rel| rel.to_string_lossy().to_string())
    }

    fn session_id_from_path(path: &Path) -> String {
        path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                blake3::hash(path.to_string_lossy().as_bytes())
                    .to_hex()
                    .to_string()
            })
    }
}

impl Default for ClaudeCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for ClaudeCodeAdapter {
    fn name(&self) -> &str {
        "claude_code"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let files = self.discover_session_files()?;
        let mut sessions = Vec::new();

        for file_path in files {
            let meta = fs::metadata(&file_path)?;
            let mtime = meta
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let session_id = Self::session_id_from_path(&file_path);
            let project_path = self.extract_project_path(&file_path);

            sessions.push(SessionMeta {
                id: session_id,
                source: "claude_code".to_string(),
                project_path,
                file_path: file_path.to_string_lossy().to_string(),
                last_offset: 0,
                mtime,
            });
        }

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let file = fs::File::open(path)
            .with_context(|| format!("failed to open {}", session.file_path))?;
        let file_size = file.metadata()?.len();

        if file_size <= session.last_offset {
            return Ok(Vec::new());
        }

        let mut reader = BufReader::new(file);
        if session.last_offset > 0 {
            reader.seek(SeekFrom::Start(session.last_offset))?;
        }

        let mut messages = Vec::new();
        let mut current_offset = session.last_offset;

        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    warn!("failed to read line at offset {}: {}", current_offset, e);
                    break;
                }
            };
            current_offset += line.len() as u64 + 1;

            if line.trim().is_empty() {
                continue;
            }

            let parsed: ClaudeMessage = match serde_json::from_str(&line) {
                Ok(m) => m,
                Err(e) => {
                    debug!("skipping malformed JSON line: {}", e);
                    continue;
                }
            };

            if let Some(raw_msg) = convert_claude_message(&parsed, &session.id, current_offset) {
                messages.push(raw_msg);
            }
        }

        Ok(messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;
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
            id: "session1".into(), source: "claude_code".into(),
            project_path: Some("project".into()),
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0, mtime: 0,
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
            id: "empty".into(), source: "claude_code".into(),
            project_path: None,
            file_path: file_path.to_string_lossy().to_string(),
            last_offset: 0, mtime: 0,
        };
        let messages = adapter.collect(&session).unwrap();
        assert!(messages.is_empty());
    }
}
