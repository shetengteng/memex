//! Embedded Web UI static file serving.
//! Serves from `~/.memex/web/` if it exists, otherwise returns embedded assets.

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;

const EMBEDDED_INDEX: &str = include_str!("../../../web-ui/index.html");
const EMBEDDED_CSS: &str = include_str!("../../../web-ui/style.css");
const EMBEDDED_JS: &str = include_str!("../../../web-ui/app.js");

pub fn static_router() -> Router {
    let web_dir = memex_core::memex_dir().join("web");
    if web_dir.exists() && web_dir.join("index.html").exists() {
        let serve = tower_http::services::ServeDir::new(&web_dir)
            .not_found_service(tower_http::services::ServeFile::new(web_dir.join("index.html")));
        Router::new().fallback_service(axum::routing::any_service(serve))
    } else {
        Router::new()
            .route("/style.css", get(embedded_css))
            .route("/app.js", get(embedded_js))
            .fallback(embedded_index)
    }
}

async fn embedded_index() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        EMBEDDED_INDEX,
    )
}

async fn embedded_css() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
        EMBEDDED_CSS,
    )
}

async fn embedded_js() -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/javascript; charset=utf-8")],
        EMBEDDED_JS,
    )
}
