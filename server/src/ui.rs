//! Serves the compiled Svelte app embedded in the binary (ui/dist).
//! If ui/dist is missing at build time, "/" explains how to build it.

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "../ui/dist/"]
struct Assets;

async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path).or_else(|| Assets::get("index.html")) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], Body::from(file.data.into_owned())).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            "UI not built. Run `pnpm --dir ui install && pnpm --dir ui build`, then rebuild the server.",
        )
            .into_response(),
    }
}

pub fn router() -> Router {
    Router::new().fallback(get(serve))
}
