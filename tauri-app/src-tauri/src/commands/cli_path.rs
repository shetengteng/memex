use std::env;
use std::fs;
use std::os::unix::fs as unix_fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::error::{CmdError, CmdResult};

/// CLI 安装到的最终位置 — 用户 home 下的 `~/.local/bin`，不需 sudo，
/// 也是 XDG / FHS 共识中给"用户自己装的可执行文件"的标准位置。
/// 选这里而不是 `/usr/local/bin`，是为了避免 macOS 上 SIP 与 sudo 摩擦。
///
/// CLI symlink 映射表：`(link_name_in_.local/bin, sidecar_binary_in_bundle)`。
/// 解耦两边是因为 APFS 大小写不敏感：bundle 里 GUI 主 binary `Memex` 跟 CLI
/// `memex` 会撞名，所以 sidecar 物理改名为 `memex-cli`，但用户视角的命令名
/// 仍要保留 `memex`。
const CLI_LINKS: &[(&str, &str)] = &[("memex", "memex-cli")];

fn target_dir() -> PathBuf {
    let home = env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join(".local/bin")
}

/// 当前正在运行的 menubar binary 所在目录（`Memex.app/Contents/MacOS/`），
/// 同目录里就放着 `memex-cli` sidecar 二进制。
fn app_macos_dir() -> Option<PathBuf> {
    env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
}

#[derive(Debug, Serialize)]
pub struct CliStatus {
    /// 用户 PATH 里是否包含 `target_dir`。这是 install 真正生效的前提。
    pub path_contains_target_dir: bool,
    /// 当前 PATH（用于调试与构造错误消息）。
    pub path: String,
    /// 期望安装到的目录。
    pub target_dir: String,
    /// `CLI_LINKS` 列表里所有 symlink 都已正确指向 .app 内对应 sidecar binary。
    pub installed: bool,
    /// 给出建议把目录加入 PATH 的命令（无论 install / not installed，前端都可用来引导用户）。
    pub path_export_hint: String,
}

fn is_dir_in_path(dir: &Path, path: &str) -> bool {
    let dir_str = dir.to_string_lossy();
    path.split(':').any(|p| p == dir_str)
}

fn is_correct_symlink(link: &Path, expected_target: &Path) -> bool {
    match fs::read_link(link) {
        Ok(actual) => actual == expected_target,
        Err(_) => false,
    }
}

/// 检查当前 CLI 安装状态。前端在 Settings 页挂载时调用一次。
#[tauri::command]
pub async fn cli_status() -> CmdResult<CliStatus> {
    let dir = target_dir();
    let path_env = env::var("PATH").unwrap_or_default();
    let app_dir = app_macos_dir();

    let installed = match &app_dir {
        Some(app_dir) => CLI_LINKS.iter().all(|(link_name, target_name)| {
            is_correct_symlink(&dir.join(link_name), &app_dir.join(target_name))
        }),
        None => false,
    };

    let path_export_hint = format!("export PATH=\"{}:$PATH\"", dir.display());

    Ok(CliStatus {
        path_contains_target_dir: is_dir_in_path(&dir, &path_env),
        path: path_env,
        target_dir: dir.to_string_lossy().to_string(),
        installed,
        path_export_hint,
    })
}

/// 在 `~/.local/bin` 下创建 symlink，按 `CLI_LINKS` 表把命令名指向 bundle 内 sidecar。
/// 不主动写 shell rc。如果 PATH 里没有目标目录，仍然创建 symlink，但返回 warning，
/// 让前端提示用户手动 export。
#[tauri::command]
pub async fn cli_install() -> CmdResult<CliStatus> {
    let dir = target_dir();
    let app_dir = app_macos_dir()
        .ok_or_else(|| CmdError::NotFound("无法确定 Memex.app 的 MacOS 目录".into()))?;

    fs::create_dir_all(&dir)?;

    for (link_name, target_name) in CLI_LINKS {
        let link = dir.join(link_name);
        let target = app_dir.join(target_name);
        if !target.exists() {
            return Err(CmdError::NotFound(format!(
                "sidecar binary 不存在：{}",
                target.display()
            )));
        }
        if link.exists() || fs::symlink_metadata(&link).is_ok() {
            // 先把旧的链接/文件清掉，避免 "File exists"
            fs::remove_file(&link)?;
        }
        unix_fs::symlink(&target, &link)?;
    }

    cli_status().await
}

/// 移除 `~/.local/bin` 下指向 .app 内部的 symlink。
/// 只删 symlink，不删用户自己放的真实文件（双保险检查 read_link）。
#[tauri::command]
pub async fn cli_uninstall() -> CmdResult<CliStatus> {
    let dir = target_dir();
    let app_dir = app_macos_dir();

    for (link_name, target_name) in CLI_LINKS {
        let link = dir.join(link_name);
        if !link.exists() && fs::symlink_metadata(&link).is_err() {
            continue;
        }
        let is_ours = app_dir
            .as_ref()
            .map(|app| is_correct_symlink(&link, &app.join(target_name)))
            .unwrap_or(false);
        if !is_ours {
            continue;
        }
        fs::remove_file(&link)?;
    }

    cli_status().await
}
