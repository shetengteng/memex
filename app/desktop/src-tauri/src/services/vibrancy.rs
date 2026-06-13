//! 窗口毛玻璃 (vibrancy / mica) 服务。
//!
//! 职责：在用户切换 Settings → Surface 时，给主窗口和 tray-popup 应用 / 撤销
//! 系统级毛玻璃效果。CSS 端 `.surface-glass` 把 body / 主要容器置成半透明，
//! 真正的"磨砂感"由这里调系统 API 提供（macOS NSVisualEffectView /
//! Windows 11 Mica），其它平台是 no-op。
//!
//! 设计原则：
//! * 失败永远不 panic —— 即便 vibrancy 调用返回错误（比如老系统不支持），
//!   也只 `tracing::warn!` 一行，让前端 / CSS 自己降级到半透明色块。
//! * 所有窗口统一通过 `apply_glass(handle, mode)` 入口，模式只两个：
//!   `SurfaceMode::Solid`（清掉 vibrancy）和 `SurfaceMode::Glass`（应用）。
//!   当下系统是 light 还是 dark 不在 Rust 这层判断 —— `NSVisualEffectMaterial`
//!   会自动跟随窗口的 `NSAppearance`，前端切 light/dark 就够了。

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, WebviewWindow};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SurfaceMode {
    Solid,
    Glass,
}

impl SurfaceMode {
    /// 解析持久化字符串。未识别的值 fallback 到 Solid，这样老版本写入的
    /// kv 不会让新版崩。
    pub fn from_str_lossy(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "glass" => SurfaceMode::Glass,
            _ => SurfaceMode::Solid,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SurfaceMode::Solid => "solid",
            SurfaceMode::Glass => "glass",
        }
    }
}

/// 给两个已有窗口（main + tray-popup）批量应用 surface 模式。
/// 启动时和切换时都走它，调用方不需要自己分发到具体窗口。
pub fn apply_to_all(app: &AppHandle, mode: SurfaceMode) {
    for label in ["main", "tray-popup"] {
        let Some(win) = app.get_webview_window(label) else {
            continue;
        };
        apply_to_window(&win, label, mode);
    }
}

/// 给单个窗口应用 surface 模式。
///
/// 不同窗口选不同的 material：
/// * main 用 `Sidebar` —— 经典 Finder 侧栏感，文字对比度高
/// * tray-popup 用 `Popover` / `HudWindow` —— 更糊更轻，HUD 风
///
/// 切回 solid 时调 `clear_vibrancy`，让 webview 直接显示 CSS 背景色，
/// 不会留有"半玻璃"残影。
pub fn apply_to_window(win: &WebviewWindow, label: &str, mode: SurfaceMode) {
    match mode {
        SurfaceMode::Glass => {
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState, apply_vibrancy};
                let material = if label == "tray-popup" {
                    NSVisualEffectMaterial::Popover
                } else {
                    NSVisualEffectMaterial::Sidebar
                };
                if let Err(e) =
                    apply_vibrancy(win, material, Some(NSVisualEffectState::Active), None)
                {
                    tracing::warn!("apply_vibrancy({label}, {material:?}) failed: {e:?}");
                }
            }
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_mica;
                if let Err(e) = apply_mica(win, None) {
                    tracing::warn!("apply_mica({label}) failed: {e:?}");
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                tracing::debug!("glass surface not supported on this platform; falling back to CSS only");
            }
        }
        SurfaceMode::Solid => {
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::clear_vibrancy;
                if let Err(e) = clear_vibrancy(win) {
                    tracing::warn!("clear_vibrancy({label}) failed: {e:?}");
                }
            }
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::clear_mica;
                if let Err(e) = clear_mica(win) {
                    tracing::warn!("clear_mica({label}) failed: {e:?}");
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {}
        }
    }
}

/// 启动时从 KV 读 `ui.surface`，缺失视为 Solid。
pub fn load_persisted(db: &memex_core::storage::db::Db) -> SurfaceMode {
    db.kv_get("ui.surface")
        .ok()
        .flatten()
        .map(|s| SurfaceMode::from_str_lossy(&s))
        .unwrap_or(SurfaceMode::Solid)
}
