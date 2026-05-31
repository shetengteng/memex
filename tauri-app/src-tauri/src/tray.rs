use std::time::Duration;

use memex_core::memex_dir;
use memex_core::storage::db::Db;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    AppHandle, Manager, Wry,
};
use tracing::{info, warn};

fn session_count() -> u64 {
    let db_path = memex_dir().join("memex.db");
    if !db_path.exists() {
        return 0;
    }
    Db::open(&db_path)
        .and_then(|db| db.session_count())
        .unwrap_or(0)
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let show_item = MenuItemBuilder::with_id("show", "Open Memex").build(app)?;
    let search_item = MenuItemBuilder::with_id("search", "Search Memories").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit Memex").build(app)?;

    let count_item = MenuItemBuilder::with_id("count", "Sessions: 0")
        .enabled(false)
        .build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&count_item])
        .separator()
        .items(&[&show_item, &search_item])
        .separator()
        .items(&[&quit_item])
        .build()?;

    let _tray = TrayIconBuilder::with_id("memex")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Memex — Local AI Memory Hub")
        .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
            tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1)
        }))
        .icon_as_template(true)
        .title("Memex")
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::Click { .. } = event {
                if let Some(win) = tray.app_handle().get_webview_window("main") {
                    let _ = win.show();
                    let _ = win.set_focus();
                }
            }
        })
        .on_menu_event(handle_menu_event)
        .build(app)?;

    let app_handle = app.clone();
    let count_h = count_item.clone();
    tauri::async_runtime::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            let count = session_count();
            if let Err(e) = count_h.set_text(format!("Sessions: {count}")) {
                warn!("tray: failed to update count: {e:?}");
            }
            if let Some(tray) = app_handle.tray_by_id("memex") {
                let _ = tray.set_title(Some(&format!("Memex {count}")));
            }
        }
    });

    info!("tray icon installed");
    Ok(())
}

fn handle_menu_event(app: &AppHandle<Wry>, ev: tauri::menu::MenuEvent) {
    match ev.id.as_ref() {
        "show" | "search" => {
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
