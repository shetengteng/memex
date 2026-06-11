//! IDE SessionStart hook 管理。这一层完全靠 spawn `memex-cli hooks ... --json` 工作；
//! 真正的脏活（写 `~/.claude/settings.json`、`~/.cursor/hooks.json` 等）在 CLI 那边。
//!
//! 之所以走 CLI spawn 而不是把 `memex-cli::commands::hooks` 直接挂到 Tauri 的依赖里，
//! 是为了和 `ide_integration` / `skill_*` 的实现保持一致 —— 那边已经踩过路径解析、
//! sudo 不需要、错误 surface 这些坑，照搬最稳。
use std::path::PathBuf;
use std::process::Command;

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

fn locate_memex_cli() -> Option<PathBuf> {
    // bundle 里 sidecar 名是 `memex-cli`（GUI 主 binary 占了 `Memex`，APFS 大小写
    // 不敏感会撞名）。详见 commands/cli_path.rs 的 CLI_LINKS。
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let p = parent.join("memex-cli");
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(out) = Command::new("which").arg("memex").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    None
}

fn run_cli_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> CmdResult<T> {
    let bin = locate_memex_cli().ok_or_else(|| {
        CmdError::NotFound("找不到 memex CLI（既不在 app 同目录，也不在 PATH）".into())
    })?;
    let output = Command::new(&bin).args(args).output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(CmdError::Backend(format!(
            "memex {:?} 返回非零（{}）：{}",
            args, output.status, stderr
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .map_err(|e| CmdError::Backend(format!("无法解析 CLI 输出（{}）：{}", e, stdout)))
}

#[tauri::command]
pub async fn hook_list_status() -> CmdResult<Vec<HookStatus>> {
    run_cli_json::<Vec<HookStatus>>(&["--json", "hooks", "all"])
}

#[tauri::command]
pub async fn hook_install(ide: String) -> CmdResult<HookStatus> {
    run_cli_json::<HookStatus>(&["--json", "hooks", "install", &ide])
}

#[tauri::command]
pub async fn hook_uninstall(ide: String) -> CmdResult<HookStatus> {
    run_cli_json::<HookStatus>(&["--json", "hooks", "uninstall", &ide])
}
