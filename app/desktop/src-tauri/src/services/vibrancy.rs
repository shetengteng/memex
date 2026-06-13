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
    // webview 自身的 backgroundColor 必须显式置为透明，否则 wkwebview 会以
    // 实色绘制，把窗口下层的 NSVisualEffectView 完全遮住，看起来就像「毛玻璃
    // 没生效」。Tauri v2 的 `set_background_color(None)` 在 wry 0.55.1 上覆盖
    // 不到 wkwebview drawsBackground KVC（实测仍然白底），所以 macOS 上额外
    // 走一段直接的 objc 调用，确保 NSWindow 与 contentView 子树都透明。
    if let Err(e) = win.set_background_color(None) {
        tracing::warn!("set_background_color({label}, None) failed: {e:?}");
    }

    #[cfg(target_os = "macos")]
    force_transparent_macos(win, label);

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
                match apply_vibrancy(win, material, Some(NSVisualEffectState::Active), None) {
                    Ok(()) => tracing::info!(
                        "vibrancy applied: window={label} material={material:?}"
                    ),
                    Err(e) => tracing::warn!(
                        "apply_vibrancy({label}, {material:?}) failed: {e:?}"
                    ),
                }
            }
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::apply_mica;
                match apply_mica(win, None) {
                    Ok(()) => tracing::info!("mica applied: window={label}"),
                    Err(e) => tracing::warn!("apply_mica({label}) failed: {e:?}"),
                }
            }
            #[cfg(not(any(target_os = "macos", target_os = "windows")))]
            {
                tracing::debug!(
                    "glass surface not supported on this platform; falling back to CSS only"
                );
            }
        }
        SurfaceMode::Solid => {
            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::clear_vibrancy;
                match clear_vibrancy(win) {
                    Ok(removed) => tracing::info!(
                        "vibrancy cleared: window={label} removed={removed}"
                    ),
                    Err(e) => tracing::warn!("clear_vibrancy({label}) failed: {e:?}"),
                }
            }
            #[cfg(target_os = "windows")]
            {
                use window_vibrancy::clear_mica;
                match clear_mica(win) {
                    Ok(()) => tracing::info!("mica cleared: window={label}"),
                    Err(e) => tracing::warn!("clear_mica({label}) failed: {e:?}"),
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

/// macOS 私有透明路径。tauri.conf.json 的 `transparent: true` + `macOSPrivateApi`
/// 只让 NSWindow 透明，但 wkwebview 自身仍可能保持白底，把窗口下方的
/// NSVisualEffectView 完全遮住。最稳的修复是直接给 NSWindow 设 setOpaque:NO +
/// clearColor，并递归把 contentView 树里所有 layer 的 backgroundColor 清成
/// nil，再给 wkwebview 走 setValue:@NO forKey:"drawsBackground"。
///
/// 失败永远只 warn，不影响功能 —— 调用前已经走过 `set_background_color(None)`，
/// 这里只是社区验证过的兜底路径。
#[cfg(target_os = "macos")]
fn force_transparent_macos(win: &WebviewWindow, label: &str) {
    use objc2::runtime::AnyObject;
    use objc2::msg_send;

    let raw = match win.ns_window() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("ns_window({label}) failed: {e:?}");
            return;
        }
    };
    if raw.is_null() {
        return;
    }
    let ns_window = raw as *mut AnyObject;

    unsafe {
        // 1. NSWindow.setOpaque(NO) + clearColor —— 让窗口本身透明。
        let _: () = msg_send![ns_window, setOpaque: false];
        let clear_class: *mut AnyObject = msg_send![objc2::class!(NSColor), clearColor];
        let _: () = msg_send![ns_window, setBackgroundColor: clear_class];

        // 2. 递归 contentView：把所有 NSView 的 wantsLayer 打开 + layer.backgroundColor=nil，
        //    再给 wkwebview KVC `drawsBackground=NO`。
        let content_view: *mut AnyObject = msg_send![ns_window, contentView];
        if !content_view.is_null() {
            clear_subview_backgrounds(content_view, 0);
        }
    }
    tracing::info!("force_transparent_macos applied: window={label}");
}

/// 递归把一个 NSView 子树的 layer.backgroundColor 全部清成 nil，并对其中
/// 出现的 wkwebview 走 `setValue:@NO forKey:"drawsBackground"`。
///
/// `depth` 仅用于防御性递归保护——subview 树理论上不会循环，但 cocoa 历史
/// 上有过 corner case，深度 >32 直接 bail，避免 vibrancy 脚本卡住主线程。
#[cfg(target_os = "macos")]
unsafe fn clear_subview_backgrounds(view: *mut objc2::runtime::AnyObject, depth: u32) {
    use objc2::runtime::AnyObject;
    use objc2::msg_send;
    use objc2_foundation::NSString;

    if depth > 32 {
        return;
    }

    unsafe {
        let _: () = msg_send![view, setWantsLayer: true];
        let layer: *mut AnyObject = msg_send![view, layer];
        if !layer.is_null() {
            let nil_color: *mut AnyObject = std::ptr::null_mut();
            let _: () = msg_send![layer, setBackgroundColor: nil_color];
        }

        // 检测 wkwebview 类型：用 className 字符串比对，比拿 Class 指针更稳。
        let cls_name_obj: *mut AnyObject = msg_send![view, className];
        if !cls_name_obj.is_null() {
            let utf8: *const std::ffi::c_char = msg_send![cls_name_obj, UTF8String];
            if !utf8.is_null() {
                let cls_str = std::ffi::CStr::from_ptr(utf8).to_string_lossy();
                if cls_str.contains("WKWebView") {
                    let key = NSString::from_str("drawsBackground");
                    let no_obj: *mut AnyObject =
                        msg_send![objc2::class!(NSNumber), numberWithBool: false];
                    let _: () = msg_send![view, setValue: no_obj, forKey: &*key];
                }
            }
        }

        // 递归子视图
        let subviews: *mut AnyObject = msg_send![view, subviews];
        if subviews.is_null() {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for i in 0..count {
            let sub: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
            if !sub.is_null() {
                clear_subview_backgrounds(sub, depth + 1);
            }
        }
    }
}
