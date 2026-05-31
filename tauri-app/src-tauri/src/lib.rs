mod commands;
mod tray;

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
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Err(e) = tray::install(app.handle()) {
                tracing::error!("failed to install tray icon: {e:?}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_stats,
            commands::list_recent,
            commands::search_memex,
        ])
        .run(tauri::generate_context!())
        .expect("error while running memex menubar");
}
