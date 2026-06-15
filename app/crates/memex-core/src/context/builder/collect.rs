//! 从 SQLite 拉数据 + 信号过滤 + 装配 [`super::ProjectContext`]。
//!
//! 入口 [`build_context`] 给 CLI / MCP 共用：collect → render → 可选 redact。

use anyhow::Result;

use super::render::render_markdown;
use super::text::short_date;
use super::{ContextOptions, ProjectContext, SessionContext};
use crate::storage::db::Db;

/// 给一个 project_path 拉出完整上下文结构。在 CLI / MCP 工具中都会用。
///
/// 隐私过滤：`skip_private = true` 时，标记为 `is_private` 的会话以及命中
/// `redactions.yaml` 路径规则的会话都会被剔除，避免 IDE hook 注入的 banner
/// 和 MCP `get_project_context` 工具暴露私有内容。`total_sessions` 也按过滤
/// 后的数量统计，对外不泄漏私有会话的存在。
///
/// 注意：本函数保持纯——`skip_private` 由调用方（一般是 [`build_context`]）
/// 从 `MemexConfig::privacy.skip_private_sessions` 决定后传入；不直接读全局
/// 配置文件，这样单元测试不会受测试机的 `~/.memex/config.toml` 干扰。
pub fn collect_project_context(
    db: &Db,
    project_path: &str,
    top_n: usize,
    skip_private: bool,
) -> Result<ProjectContext> {
    let all_sessions = db.list_sessions_by_project(project_path)?;

    let visible_sessions: Vec<_> = all_sessions
        .iter()
        .filter(|s| {
            if !skip_private {
                return true;
            }
            if s.is_private {
                return false;
            }
            !crate::processor::privacy::is_private_session("", s.project_path.as_deref())
        })
        .cloned()
        .collect();

    let total = visible_sessions.len() as i64;
    let last_active = visible_sessions.first().map(|s| short_date(&s.updated_at));

    // 过滤掉信息量为零的 session（没标题、没 summary、没 intent、且 first_user_message
    // 是 noise 模板）—— 让"近期会话"块只展示有价值的内容，而不是把空骨架占进 top_n。
    let sessions: Vec<_> = visible_sessions
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

/// CLI / MCP 都走这个入口。`skip_private` 默认从 `~/.memex/config.toml` 的
/// `privacy.skip_private_sessions` 读出（fail-closed：load 失败时按 true 处理，
/// 优先保护用户隐私）。
pub fn build_context(db: &Db, project_path: &str, opts: &ContextOptions) -> Result<String> {
    let skip_private = crate::config::MemexConfig::load(&crate::memex_dir())
        .map(|c| c.privacy.skip_private_sessions)
        .unwrap_or(true);
    let ctx = collect_project_context(db, project_path, opts.top_n, skip_private)?;
    let mut md = render_markdown(&ctx);
    if opts.redact {
        // 走一遍既有的 redact 规则。这一步对小段文本开销极低（仅
        // 正则匹配），但能拦住典型 sk-xxx / Bearer xxx 这类 token
        // 泄漏到下游 LLM 的最常见场景。
        md = crate::processor::redact::redact(&md);
    }
    Ok(md)
}

/// 识别 IDE agent 框架塞进"第一条 user 消息"的 system prompt 模板，避免它们
/// 作为 "最后提示" 注入到下游 LLM。这类内容对接收方完全无意义，纯噪声。
///
/// 目前覆盖：
/// - claude_code workflow agent 框架：`=== Role ===`, `=== Skills`, `=== Task ===`
/// - cursor / 其他 IDE 的内嵌 system header（一般以多行模板形式开头）
pub(super) fn is_noise_prompt(s: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::collect_project_context;
    use crate::context::builder::test_support::seed;
    use crate::storage::db::Db;

    #[test]
    fn collect_context_pulls_recent_summaries_and_project_blurb() {
        let db = Db::open_in_memory().unwrap();
        seed(&db, "/Users/me/work/memex");

        let ctx = collect_project_context(&db, "/Users/me/work/memex", 3, false).unwrap();
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

    /// Cursor 在某些场景下把 project_path 写成 "-Users-me-work-memex" 形式
    /// （`/` 被 `-` 替换）。直接 rsplit('/') 会拿到这个怪名字本身，让
    /// 注入到 AI 的项目名看着像一串路径碎片。验证 builder 的兜底逻辑。
    #[test]
    fn project_name_extraction_handles_dashed_path() {
        let db = Db::open_in_memory().unwrap();
        seed(&db, "-Users-me-work-memex");
        let ctx = collect_project_context(&db, "-Users-me-work-memex", 3, false).unwrap();
        assert_eq!(ctx.project_name, "memex");
    }

    /// 把其中一条 session 标记为私有，skip_private=true 时它不应当
    /// 出现在 recent_sessions 里，且 total_sessions 也按过滤后计数——
    /// 后者很关键：暴露 total = 2 等于告诉 IDE "这里还有一条你看不到的私有会话"。
    #[test]
    fn skip_private_filters_out_private_sessions_and_total_count() {
        let db = Db::open_in_memory().unwrap();
        seed(&db, "/proj");
        db.set_session_private("s1", true).unwrap();

        let ctx = collect_project_context(&db, "/proj", 5, true).unwrap();
        assert_eq!(ctx.total_sessions, 1, "total 不应包含私有会话");
        assert_eq!(ctx.recent_sessions.len(), 1);
        assert_eq!(ctx.recent_sessions[0].session_id, "s2");

        // skip_private=false 时所有会话仍然可见——给用户在终端
        // 直接调用 collect_project_context（用配置开关关闭）的场景留口子。
        let ctx_unfiltered = collect_project_context(&db, "/proj", 5, false).unwrap();
        assert_eq!(ctx_unfiltered.total_sessions, 2);
    }
}
