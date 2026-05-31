use std::path::Path;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

pub fn read_lock(memex_dir: &Path) -> Option<LockInfo> {
    let path = memex_dir.join("daemon.lock");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

pub fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

pub fn check_health(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    ureq::get(&url).call().is_ok()
}

pub fn daemon_port(memex_dir: &Path) -> Option<u16> {
    let info = read_lock(memex_dir)?;
    if is_process_alive(info.pid) && check_health(info.port) {
        Some(info.port)
    } else {
        None
    }
}

pub fn http_get_json(port: u16, path: &str) -> anyhow::Result<serde_json::Value> {
    let url = format!("http://127.0.0.1:{}{}", port, path);
    let body: serde_json::Value = ureq::get(&url).call()?.body_mut().read_json()?;
    Ok(body)
}
