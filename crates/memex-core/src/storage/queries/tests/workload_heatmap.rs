//! Heatmap-specific regressions: dual-path (timestamp vs session
//! fallback), JOIN-removal sanity, performance guard, double-count
//! avoidance.

use rusqlite::params;

use super::ws_seed_session;
use crate::storage::db::Db;

/// 验证 heatmap 的双通路口径：
///   有 messages.timestamp 的 session 走 message 维度分桶，
///   没 timestamp 的 session 退化用 session.updated_at。
/// 关键断言：一个跨多小时的会话，message 通路里能摊到多个 hour 桶，
/// 而 session 通路下永远只在 last_updated_at 的桶里出现一次。
#[test]
fn test_workload_heatmap_messages_vs_session_fallback() {
    let db = Db::open_in_memory().unwrap();
    // 同一天，三个 session：
    //   s1 claude_code 9 点和 13 点各 1 条 message（有 timestamp）
    //   s2 cursor     全无 timestamp，session.updated_at 15 点
    //   s3 codex      11 点 1 条 message（有 timestamp）
    let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
    ws_seed_session(
        &db,
        "s1",
        "claude_code",
        Some("/a"),
        &format!("{today_local}T13:30:00+00:00"),
        2,
    );
    ws_seed_session(
        &db,
        "s2",
        "cursor",
        Some("/a"),
        &format!("{today_local}T15:00:00+00:00"),
        7,
    );
    ws_seed_session(
        &db,
        "s3",
        "codex",
        Some("/b"),
        &format!("{today_local}T11:00:00+00:00"),
        1,
    );

    // 给 s1 / s3 插带 timestamp 的 message，s2 不插（模拟 cursor 没 timestamp）
    let conn = db.conn.lock();
    for (mid, sid, ts) in [
        ("m1", "s1", format!("{today_local}T09:00:00+00:00")),
        ("m2", "s1", format!("{today_local}T13:00:00+00:00")),
        ("m3", "s3", format!("{today_local}T11:00:00+00:00")),
    ] {
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES (?1, ?2, 'user', 'x', ?3, 0, ?1)",
            params![mid, sid, ts],
        )
        .unwrap();
    }
    drop(conn);

    let r = db.workload_report(2).unwrap();

    // 把 heatmap 拍平到 hour，按 UTC 计算（测试在不同时区跑会浮动）。
    // 我们仅断言总和：3 条带 timestamp 的 message → message 通路出 3 桶（9/11/13），
    // 1 个无 timestamp 的 cursor session → fallback 通路出 1 桶（15）。
    let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
    // 来源：message 通路 3 条 + cursor session 通路用 message_count=7 → 共 10
    assert_eq!(total_msgs, 10, "messages aggregate across both paths");

    // sessions 聚合是按 (weekday, hour) 桶后再求和，跨多小时的 session 在每个桶里
    // 都贡献 1：
    //   s1 命中 9 / 13 两桶 = 2
    //   s3 命中 11 桶 = 1
    //   s2 fallback 命中 15 桶 = 1
    //   合计 = 4
    // 这是符合"按时间段看活动"语义的预期，而不是 distinct session 总数。
    let total_sessions: i64 = r.heatmap.iter().map(|c| c.sessions).sum();
    assert_eq!(total_sessions, 4);

    // 4 个不同的 hour 桶（9/11/13 + 15）。
    let distinct_hours: std::collections::HashSet<u8> = r.heatmap.iter().map(|c| c.hour).collect();
    assert_eq!(
        distinct_hours.len(),
        4,
        "expected 4 distinct hour buckets, got {:?}",
        distinct_hours
    );
}

/// 回归测试：去掉 messages × sessions JOIN 之后，结果应当与有 JOIN 时一致。
/// 旧 SQL 的 JOIN 只是用来过滤孤儿 message_id（实际项目里 messages.session_id
/// 受外键约束，绝不出现孤儿），所以拆掉 JOIN 不会改变任何业务输出。
#[test]
fn test_workload_heatmap_no_orphan_messages_after_join_removal() {
    let db = Db::open_in_memory().unwrap();
    let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
    ws_seed_session(
        &db,
        "s1",
        "claude_code",
        Some("/a"),
        &format!("{today_local}T10:00:00+00:00"),
        3,
    );
    let conn = db.conn.lock();
    for (mid, ts) in [
        ("m1", format!("{today_local}T08:00:00+00:00")),
        ("m2", format!("{today_local}T09:00:00+00:00")),
        ("m3", format!("{today_local}T10:00:00+00:00")),
    ] {
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES (?1, 's1', 'user', 'x', ?2, 0, ?1)",
            params![mid, ts],
        )
        .unwrap();
    }
    drop(conn);

    let r = db.workload_report(2).unwrap();
    let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
    assert_eq!(total_msgs, 3, "3 messages should all be counted");
    // 三个不同的小时桶 8 / 9 / 10
    let hours: std::collections::HashSet<u8> = r.heatmap.iter().map(|c| c.hour).collect();
    assert_eq!(hours.len(), 3);
}

/// 性能护栏：跑一个有 200 条带 timestamp 的 messages 的库，
/// workload_report(30) 必须在 1 秒内完成。
/// 真实场景里 30w+ messages 没索引会 6s+，这里用小数据集只验证 SQL 形态
/// 不会再退化成 JOIN 全表扫描。
#[test]
fn test_workload_heatmap_completes_in_reasonable_time() {
    let db = Db::open_in_memory().unwrap();
    let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
    ws_seed_session(
        &db,
        "perf-s1",
        "claude_code",
        Some("/perf"),
        &format!("{today_local}T12:00:00+00:00"),
        200,
    );
    let conn = db.conn.lock();
    for i in 0..200 {
        let hour = i % 24;
        let ts = format!("{today_local}T{:02}:00:00+00:00", hour);
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES (?1, 'perf-s1', 'user', 'x', ?2, ?3, ?1)",
            params![format!("perf-m{i}"), ts, i as i64],
        )
        .unwrap();
    }
    drop(conn);

    let start = std::time::Instant::now();
    let r = db.workload_report(30).unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "workload_report(30) took {} ms, expected < 1000 ms",
        elapsed.as_millis()
    );
    let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
    assert_eq!(total_msgs, 200);
}

/// session 表里所有 message 都有 timestamp 时，session.updated_at fallback 不应
/// 再被算一次（避免 double-count）。
#[test]
fn test_workload_heatmap_no_double_count_when_messages_have_ts() {
    let db = Db::open_in_memory().unwrap();
    let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
    ws_seed_session(
        &db,
        "s1",
        "claude_code",
        Some("/a"),
        &format!("{today_local}T13:00:00+00:00"),
        // 故意把 session.message_count 设成 99 —— 如果 fallback 误触发就会被加上
        99,
    );
    let conn = db.conn.lock();
    conn.execute(
        "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
         VALUES ('m1', 's1', 'user', 'x', ?1, 0, 'h1')",
        params![format!("{today_local}T13:00:00+00:00")],
    )
    .unwrap();
    drop(conn);

    let r = db.workload_report(2).unwrap();
    let total_msgs: i64 = r.heatmap.iter().map(|c| c.messages).sum();
    assert_eq!(
        total_msgs, 1,
        "must NOT include session.message_count=99 fallback"
    );
    let total_sessions: i64 = r.heatmap.iter().map(|c| c.sessions).sum();
    assert_eq!(total_sessions, 1);
}
