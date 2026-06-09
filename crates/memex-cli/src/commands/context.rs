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
use std::path::PathBuf;

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

    let project_path = match search_by_project(&db, &cwd)? {
        Some(m) => {
            tracing::debug!("matched project_path={} tier={:?}", m.project_path, m.tier);
            // Tier 1 / 2 都比较可靠；Tier 3 在 stderr 提示一下，方便用户在
            // hook 日志里发现"匹配错项目"的情况，但不打到 stdout 污染上下文。
            if matches!(m.tier, MatchTier::FuzzySubstring) {
                eprintln!(
                    "[memex] using fuzzy-substring match for project {} (cwd={})",
                    m.project_path,
                    cwd.display()
                );
            }
            m.project_path
        }
        None => {
            emit_empty(
                &args,
                &format!(
                    "Memex 当前目录 {} 暂无关联会话记忆 —— 后续在此目录内的 AI 会话会被自动采集。",
                    cwd.display()
                ),
            );
            return Ok(());
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
        });
        println!("{}", serde_json::to_string(&v)?);
    } else {
        // 注意：stdout 不带额外修饰，让 wrapper / 直接输出场景都能拿到干净内容
        print!("{}", md);
    }
    Ok(())
}

fn emit_empty(args: &ContextArgs, banner: &str) {
    if args.json {
        let v = serde_json::json!({
            "project_path": serde_json::Value::Null,
            "markdown": format!("**Memex 工作记忆**\n\n{}\n", banner),
        });
        if let Ok(s) = serde_json::to_string(&v) {
            println!("{}", s);
        }
    } else {
        println!("**Memex 工作记忆**\n");
        println!("{}", banner);
    }
}
