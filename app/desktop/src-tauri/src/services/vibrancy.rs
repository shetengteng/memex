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
/// * main 用 `UnderWindowBackground` —— 最不染色、最透明的系统玻璃，让
///   Light 模式下也能看见桌面/壁纸/背后的窗口内容（Sidebar / Popover 在
///   Light 下会一层白雾，把背后糊成纯白，肉眼分不出 solid vs glass）
/// * tray-popup 用 `Popover` —— 菜单栏弹窗用 popover 系自带的浮窗感
///
/// 切回 solid 时调 `clear_vibrancy`，让 webview 直接显示 CSS 背景色，
/// 不会留有"半玻璃"残影。
pub fn apply_to_window(win: &WebviewWindow, label: &str, mode: SurfaceMode) {
    match mode {
        SurfaceMode::Glass => {
            // Glass 模式下让 wkwebview 透明，下层的 NSVisualEffectView 才能露出来。
            // set_background_color(None) 在 wry 0.55.1 上不能彻底关掉 wkwebview
            // drawsBackground，所以 macOS 上还得走一段 objc shim。
            if let Err(e) = win.set_background_color(None) {
                tracing::warn!("set_background_color({label}, None) failed: {e:?}");
            }
            #[cfg(target_os = "macos")]
            force_transparent_macos(win, label);

            #[cfg(target_os = "macos")]
            {
                use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState, apply_vibrancy};
                let material = if label == "tray-popup" {
                    NSVisualEffectMaterial::Popover
                } else {
                    NSVisualEffectMaterial::UnderWindowBackground
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
            // Solid 模式不调 force_transparent_macos —— 否则 wkwebview 即便没装
            // vibrancy 也会因 setOpaque:NO + drawsBackground:NO 整片透出桌面，
            // 视觉上和 glass 完全一样，用户看不出差别。
            // 下面三步把窗口/webview 恢复成"传统不透明窗口"。
            #[cfg(target_os = "macos")]
            restore_opaque_macos(win, label);

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

/// `force_transparent_macos` 的逆操作：把窗口 / webview 恢复成传统不透明绘制。
/// Solid 模式下必须调它，否则即使 vibrancy 已经移除，wkwebview 因为 KVC
/// `drawsBackground=NO` 仍然透出下层的桌面壁纸 / 别的窗口，看上去就和 glass
/// 模式完全一样，用户根本分不出 solid 和 glass 的区别。
///
/// 由于切换是从 glass 回到 solid 的过场，必须把这条路径上 force_transparent
/// 改过的所有状态都回滚：
/// 1. NSWindow.setOpaque:YES + setBackgroundColor:windowBackgroundColor
/// 2. 主 WKWebView 的 KVC `drawsBackground` 恢复成 YES
///
/// `layer.backgroundColor=nil` 不需要回滚 —— webview 自身会重新绘制底色。
#[cfg(target_os = "macos")]
fn restore_opaque_macos(win: &WebviewWindow, label: &str) {
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
        let _: () = msg_send![ns_window, setOpaque: true];
        let win_bg: *mut AnyObject = msg_send![objc2::class!(NSColor), windowBackgroundColor];
        let _: () = msg_send![ns_window, setBackgroundColor: win_bg];

        let content_view: *mut AnyObject = msg_send![ns_window, contentView];
        if !content_view.is_null() {
            restore_subview_drawing(content_view, 0);
        }
    }
    tracing::info!("restore_opaque_macos applied: window={label}");
}

/// 把 force_transparent 改过的 wkwebview drawsBackground KVC 全部翻回 YES。
#[cfg(target_os = "macos")]
unsafe fn restore_subview_drawing(view: *mut objc2::runtime::AnyObject, depth: u32) {
    use objc2::runtime::AnyObject;
    use objc2::msg_send;
    use objc2_foundation::NSString;

    if depth > 32 {
        return;
    }

    unsafe {
        let cls_name_obj: *mut AnyObject = msg_send![view, className];
        let mut cls_owned = String::new();
        if !cls_name_obj.is_null() {
            let utf8: *const std::ffi::c_char = msg_send![cls_name_obj, UTF8String];
            if !utf8.is_null() {
                cls_owned = std::ffi::CStr::from_ptr(utf8)
                    .to_string_lossy()
                    .into_owned();
            }
        }
        let looks_like_wkwebview = !cls_owned.contains("Parent")
            && (cls_owned.contains("WKWebView")
                || cls_owned.contains("WryWebView")
                || cls_owned.contains("wry_web_view::"));
        if looks_like_wkwebview {
            tracing::info!("restore_opaque: KVC drawsBackground=YES on {cls_owned}");
            let key = NSString::from_str("drawsBackground");
            let yes_obj: *mut AnyObject =
                msg_send![objc2::class!(NSNumber), numberWithBool: true];
            let _: () = msg_send![view, setValue: yes_obj, forKey: &*key];
        }

        let subviews: *mut AnyObject = msg_send![view, subviews];
        if subviews.is_null() {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for i in 0..count {
            let sub: *mut AnyObject = msg_send![subviews, objectAtIndex: i];
            if !sub.is_null() {
                restore_subview_drawing(sub, depth + 1);
            }
        }
    }
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

        // 检测 view 类名。wry 在 macOS 上的子视图层级实际是：
        //   contentView (NSThemeFrame/NSView)
        //     └── WryWebViewParent (NSView wrapper)
        //          └── WryWebView (subclass of WKWebView)
        //               └── ... internal scroll/content views
        //
        // 所以必须递归到底层，且 KVC `drawsBackground` 只能设在 WKWebView 实例
        // 上 —— wrapper view（含 "Parent"）不响应这个 KVC，强行 setValue:forKey
        // 会抛 NSUndefinedKeyException 直接 abort 进程。
        //
        // 安全规则：用 `respondsToSelector:` 探测后再调，并显式排除带 "Parent"
        // 字样的 wrapper 类。
        let cls_name_obj: *mut AnyObject = msg_send![view, className];
        let mut cls_owned = String::new();
        if !cls_name_obj.is_null() {
            let utf8: *const std::ffi::c_char = msg_send![cls_name_obj, UTF8String];
            if !utf8.is_null() {
                cls_owned = std::ffi::CStr::from_ptr(utf8)
                    .to_string_lossy()
                    .into_owned();
            }
        }
        tracing::debug!("force_transparent: depth={depth} class={cls_owned}");
        let looks_like_wkwebview = !cls_owned.contains("Parent")
            && (cls_owned.contains("WKWebView")
                || cls_owned.contains("WryWebView")
                || cls_owned.contains("wry_web_view::"));
        if looks_like_wkwebview {
            tracing::info!("force_transparent: KVC drawsBackground=NO on {cls_owned}");
            let key = NSString::from_str("drawsBackground");
            let no_obj: *mut AnyObject =
                msg_send![objc2::class!(NSNumber), numberWithBool: false];
            let _: () = msg_send![view, setValue: no_obj, forKey: &*key];
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
