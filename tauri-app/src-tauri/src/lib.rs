//! Memex Tauri menubar 应用的 backend。
//!
//! 这里只暴露 `run()` 给同 crate 下的 `main.rs`，并通过 `#[tauri::command]`
//! 把 [`commands`] 子模块的 IPC handler 注册到 Tauri runtime。webview 渲染
//! 在 `tauri-app/src/` 下的 Vue3 前端。
//!
//! 这是 desktop app crate —— 比纯 library crate 更宽松（webview / 系统集成
//! 等会触发 pedantic clippy 误报），所以只启 `clippy::all`（默认级别），
//! 不启 `rust_2018_idioms`（idiom lint 与 Tauri macro 展开偶有冲突）。

#![warn(clippy::all)]

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
        if let Some(path) = navigate.as_deref()
            && !path.is_empty()
        {
            let _ = win.emit("navigate", path);
        }
    }
    update_activation_policy(&app);
}

/// 显示主窗口（如果隐藏），并把 deep link 转发给前端 router 处理。
fn forward_deep_links(app: &AppHandle, urls: &[url::Url]) {
    for url in urls {
        let url_str = url.as_str().to_string();
        tracing::info!("deep link received: {}", url_str);

        if let Some(state) = app.try_state::<DeepLinkState>()
            && let Ok(mut g) = state.pending.lock()
        {
            *g = Some(url_str.clone());
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
                if let Ok(parsed) = url::Url::parse(arg)
                    && parsed.scheme() == "memex"
                {
                    forward_deep_links(app, &[parsed]);
                }
            }
        }))
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_dialog::init())
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
                match commands::daemon::daemon_status().await {
                    Ok(status) if status.running && status.http_ok => {
                        tracing::info!(
                            "daemon already running pid={:?} port={:?}, skip auto-start",
                            status.pid,
                            status.port
                        );
                    }
                    _ => {
                        tracing::info!("daemon not running, auto-starting…");
                        match commands::daemon::daemon_restart().await {
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
            commands::stats::get_stats,
            commands::stats::get_breakdown,
            commands::stats::get_timeline,
            commands::stats::list_projects,
            commands::stats::get_workload,
            commands::sessions::list_recent,
            commands::sessions::list_sessions_filtered,
            commands::sessions::get_session,
            commands::sessions::retry_summary,
            commands::sessions::batch_summarize,
            commands::sessions::abort_summarize,
            commands::search::search_memex,
            commands::config::get_config,
            commands::config::set_config,
            commands::config::toggle_adapter,
            commands::reports::list_reports,
            commands::reports::regenerate_report,
            commands::threads::list_threads,
            commands::threads::get_thread_detail,
            commands::threads::regenerate_threads,
            commands::threads::delete_thread,
            commands::threads::search_thread_by_query,
            commands::daemon::daemon_status,
            commands::daemon::daemon_restart,
            commands::daemon::daemon_log_path,
            commands::backup::memex_data_dir,
            commands::backup::backup_now,
            commands::backup::ensure_backup_dir,
            commands::backup::export_db,
            commands::backup::import_db,
            commands::mcp_activity::mcp_recent_calls,
            commands::mcp_activity::mcp_call_stats_24h,
            commands::logs::list_daemon_log_files,
            commands::logs::read_daemon_log,
            commands::ingest::trigger_ingest,
            commands::cli_path::cli_status,
            commands::cli_path::cli_install,
            commands::cli_path::cli_uninstall,
            commands::ide_integration::ide_list_status,
            commands::ide_integration::ide_install,
            commands::ide_integration::ide_uninstall,
            commands::ide_integration::skill_list_status,
            commands::ide_integration::skill_install,
            commands::ide_integration::skill_uninstall,
            commands::hooks::hook_list_status,
            commands::hooks::hook_install,
            commands::hooks::hook_uninstall,
            commands::doctor::doctor_run,
            commands::reflect::reflect_list,
            commands::reflect::reflect_get,
            commands::reflect::reflect_run,
            commands::update::check_for_updates,
            commands::llm_test::llm_test_ollama,
            commands::llm_providers::llm_provider_list,
            commands::llm_providers::llm_provider_upsert,
            commands::llm_providers::llm_provider_delete,
            commands::llm_providers::llm_provider_test,
            commands::llm_providers::llm_provider_test_draft,
            commands::llm_providers::llm_list_models,
            commands::maintenance::system_reset_index,
            commands::maintenance::system_reset_all,
        ])
        .build(tauri::generate_context!())
        .expect("INVARIANT: tauri Builder::build() failed — app is unstartable")
        .run(|app_handle, event| match event {
            // 兜底：托盘 quit 已显式调用 stop_daemon_blocking；但其他退出路径
            // （`app.exit(0)` 来自其他模块、菜单 Cmd+Q、`launchctl bootout`）
            // 同样应该清理 daemon，避免后台游离进程。RunEvent::ExitRequested
            // 在 `app.exit()` 即将真正退出前触发，是最后一道闸口。
            tauri::RunEvent::ExitRequested { .. } => {
                tracing::info!("exit requested: stopping daemon");
                commands::daemon::stop_daemon_blocking();
            }
            // macOS：用户点 Dock 图标 / `open -a Memex` 二次启动 → 系统派发
            // `applicationShouldHandleReopen` → Tauri 转成 `RunEvent::Reopen`。
            // 托盘被 macOS 折叠到屏幕外的现场，Dock 是用户唤起主窗口的主要路径，
            // 必须显式 show + focus，否则点击 Dock 图标会"看起来打不开"。
            #[cfg(target_os = "macos")]
            tauri::RunEvent::Reopen {
                has_visible_windows,
                ..
            } => {
                if !has_visible_windows
                    && let Some(win) = app_handle.get_webview_window("main")
                {
                    let _ = win.show();
                    let _ = win.unminimize();
                    let _ = win.set_focus();
                }
                update_activation_policy(app_handle);
            }
            _ => {}
        });
}
