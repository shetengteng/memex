//! 新装库的 schema 一致性 + hot-path 索引存在性 + EXPLAIN QUERY PLAN 命中。

use crate::storage::db::Db;

/// v9 回归：新装库的 SCHEMA_SQL 必须把 `idx_chunks_has_summary` 和
/// `idx_messages_content_dedup` 全部建出来——这两个索引以前只在 v6
/// migration 里建过，从未写进 SCHEMA_SQL，导致新装库 / v6→v8 跳级升级
/// 的库都丢了这两个 hot path 索引。
#[test]
fn test_schema_sql_creates_all_hot_path_indexes() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock();
    let mut stmt = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND sql IS NOT NULL")
        .unwrap();
    let names: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    drop(conn);

    let required = [
        "idx_messages_session_role_offset",
        "idx_summaries_session_level",
        "idx_sessions_updated_at",
        "idx_messages_timestamp",
        "idx_chunks_has_summary",
        "idx_messages_content_dedup",
    ];
    for idx in required {
        assert!(
            names.iter().any(|n| n == idx),
            "新装库缺少 hot path 索引 {idx}（现有：{:?}）",
            names
        );
    }
}

/// v9 关键路径：ingest 的 dedup 查询
/// `WHERE content_hash = ? AND session_id = ?` 必须命中
/// `idx_messages_content_dedup` 复合索引，否则在大 session 上退化到
/// 全 session 比对。
#[test]
fn test_message_dedup_uses_composite_index() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock();
    let plan: String = conn
        .query_row(
            "EXPLAIN QUERY PLAN SELECT 1 FROM messages WHERE content_hash = 'x' AND session_id = 'y'",
            [],
            |row| {
                // EXPLAIN QUERY PLAN 的 detail 列在第 4 个 (index 3)
                row.get::<_, String>(3)
            },
        )
        .unwrap();
    assert!(
        plan.contains("idx_messages_content_dedup"),
        "dedup 查询应走 idx_messages_content_dedup（实际 plan: {plan}）"
    );
}

/// v9 关键路径：chunks_with_summary_count 必须命中
/// `idx_chunks_has_summary` 局部索引，否则 70w+ chunks 真实库上
/// COUNT(*) 要 15+ 秒，会卡住「摘要进度百分比」展示。
#[test]
fn test_chunks_with_summary_count_uses_partial_index() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock();
    let plan: String = conn
        .query_row(
            "EXPLAIN QUERY PLAN SELECT COUNT(*) FROM chunks WHERE summary IS NOT NULL",
            [],
            |row| row.get::<_, String>(3),
        )
        .unwrap();
    assert!(
        plan.contains("idx_chunks_has_summary"),
        "chunks_with_summary_count 应走 idx_chunks_has_summary（实际 plan: {plan}）"
    );
}

#[test]
fn test_v10_schema_has_threads_tables_and_indexes() {
    let db = Db::open_in_memory().unwrap();
    let conn = db.conn.lock();

    let tables: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(tables.contains(&"threads".to_string()));
    assert!(tables.contains(&"thread_sessions".to_string()));

    let indexes: Vec<String> = conn
        .prepare("SELECT name FROM sqlite_master WHERE type='index' AND sql IS NOT NULL")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .filter_map(|r| r.ok())
        .collect();
    assert!(
        indexes.contains(&"idx_thread_sessions_session".to_string()),
        "thread_sessions(session_id) 索引必须存在"
    );
    assert!(
        indexes.contains(&"idx_threads_updated_at".to_string()),
        "threads(updated_at DESC) 索引必须存在"
    );
}
