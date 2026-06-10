use std::process::Command;

use anyhow::{Context, Result};
use memex_core::memex_dir;

use crate::commands::daemon_client;

pub fn start(json: bool) -> Result<()> {
    let memex = memex_dir();

    if let Some(info) =
        daemon_client::read_lock(&memex).filter(|i| daemon_client::is_process_alive(i.pid))
    {
        if json {
            crate::io::json(&serde_json::json!({
                "status": "already_running",
                "pid": info.pid,
                "port": info.port,
            }))?;
        } else {
            crate::out!(
                "daemon already running (pid={}, port={})",
                info.pid,
                info.port
            );
        }
        return Ok(());
    }

    let daemon_bin = find_daemon_binary()?;

    let child = Command::new(&daemon_bin)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .with_context(|| format!("failed to start daemon: {}", daemon_bin))?;

    if json {
        crate::io::json(&serde_json::json!({
            "status": "started",
            "pid": child.id(),
        }))?;
    } else {
        crate::out!("daemon started (pid={})", child.id());
    }
    Ok(())
}

pub fn stop(json: bool) -> Result<()> {
    let memex = memex_dir();

    match daemon_client::read_lock(&memex) {
        Some(info) if daemon_client::is_process_alive(info.pid) => {
            unsafe { libc::kill(info.pid as i32, libc::SIGTERM) };
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = std::fs::remove_file(memex.join("daemon.lock"));

            if json {
                crate::io::json(&serde_json::json!({
                    "status": "stopped",
                    "pid": info.pid,
                }))?;
            } else {
                crate::out!("daemon stopped (pid={})", info.pid);
            }
        }
        _ => {
            if json {
                crate::io::json(&serde_json::json!({ "status": "not_running" }))?;
            } else {
                crate::out!("daemon is not running");
            }
        }
    }
    Ok(())
}

pub fn status(json: bool) -> Result<()> {
    let memex = memex_dir();

    match daemon_client::read_lock(&memex) {
        Some(info) if daemon_client::is_process_alive(info.pid) => {
            let health = daemon_client::check_health(info.port);
            if json {
                crate::io::json(&serde_json::json!({
                    "running": true,
                    "pid": info.pid,
                    "port": info.port,
                    "started_at": info.started_at,
                    "http_ok": health,
                }))?;
            } else {
                crate::out!(
                    "daemon running (pid={}, port={}, http={})",
                    info.pid,
                    info.port,
                    if health { "ok" } else { "unreachable" }
                );
            }
        }
        _ => {
            if json {
                crate::io::json(&serde_json::json!({ "running": false }))?;
            } else {
                crate::out!("daemon is not running");
            }
        }
    }
    Ok(())
}

fn find_daemon_binary() -> Result<String> {
    let exe = std::env::current_exe()?;
    let dir = exe.parent().unwrap_or(std::path::Path::new("."));
    let candidate = dir.join("memex-daemon");
    if candidate.exists() {
        return Ok(candidate.to_string_lossy().into_owned());
    }
    Ok("memex-daemon".to_string())
}
