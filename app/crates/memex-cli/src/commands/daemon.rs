//! `memex daemon` 三个子命令 —— Phase 5a 起 daemon 跟 Memex.app GUI 同生共死，
//! CLI 不再 spawn / kill 独立子进程。
//!
//! * `start` / `stop` → 打印用户指引（提示开 / 关 Memex.app），非零退出码方便
//!   脚本判断
//! * `status` → 借 `MemexClient::connect()` 做"lock + PID 判活 + HTTP /health"
//!   一站式探活

use anyhow::Result;

use crate::client::MemexClient;

/// `memex daemon start` —— 在 in-process 架构下没有"启动 daemon"这回事，
/// 通过友好提示引导用户去打开 Memex.app；返回 Err 让脚本 exit code = 1。
pub fn start(json: bool) -> Result<()> {
    let msg = "Memex daemon 现在跟 Memex.app 主进程同生命周期，无法通过 CLI 单独启动。\n\
               请打开 /Applications/Memex.app（菜单栏 M 图标）。";
    if json {
        crate::io::json(&serde_json::json!({
            "status": "manual_action_required",
            "hint": "open Memex.app",
            "message": msg,
        }))?;
    } else {
        crate::err!("{}", msg);
    }
    anyhow::bail!("memex daemon start: manual action required");
}

/// `memex daemon stop` —— 同上，提示用户从 Memex 菜单栏退出。
pub fn stop(json: bool) -> Result<()> {
    let msg = "Memex daemon 跟 Memex.app 同生命周期。\n\
               请从屏幕右上角菜单栏 Memex (M) 图标点击 Quit。";
    if json {
        crate::io::json(&serde_json::json!({
            "status": "manual_action_required",
            "hint": "quit Memex.app from menubar",
            "message": msg,
        }))?;
    } else {
        crate::err!("{}", msg);
    }
    anyhow::bail!("memex daemon stop: manual action required");
}

/// `memex daemon status` —— 走 client.connect() 的探活逻辑。
/// connect 成功就 print pid / port / started_at；失败就 print 错误，不 panic。
pub fn status(json: bool) -> Result<()> {
    match MemexClient::connect() {
        Ok(client) => {
            if json {
                crate::io::json(&serde_json::json!({
                    "running": true,
                    "pid": client.pid,
                    "port": client.port(),
                    "started_at": client.started_at,
                    "http_ok": true,
                }))?;
            } else {
                crate::out!(
                    "daemon running (pid={}, port={}, started_at={})",
                    client.pid,
                    client.port(),
                    client.started_at
                );
            }
            Ok(())
        }
        Err(e) => {
            if json {
                crate::io::json(&serde_json::json!({
                    "running": false,
                    "error": format!("{}", e),
                }))?;
            } else {
                crate::err!("{}", e);
            }
            // 不 bail：status 命令本身的"daemon 未在跑"是合法返回，exit code = 0
            // 保留 Ok(()) 让 shell 脚本可以 `memex --json daemon status | jq .running`
            // 做条件判断。
            Ok(())
        }
    }
}
