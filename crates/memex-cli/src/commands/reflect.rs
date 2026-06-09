//! `memex reflect` — 基于 daily 摘要做反思级别的回顾。
//!
//! 子命令：
//!   - `run --period <week|month|Nd>`：调 LLM 生成新 reflect，写 DB + markdown
//!   - `list --limit N`：列已有 reflect 行
//!   - `show <scope_key>`：输出某条 markdown 全文
//!
//! 默认（不带子命令时）行为 = `run --period week`，向后兼容旧 CLI。

use anyhow::{Result, anyhow};
use serde::Serialize;

use memex_core::config::MemexConfig;
use memex_core::llm::select_provider_unified;
use memex_core::memex_dir;
use memex_core::reflect::{ReflectPeriod, run_reflect, today_utc};
use memex_core::storage::db::Db;

#[derive(Serialize)]
struct ReflectJsonOutput {
    period_label: String,
    scope_key: String,
    digest_count: usize,
    shipped: Vec<String>,
    patterns: Vec<String>,
    open_loops: Vec<String>,
    markdown_path: Option<String>,
}

pub fn run(period_input: &str, json: bool) -> Result<()> {
    let period = ReflectPeriod::parse(period_input)?;

    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err(anyhow!(
            "数据库不存在（{}）。请先运行 `memex ingest`。",
            db_path.display()
        ));
    }
    let db = Db::open(&db_path)?;

    let config = MemexConfig::load(&memex).unwrap_or_default();
    let provider = select_provider_unified(&db, &config.llm, &memex).ok_or_else(|| {
        anyhow!(
            "没有可用的 LLM provider。请在 Settings → LLM Providers 注册一个，\
            或开启 ollama 并确保 `{}` 上有可用模型。",
            config.llm.ollama_url
        )
    })?;

    let today = today_utc();
    let artifacts = run_reflect(&db, provider.as_ref(), period, today, Some(&memex))?;

    if json {
        let out = ReflectJsonOutput {
            period_label: artifacts.period_label.clone(),
            scope_key: artifacts.scope_key.clone(),
            digest_count: artifacts.digest_count,
            shipped: artifacts.output.shipped.clone(),
            patterns: artifacts.output.patterns.clone(),
            open_loops: artifacts.output.open_loops.clone(),
            markdown_path: artifacts
                .markdown_path
                .as_ref()
                .map(|p| p.display().to_string()),
        };
        println!("{}", serde_json::to_string_pretty(&out)?);
    } else {
        println!("{}", artifacts.markdown);
        if let Some(p) = &artifacts.markdown_path {
            println!("\n📝 已写入：{}", p.display());
        }
        println!(
            "💾 已存进 DB（scope_type=reflect, scope_key={}），基于 {} 份 daily 摘要。",
            artifacts.scope_key, artifacts.digest_count
        );
    }

    Ok(())
}

#[derive(Serialize)]
struct ReflectListEntry {
    scope_key: String,
    title: Option<String>,
    digest_count: i64,
    created_at: String,
}

pub fn list(limit: u32, json: bool) -> Result<()> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err(anyhow!(
            "数据库不存在（{}）。请先运行 `memex ingest`。",
            db_path.display()
        ));
    }
    let db = Db::open(&db_path)?;
    let rows = db.list_aggregate_summaries("reflect", limit)?;

    if json {
        let out: Vec<_> = rows
            .iter()
            .map(|r| ReflectListEntry {
                scope_key: r.scope_key.clone(),
                title: r.title.clone(),
                digest_count: r.session_count,
                created_at: r.created_at.clone(),
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&out)?);
        return Ok(());
    }

    if rows.is_empty() {
        println!("还没有 reflect 记录。先跑 `memex reflect run --period week` 生成一份。");
        return Ok(());
    }

    println!(
        "{:<26} {:<6}  {:<32}  {}",
        "SCOPE_KEY", "DIGEST", "TITLE", "CREATED"
    );
    println!("{}", "-".repeat(100));
    for r in &rows {
        let title = r.title.as_deref().unwrap_or("");
        let title_disp = if title.chars().count() > 30 {
            let trimmed: String = title.chars().take(30).collect();
            format!("{}…", trimmed)
        } else {
            title.to_string()
        };
        println!(
            "{:<26} {:<6}  {:<32}  {}",
            r.scope_key, r.session_count, title_disp, r.created_at
        );
    }
    Ok(())
}

pub fn show(scope_key: &str, json: bool) -> Result<()> {
    let memex = memex_dir();
    let db_path = memex.join("memex.db");
    if !db_path.exists() {
        return Err(anyhow!(
            "数据库不存在（{}）。请先运行 `memex ingest`。",
            db_path.display()
        ));
    }
    let db = Db::open(&db_path)?;
    let row = db
        .get_aggregate_summary("reflect", scope_key)?
        .ok_or_else(|| {
            anyhow!(
                "没有找到 scope_key={}。试试 `memex reflect list` 看可用 keys。",
                scope_key
            )
        })?;

    if json {
        // 复用 aggregate_summary 的字段命名，但显式标注是 reflect
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "scope_type": row.scope_type,
                "scope_key": row.scope_key,
                "title": row.title,
                "summary": row.summary,
                "patterns": row.topics,
                "open_loops": row.decisions,
                "digest_count": row.session_count,
                "created_at": row.created_at,
            }))?
        );
    } else {
        println!("{}", row.summary);
    }
    Ok(())
}
