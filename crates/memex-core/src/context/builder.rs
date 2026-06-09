//! 把 SQLite 里的项目记忆拼装成一份给 AI 看的 Markdown 上下文。
//!
//! 输出格式向 TARS `tars inject` 看齐（用户在文档里明确说"参考一下
//! tars"），但内容来源完全是 Memex 自己的表：
//!
//! - 项目概览 / 总会话数 / 最近活跃 → `sessions` 表聚合
//! - L3 项目级摘要 → `aggregate_summaries (scope_type='project')`
//! - 最近 N 个会话的 L2 摘要 → `summaries (level='L2_session')`
//! - "最后提示" → 每个 session 的第一条 user 消息
//!
//! 脱敏：跟随 `config.privacy.redaction_enabled`。打开时，最终输出的
//! Markdown 文本走一遍 redact 规则，避免把 token / 密码片段塞进
//! 任何外部 LLM 的会话上下文（这一点比直接打印 raw 文本更安全）。

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::storage::db::Db;

#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// 最多列多少个最近会话。TARS 默认 2-3，我们默认 3，让 AI 既能看到
    /// 当下任务又能感知近期上下文。
    pub top_n: usize,
    /// 是否在最终输出上应用 redaction 规则。一般跟随
    /// `config.privacy.redaction_enabled`；CLI 也允许显式覆盖。
    pub redact: bool,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            top_n: 3,
            redact: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectContext {
    pub project_path: String,
    pub project_name: String,
    pub total_sessions: i64,
    pub last_active: Option<String>,
    pub project_summary: Option<String>,
    pub project_topics: Vec<String>,
    pub recent_sessions: Vec<SessionContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    pub session_id: String,
    pub adapter: String,
    pub updated_at: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub topics: Vec<String>,
    pub decisions: Vec<String>,
    /// L2 摘要推断出的"用户真实意图"，渲染时优先于 first_user_message。
    pub intent: Option<String>,
    pub first_user_message: Option<String>,
}

/// 给一个 project_path 拉出完整上下文结构。在 CLI / MCP 工具中都会用。
pub fn collect_project_context(
    db: &Db,
    project_path: &str,
    top_n: usize,
) -> Result<ProjectContext> {
    let all_sessions = db.list_sessions_by_project(project_path)?;
    let total = all_sessions.len() as i64;
    let last_active = all_sessions.first().map(|s| short_date(&s.updated_at));

    // 过滤掉信息量为零的 session（没标题、没 summary、没 intent、且 first_user_message
    // 是 noise 模板）—— 让"近期会话"块只展示有价值的内容，而不是把空骨架占进 top_n。
    let sessions: Vec<_> = all_sessions
        .iter()
        .filter(|s| {
            let has_signal_title = s
                .summary_title
                .as_deref()
                .or(s.title.as_deref())
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            let has_signal_intent = s
                .intent
                .as_deref()
                .map(|t| !t.trim().is_empty())
                .unwrap_or(false);
            let has_signal_prompt = s
                .first_user_message
                .as_deref()
                .map(|t| !is_noise_prompt(t))
                .unwrap_or(false);
            has_signal_title || has_signal_intent || has_signal_prompt
        })
        .cloned()
        .collect();

    // 项目名提取：先按 '/' 取最后一段（标准 unix 路径），如果整段还含
    // '-Users-…' 这种 Cursor 转义形式，再按 '-' 取最后一段兜底。
    // 这样无论 adapter 写进来的是真路径还是转义路径，都能拿到可读的名字。
    let project_name = {
        let raw = sessions
            .iter()
            .find_map(|s| s.project_path.as_deref())
            .unwrap_or(project_path);
        let tail = raw.rsplit('/').next().unwrap_or(raw);
        if tail.starts_with('-') && tail.contains('-') {
            tail.rsplit('-')
                .find(|s| !s.is_empty())
                .unwrap_or(tail)
                .to_string()
        } else {
            tail.to_string()
        }
    };

    let (project_summary, project_topics) =
        match db.get_aggregate_summary("project", project_path)? {
            Some(row) => (Some(row.summary), row.topics),
            None => (None, vec![]),
        };

    let mut recent = Vec::new();
    for s in sessions.iter().take(top_n) {
        let l2 = db.get_summary(&s.id, "L2_session")?;
        // first_user_message 在 noise 模板上直接置空，不让 "=== Role ===" 这类
        // claude_code system prompt 占位文本污染下游 LLM 的注意力。
        let first_user_message = s
            .first_user_message
            .clone()
            .filter(|t| !t.trim().is_empty() && !is_noise_prompt(t));
        recent.push(SessionContext {
            session_id: s.id.clone(),
            adapter: s.source.clone(),
            updated_at: s.updated_at.clone(),
            title: s
                .summary_title
                .clone()
                .or_else(|| s.title.clone())
                .filter(|t| !t.trim().is_empty()),
            summary: l2.as_ref().map(|r| r.summary.clone()),
            topics: l2.as_ref().map(|r| r.topics.clone()).unwrap_or_default(),
            decisions: l2.as_ref().map(|r| r.decisions.clone()).unwrap_or_default(),
            intent: s.intent.clone().filter(|t| !t.trim().is_empty()),
            first_user_message,
        });
    }

    Ok(ProjectContext {
        project_path: project_path.to_string(),
        project_name,
        total_sessions: total,
        last_active,
        project_summary,
        project_topics,
        recent_sessions: recent,
    })
}

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
    }

    out.push_str("\n*由 Memex 自动注入 · 关闭：`memex hooks uninstall <ide>`*\n");
    out
}

/// CLI / MCP 都走这个入口。
pub fn build_context(db: &Db, project_path: &str, opts: &ContextOptions) -> Result<String> {
    let ctx = collect_project_context(db, project_path, opts.top_n)?;
    let mut md = render_markdown(&ctx);
    if opts.redact {
        // 走一遍既有的 redact 规则。这一步对小段文本开销极低（仅
        // 正则匹配），但能拦住典型 sk-xxx / Bearer xxx 这类 token
        // 泄漏到下游 LLM 的最常见场景。
        md = crate::processor::redact::redact(&md);
    }
    Ok(md)
}

// ---- helpers ----

fn short_date(rfc3339: &str) -> String {
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

/// 识别 IDE agent 框架塞进"第一条 user 消息"的 system prompt 模板，避免它们
/// 作为 "最后提示" 注入到下游 LLM。这类内容对接收方完全无意义，纯噪声。
///
/// 目前覆盖：
/// - claude_code workflow agent 框架：`=== Role ===`, `=== Skills`, `=== Task ===`
/// - cursor / 其他 IDE 的内嵌 system header（一般以多行模板形式开头）
fn is_noise_prompt(s: &str) -> bool {
    let trimmed = s.trim_start();
    // 头部就是 `=== ... ===` 这种 ALL-CAPS 段落分隔符，几乎都是 agent 框架模板。
    if trimmed.starts_with("=== Role")
        || trimmed.starts_with("=== Task")
        || trimmed.starts_with("=== Skills")
        || trimmed.starts_with("=== System")
        || trimmed.starts_with("=== Goal")
    {
        return true;
    }
    // 极短或只有标点的 fallback。
    let visible: String = trimmed
        .chars()
        .filter(|c| !c.is_whitespace())
        .take(4)
        .collect();
    visible.len() < 2
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
    use super::*;
    use crate::storage::db::Db;

    fn seed(db: &Db, project_path: &str) {
        db.insert_session(
            "s1",
            "claude_code",
            Some(project_path),
            "/f1.jsonl",
            1717000000,
            1717010000,
        )
        .unwrap();
        db.insert_session(
            "s2",
            "cursor",
            Some(project_path),
            "/f2.jsonl",
            1717100000,
            1717110000,
        )
        .unwrap();
        let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
        // s1: 第一条 user 消息 + 2 条 assistant，便于触发 message_count >= 2
        db.insert_message("m1", "s1", "user", "fix the login bug", None, 0, &h("a"))
            .unwrap();
        db.insert_message("m2", "s1", "assistant", "ok", None, 1, &h("b"))
            .unwrap();
        db.insert_message("m3", "s2", "user", "design the migration", None, 0, &h("c"))
            .unwrap();
        db.insert_message("m4", "s2", "assistant", "ack", None, 1, &h("d"))
            .unwrap();

        db.upsert_summary(
            "s1",
            "L2_session",
            Some("Fix login bug"),
            "Fixed the JWT parser when audience claim is missing.",
            &["auth".into(), "jwt".into()],
            &["use RS256".into()],
            /* message_count_at_creation */ 2,
        )
        .unwrap();

        db.upsert_aggregate_summary(
            "project",
            project_path,
            Some("My Project"),
            "Project-wide work on Memex —— Rust + Tauri + Vue.",
            &["memex".into(), "rust".into()],
            &[],
            5,
        )
        .unwrap();
    }

    #[test]
    fn collect_context_pulls_recent_summaries_and_project_blurb() {
        let db = Db::open_in_memory().unwrap();
        seed(&db, "/Users/me/work/memex");

        let ctx = collect_project_context(&db, "/Users/me/work/memex", 3).unwrap();
        assert_eq!(ctx.project_name, "memex");
        assert_eq!(ctx.total_sessions, 2);
        assert!(ctx.project_summary.unwrap().contains("Memex"));
        assert_eq!(ctx.project_topics, vec!["memex", "rust"]);
        // s2 比 s1 新，应该排在前面
        assert_eq!(ctx.recent_sessions.len(), 2);
        assert_eq!(ctx.recent_sessions[0].session_id, "s2");
        // s1 有 L2 摘要，应正确填充
        let s1 = &ctx.recent_sessions[1];
        assert_eq!(s1.title.as_deref(), Some("Fix login bug"));
        assert!(s1.summary.as_ref().unwrap().contains("JWT"));
        assert_eq!(s1.decisions, vec!["use RS256"]);
        assert_eq!(s1.first_user_message.as_deref(), Some("fix the login bug"));
    }

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
        db.upsert_summary(
            "s1",
            "L2_session",
            Some("推进 JIRA 状态"),
            "对 ZOOM-1269895 做 In Progress → Ready for Review 推进。",
            &["jira".into()],
            &[],
            2,
        )
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
        db.upsert_summary(
            "s2",
            "L2_session",
            Some("有标题的 session"),
            "summary",
            &[],
            &[],
            1,
        )
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
        db.upsert_summary(
            "s1",
            "L2_session",
            Some("修 bug"),
            "做事的 summary",
            &[],
            &[],
            1,
        )
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

    /// Cursor 在某些场景下把 project_path 写成 "-Users-me-work-memex" 形式
    /// （`/` 被 `-` 替换）。直接 rsplit('/') 会拿到这个怪名字本身，让
    /// 注入到 AI 的项目名看着像一串路径碎片。验证 builder 的兜底逻辑。
    #[test]
    fn project_name_extraction_handles_dashed_path() {
        let db = Db::open_in_memory().unwrap();
        seed(&db, "-Users-me-work-memex");
        let ctx = collect_project_context(&db, "-Users-me-work-memex", 3).unwrap();
        assert_eq!(ctx.project_name, "memex");
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
