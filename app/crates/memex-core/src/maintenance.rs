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
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use tracing::{info, warn};

/// reset_all 内部针对 `Directory not empty` 这种"刚 shutdown，watcher 余晖里又写了一个
/// 文件"的 race 做的重试上限。每次重试前先 sleep 100ms 让 fs 缓冲 flush。
const REMOVE_DIR_RETRIES: u32 = 5;
const REMOVE_DIR_BACKOFF: Duration = Duration::from_millis(100);

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
/// 执行顺序很重要：
/// 1. **先删 db 文件**（`memex.db` 及其 wal/shm/journal）。
///    daemon 重启时 open db 看到半截文件会报 SQLite "disk I/O error 522
///    file truncated" —— 比 sessions 留下几个 .md 文件危险得多。
///    因此把 db 列为最高优先级、绝不能短路的一组。
/// 2. **再用带重试的递归删扫剩余条目**。`fs::remove_dir_all` 在 macOS 上遇到
///    Spotlight / fsevent / watcher 的余晖写入时会报 `Directory not empty`
///    （os error 66）。我们对每项重试最多 `REMOVE_DIR_RETRIES` 次，每次失败
///    sleep `REMOVE_DIR_BACKOFF`，把 watcher 释放 fd / fsevent 刷盘的窗口留够。
/// 3. **soft 失败不短路**：某个目录最终还是删不掉，记 warn 并继续删后面的项；
///    只要 db 文件成功删除了，下次启动就能从干净状态恢复，sessions/ 残留可以
///    在 daemon 启动时再清，或者交给用户 `rm -rf ~/.memex/sessions` 兜底。
///
/// 调用方注意：在调本函数之前必须保证已经 shutdown daemon、release 所有 Db 句柄；
/// 否则即使本函数成功删了 db 文件，daemon 内部进程持有的 fd 还会让 SQLite
/// 继续写 wal，重启时再次生成 stale 文件。
pub fn reset_all(memex_dir: &Path) -> Result<ResetReport> {
    ensure_looks_like_memex_dir(memex_dir)?;

    let mut report = ResetReport {
        removed_files: 0,
        removed_bytes: 0,
    };

    // Phase 1: db 文件必须彻底删。即使这一阶段任何一项失败，也直接 bail，
    // 因为留下半截 db 文件会让下次启动直接 SQLite I/O error。
    for name in [
        "memex.db",
        "memex.db-wal",
        "memex.db-shm",
        "memex.db-journal",
        "redactions.db",
        "redactions.db-wal",
        "redactions.db-shm",
    ] {
        try_remove_file(&memex_dir.join(name), &mut report)?;
    }

    // Phase 2: 顶层条目逐项删，每项带重试，失败 soft skip 不短路。
    let entries = match fs::read_dir(memex_dir) {
        Ok(it) => it,
        Err(e) => {
            warn!(
                "reset_all: failed to scan {} ({}); db files already cleared, returning",
                memex_dir.display(),
                e
            );
            return Ok(report);
        }
    };

    let mut soft_errors: Vec<String> = Vec::new();
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
            match remove_dir_all_with_retry(&path) {
                Ok(()) => {
                    report.removed_bytes = report.removed_bytes.saturating_add(dir_bytes);
                    report.removed_files = report.removed_files.saturating_add(1);
                }
                Err(e) => {
                    let msg = format!("failed to remove dir {}: {}", path.display(), e);
                    warn!("reset_all: {}", msg);
                    soft_errors.push(msg);
                }
            }
        } else {
            let bytes = meta.len();
            match fs::remove_file(&path) {
                Ok(()) => {
                    report.removed_bytes = report.removed_bytes.saturating_add(bytes);
                    report.removed_files = report.removed_files.saturating_add(1);
                }
                Err(e) => {
                    let msg = format!("failed to remove file {}: {}", path.display(), e);
                    warn!("reset_all: {}", msg);
                    soft_errors.push(msg);
                }
            }
        }
    }

    if soft_errors.is_empty() {
        info!(
            "reset_all complete: removed {} top-level entries ({} bytes)",
            report.removed_files, report.removed_bytes
        );
    } else {
        // db 已删，目录里还有残留 —— 让上层知道，但不当 fatal：
        // commands::maintenance::system_reset_all 拿到 Ok 后会再调
        // ensure_memex_dir 重建骨架，daemon 启动就用空 db。
        warn!(
            "reset_all completed with {} soft failures (db cleared, residue may remain in {})",
            soft_errors.len(),
            memex_dir.display()
        );
    }
    Ok(report)
}

/// `remove_dir_all` 带指数重试，专门吃 macOS 上"watcher 余晖写文件"造成的
/// `Directory not empty`。失败时 sleep 一小段再试，最多 `REMOVE_DIR_RETRIES`。
fn remove_dir_all_with_retry(path: &Path) -> std::io::Result<()> {
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..REMOVE_DIR_RETRIES {
        match fs::remove_dir_all(path) {
            Ok(()) => return Ok(()),
            Err(e) => {
                // ENOENT 已经被删完，算成功
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(());
                }
                last_err = Some(e);
                if attempt + 1 < REMOVE_DIR_RETRIES {
                    thread::sleep(REMOVE_DIR_BACKOFF);
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("remove_dir_all_with_retry: no error captured (should not happen)")))
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
        fs::write(memex.join("memex.db-wal"), b"fake-wal").unwrap();
        fs::write(memex.join("config.toml"), b"x = 1").unwrap();
        fs::write(memex.join("sessions").join("cursor").join("a.md"), b"x").unwrap();

        let report = reset_all(&memex).unwrap();
        // db + wal 在 phase 1 计入 2，sessions + config 在 phase 2 计入 2，共 ≥ 4
        assert!(report.removed_files >= 4);

        assert!(memex.exists()); // 目录本身保留
        let leftover: Vec<_> = fs::read_dir(&memex).unwrap().collect();
        assert!(
            leftover.is_empty(),
            "memex dir should be empty after reset_all"
        );
    }

    #[test]
    fn reset_all_db_files_are_always_removed_first() {
        // 即便后续目录删除失败，db 文件也必须先被清掉，避免 daemon 重启时 open
        // 半截 db 文件报 SQLite I/O error 522。
        let tmp = tempfile::TempDir::new().unwrap();
        let memex = tmp.path().join(".memex");
        fs::create_dir_all(memex.join("sessions")).unwrap();
        fs::write(memex.join("memex.db"), b"fake-db-bytes").unwrap();
        fs::write(memex.join("memex.db-wal"), b"fake-wal").unwrap();
        fs::write(memex.join("memex.db-shm"), b"fake-shm").unwrap();
        fs::write(memex.join("sessions").join("a.md"), b"x").unwrap();

        let report = reset_all(&memex).unwrap();

        // db 必删
        assert!(!memex.join("memex.db").exists(), "memex.db must be gone");
        assert!(!memex.join("memex.db-wal").exists(), "memex.db-wal must be gone");
        assert!(!memex.join("memex.db-shm").exists(), "memex.db-shm must be gone");
        // db 三件 + sessions 目录 = 4
        assert!(report.removed_files >= 4);
    }

    #[test]
    fn reset_all_soft_fails_on_residue_but_still_removes_db() {
        // 模拟 macOS race condition：sessions 目录里"先删了"，但同时又被另一个
        // 进程写入了一个文件，导致 fs::remove_dir_all 报 Directory not empty。
        // 我们的实现应该重试，最终成功；即便仍失败也不能短路掉 db 删除。
        //
        // 实际上 race condition 在单测中无法完美模拟（需要并发线程精确控制），
        // 这里只验证"db 一定被先删 + 报告里 removed_files 至少包含 db"。
        let tmp = tempfile::TempDir::new().unwrap();
        let memex = tmp.path().join(".memex");
        fs::create_dir_all(memex.join("sessions").join("nested")).unwrap();
        fs::write(memex.join("memex.db"), b"fake").unwrap();
        // 在每个目录里塞一些文件，让 remove_dir_all 有内容可删
        for i in 0..5 {
            fs::write(memex.join("sessions").join(format!("s{i}.md")), b"x").unwrap();
        }
        for i in 0..3 {
            fs::write(memex.join("sessions").join("nested").join(format!("n{i}.md")), b"y").unwrap();
        }

        let report = reset_all(&memex).unwrap();
        assert!(!memex.join("memex.db").exists());
        assert!(!memex.join("sessions").exists());
        assert!(report.removed_files >= 2); // db + sessions/
    }
}
