use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

pub fn read_lock(memex_dir: &Path) -> Option<LockInfo> {
    let path = lock_path(memex_dir);
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn is_daemon_running(memex_dir: &Path) -> Option<LockInfo> {
    let info = read_lock(memex_dir)?;
    if is_process_alive(info.pid) {
        Some(info)
    } else {
        remove_lock(memex_dir);
        None
    }
}

fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}
