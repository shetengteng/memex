mod commands;
mod tray;

use tauri::{Emitter, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,memex=debug")),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
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
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            if let Err(e) = tray::install(app.handle()) {
                tracing::error!("failed to install tray icon: {e:?}");
            }

            app.global_shortcut().register(Shortcut::new(
                Some(Modifiers::SUPER | Modifiers::SHIFT),
                Code::KeyM,
            ))?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_stats,
            commands::get_breakdown,
            commands::get_timeline,
            commands::list_recent,
            commands::get_session,
            commands::search_memex,
            commands::get_config,
            commands::set_config,
            commands::toggle_adapter,
        ])
        .run(tauri::generate_context!())
        .expect("error while running memex menubar");
}
