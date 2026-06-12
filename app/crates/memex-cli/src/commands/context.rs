//! `memex context` —— 把项目工作记忆按 TARS-style Markdown 输出到 stdout，
//! 用于 IDE hook（Claude Code SessionStart / Cursor sessionStart）的上下文注入。
//!
//! 设计原则：
//! - **永远 exit 0**：hook 失败不能阻塞用户会话。找不到匹配项目 → 静默打印
//!   一个 banner 而非报错。
//! - **轻量、快速**：只读 SQLite，不触发 LLM、不联网。冷启动 < 100ms。
//! - **纯 Markdown**：JSON 套壳（Claude Code `hookSpecificOutput.additionalContext`
//!   / Cursor `additional_context`）由 hook wrapper 脚本处理，本命令不感知 IDE。

use std::env;
use std::path::{Path, PathBuf};

use anyhow::Result;
use memex_core::config::MemexConfig;
use memex_core::context::{ContextOptions, MatchTier, build_context, search_by_project};
use memex_core::memex_dir;
use memex_core::storage::db::Db;

pub struct ContextArgs {
    /// 显式项目目录；不传则用 std::env::current_dir()
    pub project: Option<String>,
    /// 最多列多少个最近会话
    pub top: usize,
    /// 强制脱敏；不传则按 config.privacy.redaction_enabled
    pub redact: Option<bool>,
    /// JSON 全局开关 —— 输出 { project_path, markdown }，方便 wrapper 解析
    pub json: bool,
}

pub fn run(args: ContextArgs) -> Result<()> {
    let dir = memex_dir();
    let db_path = dir.join("memex.db");

    // 数据库不存在 = 全新用户。打个友好 banner，让 AI 知道 Memex 还没有
    // 任何记忆可供注入，但不要报 error。
    if !db_path.exists() {
        emit_empty(
            &args,
            "Memex 工作记忆尚未生成 —— 还没有任何 ingest 过的会话。",
        );
        return Ok(());
    }

    let db = Db::open(&db_path)?;
    let cfg = MemexConfig::load(&dir).unwrap_or_default();

    let cwd = match args.project.clone() {
        Some(p) => PathBuf::from(p),
        None => env::current_dir()?,
    };

    let (project_path, used_fallback) = match search_by_project(&db, &cwd)? {
        Some(m) => {
            tracing::debug!("matched project_path={} tier={:?}", m.project_path, m.tier);
            // Tier 1 / 2 都比较可靠；Tier 3 在 stderr 提示一下，方便用户在
            // hook 日志里发现"匹配错项目"的情况，但不打到 stdout 污染上下文。
            if matches!(m.tier, MatchTier::FuzzySubstring) {
                crate::err!(
                    "[memex] using fuzzy-substring match for project {} (cwd={})",
                    m.project_path,
                    cwd.display()
                );
            }
            (m.project_path, false)
        }
        None => {
            // Fallback：cwd 三级匹配全部失败。常见根因是 IDE 启动 sessionStart
            // hook 时 `$PWD` 指向自己的配置目录（Cursor=~/.cursor、Claude Code
            // =~/.claude 等），不在任何用户项目下。这种情况下让 banner 留空相当
            // 于告诉 AI "别调 memex 了"，反向引导。改成回退到「最近活跃项目」，
            // hook 仍能注入有用上下文；AI 在新会话里能立即看到用户当前在做什么。
            //
            // 仅对已知会触发误判的 IDE 内部目录开启 fallback，避免把不相关的
            // 项目摘要塞给真正从 /tmp / 家目录调用 memex context 的用户。
            let Some(fallback_path) = ide_internal_cwd_fallback(&db, &cwd)? else {
                emit_empty(
                    &args,
                    &format!(
                        "Memex 当前目录 {} 暂无关联会话记忆 —— 后续在此目录内的 AI 会话会被自动采集。",
                        cwd.display()
                    ),
                );
                return Ok(());
            };
            crate::err!(
                "[memex] cwd {} looks like an IDE internal dir; falling back to latest active project {}",
                cwd.display(),
                fallback_path
            );
            tracing::info!(
                cwd = %cwd.display(),
                fallback = %fallback_path,
                "context fallback: ide internal cwd"
            );
            (fallback_path, true)
        }
    };

    let redact = args.redact.unwrap_or(cfg.privacy.redaction_enabled);

    let md = build_context(
        &db,
        &project_path,
        &ContextOptions {
            top_n: args.top,
            redact,
        },
    )?;

    if args.json {
        let v = serde_json::json!({
            "project_path": project_path,
            "markdown": md,
            "fallback_from_ide_dir": used_fallback,
        });
        crate::io::json(&v)?;
    } else {
        // 注意：stdout 不带额外修饰，让 wrapper / 直接输出场景都能拿到干净内容
        crate::out!("{}", md);
    }
    Ok(())
}

/// 当 `cwd` 看起来是 IDE 自身的配置目录（Cursor sessionStart hook 启动时
/// `$PWD = ~/.cursor`、Claude Code SessionStart 时 `$PWD = ~/.claude` 等），
/// 用「最近活跃项目」作为兜底 cwd 返回。否则返回 `None`，由调用方继续走
/// 「暂无关联会话记忆」的 banner 路径。
///
/// 收紧范围的目的：避免用户从 `/tmp` / 家目录其他子目录主动调用
/// `memex context` 时，被"莫名其妙"塞了一份不相关项目的摘要。
fn ide_internal_cwd_fallback(db: &Db, cwd: &Path) -> Result<Option<String>> {
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };
    if !is_ide_internal_dir(cwd, &home) {
        return Ok(None);
    }
    db.latest_active_project()
}

/// 已知 IDE / agent 启动 hook 时 `$PWD` 会指向自身配置目录的白名单。
///
/// 判定为「IDE 内部目录」的两类路径：
/// 1. `$HOME/.<ide>` 形式的 dotfile 配置目录（最常见，覆盖 Cursor / Claude
///    Code / Codex / OpenAI 等）。
/// 2. macOS 下的 `$HOME/Library/Application Support/<ide>`。
///
/// 纯函数：`home` 显式传入，不读环境也不查文件系统，方便单测。
fn is_ide_internal_dir(cwd: &Path, home: &Path) -> bool {
    let cwd_norm = cwd.to_string_lossy().trim_end_matches('/').to_string();
    let home_str = home.to_string_lossy().to_string();

    const IDE_DOTFILES: &[&str] = &[
        ".cursor",
        ".claude",
        ".codex",
        ".cursor-server",
        ".aider",
        ".continue",
        ".cline",
        ".opencode",
    ];
    for ide in IDE_DOTFILES {
        let prefix = format!("{home_str}/{ide}");
        if cwd_norm == prefix || cwd_norm.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }

    // macOS：`~/Library/Application Support/<ide>` 也常作为 hook cwd 出现。
    let lib_app_support = format!("{home_str}/Library/Application Support");
    if cwd_norm.starts_with(&format!("{lib_app_support}/")) {
        return true;
    }

    false
}

fn emit_empty(args: &ContextArgs, banner: &str) {
    if args.json {
        let v = serde_json::json!({
            "project_path": serde_json::Value::Null,
            "markdown": format!("**Memex 工作记忆**\n\n{}\n", banner),
            "fallback_from_ide_dir": false,
        });
        let _ = crate::io::json(&v);
    } else {
        crate::out!("**Memex 工作记忆**\n");
        crate::out!("{}", banner);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOME: &str = "/Users/u";

    #[test]
    fn is_ide_internal_dir_detects_cursor_dotfile() {
        let home = Path::new(HOME);
        assert!(is_ide_internal_dir(Path::new("/Users/u/.cursor"), home));
        assert!(is_ide_internal_dir(Path::new("/Users/u/.cursor/"), home));
        assert!(is_ide_internal_dir(
            Path::new("/Users/u/.cursor/User/globalStorage"),
            home
        ));
    }

    #[test]
    fn is_ide_internal_dir_detects_claude_and_codex_dotfiles() {
        let home = Path::new(HOME);
        assert!(is_ide_internal_dir(Path::new("/Users/u/.claude"), home));
        assert!(is_ide_internal_dir(Path::new("/Users/u/.codex"), home));
        assert!(is_ide_internal_dir(
            Path::new("/Users/u/.cursor-server"),
            home
        ));
    }

    #[test]
    fn is_ide_internal_dir_detects_macos_application_support() {
        let home = Path::new(HOME);
        assert!(is_ide_internal_dir(
            Path::new("/Users/u/Library/Application Support/Cursor"),
            home
        ));
    }

    #[test]
    fn is_ide_internal_dir_rejects_user_project_dirs() {
        let home = Path::new(HOME);
        assert!(!is_ide_internal_dir(
            Path::new("/Users/u/Documents/personal/memex"),
            home
        ));
        assert!(!is_ide_internal_dir(Path::new("/tmp/random"), home));
        assert!(!is_ide_internal_dir(Path::new("/Users/u"), home));
        // ".cursor" 出现在用户项目名里时不能误判
        assert!(!is_ide_internal_dir(
            Path::new("/Users/u/work/my-cursor-tools"),
            home
        ));
    }
}
