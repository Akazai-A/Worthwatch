use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Local, NaiveDate};
use rust_decimal::Decimal;
use serde::Serialize;

use ledger::{Account, Mutation};

use crate::state::AppState;

/// Everything the Home screen needs, assembled server-side.
#[derive(Serialize)]
pub struct LedgerView {
    pub as_of: NaiveDate,
    pub base_currency: String,
    pub totals: derive::Totals,
    pub series: Vec<derive::Point>,
    pub accounts: Vec<AccountView>,
}

#[derive(Serialize)]
pub struct AccountView {
    #[serde(flatten)]
    pub account: Account,
    pub group: ledger::Group,
    pub value: Decimal,
    pub days_stale: Option<i64>,
    pub missing_fx: bool,
    pub missing_price: bool,
}

fn build_view(l: &ledger::Ledger, today: NaiveDate) -> LedgerView {
    let accounts = l
        .accounts
        .iter()
        .map(|a| {
            let v = derive::value_account(l, a, today);
            AccountView {
                account: a.clone(),
                group: a.kind.group(),
                value: v.value,
                days_stale: derive::days_stale(l, a, today),
                missing_fx: v.missing_fx,
                missing_price: v.missing_price,
            }
        })
        .collect();
    LedgerView {
        as_of: today,
        base_currency: l.settings.base_currency.clone(),
        totals: derive::totals(l, today),
        series: derive::net_worth_series(l),
        accounts,
    }
}

async fn health() -> &'static str {
    "ok"
}

async fn get_view(State(s): State<AppState>) -> Json<LedgerView> {
    let l = s.read();
    Json(build_view(&l, Local::now().date_naive()))
}

#[derive(Serialize)]
struct ApiError {
    message: String,
    line: Option<usize>,
}

async fn apply(
    State(s): State<AppState>,
    Json(ms): Json<Vec<Mutation>>,
) -> Result<Json<LedgerView>, (StatusCode, Json<ApiError>)> {
    match s.apply_many(ms) {
        Ok(l) => Ok(Json(build_view(&l, Local::now().date_naive()))),
        Err(e) => {
            let d = ledger::Diagnostic::from(&e);
            Err((StatusCode::UNPROCESSABLE_ENTITY, Json(ApiError { message: d.message, line: d.line })))
        }
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/view", get(get_view))
        .route("/api/apply", post(apply))
        .with_state(state)
}
