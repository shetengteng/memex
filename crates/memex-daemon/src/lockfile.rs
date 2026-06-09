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
