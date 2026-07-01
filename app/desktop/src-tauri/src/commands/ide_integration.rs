//! IDE 集成（MCP server / SKILL 投递）IPC 命令。
//!
//! 这一层**直接 lib 调用 memex-cli** 内部的 `setup::*` / `skill::*` 函数，
//! 完全不 spawn sidecar 子进程。
//!
//! ## 历史
//!
//! 之前曾走 `tokio::process::Command` / `std::process::Command + spawn_blocking`
//! 跑 `memex-cli --json setup-status` 等子进程：在 macOS release-bundled GUI
//! 上偶发 / 必现 `Bad file descriptor (os error 9)`，根因是 LaunchServices
//! 启动 GUI 后父进程 fd 0/1/2 表异常（fd 0 是 closed unix socket → none，
//! fd 1/2 完全缺位）。即便我们启动时把 fd 0/1/2 reset 到 `/dev/null`，
//! Tauri runtime 在初始化期间仍然可能再次"污染"这张 fd 表（IPC channel /
//! webview process fork / global shortcut listener 等），所以无论用哪条
//! spawn 路径，都还是会偶发 spawn 失败。
//!
//! 改成 lib 直调后这条 fault domain 直接消失：所有逻辑都在父进程内同进程
//! 跑，没有 spawn 路径，自然没 EBADF。

use std::path::PathBuf;

use memex_cli::commands::setup as cli_setup;
use memex_cli::commands::skill as cli_skill;
use serde::{Deserialize, Serialize};

use super::error::{CmdError, CmdResult};

/// 与 memex-cli `setup::IdeStatus` 字段对齐的 IPC DTO。
///
/// 维持独立 struct 而不是直接 re-export `cli_setup::IdeStatus`：CLI 那边将来
/// 可能加非 GUI 关心的字段（如 server_args / 调试信息），通过这一层显式 map
/// 就能让前端契约不被无心修改。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeStatus {
    pub ide: String,
    pub config_path: String,
    pub config_exists: bool,
    pub installed: bool,
    pub command: Option<String>,
}

impl From<cli_setup::IdeStatus> for IdeStatus {
    fn from(s: cli_setup::IdeStatus) -> Self {
        Self {
            ide: s.ide,
            config_path: s.config_path,
            config_exists: s.config_exists,
            installed: s.installed,
            command: s.command,
        }
    }
}

/// SKILL.md 投递状态 DTO。映射 `cli_skill::SkillStatus`。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStatus {
    pub ide: String,
    pub dest_path: String,
    pub installed: bool,
    pub size: Option<u64>,
}

impl From<cli_skill::SkillStatus> for SkillStatus {
    fn from(s: cli_skill::SkillStatus) -> Self {
        Self {
            ide: s.ide,
            dest_path: s.dest_path,
            installed: s.installed,
            size: s.size,
        }
    }
}

fn parse_ide(ide: &str) -> CmdResult<cli_setup::Ide> {
    cli_setup::Ide::parse(ide).ok_or_else(|| {
        CmdError::Validation(format!(
            "Unknown IDE: {ide}. Supported: cursor, claude-code, codex, opencode, kiro"
        ))
    })
}

/// 当前 GUI binary 的物理路径——CLI 写到 IDE 配置时填这条 command，
/// 让 IDE 启动 MCP server 时就跑 bundle 内的 sidecar。
fn memex_bin_path() -> PathBuf {
    // .app/Contents/MacOS/Memex 同目录下有 memex-cli sidecar；CLI 那边的
    // setup::install 接受任意路径，传 sidecar 会更明确（IDE 拉起的就是 CLI
    // 自身，而不是 Tauri GUI 里的 mcp 子命令）。
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

#[tauri::command]
pub async fn ide_list_status() -> CmdResult<Vec<IdeStatus>> {
    let list = tokio::task::spawn_blocking(cli_setup::list_status)
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn ide_install(ide: String) -> CmdResult<IdeStatus> {
    let parsed = parse_ide(&ide)?;
    let bin = memex_bin_path();
    let result = tokio::task::spawn_blocking(move || cli_setup::install(parsed, &bin))
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?
        .map_err(|e| CmdError::Backend(format!("ide_install failed: {e:#}")))?;
    Ok(result.into())
}

#[tauri::command]
pub async fn ide_uninstall(ide: String) -> CmdResult<IdeStatus> {
    let parsed = parse_ide(&ide)?;
    let result = tokio::task::spawn_blocking(move || cli_setup::uninstall(parsed))
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?
        .map_err(|e| CmdError::Backend(format!("ide_uninstall failed: {e:#}")))?;
    Ok(result.into())
}

#[tauri::command]
pub async fn skill_list_status() -> CmdResult<Vec<SkillStatus>> {
    let list = tokio::task::spawn_blocking(cli_skill::list_status)
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?;
    Ok(list.into_iter().map(Into::into).collect())
}

#[tauri::command]
pub async fn skill_install(ide: String) -> CmdResult<SkillStatus> {
    let parsed = parse_ide(&ide)?;
    let result = tokio::task::spawn_blocking(move || cli_skill::install(parsed))
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?
        .map_err(|e| CmdError::Backend(format!("skill_install failed: {e:#}")))?;
    Ok(result.into())
}

#[tauri::command]
pub async fn skill_uninstall(ide: String) -> CmdResult<SkillStatus> {
    let parsed = parse_ide(&ide)?;
    let result = tokio::task::spawn_blocking(move || cli_skill::uninstall(parsed))
        .await
        .map_err(|e| CmdError::Backend(format!("join failed: {e}")))?
        .map_err(|e| CmdError::Backend(format!("skill_uninstall failed: {e:#}")))?;
    Ok(result.into())
}
