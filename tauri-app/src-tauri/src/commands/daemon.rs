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

/// 暴露给 maintenance.rs 复用同样的杀 daemon 流程，避免在两个文件里重复
/// 实现 / 维护一份 lock 文件解析。仅对同 crate 可见。
pub(crate) fn read_lock_for_maintenance() -> Option<LockInfo> {
    read_lock()
}

/// 同上：暴露 `kill -0` 判活给 maintenance.rs。
pub(crate) fn is_process_alive_for_maintenance(pid: u32) -> bool {
    is_process_alive(pid)
}

fn is_process_alive(pid: u32) -> bool {
    // kill -0 在进程存在且我们有权限给它发信号时返回 0。
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
    // 优先用跟 menubar 主程序同目录的二进制；
    // bundle 打包出来的目录结构就是这样放的。
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("memex-daemon");
            if p.exists() {
                return Some(p);
            }
        }
    }
    // 退而求其次：从 PATH 里找。
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
    // 不管当前是不是过期 lock，先把存活的进程杀掉。
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
        "在 app 同目录和 PATH 上都找不到 memex-daemon 可执行文件".to_string()
    })?;

    Command::new(&bin)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("启动 daemon 失败（{}）：{}", bin.display(), e))?;

    // 等 daemon 写 lock 文件、绑端口。
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
    // 即使 HTTP 还没就绪，也把已知的状态返回给 UI，
    // 让它能显示"启动中"而不是"未运行"。
    Ok(daemon_status().await.unwrap_or(DaemonStatus {
        running: false,
        pid: None,
        port: None,
        http_ok: false,
        started_at: None,
    }))
}
