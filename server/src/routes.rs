use std::convert::Infallible;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{Datelike, Local, NaiveDate};
use futures_util::stream::Stream;
use rust_decimal::Decimal;
use serde::Serialize;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use ledger::{Account, Diagnostic, Mutation};

use crate::state::{AppState, Snapshot};

/// Everything the Home, Check-in and Account screens need, assembled server-side.
#[derive(Serialize)]
pub struct LedgerView {
    pub version: u64,
    pub as_of: NaiveDate,
    pub base_currency: String,
    pub ledger_path: String,
    /// Set while the file on disk is broken; everything else is the last good state.
    pub problem: Option<Diagnostic>,
    pub totals: derive::Totals,
    pub series: Vec<derive::Point>,
    pub deltas: Deltas,
    pub accounts: Vec<AccountView>,
    pub fx: std::collections::BTreeMap<String, Decimal>,
    pub events: Vec<ledger::Event>,
}

#[derive(Serialize)]
pub struct Deltas {
    /// Since the previous recorded point.
    pub previous: Option<derive::Delta>,
    pub ytd: Option<derive::Delta>,
    pub one_year: Option<derive::Delta>,
}

#[derive(Serialize)]
pub struct AccountView {
    #[serde(flatten)]
    pub account: Account,
    pub group: ledger::Group,
    pub liability: bool,
    pub value: Decimal,
    /// Native-currency figure behind `value` (balance, or sum of positions).
    pub native_value: Decimal,
    pub as_of: Option<NaiveDate>,
    pub days_stale: Option<i64>,
    pub missing_fx: bool,
    pub missing_price: bool,
    pub history: Vec<derive::HistoryPoint>,
    pub holdings: Vec<HoldingView>,
}

#[derive(Serialize)]
pub struct HoldingView {
    pub symbol: String,
    pub units: Decimal,
    pub price: Option<Decimal>,
    pub price_date: Option<NaiveDate>,
    pub book_cost: Option<Decimal>,
}

pub fn build_view(snap: &Snapshot, path: &str, today: NaiveDate) -> LedgerView {
    let l = &snap.ledger;
    let accounts = l
        .accounts
        .iter()
        .map(|a| {
            let v = derive::value_account(l, a, today);
            let native_value = match &a.positions {
                Some(_) => derive::to_base(l, Decimal::ONE, &a.currency)
                    .filter(|r| !r.is_zero())
                    .map_or(v.value, |r| v.value / r),
                None => derive::balance_at(l, &a.id, today).map_or(Decimal::ZERO, |(_, b)| b),
            };
            let holdings = a
                .positions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|p| {
                    let px = derive::price_at(l, &p.symbol, today);
                    HoldingView {
                        symbol: p.symbol.clone(),
                        units: p.units,
                        price: px.map(|(_, x)| x),
                        price_date: px.map(|(d, _)| d),
                        book_cost: p.book_cost,
                    }
                })
                .collect();
            AccountView {
                account: a.clone(),
                group: a.kind.group(),
                liability: a.kind.is_liability(),
                value: v.value,
                native_value,
                as_of: v.as_of,
                days_stale: derive::days_stale(l, a, today),
                missing_fx: v.missing_fx,
                missing_price: v.missing_price,
                history: derive::account_history(l, a),
                holdings,
            }
        })
        .collect();
    let series = derive::net_worth_series(l);
    let previous = series
        .len()
        .checked_sub(2)
        .and_then(|i| series.get(i))
        .and_then(|p| derive::delta_since(&series, p.date));
    let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);
    let year_ago = today.with_year(today.year() - 1).unwrap_or_else(|| today - chrono::Days::new(365));
    LedgerView {
        version: snap.version,
        as_of: today,
        base_currency: l.settings.base_currency.clone(),
        ledger_path: path.to_string(),
        problem: snap.problem.clone(),
        totals: derive::totals(l, today),
        deltas: Deltas {
            previous,
            ytd: derive::delta_since(&series, jan1),
            one_year: derive::delta_since(&series, year_ago),
        },
        series,
        accounts,
        fx: l.settings.fx.clone(),
        events: l.events.clone(),
    }
}

async fn health() -> &'static str {
    "ok"
}

fn view_of(s: &AppState) -> LedgerView {
    build_view(&s.snapshot(), &s.ledger_path.display().to_string(), Local::now().date_naive())
}

async fn get_view(State(s): State<AppState>) -> Json<LedgerView> {
    Json(view_of(&s))
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
        Ok(snap) => {
            Ok(Json(build_view(&snap, &s.ledger_path.display().to_string(), Local::now().date_naive())))
        }
        Err(e) => {
            let d = ledger::Diagnostic::from(&e);
            let status = match e {
                ledger::LedgerError::FileHasProblem(_) => StatusCode::CONFLICT,
                ledger::LedgerError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
                _ => StatusCode::UNPROCESSABLE_ENTITY,
            };
            Err((status, Json(ApiError { message: d.message, line: d.line })))
        }
    }
}

/// Server-sent events: one `change` event with the new version whenever the
/// ledger changes (a save from the UI or a hand edit on disk).
async fn events(State(s): State<AppState>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let current = s.snapshot().version;
    let live = BroadcastStream::new(s.subscribe()).filter_map(|r| r.ok());
    let stream = tokio_stream::once(current)
        .chain(live)
        .map(|v| Ok(Event::default().event("change").data(v.to_string())));
    Sse::new(stream).keep_alive(KeepAlive::default())
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/view", get(get_view))
        .route("/api/apply", post(apply))
        .route("/api/events", get(events))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_view_has_deltas_and_history() {
        let l = ledger::io::parse(include_str!("../../data/ledger.example.yaml")).unwrap();
        let snap = Snapshot { ledger: l, problem: None, version: 1 };
        let v = build_view(&snap, "x", NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        assert_eq!(v.series.len(), 3);
        let prev = v.deltas.previous.unwrap();
        assert_eq!(prev.from, NaiveDate::from_ymd_opt(2026, 10, 15).unwrap());
        assert!(v.deltas.one_year.is_none());
        let tfsa = v.accounts.iter().find(|a| a.account.id == "ws-tfsa").unwrap();
        assert_eq!(tfsa.holdings.len(), 1);
        assert_eq!(tfsa.holdings[0].price, Some(rust_decimal_macros::dec!(34.12)));
        assert_eq!(tfsa.native_value, tfsa.value);
        let condo = v.accounts.iter().find(|a| a.account.id == "condo").unwrap();
        assert_eq!(condo.history.len(), 3);
        assert_eq!(condo.days_stale, Some(32));
    }
}
