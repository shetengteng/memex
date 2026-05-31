use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::models::{RawMessage, Role};

pub fn session_md_path(base_dir: &Path, session_id: &str, source: &str) -> PathBuf {
    base_dir
        .join("sessions")
        .join(source)
        .join(format!("{}.md", session_id))
}

pub fn write_session_markdown(
    base_dir: &Path,
    session_id: &str,
    source: &str,
    project_path: Option<&str>,
    messages: &[RawMessage],
) -> Result<()> {
    let path = session_md_path(base_dir, session_id, source);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory: {}", parent.display()))?;
    }

    let mut content = String::new();

    if !path.exists() {
        content.push_str("---\n");
        content.push_str(&format!("session_id: {}\n", session_id));
        content.push_str(&format!("source: {}\n", source));
        if let Some(proj) = project_path {
            content.push_str(&format!("project: {}\n", proj));
        }
        content.push_str(&format!("created: {}\n", chrono::Utc::now().to_rfc3339()));
        content.push_str("---\n\n");
    }

    for msg in messages {
        content.push_str(&format_message(msg));
    }

    if path.exists() {
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut f| {
                use std::io::Write;
                f.write_all(content.as_bytes())
            })
            .with_context(|| format!("failed to append to {}", path.display()))?;
    } else {
        fs::write(&path, &content)
            .with_context(|| format!("failed to write {}", path.display()))?;
    }

    Ok(())
}

fn format_message(msg: &RawMessage) -> String {
    let mut s = String::new();
    let role_header = match msg.role {
        Role::User => "## 👤 User",
        Role::Assistant => "## 🤖 Assistant",
        Role::System => "## ⚙️ System",
        Role::Tool => "## 🔧 Tool",
    };
    s.push_str(role_header);
    if let Some(ts) = &msg.timestamp {
        s.push_str(&format!(" ({})", ts.format("%Y-%m-%d %H:%M:%S")));
    }
    s.push_str("\n\n");
    s.push_str(&msg.content);
    s.push_str("\n\n---\n\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_write_session_markdown() {
        let tmp = TempDir::new().unwrap();
        let messages = vec![RawMessage {
            id: "m1".to_string(),
            session_id: "s1".to_string(),
            role: Role::User,
            content: "Hello, how do I use redis?".to_string(),
            timestamp: None,
            source_offset: 0,
        }];

        write_session_markdown(tmp.path(), "s1", "claude_code", Some("/proj"), &messages).unwrap();

        let md_path = session_md_path(tmp.path(), "s1", "claude_code");
        assert!(md_path.exists());
        let content = fs::read_to_string(&md_path).unwrap();
        assert!(content.contains("session_id: s1"));
        assert!(content.contains("source: claude_code"));
        assert!(content.contains("Hello, how do I use redis?"));
    }

    #[test]
    fn test_append_messages() {
        let tmp = TempDir::new().unwrap();
        let msg1 = vec![RawMessage {
            id: "m1".to_string(),
            session_id: "s1".to_string(),
            role: Role::User,
            content: "first message".to_string(),
            timestamp: None,
            source_offset: 0,
        }];
        write_session_markdown(tmp.path(), "s1", "claude_code", None, &msg1).unwrap();

        let msg2 = vec![RawMessage {
            id: "m2".to_string(),
            session_id: "s1".to_string(),
            role: Role::Assistant,
            content: "second message".to_string(),
            timestamp: None,
            source_offset: 100,
        }];
        write_session_markdown(tmp.path(), "s1", "claude_code", None, &msg2).unwrap();

        let md_path = session_md_path(tmp.path(), "s1", "claude_code");
        let content = fs::read_to_string(&md_path).unwrap();
        assert!(content.contains("first message"));
        assert!(content.contains("second message"));
    }
}
