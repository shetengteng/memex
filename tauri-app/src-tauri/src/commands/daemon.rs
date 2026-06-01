use std::path::PathBuf;
use std::process::{Command, Stdio};

use memex_core::memex_dir;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockInfo {
    pub pid: u32,
    pub port: u16,
    pub started_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DaemonStatus {
    pub running: bool,
    pub pid: Option<u32>,
    pub port: Option<u16>,
    pub http_ok: bool,
    pub started_at: Option<String>,
}

fn read_lock() -> Option<LockInfo> {
    let path = memex_dir().join("daemon.lock");
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn is_process_alive(pid: u32) -> bool {
    // kill -0 returns 0 if the process exists and we can signal it.
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn http_health_ok(port: u16) -> bool {
    let url = format!("http://127.0.0.1:{}/health", port);
    Command::new("curl")
        .args(["-s", "-o", "/dev/null", "-w", "%{http_code}", "--max-time", "2", &url])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim() == "200")
        .unwrap_or(false)
}

fn find_daemon_binary() -> Option<PathBuf> {
    // Prefer a binary that sits next to the menubar app's own executable,
    // which is how the bundle layout ships it.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("memex-daemon");
            if p.exists() {
                return Some(p);
            }
        }
    }
    // Fall back to PATH.
    if let Ok(out) = Command::new("which").arg("memex-daemon").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    None
}

#[tauri::command]
pub async fn daemon_status() -> Result<DaemonStatus, String> {
    let info = read_lock();
    let mut status = DaemonStatus {
        running: false,
        pid: None,
        port: None,
        http_ok: false,
        started_at: None,
    };
    if let Some(info) = info {
        let alive = is_process_alive(info.pid);
        let http = if alive { http_health_ok(info.port) } else { false };
        status.running = alive;
        status.pid = Some(info.pid);
        status.port = Some(info.port);
        status.http_ok = http;
        status.started_at = Some(info.started_at);
    }
    Ok(status)
}

#[tauri::command]
pub async fn daemon_restart() -> Result<DaemonStatus, String> {
    // Stop whatever is running, even if the lock is stale.
    if let Some(info) = read_lock() {
        if is_process_alive(info.pid) {
            let _ = Command::new("kill")
                .args(["-TERM", &info.pid.to_string()])
                .status();
            std::thread::sleep(std::time::Duration::from_millis(800));
            if is_process_alive(info.pid) {
                let _ = Command::new("kill")
                    .args(["-KILL", &info.pid.to_string()])
                    .status();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    }
    let _ = std::fs::remove_file(memex_dir().join("daemon.lock"));

    let bin = find_daemon_binary().ok_or_else(|| {
        "memex-daemon binary not found next to the app or on PATH".to_string()
    })?;

    Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn daemon at {}: {}", bin.display(), e))?;

    // Give the daemon a moment to write its lock file and bind its port.
    for _ in 0..20 {
        std::thread::sleep(std::time::Duration::from_millis(150));
        if let Some(info) = read_lock() {
            if is_process_alive(info.pid) && http_health_ok(info.port) {
                return Ok(DaemonStatus {
                    running: true,
                    pid: Some(info.pid),
                    port: Some(info.port),
                    http_ok: true,
                    started_at: Some(info.started_at),
                });
            }
        }
    }
    // Even if HTTP isn't responding yet, report what we know so the UI can
    // surface "starting…" instead of "offline".
    Ok(daemon_status().await.unwrap_or(DaemonStatus {
        running: false,
        pid: None,
        port: None,
        http_ok: false,
        started_at: None,
    }))
}
