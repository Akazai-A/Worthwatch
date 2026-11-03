//! Backups from the browser: list, make one now, download one.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Local;

use crate::state::AppState;

use super::{bad, ApiResult};

pub async fn list_backups(State(s): State<AppState>) -> Json<Vec<crate::backup::BackupInfo>> {
    Json(crate::backup::list(&s.data_dir()))
}

pub async fn create_backup(State(s): State<AppState>) -> ApiResult<Vec<crate::backup::BackupInfo>> {
    let dir = s.data_dir();
    let made = tokio::task::spawn_blocking(move || crate::backup::create(&dir, Local::now())).await;
    match made {
        Ok(Ok(_)) => Ok(Json(crate::backup::list(&s.data_dir()))),
        Ok(Err(e)) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
        Err(e) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

pub async fn download_backup(
    State(s): State<AppState>,
    Path(name): Path<String>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match crate::backup::path_of(&s.data_dir(), &name).and_then(|p| std::fs::read(p).ok()) {
        Some(bytes) => (
            [
                (axum::http::header::CONTENT_TYPE, "application/gzip".to_string()),
                (axum::http::header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\"")),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
