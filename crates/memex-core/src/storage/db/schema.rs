//! SQLite schema（v3）。表结构、FTS5 虚拟表，以及把 `chunks` 上的
//! INSERT / UPDATE / DELETE 同步到 `chunks_fts` 的影子触发器。
//!
//! v2 新增：
//! - `chunks.summary` 列，用来存 L1 的一句话摘要。
//! - `aggregate_summaries` 表，用来存 L3（项目）/ L4（周期）摘要。
//!
//! v3 新增：
//! - 索引 `idx_messages_session_role_offset` 在 `messages(session_id, role,
//!   source_offset)` 上 —— popup / dashboard 的"首条 user 消息预览"
//!   子查询必须用到它，否则会做全表扫描（实际数据库上有 ≥10× 加速）。
//! - 索引 `idx_summaries_session_level` 在 `summaries(session_id, level)` 上，
//!   能加速 `list_sessions_paged` 中的 `LEFT JOIN summaries`。

pub(super) const SCHEMA_VERSION: u32 = 6;

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
    metadata_json TEXT NOT NULL DEFAULT '{}',
    summary TEXT
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

CREATE TABLE IF NOT EXISTS summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    level TEXT NOT NULL,  -- 'L2_session'
    title TEXT,
    summary TEXT NOT NULL,
    topics_json TEXT NOT NULL DEFAULT '[]',
    decisions_json TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    -- v5: 摘要生成时该 session 的 message_count 快照。
    -- 用于「过期检测」：如果 sessions.message_count > 此值，说明
    -- 摘要生成后又有新消息写入，需要重新生成。
    -- 老库迁移时回填为当前 session 的 message_count（视为未过期）。
    message_count_at_creation INTEGER NOT NULL DEFAULT 0,
    UNIQUE(session_id, level)
);

CREATE TABLE IF NOT EXISTS aggregate_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scope_type TEXT NOT NULL,  -- 'project' | 'daily' | 'weekly'
    scope_key TEXT NOT NULL,
    title TEXT,
    summary TEXT NOT NULL,
    topics_json TEXT NOT NULL DEFAULT '[]',
    decisions_json TEXT NOT NULL DEFAULT '[]',
    session_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    UNIQUE(scope_type, scope_key)
);

-- v3: indexes for popup / dashboard list_sessions_paged hot path
CREATE INDEX IF NOT EXISTS idx_messages_session_role_offset
    ON messages(session_id, role, source_offset);

CREATE INDEX IF NOT EXISTS idx_summaries_session_level
    ON summaries(session_id, level);

CREATE INDEX IF NOT EXISTS idx_sessions_updated_at
    ON sessions(updated_at DESC);

-- v4: generic LLM provider registry
CREATE TABLE IF NOT EXISTS llm_providers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,           -- 'openai_compat' | 'anthropic' | 'ollama'
    base_url TEXT NOT NULL,
    model TEXT NOT NULL DEFAULT '',
    api_key TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    is_default INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'untested',  -- 'untested' | 'ok' | 'error'
    latency_ms INTEGER,
    updated_at TEXT NOT NULL
);
";
