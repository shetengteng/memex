use super::*;

use anyhow::anyhow;
use tempfile::TempDir;

use crate::storage::markdown;
use crate::storage::models::{RawMessage, Role, SessionMeta};

struct FakeAdapter {
    sessions: Vec<SessionMeta>,
    messages: Vec<RawMessage>,
    fail_collect: bool,
}

impl Adapter for FakeAdapter {
    fn name(&self) -> &str {
        "fake"
    }

    fn scan(&self) -> Result<Vec<SessionMeta>> {
        Ok(self.sessions.clone())
    }

    fn collect(&self, session: &SessionMeta) -> Result<Vec<RawMessage>> {
        if self.fail_collect {
            return Err(anyhow!("collect failed"));
        }

        Ok(self
            .messages
            .iter()
            .filter(|msg| msg.session_id == session.id)
            .cloned()
            .collect())
    }
}

fn session(id: &str) -> SessionMeta {
    SessionMeta {
        id: id.to_string(),
        source: "fake".to_string(),
        project_path: Some("/tmp/project".to_string()),
        file_path: format!("/tmp/{id}.jsonl"),
        last_offset: 0,
        mtime: 100,
        created_secs: 10,
        title: Some("Fake session".to_string()),
    }
}

fn message(id: &str, session_id: &str, content: &str, source_offset: u64) -> RawMessage {
    RawMessage {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role: Role::User,
        content: content.to_string(),
        timestamp: None,
        source_offset,
    }
}

#[test]
fn ingest_adapter_persists_session_messages_chunks_source_and_markdown() {
    let temp = TempDir::new().unwrap();
    let db = Db::open_in_memory().unwrap();
    let adapter = FakeAdapter {
        sessions: vec![session("s1")],
        messages: vec![
            message("m1", "s1", "first message about redis", 10),
            message("m2", "s1", "second message about sqlite", 22),
        ],
        fail_collect: false,
    };

    let (messages, chunks) = ingest_adapter(&adapter, &db, temp.path()).unwrap();

    assert_eq!(messages, 2);
    assert_eq!(chunks, 2);
    assert_eq!(db.get_source_offset("/tmp/s1.jsonl").unwrap(), 22);

    let detail = db.get_session_detail("s1").unwrap().unwrap();
    assert_eq!(detail.messages.len(), 2);
    assert_eq!(detail.title.as_deref(), Some("Fake session"));

    let md_path = markdown::session_md_path(temp.path(), "s1", "fake");
    assert!(md_path.exists(), "ingest should write the session markdown");
}

#[test]
fn ingest_adapter_deduplicates_messages_before_chunking() {
    let temp = TempDir::new().unwrap();
    let db = Db::open_in_memory().unwrap();
    let adapter = FakeAdapter {
        sessions: vec![session("s1")],
        messages: vec![
            message("m1", "s1", "same content", 10),
            message("m2", "s1", "same content", 20),
        ],
        fail_collect: false,
    };

    let (messages, chunks) = ingest_adapter(&adapter, &db, temp.path()).unwrap();

    assert_eq!(messages, 1);
    assert_eq!(chunks, 1);
    let detail = db.get_session_detail("s1").unwrap().unwrap();
    assert_eq!(detail.messages.len(), 1);
}

#[test]
fn ingest_adapter_upserts_session_even_when_no_messages() {
    let temp = TempDir::new().unwrap();
    let db = Db::open_in_memory().unwrap();
    let adapter = FakeAdapter {
        sessions: vec![session("empty")],
        messages: vec![],
        fail_collect: false,
    };

    let (messages, chunks) = ingest_adapter(&adapter, &db, temp.path()).unwrap();

    assert_eq!(messages, 0);
    assert_eq!(chunks, 0);
    let detail = db.get_session_detail("empty").unwrap().unwrap();
    assert_eq!(detail.message_count, 0);
    assert_eq!(detail.title.as_deref(), Some("Fake session"));

    let md_path = markdown::session_md_path(temp.path(), "empty", "fake");
    assert!(
        !md_path.exists(),
        "empty sessions should not create markdown without messages"
    );
}

#[test]
fn ingest_adapter_updates_source_offset_across_batches() {
    let temp = TempDir::new().unwrap();
    let db = Db::open_in_memory().unwrap();
    let messages = (1..=105)
        .map(|i| message(&format!("m{i}"), "s1", &format!("unique message {i}"), i))
        .collect();
    let adapter = FakeAdapter {
        sessions: vec![session("s1")],
        messages,
        fail_collect: false,
    };

    let (messages, chunks) = ingest_adapter(&adapter, &db, temp.path()).unwrap();

    assert_eq!(messages, 105);
    assert_eq!(chunks, 105);
    assert_eq!(db.get_source_offset("/tmp/s1.jsonl").unwrap(), 105);
}

#[test]
fn ingest_adapter_returns_collect_error() {
    let temp = TempDir::new().unwrap();
    let db = Db::open_in_memory().unwrap();
    let adapter = FakeAdapter {
        sessions: vec![session("s1")],
        messages: vec![],
        fail_collect: true,
    };

    let err = ingest_adapter(&adapter, &db, temp.path()).unwrap_err();

    assert!(err.to_string().contains("collect failed"));
    assert!(db.get_session_detail("s1").unwrap().is_none());
}
