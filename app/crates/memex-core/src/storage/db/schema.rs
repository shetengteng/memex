//! Memex SQLite baseline schema.
//!
//! This is the source of truth for the latest table / index / FTS5 /
//! trigger shape. [`super::migrations::build_migrations`] embeds it as
//! the only `M::up(...)` entry, so a fresh DB ends up with exactly this
//! layout and a pre-existing DB is reset to the same layout
//! (`rusqlite_migration` tracks the applied version via PRAGMA
//! `user_version`).
//!
//! When the schema changes:
//! * append the additive DDL (`ALTER TABLE …`, `CREATE INDEX …`) as a
//!   **new** `M::up(...)` in `migrations.rs`, and
//! * mirror the same final state into the constant below so a fresh
//!   install does not need to replay every historical migration.

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
    message_count INTEGER NOT NULL DEFAULT 0,
    -- v7: intent —— L2 摘要从原始对话推断出的「用户真实意图」一句话，
    -- 用于 Library 列表行二级文字与 detail 弹框的「用户意图」段。
    -- 不带摘要的会话此列为 NULL；摘要重生成时会覆盖。
    intent TEXT
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

-- v8: 给 workload_report 的 heatmap message 通路加索引。
-- 之前 messages 表上仅有 (session_id, role, source_offset) 索引，
-- 按 timestamp 做范围扫描会全表扫 30w+ 行，30 天查询要 6+ 秒，
-- 导致 Insights 趋势 tab 在大库上加载超时显示空白。
-- 局部索引（partial index）只覆盖非空 timestamp，体积小，命中率高。
CREATE INDEX IF NOT EXISTS idx_messages_timestamp
    ON messages(timestamp)
    WHERE timestamp IS NOT NULL;

-- v9: 把 v6 加进来过、但只在 migration 里建过的两个 hot path 索引
-- 也放到 SCHEMA_SQL 里——这样新装库会自动有，老库 v9 migration 也会补建。
--
-- (a) idx_chunks_has_summary：chunks_with_summary_count 用于「摘要进度
--     百分比」展示。chunks 表 70w+ 行，无索引时 COUNT(*) 要 15+ 秒；
--     局部索引只覆盖 summary IS NOT NULL 的行（实际只占 ~0.03%），
--     COUNT 降到 ~500ms（~30× 提速）。
--
-- (b) idx_messages_content_dedup：ingest 每条新消息都跑
--     `EXISTS(SELECT 1 FROM messages WHERE content_hash = ?1 AND session_id = ?2)`
--     做幂等去重。原本只有 (session_id, role, source_offset)，dedup
--     退化为「按 session 全扫比对 content_hash」，大 session（3000+ 条）
--     上一次 dedup 50+ ms，一次 ingest 1000 条消息要 1 分钟。
--     加 (content_hash, session_id) 复合索引后单次 dedup ~17 ms（3× 提速）。
CREATE INDEX IF NOT EXISTS idx_chunks_has_summary
    ON chunks(id) WHERE summary IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_messages_content_dedup
    ON messages(content_hash, session_id);

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

-- v10: threads —— L5 \"主题线索\" 的 N:N 中间表。
-- LLM 把多个 session 的 L2 摘要聚类成「线索」（如 'memex 桌面化迁移'、
-- 'cursor 适配器问题'），同一个 session 可以属于多个 thread。
-- 数据来源：try_l5_thread_clustering（在 ingest.rs 中）。
-- 重新生成：每次只有手动/周期触发，结果会增量插入 + 已不再相关的 thread 不删除（保留历史）。
CREATE TABLE IF NOT EXISTS threads (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    summary TEXT NOT NULL DEFAULT '',
    session_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(name)
);

CREATE TABLE IF NOT EXISTS thread_sessions (
    thread_id INTEGER NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
    session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
    confidence REAL NOT NULL DEFAULT 1.0,
    created_at TEXT NOT NULL,
    PRIMARY KEY (thread_id, session_id)
);

-- 反查\"一个 session 属于哪些 thread\" 的热路径。
CREATE INDEX IF NOT EXISTS idx_thread_sessions_session
    ON thread_sessions(session_id);

CREATE INDEX IF NOT EXISTS idx_threads_updated_at
    ON threads(updated_at DESC);

-- v11: mcp_call_log —— MCP 工具单次调用明细。
-- metrics 表只能按日按 metric_name 聚合，对 \"哪个工具被调了多少次 / 平均延迟
-- / 最近一次发生在什么时候\" 这类问题无能为力。这张表把每次 MCP 调用作为单
-- 独一行写入，给 menubar \"MCP 活动\" 卡片做 24h 聚合 + 准实时事件流。
-- 写入路径：app/crates/memex-cli/src/commands/mcp/server/tools.rs::handle_tool_call。
--
-- 注意：`arguments_json` / `result_json` 两列是 v3 migration 才追加的，
-- baseline 这里**故意不包含**——否则 fresh install 跑完 baseline 再跑 v3 的
-- `ALTER TABLE ADD COLUMN` 会撞 \"duplicate column name\"。rusqlite_migration
-- 总是从 v1 baseline 跑到最新版本，最终 schema 一定有这两列。隐私权衡：
-- v3 起 query / project path / result 都会进 SQLite，但 db 本身就只在本机
-- 用户目录，跟 sessions / messages 已经入库的隐私层级一致；为防写爆，单字段
-- ≥ 32KB 时由写入端截断。
CREATE TABLE IF NOT EXISTS mcp_call_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    occurred_at TEXT NOT NULL,
    tool_name TEXT NOT NULL,
    latency_ms INTEGER NOT NULL DEFAULT 0,
    success INTEGER NOT NULL DEFAULT 1,
    error_message TEXT
);

-- 时间倒序读最近 N 条事件用。
CREATE INDEX IF NOT EXISTS idx_mcp_call_log_occurred_desc
    ON mcp_call_log(occurred_at DESC);

-- 按 tool 聚合 24h 调用计数 / 平均延迟用。
CREATE INDEX IF NOT EXISTS idx_mcp_call_log_tool
    ON mcp_call_log(tool_name);

-- v12: notifications —— 用户通知中心数据源。
-- 后端在以下场景调 Db::insert_notification 写一行：
--   * 采集失败（ingest_failed）—— 解析某个 jsonl 出错
--   * 摘要完成（summary_done）—— LLM 摘要生成成功
--   * 反思待处理（reflect_pending）—— 超过 24h 没处理
--   * 周报生成（weekly_report）—— 每周日 22:00 触发
-- 前端 SiteHeader Bell 按钮通过 list_notifications + count_unread_notifications 拉数据；
-- 点击通知 → 弹 Dialog 显示详情（payload_json 解析后渲染）。
-- 写入路径：app/crates/memex-core/src/storage/notifications.rs。
CREATE TABLE IF NOT EXISTS notifications (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    kind TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    payload_json TEXT,
    created_at TEXT NOT NULL,
    read_at TEXT
);

-- 列表页倒序拉最近 N 条用。
CREATE INDEX IF NOT EXISTS idx_notifications_created_desc
    ON notifications(created_at DESC);

-- count_unread 与 unread_only 查询用（部分索引：只索引未读行，更紧凑）。
CREATE INDEX IF NOT EXISTS idx_notifications_unread
    ON notifications(read_at) WHERE read_at IS NULL;
";
