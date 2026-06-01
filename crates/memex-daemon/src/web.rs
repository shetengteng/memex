//! Embedded Web UI static file serving.
//! Serves from `~/.memex/web/` if it exists, otherwise returns embedded index.html.

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;

const EMBEDDED_INDEX: &str = include_str!("../../../web-ui/index.html");

pub fn static_service() -> axum::routing::MethodRouter {
    let web_dir = memex_core::memex_dir().join("web");
    if web_dir.exists() && web_dir.join("index.html").exists() {
        let serve = tower_http::services::ServeDir::new(&web_dir)
            .not_found_service(tower_http::services::ServeFile::new(web_dir.join("index.html")));
        axum::routing::any_service(serve)
    } else {
        axum::routing::any(embedded_index)
    }
}

pub async fn embedded_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        EMBEDDED_INDEX,
    )
}
