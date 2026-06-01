pub mod commands;
mod tray;

use std::sync::Mutex;
use tauri::{Emitter, Manager};
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

fn forward_deep_links(app: &tauri::AppHandle, urls: &[url::Url]) {
    for url in urls {
        let url_str = url.as_str().to_string();
        tracing::info!("deep link received: {}", url_str);

        if let Some(dash) = app.get_webview_window("dashboard") {
            let _ = dash.show();
            let _ = dash.set_focus();
            let _ = dash.emit("deep-link", &url_str);
        } else {
            if let Some(state) = app.try_state::<DeepLinkState>() {
                if let Ok(mut g) = state.pending.lock() {
                    *g = Some(url_str.clone());
                }
            }

            let url_arg = url_str.clone();
            let app_handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let dev = cfg!(debug_assertions);
                let dash_url = if dev {
                    "http://localhost:1420/#/dashboard".to_string()
                } else {
                    "index.html#/dashboard".to_string()
                };
                let _ = tauri::WebviewWindowBuilder::new(
                    &app_handle,
                    "dashboard",
                    tauri::WebviewUrl::App(dash_url.into()),
                )
                .title("Memex Dashboard")
                .inner_size(1100.0, 720.0)
                .min_inner_size(800.0, 500.0)
                .center()
                .decorations(true)
                .resizable(true)
                .transparent(false)
                .build();

                if let Some(dash) = app_handle.get_webview_window("dashboard") {
                    for _ in 0..50 {
                        if dash.is_visible().unwrap_or(false) {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    }
                    let _ = dash.emit("deep-link", url_arg);
                }
            });
        }
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
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                            let _ = win.emit("global-shortcut", "toggle");
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            app.manage(DeepLinkState::default());

            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let Err(e) = tray::install(app.handle()) {
                tracing::error!("failed to install tray icon: {e:?}");
            }

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running memex menubar");
}
