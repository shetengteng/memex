pub mod commands;
mod tray;

use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing_subscriber::EnvFilter;

#[derive(Default)]
pub struct DeepLinkState {
    pub pending: Mutex<Option<String>>,
}

#[tauri::command]
fn take_pending_deep_link(state: tauri::State<'_, DeepLinkState>) -> Option<String> {
    state.pending.lock().ok().and_then(|mut g| g.take())
}

/// 返回当前操作系统的登录用户名，用于 Today 页 "晚上好，xxx" 问候语。
/// macOS / Linux 读 `USER`，Windows 读 `USERNAME`，都没有就回退到 "User"。
/// 不引入额外 crate，避免给 Cargo.lock 加噪音。
#[tauri::command]
fn get_system_username() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| "User".to_string())
}

/// 让前端（包括 tray-popup）可以主动唤起主窗口。
///
/// 如果传入 `navigate` 参数（例如 "/today"、"/settings"），后端会在主窗口 show 之后
/// 直接 `main.emit("navigate", path)`，避免「popup 前端 show + popup 前端 emit」
/// 的竞态：当主窗口刚被 show 时它的 navigate listener 还没 mount 就丢事件了。
///
/// 用于 popup 中的 "打开 Memex" / 会话条目点击场景。
#[tauri::command]
fn show_main_window(app: AppHandle, navigate: Option<String>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        if let Some(path) = navigate.as_deref() {
            if !path.is_empty() {
                let _ = win.emit("navigate", path);
            }
        }
    }
    update_activation_policy(&app);
}

/// 显示主窗口（如果隐藏），并把 deep link 转发给前端 router 处理。
fn forward_deep_links(app: &AppHandle, urls: &[url::Url]) {
    for url in urls {
        let url_str = url.as_str().to_string();
        tracing::info!("deep link received: {}", url_str);

        if let Some(state) = app.try_state::<DeepLinkState>() {
            if let Ok(mut g) = state.pending.lock() {
                *g = Some(url_str.clone());
            }
        }

        if let Some(main) = app.get_webview_window("main") {
            let _ = main.show();
            let _ = main.unminimize();
            let _ = main.set_focus();
            let _ = main.emit("deep-link", &url_str);
        }
        update_activation_policy(app);
    }
}

/// 全局快捷键 ⌘⇧M：toggle 主窗口显隐。
/// - 主窗口已聚焦 → 隐藏
/// - 主窗口不可见或未聚焦 → 显示 + focus
fn toggle_main_window(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        let focused = win.is_focused().unwrap_or(false);
        if visible && focused {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
    update_activation_policy(app);
}

/// macOS 下根据"是否还有任何非托盘窗口可见"动态切换激活策略：
/// - 有非托盘窗口可见 → Regular（Dock 显示）
/// - 全部隐藏          → Accessory（Dock 隐藏，菜单栏模式）
///
/// 非 macOS 平台是 no-op。
pub fn update_activation_policy(_app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let any_visible = _app
            .webview_windows()
            .into_iter()
            .any(|(label, win)| label != "tray-popup" && win.is_visible().unwrap_or(false));
        let policy = if any_visible {
            tauri::ActivationPolicy::Regular
        } else {
            tauri::ActivationPolicy::Accessory
        };
        let _ = _app.set_activation_policy(policy);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,memex=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
            for arg in args.iter().skip(1) {
                if let Ok(parsed) = url::Url::parse(arg) {
                    if parsed.scheme() == "memex" {
                        forward_deep_links(app, &[parsed]);
                    }
                }
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == ShortcutState::Pressed
                        && shortcut
                            == &Shortcut::new(Some(Modifiers::SUPER | Modifiers::SHIFT), Code::KeyM)
                    {
                        toggle_main_window(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            app.manage(DeepLinkState::default());

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Regular);

            // 桌面应用形态：首启显示主窗口（tauri.conf.json 中 visible: false 兜底防止首帧白屏）
            let handle_for_close = app.handle().clone();
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();

                // 关闭按钮（红色圆点）→ 拦截后隐藏到托盘，不退出进程
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(win) = handle_for_close.get_webview_window("main") {
                            let _ = win.hide();
                        }
                        update_activation_policy(&handle_for_close);
                    }
                });
            }

            if let Err(e) = tray::install(app.handle()) {
                tracing::error!("failed to install tray icon: {e:?}");
            }

            // setup 阶段：daemon 探活 → 缺失则用现有命令拉起。无 monitor loop / shutdown hook。
            tauri::async_runtime::spawn(async {
                match commands::daemon_status().await {
                    Ok(status) if status.running && status.http_ok => {
                        tracing::info!(
                            "daemon already running pid={:?} port={:?}, skip auto-start",
                            status.pid,
                            status.port
                        );
                    }
                    _ => {
                        tracing::info!("daemon not running, auto-starting…");
                        match commands::daemon_restart().await {
                            Ok(status) => tracing::info!(
                                "auto-start daemon ok pid={:?} port={:?} http_ok={}",
                                status.pid,
                                status.port,
                                status.http_ok
                            ),
                            Err(e) => tracing::error!("auto-start daemon failed: {e}"),
                        }
                    }
                }
            });

            app.global_shortcut().register(Shortcut::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyM,
            ))?;

            #[cfg(any(windows, target_os = "linux"))]
            {
                if let Err(e) = app.deep_link().register("memex") {
                    tracing::warn!("deep-link register failed: {e:?}");
                }
            }

            let handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                forward_deep_links(&handle, event.urls().as_slice());
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            take_pending_deep_link,
            show_main_window,
            get_system_username,
            commands::get_stats,
            commands::get_breakdown,
            commands::get_timeline,
            commands::list_projects,
            commands::list_recent,
            commands::get_session,
            commands::retry_summary,
            commands::batch_summarize,
            commands::search_memex,
            commands::get_config,
            commands::set_config,
            commands::toggle_adapter,
            commands::list_reports,
            commands::regenerate_report,
            commands::daemon_status,
            commands::daemon_restart,
            commands::daemon_log_path,
            commands::trigger_ingest,
            commands::cli_status,
            commands::cli_install,
            commands::cli_uninstall,
            commands::ide_list_status,
            commands::ide_install,
            commands::ide_uninstall,
            commands::skill_list_status,
            commands::skill_install,
            commands::skill_uninstall,
            commands::hook_list_status,
            commands::hook_install,
            commands::hook_uninstall,
            commands::doctor_run,
            commands::reflect_list,
            commands::reflect_get,
            commands::reflect_run,
            commands::get_workload,
            commands::check_for_updates,
            commands::llm_test_ollama,
            commands::llm_provider_list,
            commands::llm_provider_upsert,
            commands::llm_provider_delete,
            commands::llm_provider_test,
            commands::llm_provider_test_draft,
            commands::llm_list_models,
            commands::system_reset_index,
            commands::system_reset_all,
        ])
        .run(tauri::generate_context!())
        .expect("error while running memex menubar");
}
