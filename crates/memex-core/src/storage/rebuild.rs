use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use tracing::{info, warn};

use super::db::Db;
use super::models::{RawMessage, Role};
use crate::processor;

pub struct RebuildStats {
    pub sessions: u64,
    pub messages: u64,
    pub chunks: u64,
    pub errors: u64,
}

pub fn rebuild_from_markdown(memex_dir: &Path, db: &Db) -> Result<RebuildStats> {
    let sessions_dir = memex_dir.join("sessions");
    if !sessions_dir.exists() {
        return Ok(RebuildStats { sessions: 0, messages: 0, chunks: 0, errors: 0 });
    }

    let mut stats = RebuildStats { sessions: 0, messages: 0, chunks: 0, errors: 0 };

    for entry in walkdir::WalkDir::new(&sessions_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.extension().is_some_and(|ext| ext != "md") {
            continue;
        }

        match rebuild_session_file(path, &sessions_dir, db) {
            Ok((msgs, chunks)) => {
                stats.sessions += 1;
                stats.messages += msgs;
                stats.chunks += chunks;
            }
            Err(e) => {
                warn!("rebuild: failed to process {}: {}", path.display(), e);
                stats.errors += 1;
            }
        }
    }

    info!("rebuild complete: {} sessions, {} messages, {} chunks, {} errors",
        stats.sessions, stats.messages, stats.chunks, stats.errors);
    Ok(stats)
}

fn rebuild_session_file(path: &Path, sessions_root: &Path, db: &Db) -> Result<(u64, u64)> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;

    let (frontmatter, body) = split_frontmatter(&content);
    let session_id = extract_field(&frontmatter, "session_id")
        .unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        });
    let source = extract_field(&frontmatter, "source").unwrap_or_else(|| {
        path.strip_prefix(sessions_root)
            .ok()
            .and_then(|p| p.components().next())
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string())
    });
    let project = extract_field(&frontmatter, "project");

    db.insert_session(&session_id, &source, project.as_deref(), &path.to_string_lossy())?;

    let messages = parse_messages_from_body(&body, &session_id);
    let mut msg_count = 0u64;
    let mut chunk_count = 0u64;

    for msg in &messages {
        let content_hash = blake3::hash(msg.content.as_bytes()).to_hex().to_string();
        let ts_str = msg.timestamp.map(|t| t.to_rfc3339());
        let inserted = db.insert_message(
            &msg.id, &session_id, &msg.role.to_string(),
            &msg.content, ts_str.as_deref(), msg.source_offset, &content_hash,
        )?;
        if inserted {
            msg_count += 1;
        }
    }

    let chunks = processor::process_messages(&messages)?;
    for chunk in &chunks {
        db.insert_chunk(chunk)?;
        chunk_count += 1;
    }

    Ok((msg_count, chunk_count))
}

fn split_frontmatter(content: &str) -> (String, String) {
    if !content.starts_with("---") {
        return (String::new(), content.to_string());
    }
    if let Some(end) = content[3..].find("\n---") {
        let fm = content[3..3 + end].to_string();
        let body = content[3 + end + 4..].to_string();
        (fm, body)
    } else {
        (String::new(), content.to_string())
    }
}

fn extract_field(frontmatter: &str, key: &str) -> Option<String> {
    let prefix = format!("{}: ", key);
    frontmatter
        .lines()
        .find(|line| line.trim_start().starts_with(&prefix))
        .map(|line| line.trim_start().strip_prefix(&prefix).unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
}

fn parse_messages_from_body(body: &str, session_id: &str) -> Vec<RawMessage> {
    let mut messages = Vec::new();
    let mut current_role: Option<Role> = None;
    let mut current_content = String::new();
    let mut msg_index: u64 = 0;

    for line in body.lines() {
        if let Some(role) = detect_message_header(line) {
            if let Some(prev_role) = current_role.take() {
                let trimmed = current_content.trim().to_string();
                if !trimmed.is_empty() {
                    let id = blake3::hash(
                        format!("rebuild:{}:{}:{}", session_id, msg_index, &trimmed[..trimmed.len().min(100)])
                            .as_bytes(),
                    ).to_hex().to_string();
                    messages.push(RawMessage {
                        id, session_id: session_id.to_string(),
                        role: prev_role, content: trimmed,
                        timestamp: None, source_offset: msg_index,
                    });
                    msg_index += 1;
                }
            }
            current_role = Some(role);
            current_content.clear();
        } else if line == "---" {
            continue;
        } else {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    if let Some(role) = current_role {
        let trimmed = current_content.trim().to_string();
        if !trimmed.is_empty() {
            let id = blake3::hash(
                format!("rebuild:{}:{}:{}", session_id, msg_index, &trimmed[..trimmed.len().min(100)])
                    .as_bytes(),
            ).to_hex().to_string();
            messages.push(RawMessage {
                id, session_id: session_id.to_string(),
                role, content: trimmed,
                timestamp: None, source_offset: msg_index,
            });
        }
    }

    messages
}

fn detect_message_header(line: &str) -> Option<Role> {
    let trimmed = line.trim();
    if trimmed.starts_with("## 👤 User") || trimmed.starts_with("## User") {
        Some(Role::User)
    } else if trimmed.starts_with("## 🤖 Assistant") || trimmed.starts_with("## Assistant") {
        Some(Role::Assistant)
    } else if trimmed.starts_with("## ⚙️ System") || trimmed.starts_with("## System") {
        Some(Role::System)
    } else if trimmed.starts_with("## 🔧 Tool") || trimmed.starts_with("## Tool") {
        Some(Role::Tool)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_frontmatter() {
        let content = "---\nsession_id: s1\nsource: test\n---\n\n## User\nhello\n";
        let (fm, body) = split_frontmatter(content);
        assert!(fm.contains("session_id: s1"));
        assert!(body.contains("hello"));
    }

    #[test]
    fn test_extract_field() {
        let fm = "session_id: abc-123\nsource: claude_code\nproject: memex\n";
        assert_eq!(extract_field(fm, "session_id").unwrap(), "abc-123");
        assert_eq!(extract_field(fm, "source").unwrap(), "claude_code");
        assert!(extract_field(fm, "missing").is_none());
    }

    #[test]
    fn test_parse_messages() {
        let body = "\n## 👤 User (2026-05-31 10:00:00)\n\nhello world\n\n---\n\n## 🤖 Assistant (2026-05-31 10:00:01)\n\nhi there\n";
        let messages = parse_messages_from_body(body, "s1");
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert!(messages[0].content.contains("hello world"));
        assert_eq!(messages[1].role, Role::Assistant);
    }

    #[test]
    fn test_detect_header() {
        assert_eq!(detect_message_header("## 👤 User"), Some(Role::User));
        assert_eq!(detect_message_header("## 🤖 Assistant (ts)"), Some(Role::Assistant));
        assert_eq!(detect_message_header("## System"), Some(Role::System));
        assert!(detect_message_header("normal line").is_none());
    }

    #[test]
    fn test_rebuild_roundtrip() {
        let tmp = tempfile::TempDir::new().unwrap();
        let sessions_dir = tmp.path().join("sessions").join("test_source");
        fs::create_dir_all(&sessions_dir).unwrap();

        let md = "---\nsession_id: s1\nsource: test_source\nproject: demo\n---\n\n## 👤 User\n\nhello rebuild\n\n---\n\n## 🤖 Assistant\n\nrebuilt response\n";
        fs::write(sessions_dir.join("s1.md"), md).unwrap();

        let db = Db::open_in_memory().unwrap();
        let stats = rebuild_from_markdown(tmp.path(), &db).unwrap();
        assert_eq!(stats.sessions, 1);
        assert_eq!(stats.messages, 2);
        assert!(stats.chunks > 0);
        assert_eq!(stats.errors, 0);
    }
}
