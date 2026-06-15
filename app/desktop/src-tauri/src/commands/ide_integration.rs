use std::path::PathBuf;
use std::process::Command as StdCommand;

use serde::{Deserialize, Serialize};
use tokio::process::Command;

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
    // bundle 里跟 menubar 同目录的 sidecar，名字就是 `memex-cli`：bundle 内 GUI
    // 主 binary 叫 `Memex`，CLI 不能用同名（APFS 大小写不敏感会撞），所以物理
    // 名 + 用户视角命令名都统一为 `memex-cli`。
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let p = parent.join("memex-cli");
        if p.exists() {
            return Some(p);
        }
    }
    // PATH 兜底，方便 dev 模式直接跑。`which` 通过 sync std::process::Command
    // 跑——locate 路径只在第一次打开 sidecar 不存在时走（dev 模式），生产 .app
    // bundle 永远命中上一段，没有竞态。
    if let Ok(out) = StdCommand::new("which").arg("memex-cli").output() {
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

/// async-原生 spawn + 读 stdout/stderr。
///
/// 之前用同步 `std::process::Command::output()` 在 daemon shutdown→restart
/// 窗口期会必现 `Bad file descriptor (os error 9)`：sync output 内部 read pipe
/// 的 fd 在 tokio runtime 与其他维护任务并发关闭/复用 fd 时被破坏。
/// `tokio::process::Command` 走 reactor 异步读 pipe，整个流程整合到 runtime
/// 上，避开 sync read 撞 fd 失效；额外把 stdin 显式设为 `null`，防止子进程
/// inherit 父进程那边可能已无效的 stdin fd。
async fn run_cli_json<T: for<'de> Deserialize<'de>>(args: &[&str]) -> CmdResult<T> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;

    let output = Command::new(&bin)
        .args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, args = ?args, "memex-cli spawn/output failed");
            CmdError::from(e)
        })?;

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
    run_cli_json::<Vec<IdeStatus>>(&["--json", "setup-status"]).await
}

#[tauri::command]
pub async fn ide_install(ide: String) -> CmdResult<IdeStatus> {
    // 先 install（普通输出），再读 status（--json）。
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let install = Command::new(&bin)
        .args(["setup", &ide])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if !install.status.success() {
        return Err(CmdError::Backend(format!(
            "memex-cli setup {} 失败：{}",
            ide,
            String::from_utf8_lossy(&install.stderr)
        )));
    }
    run_cli_json::<IdeStatus>(&["--json", "setup", &ide, "--status"]).await
}

#[tauri::command]
pub async fn ide_uninstall(ide: String) -> CmdResult<IdeStatus> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let res = Command::new(&bin)
        .args(["setup", &ide, "--uninstall"])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if !res.status.success() {
        return Err(CmdError::Backend(format!(
            "memex-cli setup {} --uninstall 失败：{}",
            ide,
            String::from_utf8_lossy(&res.stderr)
        )));
    }
    run_cli_json::<IdeStatus>(&["--json", "setup", &ide, "--status"]).await
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
    run_cli_json::<Vec<SkillStatus>>(&["--json", "skill-status"]).await
}

#[tauri::command]
pub async fn skill_install(ide: String) -> CmdResult<SkillStatus> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let res = Command::new(&bin)
        .args(["skill", &ide])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if !res.status.success() {
        return Err(CmdError::Backend(format!(
            "memex skill {} 失败：{}",
            ide,
            String::from_utf8_lossy(&res.stderr)
        )));
    }
    run_cli_json::<SkillStatus>(&["--json", "skill", &ide, "--status"]).await
}

#[tauri::command]
pub async fn skill_uninstall(ide: String) -> CmdResult<SkillStatus> {
    let bin = locate_memex_cli().ok_or_else(cli_not_found)?;
    let res = Command::new(&bin)
        .args(["skill", &ide, "--uninstall"])
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;
    if !res.status.success() {
        return Err(CmdError::Backend(format!(
            "memex skill {} --uninstall 失败：{}",
            ide,
            String::from_utf8_lossy(&res.stderr)
        )));
    }
    run_cli_json::<SkillStatus>(&["--json", "skill", &ide, "--status"]).await
}
