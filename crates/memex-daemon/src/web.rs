//! Web UI static file serving.
//! Serves from `~/.memex/web/` if it exists, otherwise returns a redirect to Tauri app.

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::Router;

pub fn static_router() -> Router {
    let web_dir = memex_core::memex_dir().join("web");
    if web_dir.exists() && web_dir.join("index.html").exists() {
        let serve = tower_http::services::ServeDir::new(&web_dir)
            .not_found_service(tower_http::services::ServeFile::new(web_dir.join("index.html")));
        Router::new().fallback_service(axum::routing::any_service(serve))
    } else {
        Router::new().fallback(fallback_page)
    }
}

async fn fallback_page() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        "<html><body style='font-family:system-ui;text-align:center;padding:60px;color:#888'>\
         <h2>Memex</h2><p>Dashboard has moved to the Tauri desktop app.</p>\
         <p style='font-size:14px'>Use the menu bar tray icon to open the dashboard.</p>\
         </body></html>",
    )
}
