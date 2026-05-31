//! SQLite schema (v1). Tables, FTS5 virtual table, and shadow triggers that
//! mirror INSERT / UPDATE / DELETE on `chunks` into `chunks_fts`.

pub(super) const SCHEMA_VERSION: u32 = 1;

pub(super) const SCHEMA_SQL: &str = "
CREATE TABLE IF NOT EXISTS sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    adapter TEXT NOT NULL,
    file_path TEXT NOT NULL UNIQUE,
    last_offset INTEGER NOT NULL DEFAULT 0,
    last_mtime INTEGER NOT NULL DEFAULT 0,
    last_scan TEXT NOT NULL DEFAULT ''
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    project_path TEXT,
    file_path TEXT NOT NULL,
    title TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    message_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    timestamp TEXT,
    source_offset INTEGER NOT NULL DEFAULT 0,
    content_hash TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS chunks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL REFERENCES messages(id),
    session_id TEXT NOT NULL REFERENCES sessions(id),
    chunk_type TEXT NOT NULL,
    content TEXT NOT NULL,
    redacted_content TEXT,
    position INTEGER NOT NULL DEFAULT 0,
    token_count INTEGER NOT NULL DEFAULT 0,
    metadata_json TEXT NOT NULL DEFAULT '{}'
);

CREATE VIRTUAL TABLE IF NOT EXISTS chunks_fts USING fts5(
    content,
    content='chunks',
    content_rowid='id',
    tokenize='unicode61'
);

CREATE TRIGGER IF NOT EXISTS chunks_ai AFTER INSERT ON chunks BEGIN
    INSERT INTO chunks_fts(rowid, content)
    VALUES (new.id, new.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_ad AFTER DELETE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content)
    VALUES ('delete', old.id, old.content);
END;

CREATE TRIGGER IF NOT EXISTS chunks_au AFTER UPDATE ON chunks BEGIN
    INSERT INTO chunks_fts(chunks_fts, rowid, content)
    VALUES ('delete', old.id, old.content);
    INSERT INTO chunks_fts(rowid, content)
    VALUES (new.id, new.content);
END;

CREATE TABLE IF NOT EXISTS access_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query TEXT NOT NULL,
    result_count INTEGER NOT NULL DEFAULT 0,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    date TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value INTEGER NOT NULL DEFAULT 0,
    UNIQUE(date, metric_name)
);

CREATE TABLE IF NOT EXISTS redactions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    message_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    redaction_type TEXT NOT NULL,
    original_length INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
";
