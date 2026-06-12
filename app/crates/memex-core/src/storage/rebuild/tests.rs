use std::fs;

use super::*;
use crate::storage::db::Db;

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
    assert_eq!(
        detect_message_header("## 🤖 Assistant (ts)"),
        Some(Role::Assistant)
    );
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

#[test]
fn test_rebuild_search_consistency() {
    let tmp = tempfile::TempDir::new().unwrap();
    let sessions_dir = tmp.path().join("sessions").join("claude_code");
    fs::create_dir_all(&sessions_dir).unwrap();

    let md = "---\nsession_id: s-redis\nsource: claude_code\nproject: memex\n---\n\n## 👤 User\n\nHow do I use redis pipeline?\n\n---\n\n## 🤖 Assistant\n\nUse MULTI/EXEC for redis pipeline operations.\n";
    fs::write(sessions_dir.join("s-redis.md"), md).unwrap();

    let db = Db::open_in_memory().unwrap();
    rebuild_from_markdown(tmp.path(), &db).unwrap();

    let results = db.fts_search("redis", 10).unwrap();
    assert!(
        !results.is_empty(),
        "search after rebuild should find 'redis'"
    );
    assert!(results.iter().any(|r| r.content.contains("redis")));
}
