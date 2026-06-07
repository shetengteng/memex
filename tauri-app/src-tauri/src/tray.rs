use std::time::Duration;

use memex_core::memex_dir;
use memex_core::storage::db::Db;
use tauri::{
    AppHandle, Emitter, Manager, Wry,
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
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

const TRAY_POPUP_LABEL: &str = "tray-popup";
const TRAY_POPUP_WIDTH: f64 = 360.0;

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let count_item = MenuItemBuilder::with_id("count", "Sessions: 0")
        .enabled(false)
        .build(app)?;
    let show_item = MenuItemBuilder::with_id("show", "Show Memex").build(app)?;
    let search_item = MenuItemBuilder::with_id("search", "Search…").build(app)?;
    let settings_item = MenuItemBuilder::with_id("settings", "Settings…").build(app)?;
    let quit_item = MenuItemBuilder::with_id("quit", "Quit Memex").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&count_item])
        .separator()
        .items(&[&show_item, &search_item, &settings_item])
        .separator()
        .items(&[&quit_item])
        .build()?;

    let _tray = TrayIconBuilder::with_id("memex")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Memex — Local AI Memory Hub")
        .icon(tauri::image::Image::from_bytes(include_bytes!("../icons/tray-22x22.png")).unwrap())
        .icon_as_template(true)
        .title("")
        .on_tray_icon_event(|tray, event| {
            tracing::debug!(target: "memex_tray", "tray event: {:?}", event);
            if let tauri::tray::TrayIconEvent::Click {
                button: tauri::tray::MouseButton::Left,
                button_state: tauri::tray::MouseButtonState::Down,
                rect,
                ..
            } = event
            {
                tracing::info!(target: "memex_tray", "left click @ Down — toggling tray popup");
                let app = tray.app_handle();
                if let Some(win) = app.get_webview_window(TRAY_POPUP_LABEL) {
                    if win.is_visible().unwrap_or(false) {
                        let _ = win.hide();
                        return;
                    }
                    let (ix, iy) = match rect.position {
                        tauri::Position::Physical(p) => (p.x as f64, p.y as f64),
                        tauri::Position::Logical(p) => (p.x, p.y),
                    };
                    let (iw, ih) = match rect.size {
                        tauri::Size::Physical(s) => (s.width as f64, s.height as f64),
                        tauri::Size::Logical(s) => (s.width, s.height),
                    };
                    let x = ix - TRAY_POPUP_WIDTH / 2.0 + iw / 2.0;
                    let y = iy + ih;
                    let _ = win.set_position(tauri::PhysicalPosition::new(x as i32, y as i32));
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
                let _ = tray.set_title(Some(""));
            }
        }
    });

    info!("tray icon installed");
    Ok(())
}

fn show_main_and<F: FnOnce(&AppHandle)>(app: &AppHandle, then: F) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
    then(app);
    crate::update_activation_policy(app);
}

fn handle_menu_event(app: &AppHandle<Wry>, ev: tauri::menu::MenuEvent) {
    match ev.id.as_ref() {
        "show" => {
            show_main_and(app, |_| {});
        }
        "search" => {
            show_main_and(app, |a| {
                let _ = a.emit("open-command-palette", ());
            });
        }
        "settings" => {
            show_main_and(app, |a| {
                let _ = a.emit("navigate", "/settings");
            });
        }
        "quit" => app.exit(0),
        _ => {}
    }
}
