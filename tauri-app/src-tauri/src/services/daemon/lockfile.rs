//! `~/.memex/daemon.lock` 的读 / 写 / 删。
//!
//! Phase 4 起 daemon 跑在 Tauri 主进程内，lockfile 的 `pid` 字段写的是
//! 主进程 PID。lock 文件存在仅为给 **外部进程**（memex-cli）做 RPC discovery。
//! 单进程内同步 / 防双开已经不需要靠文件锁 —— [`super::server::run_in_process`] 一直只
//! 起一份。
//!
//! 因此本模块只暴露 3 个原语：[`write_lock`] / [`remove_lock`] / [`read_lock`]，
//! 旧版本 standalone binary 用的 `is_daemon_running` 已删除。

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockInfo {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

pub fn lock_path(memex_dir: &Path) -> PathBuf {
    memex_dir.join("daemon.lock")
}

pub fn write_lock(memex_dir: &Path, port: u16) -> Result<()> {
    let info = LockInfo {
        pid: std::process::id(),
        port,
        started_at: chrono::Utc::now().to_rfc3339(),
    };
    let content = serde_json::to_string_pretty(&info)?;
    fs::write(lock_path(memex_dir), content).context("failed to write daemon.lock")?;
    Ok(())
}

pub fn remove_lock(memex_dir: &Path) {
    let _ = fs::remove_file(lock_path(memex_dir));
}

/// 读 lock 文件并反序列化。返回 `None` 表示文件不存在或损坏。
///
/// 当前的非测试代码不需要读 lock —— memex-cli / memex-mcp 各自实现的 client
/// 都自带 lock 读取逻辑。本函数主要给同模块下的集成测试用，所以打了
/// `cfg(test)` 避免 dead_code 警告。如果未来 main daemon 也需要读 lock（比如
/// healthcheck 命令），可以摘掉这个 cfg。
#[cfg(test)]
pub fn read_lock(memex_dir: &Path) -> Option<LockInfo> {
    let path = lock_path(memex_dir);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lock_info_round_trip_known_fields() {
        // Sanity: serialize then deserialize must preserve all fields.
        let info = LockInfo {
            pid: 4242,
            port: 51530,
            started_at: "2026-06-09T10:00:00Z".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let parsed: LockInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pid, 4242);
        assert_eq!(parsed.port, 51530);
        assert_eq!(parsed.started_at, "2026-06-09T10:00:00Z");
    }

    #[test]
    fn lock_info_rejects_unknown_fields() {
        // Regression guard for #[serde(deny_unknown_fields)]. Older daemons
        // writing an extra field, or anyone manually tampering with the lock
        // file, must surface as a parse error instead of being silently
        // ignored — otherwise we may keep the stale lock alive.
        let json = r#"{"pid":1,"port":8080,"started_at":"now","extra":true}"#;
        let err = serde_json::from_str::<LockInfo>(json)
            .expect_err("unknown field `extra` must be rejected");
        assert!(
            err.to_string().contains("extra"),
            "error should mention the offending field, got: {err}"
        );
    }
}
