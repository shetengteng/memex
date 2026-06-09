//! Daily-aggregation regressions: lifetime-message bleed-through and
//! dual-path combination on the per-day output.

use rusqlite::params;

use super::ws_seed_session;
use crate::storage::db::Db;

/// 回归 bug：跨天长会话不能把生命周期累计消息计入"今天"。
///
/// 修复前 `daily.messages` / `overall.messages` 都是 SUM(sessions.message_count)，
/// 长生命会话每次活动时都会把全部 message_count 摊到 updated_at 的桶里。
/// 修复后改为两条数据通路：有 timestamp 的真实分桶 + 无 timestamp 的退化。
///
/// 场景：session s1（claude_code）的 message_count 字段是 1000（modeling a
/// 6-month long session with 1000 lifetime messages），今天只新增 3 条带
/// timestamp 的 message。期望 daily today.messages == 3、overall.messages
/// == 3，**而不是** 1000。
#[test]
fn test_workload_daily_today_does_not_include_lifetime_messages() {
    let db = Db::open_in_memory().unwrap();
    let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();
    ws_seed_session(
        &db,
        "s1",
        "claude_code",
        Some("/long-running"),
        &format!("{today_local}T10:00:00+00:00"),
        1000, // ← session 生命周期累计 1000 条消息（bug 时会被算进今天）
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

    let r = db.workload_report(1).unwrap();
    let today_row = r
        .daily
        .iter()
        .find(|d| d.date == today_local)
        .expect("today bucket should exist");
    assert_eq!(
        today_row.messages, 3,
        "daily.messages 应只包含今天 timestamp 真实新增的 3 条，而非 session.message_count=1000",
    );
    assert_eq!(today_row.sessions, 1);

    assert_eq!(
        r.overall.messages, 3,
        "overall.messages 同样应基于 timestamp 通路，而非 SUM(message_count)=1000",
    );
    assert_eq!(r.overall.sessions, 1);
}

/// 混合通路：claude_code 用 timestamp 算 + cursor 退化用 message_count，
/// 两条通路的 daily/overall 加总应当一致。
#[test]
fn test_workload_daily_combines_both_paths() {
    let db = Db::open_in_memory().unwrap();
    let today_local = chrono::Local::now().format("%Y-%m-%d").to_string();

    // claude_code: 生命周期 message_count=500，但今天只新增 2 条带 ts。
    ws_seed_session(
        &db,
        "cc1",
        "claude_code",
        Some("/a"),
        &format!("{today_local}T12:00:00+00:00"),
        500,
    );
    // cursor: 没 timestamp，session.message_count=7 → fallback 路径应算 7。
    ws_seed_session(
        &db,
        "cu1",
        "cursor",
        Some("/b"),
        &format!("{today_local}T13:00:00+00:00"),
        7,
    );
    let conn = db.conn.lock();
    for (mid, ts) in [
        ("m1", format!("{today_local}T11:00:00+00:00")),
        ("m2", format!("{today_local}T12:00:00+00:00")),
    ] {
        conn.execute(
            "INSERT INTO messages (id, session_id, role, content, timestamp, source_offset, content_hash)
             VALUES (?1, 'cc1', 'user', 'x', ?2, 0, ?1)",
            params![mid, ts],
        )
        .unwrap();
    }
    drop(conn);

    let r = db.workload_report(1).unwrap();
    let today_row = r
        .daily
        .iter()
        .find(|d| d.date == today_local)
        .expect("today bucket should exist");

    // 通路 A = 2（cc1 的 2 条 timestamp），通路 B = 7（cu1 message_count）
    // 关键：cc1 的 message_count=500 不应被算进来。
    assert_eq!(
        today_row.messages, 9,
        "expected 2 + 7 = 9, got {}",
        today_row.messages
    );
    assert_eq!(today_row.sessions, 2);
    assert_eq!(r.overall.messages, 9);
    assert_eq!(r.overall.sessions, 2);
}
