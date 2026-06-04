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
    pub first_user_message: Option<String>,
}

/// 给一个 project_path 拉出完整上下文结构。在 CLI / MCP 工具中都会用。
pub fn collect_project_context(
    db: &Db,
    project_path: &str,
    top_n: usize,
) -> Result<ProjectContext> {
    let sessions = db.list_sessions_by_project(project_path)?;
    let total = sessions.len() as i64;
    let last_active = sessions
        .first()
        .map(|s| short_date(&s.updated_at));

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

    let (project_summary, project_topics) = match db.get_aggregate_summary("project", project_path)? {
        Some(row) => (Some(row.summary), row.topics),
        None => (None, vec![]),
    };

    let mut recent = Vec::new();
    for s in sessions.iter().take(top_n) {
        let l2 = db.get_summary(&s.id, "L2_session")?;
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
            first_user_message: s.first_user_message.clone(),
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

    if let Some(summary) = ctx.project_summary.as_ref().filter(|s| !s.trim().is_empty()) {
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

    for s in &ctx.recent_sessions {
        out.push('\n');
        out.push_str(&format!(
            "**{}** · {} · {}\n",
            ctx.project_name,
            short_date(&s.updated_at),
            s.adapter
        ));
        if let Some(t) = s.title.as_ref() {
            out.push_str(&format!("标题：{}\n", first_line(t, 100)));
        }
        if let Some(summary) = s.summary.as_ref().filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("{}\n", first_paragraph(summary, 480)));
        }
        if !s.decisions.is_empty() {
            out.push_str(&format!(
                "已决定：{}\n",
                s.decisions
                    .iter()
                    .take(3)
                    .map(|d| first_line(d, 100))
                    .collect::<Vec<_>>()
                    .join("；")
            ));
        }
        if let Some(prompt) = s.first_user_message.as_ref().filter(|t| !t.trim().is_empty()) {
            out.push_str(&format!("最后提示：{}\n", first_line(prompt, 120)));
        }
    }

    out.push_str("\n*由 Memex 自动注入 · 关闭：`memex hooks uninstall <ide>`*\n");
    out
}

/// CLI / MCP 都走这个入口。
pub fn build_context(
    db: &Db,
    project_path: &str,
    opts: &ContextOptions,
) -> Result<String> {
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
            "s1", "claude_code", Some(project_path), "/f1.jsonl",
            1717000000, 1717010000,
        ).unwrap();
        db.insert_session(
            "s2", "cursor", Some(project_path), "/f2.jsonl",
            1717100000, 1717110000,
        ).unwrap();
        let h = |s: &str| blake3::hash(s.as_bytes()).to_hex().to_string();
        // s1: 第一条 user 消息 + 2 条 assistant，便于触发 message_count >= 2
        db.insert_message("m1", "s1", "user", "fix the login bug", None, 0, &h("a")).unwrap();
        db.insert_message("m2", "s1", "assistant", "ok", None, 1, &h("b")).unwrap();
        db.insert_message("m3", "s2", "user", "design the migration", None, 0, &h("c")).unwrap();
        db.insert_message("m4", "s2", "assistant", "ack", None, 1, &h("d")).unwrap();

        db.upsert_summary(
            "s1", "L2_session",
            Some("Fix login bug"),
            "Fixed the JWT parser when audience claim is missing.",
            &["auth".into(), "jwt".into()],
            &["use RS256".into()],
            /* message_count_at_creation */ 2,
        ).unwrap();

        db.upsert_aggregate_summary(
            "project", project_path,
            Some("My Project"),
            "Project-wide work on Memex —— Rust + Tauri + Vue.",
            &["memex".into(), "rust".into()],
            &[],
            5,
        ).unwrap();
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
            &ContextOptions { top_n: 3, redact: false },
        ).unwrap();

        assert!(md.starts_with("**Memex 工作记忆**"), "缺少 banner:\n{}", md);
        assert!(md.contains("**memex**"), "缺少项目名:\n{}", md);
        assert!(md.contains("2 个会话"), "缺少会话计数:\n{}", md);
        assert!(md.contains("概览："), "缺少概览行:\n{}", md);
        assert!(md.contains("最后提示：fix the login bug"), "缺少 last prompt:\n{}", md);
        assert!(md.contains("已决定：use RS256"), "缺少 decisions:\n{}", md);
        assert!(
            md.contains("memex hooks uninstall"),
            "缺少 opt-out 提示，避免用户找不到怎么关:\n{}", md,
        );
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
        db.insert_session("s1", "cursor", Some("/work/foo"), "/f.jsonl", 0, 0).unwrap();
        let h = blake3::hash(b"x").to_hex().to_string();
        db.insert_message("m1", "s1", "user", "explore the design", None, 0, &h).unwrap();

        let md = build_context(
            &db,
            "/work/foo",
            &ContextOptions { top_n: 3, redact: false },
        ).unwrap();
        assert!(md.contains("1 个会话"));
        assert!(
            md.contains("explore the design"),
            "fallback 应当用 first_user_message:\n{}", md,
        );
    }
}
