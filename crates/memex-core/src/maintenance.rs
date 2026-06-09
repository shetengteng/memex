//! 维护操作：重建索引、彻底重置。
//!
//! 调用方（GUI / CLI）在停掉所有持有 DB 句柄的进程之后再调这里，
//! 我们只负责文件系统层面的删除。
//!
//! 设计：
//! - `reset_index_only`：只删 `memex.db*`（含 `-shm`/`-wal`）+ `redactions.db*`。
//!   保留 `sessions/*.md`、`config.toml`、`redactions.yaml`、`llm_providers` 行
//!   不属于这里（在 db 内）—— 但 llm_providers 表会随 db 一起被清掉。
//!   重启后由 daemon 重新跑 `rebuild_from_markdown` + ingest 把会话回放回数据库。
//!   **LLM 摘要（chunks.summary / summaries / aggregate_summaries）会丢失**，需要用户重跑。
//!
//! - `reset_all`：删除整个 memex 目录（除了我们刚创建的目录本身保留）。
//!   等价于首次启动，需要用户重新配置 LLM provider、重新触发 ingest。
//!
//! 安全保护：拒绝在不像 memex 目录的路径上执行（避免环境变量 / 测试错配导致误删）。

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ResetReport {
    pub removed_files: u32,
    pub removed_bytes: u64,
}

/// 只清理索引数据库（`memex.db` 及其 WAL/SHM 旁路文件）。
///
/// 调用前必须确保进程内不再持有该 db 的 [`Db`] 句柄；否则在 Windows 上会失败，
/// 在 macOS/Linux 上即使能删，残留的连接也会让重启后出现脏读。
pub fn reset_index_only(memex_dir: &Path) -> Result<ResetReport> {
    ensure_looks_like_memex_dir(memex_dir)?;

    let mut report = ResetReport {
        removed_files: 0,
        removed_bytes: 0,
    };

    // 主 db 及其 sqlite WAL/SHM 旁路文件
    for name in [
        "memex.db",
        "memex.db-wal",
        "memex.db-shm",
        "memex.db-journal",
    ] {
        try_remove_file(&memex_dir.join(name), &mut report)?;
    }

    info!(
        "reset_index_only complete: removed {} files ({} bytes)",
        report.removed_files, report.removed_bytes
    );
    Ok(report)
}

/// 彻底删除 memex 目录下的所有内容（包括 sessions、config、db、redactions）。
///
/// 注意：调用方应在调用结束后立刻退出进程，否则当前进程持有的 db 句柄会让
/// SQLite 在某些平台上继续写日志，重启时可能再次生成 stale 文件。
pub fn reset_all(memex_dir: &Path) -> Result<ResetReport> {
    ensure_looks_like_memex_dir(memex_dir)?;

    let mut report = ResetReport {
        removed_files: 0,
        removed_bytes: 0,
    };

    // 顶层遍历一层，确保我们看得到要删的每一项（便于日志和审计）。
    let entries = fs::read_dir(memex_dir)
        .with_context(|| format!("failed to read {}", memex_dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                warn!("reset_all: skip unreadable entry: {}", e);
                continue;
            }
        };
        let path = entry.path();
        let meta = match entry.metadata() {
            Ok(m) => m,
            Err(e) => {
                warn!("reset_all: skip {} (metadata error: {})", path.display(), e);
                continue;
            }
        };

        if meta.is_dir() {
            let dir_bytes = dir_size(&path);
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove dir {}", path.display()))?;
            report.removed_bytes = report.removed_bytes.saturating_add(dir_bytes);
            report.removed_files = report.removed_files.saturating_add(1);
        } else {
            let bytes = meta.len();
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove file {}", path.display()))?;
            report.removed_bytes = report.removed_bytes.saturating_add(bytes);
            report.removed_files = report.removed_files.saturating_add(1);
        }
    }

    info!(
        "reset_all complete: removed {} top-level entries ({} bytes)",
        report.removed_files, report.removed_bytes
    );
    Ok(report)
}

/// 保护性检查：路径必须存在、是目录，并且基名以 `.memex` 开头或包含 `memex`。
/// 避免误删 home、`/`、`/tmp` 之类的目录。
fn ensure_looks_like_memex_dir(memex_dir: &Path) -> Result<()> {
    if !memex_dir.exists() {
        bail!("memex dir does not exist: {}", memex_dir.display());
    }
    if !memex_dir.is_dir() {
        bail!("memex dir is not a directory: {}", memex_dir.display());
    }
    let name = memex_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    let looks_ok = name.starts_with(".memex") || name.contains("memex");
    if !looks_ok {
        bail!(
            "refusing to operate on {}: directory name does not look like a memex dir",
            memex_dir.display()
        );
    }
    Ok(())
}

fn try_remove_file(path: &Path, report: &mut ResetReport) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let bytes = fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    report.removed_bytes = report.removed_bytes.saturating_add(bytes);
    report.removed_files = report.removed_files.saturating_add(1);
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file()
            && let Ok(meta) = entry.metadata()
        {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_to_operate_on_non_memex_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let weird = tmp.path().join("not-related");
        fs::create_dir_all(&weird).unwrap();
        let err = reset_all(&weird).unwrap_err();
        assert!(err.to_string().contains("does not look like"));
    }

    #[test]
    fn reset_index_only_keeps_sessions_and_config() {
        let tmp = tempfile::TempDir::new().unwrap();
        let memex = tmp.path().join(".memex");
        fs::create_dir_all(memex.join("sessions").join("claude_code")).unwrap();
        fs::write(memex.join("memex.db"), b"fake-db").unwrap();
        fs::write(memex.join("memex.db-wal"), b"fake-wal").unwrap();
        fs::write(memex.join("config.toml"), b"data_dir = \"~/.memex\"\n").unwrap();
        fs::write(
            memex.join("sessions").join("claude_code").join("s1.md"),
            b"hello",
        )
        .unwrap();

        let report = reset_index_only(&memex).unwrap();
        assert_eq!(report.removed_files, 2); // memex.db + wal
        assert!(!memex.join("memex.db").exists());
        assert!(!memex.join("memex.db-wal").exists());
        assert!(memex.join("config.toml").exists());
        assert!(
            memex
                .join("sessions")
                .join("claude_code")
                .join("s1.md")
                .exists()
        );
    }

    #[test]
    fn reset_all_clears_everything_inside_memex_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        let memex = tmp.path().join(".memex");
        fs::create_dir_all(memex.join("sessions").join("cursor")).unwrap();
        fs::write(memex.join("memex.db"), b"fake-db").unwrap();
        fs::write(memex.join("config.toml"), b"x = 1").unwrap();
        fs::write(memex.join("sessions").join("cursor").join("a.md"), b"x").unwrap();

        let report = reset_all(&memex).unwrap();
        assert!(report.removed_files >= 3); // sessions/, memex.db, config.toml

        assert!(memex.exists()); // 目录本身保留
        let leftover: Vec<_> = fs::read_dir(&memex).unwrap().collect();
        assert!(
            leftover.is_empty(),
            "memex dir should be empty after reset_all"
        );
    }
}
