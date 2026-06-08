//! 一次性手动备份。复用 `memex backup` CLI，避免在 menubar 这边重新实现
//! tar/flate2 打包逻辑 —— CLI 那边已经把 memex.db / config.toml / sessions/
//! 三个产物收拢好了。
use std::path::PathBuf;
use std::process::Command;

use memex_core::memex_dir;
use serde::{Deserialize, Serialize};

/// 返回 memex 数据目录（`~/.memex`）的绝对路径，给前端展示用。
/// DataTab 用它显示数据库路径，避免之前为了拿一个路径把 doctor 全跑一遍。
#[tauri::command]
pub fn memex_data_dir() -> String {
    memex_dir().to_string_lossy().to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupResult {
    pub path: String,
    pub files: u64,
    pub size_bytes: u64,
}

fn locate_memex_cli() -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let p = parent.join("memex");
            if p.exists() {
                return Some(p);
            }
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

/// 在 `~/.memex/backups/` 下生成一个带时间戳的 `.tar.gz`，返回备份信息。
#[tauri::command]
pub async fn backup_now() -> Result<BackupResult, String> {
    let bin = locate_memex_cli()
        .ok_or_else(|| "找不到 memex CLI（既不在 app 同目录，也不在 PATH）".to_string())?;

    let backup_dir = memex_dir().join("backups");
    if !backup_dir.exists() {
        std::fs::create_dir_all(&backup_dir)
            .map_err(|e| format!("创建备份目录失败：{}", e))?;
    }

    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let output_path = backup_dir.join(format!("memex-{}.tar.gz", ts));
    let output_str = output_path.to_string_lossy().to_string();

    let output = Command::new(&bin)
        .args(["--json", "backup", &output_str])
        .output()
        .map_err(|e| format!("调用 {} 失败：{}", bin.display(), e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "memex backup 返回非零（{}）：{}",
            output.status, stderr
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.trim())
        .map_err(|e| format!("无法解析 CLI 输出（{}）：{}", e, stdout))
}
