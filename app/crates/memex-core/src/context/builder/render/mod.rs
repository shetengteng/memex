//! 把 [`ProjectContext`] 渲染成 TARS 风格的 Markdown 字符串。
//!
//! 输入数据由 [`super::collect`] 装配，本模块只关心展示——
//! 字段缺失时的兜底、文本截断、列表项 layout。
//!
//! Plain-text helpers (`short_date` / `first_line` / `first_paragraph`
//! / `truncate`) live in `super::text`; end-to-end tests live in
//! `tests` sibling module.

use super::text::{first_line, first_paragraph, short_date};
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

#[cfg(test)]
mod tests;
