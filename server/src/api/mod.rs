//! The HTTP API: one route table for every `/api/...` path, and the error shape all of
//! them answer with. Each area's handlers and views live in their own module.

use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Serialize;

use crate::state::AppState;

pub mod backups;
pub mod budget;
pub mod chat;
pub mod plan;
pub mod portfolio;
pub mod view;

/// The body of every error answer. `line` points into `ledger.yaml` when the error is there.
#[derive(Debug, Serialize)]
pub struct ApiError {
    pub message: String,
    pub line: Option<usize>,
}

pub type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

pub fn ledger_error(e: ledger::LedgerError) -> (StatusCode, Json<ApiError>) {
    let d = ledger::Diagnostic::from(&e);
    let status = match e {
        ledger::LedgerError::FileHasProblem(_) => StatusCode::CONFLICT,
        ledger::LedgerError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    (status, Json(ApiError { message: d.message, line: d.line }))
}

/// A plain error answer: `(status, {"message": ..., "line": null})`.
pub fn bad(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { message: message.into(), line: None }))
}

pub async fn health() -> &'static str {
    "ok"
}

/// Every `/api/...` route, in one table. The security guard in `main.rs` wraps all of them.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        // Ledger view and the one write path
        .route("/api/view", get(view::get_view))
        .route("/api/apply", post(view::apply))
        .route("/api/refresh", post(view::refresh))
        .route("/api/parse-holdings", post(view::parse_holdings))
        .route("/api/events", get(view::events))
        // Portfolio
        .route("/api/lookthrough", get(portfolio::get_lookthrough))
        .route("/api/changes", get(portfolio::get_changes))
        .route("/api/growth", get(portfolio::get_growth))
        .route("/api/accounts/{id}/months", get(portfolio::get_account_months))
        .route("/api/refresh-holdings", post(portfolio::refresh_holdings))
        // Plan and backups
        .route("/api/plan", get(plan::get_plan))
        .route("/api/backups", get(backups::list_backups).post(backups::create_backup))
        .route("/api/backups/{name}", get(backups::download_backup))
        // Chat and MCP proposals
        .route("/api/chat", post(chat::post_chat))
        .route("/api/context", get(chat::get_context))
        .route("/api/propose", post(chat::propose))
        .route("/api/pending/{id}", get(chat::pending_status))
        .route("/api/pending/{id}/{action}", post(chat::decide))
        // Budget, the Apple Pay feed and its Settings
        .route("/api/ingest/transaction", post(budget::feed::ingest))
        .route("/api/ingest/day", get(budget::game::get_day))
        .route("/api/game", get(budget::game::get_game))
        .route("/api/budget", get(budget::get_budget))
        .route("/api/budget/import/preview", post(budget::import_preview))
        .route("/api/budget/import", post(budget::import))
        .route("/api/budget/category", post(budget::set_category))
        .route("/api/budget/spend", post(budget::add_spend))
        .route("/api/budget/spend/remove", post(budget::remove_spend))
        .route("/api/settings", get(budget::settings::get_settings))
        .route("/api/settings/token", post(budget::settings::rotate_token))
        .route("/api/settings/token/reveal", post(budget::settings::reveal_token))
        .route("/api/settings/shortcut", post(budget::settings::make_shortcut))
        .route("/api/settings/shortcut.plist", get(budget::settings::shortcut_plist))
        .with_state(state)
}
