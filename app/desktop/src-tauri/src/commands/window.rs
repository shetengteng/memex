//! 窗口外观相关 IPC：目前只有「Surface」（毛玻璃 / 实色）切换。
//!
//! 前端 `useSurface` composable 在用户点 Settings → Surface 时调
//! `set_window_surface(mode)`，本命令负责：
//!   1. 立即给主窗口 + tray-popup 应用 vibrancy（macOS）/ Mica（Win11）
//!   2. 写 `ui.surface` 到 KV，让下次冷启动直接恢复
//!
//! 失败永远不返回 Err —— vibrancy 调用在老系统上可能失败（比如 macOS 10.15
//! 之前），那只是降级到 CSS 半透明色，不影响功能。所以返回值只是布尔，
//! 表示后端是否真的尝试了应用（false = 平台不支持）。

use crate::services::vibrancy::{self, SurfaceMode};
use memex_core::memex_dir;
use memex_core::storage::db::Db;
use tauri::AppHandle;

use super::error::{CmdError, CmdResult};

/// 应用 surface 模式（solid / glass）到全部 Tauri 窗口并持久化。
#[tauri::command]
pub fn set_window_surface(app: AppHandle, mode: String) -> CmdResult<bool> {
    let parsed = SurfaceMode::from_str_lossy(&mode);
    vibrancy::apply_to_all(&app, parsed);

    let db_path = memex_dir().join("memex.db");
    let db = Db::open(&db_path).map_err(|e| CmdError::Db(format!("open: {e:?}")))?;
    db.kv_set("ui.surface", parsed.as_str())
        .map_err(|e| CmdError::Db(format!("kv_set ui.surface: {e:?}")))?;

    Ok(cfg!(any(target_os = "macos", target_os = "windows")))
}

/// 启动时同步 surface 状态给前端（前端 useSurface 不走 localStorage 决定真值，
/// 而是以 KV 为准）。返回 "solid" / "glass"。
#[tauri::command]
pub fn get_window_surface() -> CmdResult<String> {
    let db_path = memex_dir().join("memex.db");
    let db = Db::open(&db_path).map_err(|e| CmdError::Db(format!("open: {e:?}")))?;
    Ok(vibrancy::load_persisted(&db).as_str().to_string())
}
