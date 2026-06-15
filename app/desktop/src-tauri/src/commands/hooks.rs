//! IDE SessionStart hook 管理 IPC 命令。
//!
//! 通过 `memex-cli` lib 直接调用，**不 spawn sidecar 子进程**。
//! 详见 [`super::ide_integration`] 的注释——同一条根因（macOS GUI fd 表
//! 异常导致 spawn 偶发 EBADF），同一种解法（lib 调用，全程在父进程内）。

use std::path::PathBuf;

use memex_cli::commands::hooks as cli_hooks;
use memex_cli::commands::setup as cli_setup;
use serde::{Deserialize, Serialize};

use super::error::{CmdError, CmdResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookStatus {
    pub ide: String,
    pub supported: bool,
    pub installed: bool,
    pub config_path: String,
    pub wrapper_path: Option<String>,
}

impl From<cli_hooks::HookStatus> for HookStatus {
    fn from(s: cli_hooks::HookStatus) -> Self {
        Self {
            ide: s.ide,
            supported: s.supported,
            installed: s.installed,
            config_path: s.config_path,
            wrapper_path: s.wrapper_path,
        }
    }
}

fn parse_ide(ide: &str) -> CmdResult<cli_setup::Ide> {
    cli_setup::Ide::parse(ide).ok_or_else(|| {
        CmdError::Validation(format!(
            "Unknown IDE: {ide}. Supported: cursor, claude-code, codex, opencode"
        ))
    })
}

fn memex_bin_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let p = parent.join("memex-cli");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("memex-cli")
}

fn memex_home_dir() -> PathBuf {
    // 与 cli/skill 走同一个约定 —— 用户 home 下的 .memex 目录，
    // wrapper 脚本会写到 .memex/hooks/<ide>-session-start.sh。
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".memex")
}

#[tauri::command]
pub async fn hook_list_status() -> CmdResult<Vec<HookStatus>> {
    let list = tokio::task::spawn_blocking(cli_hooks::list_status)
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn hook_install(ide: String) -> CmdResult<HookStatus> {
    let parsed = parse_ide(&ide)?;
    let bin = memex_bin_path();
    let home = memex_home_dir();
    let result = tokio::task::spawn_blocking(move || cli_hooks::install(parsed, &bin, &home))
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?
        .map_err(|e| CmdError::Backend(format!("hook_install failed: {e:#}")))?;
    Ok(result.into())
}

#[tauri::command]
pub async fn hook_uninstall(ide: String) -> CmdResult<HookStatus> {
    let parsed = parse_ide(&ide)?;
    let result = tokio::task::spawn_blocking(move || cli_hooks::uninstall(parsed))
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?
        .map_err(|e| CmdError::Backend(format!("hook_uninstall failed: {e:#}")))?;
    Ok(result.into())
}
