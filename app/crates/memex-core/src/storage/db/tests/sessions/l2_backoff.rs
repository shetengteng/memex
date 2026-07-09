//! L2 摘要失败退避（record_l2_failure / reset_l2_backoff）
//! + sessions_needing_summary 对 l2_next_retry_at 的过滤。
//!
//! 修复的现象：daemon 每 2 分钟一轮 ingest，同一批 5 个 session 反复失败，
//! HTTP 请求把 DB 锁住，MCP 30s 启动超时。加退避后达到上限的坏 session
//! 永久跳过，daemon 空转停止。

use crate::storage::db::{Db, summaries::L2_MAX_ATTEMPTS};

fn seed(db: &Db, id: &str) {
    db.insert_session(id, "claude_code", None, "/f.jsonl", 0, 0)
        .unwrap();
    let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
    db.insert_message(&format!("{id}-m1"), id, "user", "hi", None, 0, &h("a"))
        .unwrap();
    db.insert_message(
        &format!("{id}-m2"),
        id,
        "assistant",
        "hello",
        None,
        1,
        &h("b"),
    )
    .unwrap();
}

/// 单次失败后：session 会在 next_retry_at 未到时被 selector 跳过。
#[test]
fn single_failure_delays_selection() {
    let db = Db::open_in_memory().unwrap();
    seed(&db, "s1");

    assert_eq!(db.sessions_needing_summary(10, 0).unwrap(), vec!["s1"]);

    db.record_l2_failure("s1").unwrap();

    // 第一次失败退避 2 分钟，selector 不应返回它
    assert!(
        db.sessions_needing_summary(10, 0).unwrap().is_empty(),
        "刚失败一次的 session 不应立刻被再次选中"
    );
}

/// 连续失败 L2_MAX_ATTEMPTS 次后，永久跳过（哨兵时间 9999-...）。
#[test]
fn max_attempts_permanently_skips() {
    let db = Db::open_in_memory().unwrap();
    seed(&db, "s1");

    for _ in 0..L2_MAX_ATTEMPTS {
        db.record_l2_failure("s1").unwrap();
    }

    // 哨兵时间比任何真实的 now 都大，selector 永远查不到
    assert!(
        db.sessions_needing_summary(10, 0).unwrap().is_empty(),
        "达到失败上限后 session 应被永久跳过"
    );
}

/// wal_checkpoint 不应报错（即使 in-memory 无 WAL 也允许调用）。
#[test]
fn wal_checkpoint_is_safe_to_call() {
    let db = Db::open_in_memory().unwrap();
    db.wal_checkpoint_truncate()
        .expect("in-memory checkpoint 应无副作用");
}

/// reset 后 session 立即回到候选池。
#[test]
fn reset_clears_backoff() {
    let db = Db::open_in_memory().unwrap();
    seed(&db, "s1");

    db.record_l2_failure("s1").unwrap();
    assert!(db.sessions_needing_summary(10, 0).unwrap().is_empty());

    db.reset_l2_backoff("s1").unwrap();
    assert_eq!(
        db.sessions_needing_summary(10, 0).unwrap(),
        vec!["s1".to_string()],
        "reset 后 session 应立即回到候选池"
    );
}
