//! End-to-end render tests for [`super::render_markdown`] (via
//! `build_context`). These intentionally don't poke `render_markdown`
//! directly: the function is tightly coupled to the `ProjectContext`
//! shape produced by `collect`, and exercising both halves together
//! catches contract drift between collector and renderer.
//!
//! Plain-text helpers live in `super::super::text` and have their own
//! focused tests there.

use crate::context::builder::ContextOptions;
use crate::context::builder::collect::{build_context, is_noise_prompt};
use crate::context::builder::test_support::seed;
use crate::storage::db::{Db, SummaryUpsert};

#[test]
fn render_markdown_matches_tars_shape() {
    let db = Db::open_in_memory().unwrap();
    seed(&db, "/Users/me/work/memex");
    let md = build_context(
        &db,
        "/Users/me/work/memex",
        &ContextOptions {
            top_n: 3,
            redact: false,
        },
    )
    .unwrap();

    assert!(md.starts_with("**Memex 工作记忆**"), "缺少 banner:\n{}", md);
    assert!(md.contains("**memex**"), "缺少项目名:\n{}", md);
    assert!(md.contains("2 个会话"), "缺少会话计数:\n{}", md);
    assert!(md.contains("概览："), "缺少概览行:\n{}", md);
    assert!(md.contains("近期会话："), "应有 list 形式 header:\n{}", md);
    assert!(
        md.contains("- Fix login bug · "),
        "应以 list 形式渲染标题:\n{}",
        md,
    );
    assert!(
        md.contains("关注：fix the login bug"),
        "应用关注字段渲染最后提示:\n{}",
        md
    );
    assert!(md.contains("已决定：use RS256"), "缺少 decisions:\n{}", md);
    assert!(
        md.contains("memex hooks uninstall"),
        "缺少 opt-out 提示，避免用户找不到怎么关:\n{}",
        md,
    );
}

/// claude_code workflow agent 框架把整段 `=== Role === ...` 当作 user
/// 消息塞进 jsonl，导致这一段被 SQL 取作"第一条 user 消息"。这类内容
/// 渲染进 work memory 完全无信息量，必须过滤。
#[test]
fn render_filters_noise_first_user_message() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "claude_code", Some("/work/foo"), "/f.jsonl", 0, 0)
        .unwrap();
    let h = blake3::hash(b"x").to_hex().to_string();
    // 标题 fallback 用 first_user_message 时会被 "=== Role ===" 污染。
    // 但是同时给一个 L2 摘要标题，让 session 仍然有 signal 不被整体过滤。
    db.insert_message(
        "m1",
        "s1",
        "user",
        "=== Role ===\n你是 Pilot Transition agent。\n=== Task ===\n做事。",
        None,
        0,
        &h,
    )
    .unwrap();
    db.upsert_summary(SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("推进 JIRA 状态"),
        summary: "对 ZOOM-1269895 做 In Progress → Ready for Review 推进。",
        topics: &["jira".into()],
        decisions: &[],
        message_count_at_creation: 2,
    })
    .unwrap();

    let md = build_context(
        &db,
        "/work/foo",
        &ContextOptions {
            top_n: 3,
            redact: false,
        },
    )
    .unwrap();

    assert!(
        !md.contains("=== Role ==="),
        "must filter out the role template:\n{}",
        md,
    );
    assert!(
        md.contains("推进 JIRA 状态"),
        "should still render the L2 title even when first_user_message is noise:\n{}",
        md,
    );
}

/// 当 session 没有标题、没有 summary、没有 intent，并且 first_user_message
/// 也是噪音时，render 应该整体跳过这个会话，而不是输出空骨架行。
#[test]
fn render_skips_session_with_no_signal() {
    let db = Db::open_in_memory().unwrap();
    // s1 全噪音：会被过滤掉
    db.insert_session(
        "s1",
        "claude_code",
        Some("/work/bar"),
        "/f1.jsonl",
        1717000000,
        1717010000,
    )
    .unwrap();
    let h1 = blake3::hash(b"a").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "=== Role ===", None, 0, &h1)
        .unwrap();
    // s2 有标题：保留
    db.insert_session(
        "s2",
        "claude_code",
        Some("/work/bar"),
        "/f2.jsonl",
        1717100000,
        1717110000,
    )
    .unwrap();
    let h2 = blake3::hash(b"b").to_hex().to_string();
    db.insert_message("m2", "s2", "user", "real question", None, 0, &h2)
        .unwrap();
    db.upsert_summary(SummaryUpsert {
        session_id: "s2",
        level: "L2_session",
        title: Some("有标题的 session"),
        summary: "summary",
        topics: &[],
        decisions: &[],
        message_count_at_creation: 1,
    })
    .unwrap();

    let md = build_context(
        &db,
        "/work/bar",
        &ContextOptions {
            top_n: 3,
            redact: false,
        },
    )
    .unwrap();

    // 总会话数仍按全量计算（让用户看到完整规模），但 list 里不再展示 s1。
    assert!(md.contains("2 个会话"), "总数应统计全量:\n{}", md);
    assert!(md.contains("有标题的 session"));
    assert!(!md.contains("（未命名会话）"));
}

/// intent 优先于 first_user_message 作为"关注"行内容。
#[test]
fn render_prefers_intent_over_first_user_message() {
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "cursor", Some("/work/baz"), "/f.jsonl", 0, 0)
        .unwrap();
    let h = blake3::hash(b"x").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "原始提问相对啰嗦的版本", None, 0, &h)
        .unwrap();
    db.upsert_summary(SummaryUpsert {
        session_id: "s1",
        level: "L2_session",
        title: Some("修 bug"),
        summary: "做事的 summary",
        topics: &[],
        decisions: &[],
        message_count_at_creation: 1,
    })
    .unwrap();
    db.update_session_intent("s1", Some("修复登录 bug"))
        .unwrap();

    let md = build_context(
        &db,
        "/work/baz",
        &ContextOptions {
            top_n: 3,
            redact: false,
        },
    )
    .unwrap();

    assert!(
        md.contains("关注：修复登录 bug"),
        "应优先用 intent:\n{}",
        md
    );
    assert!(
        !md.contains("关注：原始提问相对啰嗦"),
        "intent 在时不应用 first_user_message:\n{}",
        md
    );
}

#[test]
fn is_noise_prompt_catches_common_templates() {
    assert!(is_noise_prompt("=== Role ===\nfoo"));
    assert!(is_noise_prompt("  === Task ===  body"));
    assert!(is_noise_prompt("=== System ===\n..."));
    assert!(!is_noise_prompt("修一下登录"));
    assert!(!is_noise_prompt("帮我设计一个 schema"));
    assert!(is_noise_prompt("  "));
}

#[test]
fn render_handles_project_without_l2_summaries() {
    // 没有 L3 也没有 L2 时，至少要靠 title / first_user_message 兜底
    let db = Db::open_in_memory().unwrap();
    db.insert_session("s1", "cursor", Some("/work/foo"), "/f.jsonl", 0, 0)
        .unwrap();
    let h = blake3::hash(b"x").to_hex().to_string();
    db.insert_message("m1", "s1", "user", "explore the design", None, 0, &h)
        .unwrap();

    let md = build_context(
        &db,
        "/work/foo",
        &ContextOptions {
            top_n: 3,
            redact: false,
        },
    )
    .unwrap();
    assert!(md.contains("1 个会话"));
    assert!(
        md.contains("explore the design"),
        "fallback 应当用 first_user_message:\n{}",
        md,
    );
}
