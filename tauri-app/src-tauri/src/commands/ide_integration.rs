use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::error::{CmdError, CmdResult};

/// 与 memex-cli `setup::IdeStatus` 字段对齐——通过 spawn CLI + `--json` 解析得到。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeStatus {
    pub ide: String,
    pub config_path: String,
    pub config_exists: bool,
    pub installed: bool,
    pub command: Option<String>,
}

fn locate_memex_cli() -> Option<PathBuf> {
    // bundle 里跟 menubar 同目录的 sidecar。注意名字是 `memex-cli` 而不是 `memex`：
    // bundle 内 GUI 主 binary 叫 `Memex`，CLI 不能用同名（APFS 大小写不敏感会撞）。
    // 用户视角的命令名仍是 `memex`，靠 `~/.local/bin/memex` symlink 映射（见 cli_path.rs）。
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let p = parent.join("memex-cli");
        if p.exists() {
            return Some(p);
        }
    }
    // PATH 兜底，方便 dev 模式直接跑。
    if let Ok(out) = Command::new("which").arg("memex").output() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
            return Some(PathBuf::from(s));
        }
    }
    None
}

fn cli_not_found() -> CmdError {
    CmdError::NotFound("找不到 memex CLI（既不在 app 同目录，也不在 PATH）".into())
}

fn run_cli_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> CmdResult<T> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;

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
pub async fn ide_list_status() -> CmdResult<Vec<IdeStatus>> {
    run_cli_json::<Vec<IdeStatus>>(&["--json", "setup-status"])
}

#[tauri::command]
pub async fn ide_install(ide: String) -> CmdResult<IdeStatus> {
    // 先 install（普通输出），再读 status（--json）。
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let install = Command::new(&bin).args(["setup", &ide]).output()?;
    if !install.status.success() {
        return Err(CmdError::Backend(format!(
            "memex setup {} 失败：{}",
            ide,
            String::from_utf8_lossy(&install.stderr)
        )));
    }
    run_cli_json::<IdeStatus>(&["--json", "setup", &ide, "--status"])
}

#[tauri::command]
pub async fn ide_uninstall(ide: String) -> CmdResult<IdeStatus> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let res = Command::new(&bin)
        .args(["setup", &ide, "--uninstall"])
        .output()?;
    if !res.status.success() {
        return Err(CmdError::Backend(format!(
            "memex setup {} --uninstall 失败：{}",
            ide,
            String::from_utf8_lossy(&res.stderr)
        )));
    }
    run_cli_json::<IdeStatus>(&["--json", "setup", &ide, "--status"])
}

/// SKILL.md 投递状态（对齐 memex-cli `skill::SkillStatus`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStatus {
    pub ide: String,
    pub dest_path: String,
    pub installed: bool,
    pub size: Option<u64>,
}

#[tauri::command]
pub async fn skill_list_status() -> CmdResult<Vec<SkillStatus>> {
    run_cli_json::<Vec<SkillStatus>>(&["--json", "skill-status"])
}

#[tauri::command]
pub async fn skill_install(ide: String) -> CmdResult<SkillStatus> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let res = Command::new(&bin).args(["skill", &ide]).output()?;
    if !res.status.success() {
        return Err(CmdError::Backend(format!(
            "memex skill {} 失败：{}",
            ide,
            String::from_utf8_lossy(&res.stderr)
        )));
    }
    run_cli_json::<SkillStatus>(&["--json", "skill", &ide, "--status"])
}

#[tauri::command]
pub async fn skill_uninstall(ide: String) -> CmdResult<SkillStatus> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let res = Command::new(&bin)
        .args(["skill", &ide, "--uninstall"])
        .output()?;
    if !res.status.success() {
        return Err(CmdError::Backend(format!(
            "memex skill {} --uninstall 失败：{}",
            ide,
            String::from_utf8_lossy(&res.stderr)
        )));
    }
    run_cli_json::<SkillStatus>(&["--json", "skill", &ide, "--status"])
}
