use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::debug;

use super::Adapter;
use crate::storage::models::{RawMessage, Role, SessionMeta};

pub struct AiderAdapter {
    scan_dirs: Vec<PathBuf>,
}

const MAX_SCAN_DEPTH: usize = 4;
const HISTORY_FILENAME: &str = ".aider.chat.history.md";

impl Default for AiderAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl AiderAdapter {
    pub fn new() -> Self {
        let home = dirs::home_dir().expect("cannot determine home directory");
        let scan_dirs: Vec<PathBuf> = ["Documents", "Projects", "projects", "code", "dev", "repos", "work", "src"]
            .iter()
            .map(|d| home.join(d))
            .filter(|p| p.exists())
            .collect();
        Self { scan_dirs }
    }

    #[cfg(test)]
    pub fn with_scan_dirs(scan_dirs: Vec<PathBuf>) -> Self {
        Self { scan_dirs }
    }

    fn discover_history_files(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        for dir in &self.scan_dirs {
            if !dir.exists() {
                continue;
            }
            for entry in walkdir::WalkDir::new(dir)
                .max_depth(MAX_SCAN_DEPTH)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = entry.path();
                if path.file_name().is_some_and(|n| n == HISTORY_FILENAME) {
                    files.push(path.to_path_buf());
                }
            }
        }
        files.sort();
        files.dedup();
        Ok(files)
    }

    fn project_path_from_file(path: &Path) -> Option<String> {
        path.parent().map(|p| p.to_string_lossy().to_string())
    }
}

/// Split a history file into individual sessions.
/// Sessions are delimited by lines matching `# aider chat started at <timestamp>`.
fn split_sessions(content: &str) -> Vec<(String, String)> {
    let mut sessions = Vec::new();
    let mut start_ts = String::new();

    let lines: Vec<&str> = content.lines().collect();
    let mut seg_start = 0;
    let mut has_start = false;

    for (i, line) in lines.iter().enumerate() {
        if line.starts_with("# aider chat started at ") {
            if has_start {
                let block = lines[seg_start..i].join("\n");
                sessions.push((start_ts.clone(), block));
            }
            start_ts = line
                .strip_prefix("# aider chat started at ")
                .unwrap_or("")
                .trim()
                .to_string();
            has_start = true;
            seg_start = i + 1;
        }
    }

    if has_start && seg_start < lines.len() {
        let block = lines[seg_start..].join("\n");
        sessions.push((start_ts, block));
    }

    sessions
}

/// Parse messages from a single session block.
fn parse_session_messages(session_id: &str, block: &str) -> Vec<RawMessage> {
    let mut messages = Vec::new();
    let mut current_role: Option<Role> = None;
    let mut current_content = String::new();
    let mut msg_index: usize = 0;

    let flush =
        |role: Role, content: &str, sid: &str, idx: &mut usize, out: &mut Vec<RawMessage>| {
            let text = content.trim();
            if text.is_empty() {
                return;
            }
            let id = blake3::hash(
                format!("{}{}{}", sid, *idx, super::safe_prefix(text, 100)).as_bytes(),
            )
            .to_hex()
            .to_string();
            out.push(RawMessage {
                id,
                session_id: sid.to_string(),
                role,
                content: text.to_string(),
                timestamp: None,
                source_offset: *idx as u64,
            });
            *idx += 1;
        };

    for line in block.lines() {
        if let Some(user_text) = line.strip_prefix("#### ") {
            if let Some(role) = current_role.take() {
                flush(role, &current_content, session_id, &mut msg_index, &mut messages);
                current_content.clear();
            }
            current_role = Some(Role::User);
            current_content.push_str(user_text);
            current_content.push('\n');
        } else if line.starts_with("> ") || line == ">" {
            if current_role != Some(Role::Tool) {
                if let Some(role) = current_role.take() {
                    flush(role, &current_content, session_id, &mut msg_index, &mut messages);
                    current_content.clear();
                }
                current_role = Some(Role::Tool);
            }
            let text = line.strip_prefix("> ").unwrap_or("");
            current_content.push_str(text);
            current_content.push('\n');
        } else {
            if current_role == Some(Role::User) || current_role == Some(Role::Tool) {
                if let Some(role) = current_role.take() {
                    flush(role, &current_content, session_id, &mut msg_index, &mut messages);
                    current_content.clear();
                }
                current_role = Some(Role::Assistant);
            } else if current_role.is_none() && !line.trim().is_empty() {
                current_role = Some(Role::Assistant);
            }
            if current_role.is_some() {
                current_content.push_str(line);
                current_content.push('\n');
            }
        }
    }

    if let Some(role) = current_role {
        flush(role, &current_content, session_id, &mut msg_index, &mut messages);
    }

    messages
}

impl Adapter for AiderAdapter {
    fn name(&self) -> &str {
        "aider"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        let files = self.discover_history_files()?;
        let mut sessions = Vec::new();

        for file_path in files {
            let meta = match fs::metadata(&file_path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let content = match fs::read_to_string(&file_path) {
                Ok(c) => c,
                Err(e) => {
                    debug!("aider: failed to read {}: {}", file_path.display(), e);
                    continue;
                }
            };

            let sess_blocks = split_sessions(&content);
            let project = Self::project_path_from_file(&file_path);

            for (i, (ts, _block)) in sess_blocks.iter().enumerate() {
                let sid = format!(
                    "aider-{}-{}",
                    blake3::hash(file_path.to_string_lossy().as_bytes())
                        .to_hex()
                        .to_string()[..12]
                        .to_string(),
                    i
                );
                sessions.push(SessionMeta {
                    id: sid,
                    source: "aider".to_string(),
                    project_path: project.clone(),
                    file_path: file_path.to_string_lossy().to_string(),
                    last_offset: i as u64,
                    mtime: mtime + i as u64,
                    created_secs: 0,
                });
                let _ = ts;
            }
        }

        Ok(sessions)
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        let path = Path::new(&session.file_path);
        if !path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(path)?;
        let sess_blocks = split_sessions(&content);
        let idx = session.last_offset as usize;

        if idx >= sess_blocks.len() {
            return Ok(Vec::new());
        }

        let (_ts, block) = &sess_blocks[idx];
        Ok(parse_session_messages(&session.id, block))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_aider_history() {
        let content = r#"# aider chat started at 2026-05-30 14:00:00

#### add a hello world function

Here's the function:

```python
def hello():
    print("hello world")
```

> Applied edit to main.py

#### now add tests

Sure, I'll add tests:

```python
def test_hello():
    hello()
```

> Applied edit to test_main.py
"#;
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join(HISTORY_FILENAME);
        fs::write(&file, content).unwrap();

        let adapter = AiderAdapter::with_scan_dirs(vec![tmp.path().to_path_buf()]);
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 1);

        let messages = adapter.collect(&sessions[0]).unwrap();
        assert!(messages.len() >= 4, "expected at least 4 messages, got {}", messages.len());

        assert_eq!(messages[0].role, Role::User);
        assert!(messages[0].content.contains("hello world"));
    }

    #[test]
    fn test_multiple_sessions_in_one_file() {
        let content = r#"# aider chat started at 2026-05-30 10:00:00

#### first session message

Response to first.

# aider chat started at 2026-05-30 14:00:00

#### second session message

Response to second.
"#;
        let tmp = TempDir::new().unwrap();
        let file = tmp.path().join(HISTORY_FILENAME);
        fs::write(&file, content).unwrap();

        let adapter = AiderAdapter::with_scan_dirs(vec![tmp.path().to_path_buf()]);
        let sessions = adapter.scan().unwrap();
        assert_eq!(sessions.len(), 2);

        let m1 = adapter.collect(&sessions[0]).unwrap();
        assert!(m1[0].content.contains("first session"));

        let m2 = adapter.collect(&sessions[1]).unwrap();
        assert!(m2[0].content.contains("second session"));
    }
}
