//! 把 [`ProjectContext`] 渲染成 TARS 风格的 Markdown 字符串。
//!
//! 输入数据由 [`super::collect`] 装配，本模块只关心展示——
//! 字段缺失时的兜底、文本截断、列表项 layout。

use chrono::{DateTime, Utc};

use super::{ProjectContext, SessionContext};

/// 渲染成 Markdown。格式参考 TARS `tars inject`：
///
/// ```text
/// **Memex 工作记忆**
///
/// **<project>** · 27 个会话 · 最近活跃 2026-06-04
/// 概览：<L3 项目摘要片段 / 或最近会话标题串>
///
/// **<project>** · <date>
/// <L2 summary 正文>
/// 最后提示：<first_user_message 截断>
/// ...
/// ```
pub fn render_markdown(ctx: &ProjectContext) -> String {
    let mut out = String::new();
    out.push_str("**Memex 工作记忆**\n\n");

    out.push_str(&format!(
        "**{}** · {} 个会话{}\n",
        ctx.project_name,
        ctx.total_sessions,
        ctx.last_active
            .as_ref()
            .map(|d| format!(" · 最近活跃 {}", d))
            .unwrap_or_default()
    ));

    if let Some(summary) = ctx
        .project_summary
        .as_ref()
        .filter(|s| !s.trim().is_empty())
    {
        out.push_str(&format!("概览：{}\n", first_paragraph(summary, 240)));
    } else if !ctx.recent_sessions.is_empty() {
        // 没有 L3 摘要时，用最近若干会话的标题拼一行 fallback。
        let blurb = ctx
            .recent_sessions
            .iter()
            .filter_map(|s| s.title.clone().or_else(|| s.first_user_message.clone()))
            .map(|s| first_line(&s, 60))
            .collect::<Vec<_>>()
            .join(" | ");
        if !blurb.is_empty() {
            out.push_str(&format!("概览：{}\n", blurb));
        }
    }
    if !ctx.project_topics.is_empty() {
        out.push_str(&format!("话题：{}\n", ctx.project_topics.join(" / ")));
    }

    if !ctx.recent_sessions.is_empty() {
        out.push_str("\n近期会话：\n");
        for s in &ctx.recent_sessions {
            render_session_entry(&mut out, s);
        }
    }

    out.push_str("\n*由 Memex 自动注入 · 关闭：`memex hooks uninstall <ide>`*\n");
    out
}

fn render_session_entry(out: &mut String, s: &SessionContext) {
    // 改为紧凑的 list 式渲染，不再每条都重打项目名。
    // 行首是标题（最具信息），后面 meta（日期 · adapter），下面缩进的内容。
    let title = s
        .title
        .as_deref()
        .map(|t| first_line(t, 100))
        .unwrap_or_else(|| "（未命名会话）".to_string());
    out.push_str(&format!(
        "- {} · {} · {}\n",
        title,
        short_date(&s.updated_at),
        s.adapter
    ));
    if let Some(summary) = s.summary.as_ref().filter(|t| !t.trim().is_empty()) {
        out.push_str(&format!("  概述：{}\n", first_paragraph(summary, 360)));
    }
    if !s.decisions.is_empty() {
        out.push_str(&format!(
            "  已决定：{}\n",
            s.decisions
                .iter()
                .take(3)
                .map(|d| first_line(d, 100))
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    // intent 优先（L2 LLM 推断的"用户真实意图"），缺时退到 first_user_message。
    // first_user_message 已在 collect 时滤掉 `=== Role ===` 这类 system prompt 噪音。
    let last_prompt = s
        .intent
        .as_deref()
        .or(s.first_user_message.as_deref())
        .filter(|t| !t.trim().is_empty());
    if let Some(prompt) = last_prompt {
        out.push_str(&format!("  关注：{}\n", first_line(prompt, 120)));
    }
}

/// 把 RFC3339 字符串简化为 `YYYY-MM-DD`。`collect` 模块的
/// `ProjectContext.last_active` 也复用此函数，故标 `pub(super)`。
pub(super) fn short_date(rfc3339: &str) -> String {
    DateTime::parse_from_rfc3339(rfc3339)
        .map(|d| d.with_timezone(&Utc).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| rfc3339.split('T').next().unwrap_or(rfc3339).to_string())
}

fn first_line(s: &str, max: usize) -> String {
    let line = s.lines().next().unwrap_or(s).trim();
    truncate(line, max)
}

fn first_paragraph(s: &str, max: usize) -> String {
    let trimmed = s.trim();
    // 取到第一个空行为止，把摘要正文压成一段。
    let para = trimmed
        .split("\n\n")
        .next()
        .unwrap_or(trimmed)
        .replace('\n', " ")
        .trim()
        .to_string();
    truncate(&para, max)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
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
}
