use std::collections::BTreeMap;
use std::convert::Infallible;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Datelike, Local, NaiveDate, Utc};
use futures_util::stream::Stream;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use ledger::{Account, Diagnostic, Mutation};

use crate::state::{AppState, Snapshot};

/// Everything the Home, Check-in, Account and Portfolio screens need, assembled server-side.
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
    /// Rates typed in the ledger (settings.fx): the fallback, not necessarily what is in use.
    /// `market.rates` has the rate actually applied per currency.
    pub manual_fx: BTreeMap<String, Decimal>,
    pub events: Vec<ledger::Event>,
    pub market: MarketView,
    pub portfolio: derive::Portfolio,
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
    pub price_source: Option<derive::Source>,
    /// units x price, account currency.
    pub native_value: Option<Decimal>,
    /// Base currency.
    pub value: Option<Decimal>,
    pub book_cost: Option<Decimal>,
}

/// Where the numbers on screen came from and how fresh they are.
#[derive(Serialize)]
pub struct MarketView {
    pub price_provider: String,
    pub fx_provider: String,
    pub offline: bool,
    pub fetched_at: Option<DateTime<Utc>>,
    /// Latest date any fetched price is for.
    pub prices_as_of: Option<NaiveDate>,
    /// Days between `as_of` and `prices_as_of`; `None` when nothing was fetched.
    pub prices_age_days: Option<i64>,
    /// One per currency in use other than base.
    pub rates: Vec<RateView>,
    /// symbol or currency -> what went wrong last refresh.
    pub errors: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct RateView {
    pub currency: String,
    pub rate: Option<Decimal>,
    pub date: Option<NaiveDate>,
    pub source: Option<derive::Source>,
}

pub fn build_view(snap: &Snapshot, path: &str, today: NaiveDate, offline: bool) -> LedgerView {
    let l = &snap.ledger;
    let m = snap.market();
    let base = &l.settings.base_currency;
    let accounts = l
        .accounts
        .iter()
        .map(|a| {
            let v = derive::value_account(l, &m, a, today);
            let rate = m.rate_at(base, &a.currency, today).map(|r| r.rate);
            let native_value = match &a.positions {
                Some(_) => derive::positions_native(l, &m, a, today).0,
                None => derive::balance_at(l, &a.id, today).map_or(Decimal::ZERO, |(_, b)| b),
            };
            let holdings = a
                .positions
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(|p| {
                    let px = m.price_at(&p.symbol, today);
                    let native = px.map(|x| p.units * x.price);
                    HoldingView {
                        symbol: p.symbol.clone(),
                        units: p.units,
                        price: px.map(|x| x.price),
                        price_date: px.map(|x| x.date),
                        price_source: px.map(|x| x.source),
                        native_value: native,
                        value: match (native, rate) {
                            (Some(n), Some(r)) => Some(n * r),
                            _ => None,
                        },
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
                days_stale: derive::days_stale(l, &m, a, today),
                missing_fx: v.missing_fx,
                missing_price: v.missing_price,
                history: derive::account_history(l, &m, a),
                holdings,
            }
        })
        .collect();
    let series = derive::net_worth_series(l, &m);
    let previous = series
        .len()
        .checked_sub(2)
        .and_then(|i| series.get(i))
        .and_then(|p| derive::delta_since(&series, p.date));
    let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);
    let year_ago = today.with_year(today.year() - 1).unwrap_or_else(|| today - chrono::Days::new(365));
    let prices_as_of = snap.quotes.latest_price_date();
    let market = MarketView {
        price_provider: format!("{:?}", l.settings.providers.prices).to_lowercase(),
        fx_provider: format!("{:?}", l.settings.providers.fx)
            .to_lowercase()
            .replace("bankofcanada", "bank_of_canada"),
        offline,
        fetched_at: snap.quotes.fetched_at,
        prices_as_of,
        prices_age_days: prices_as_of.map(|d| (today - d).num_days()),
        rates: l
            .foreign_currencies()
            .into_iter()
            .map(|c| {
                let r = m.rate_at(base, &c, today);
                RateView {
                    currency: c,
                    rate: r.map(|r| r.rate),
                    date: r.and_then(|r| r.date),
                    source: r.map(|r| r.source),
                }
            })
            .collect(),
        errors: snap.quotes.errors.clone(),
    };
    LedgerView {
        version: snap.version,
        as_of: today,
        base_currency: base.clone(),
        ledger_path: path.to_string(),
        problem: snap.problem.clone(),
        totals: derive::totals(l, &m, today),
        deltas: Deltas {
            previous,
            ytd: derive::delta_since(&series, jan1),
            one_year: derive::delta_since(&series, year_ago),
        },
        series,
        accounts,
        manual_fx: l.settings.fx.clone(),
        events: l.events.clone(),
        market,
        portfolio: derive::portfolio(l, &m, today),
    }
}

async fn health() -> &'static str {
    "ok"
}

fn view_of(s: &AppState) -> LedgerView {
    build_view(&s.snapshot(), &s.ledger_path.display().to_string(), Local::now().date_naive(), s.offline)
}

async fn get_view(State(s): State<AppState>) -> Json<LedgerView> {
    Json(view_of(&s))
}

#[derive(Serialize)]
struct ApiError {
    message: String,
    line: Option<usize>,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

fn ledger_error(e: ledger::LedgerError) -> (StatusCode, Json<ApiError>) {
    let d = ledger::Diagnostic::from(&e);
    let status = match e {
        ledger::LedgerError::FileHasProblem(_) => StatusCode::CONFLICT,
        ledger::LedgerError::Io(_) => StatusCode::INTERNAL_SERVER_ERROR,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    };
    (status, Json(ApiError { message: d.message, line: d.line }))
}

async fn apply(State(s): State<AppState>, Json(ms): Json<Vec<Mutation>>) -> ApiResult<LedgerView> {
    match s.apply_many(ms) {
        Ok(snap) => Ok(Json(build_view(
            &snap,
            &s.ledger_path.display().to_string(),
            Local::now().date_naive(),
            s.offline,
        ))),
        Err(e) => Err(ledger_error(e)),
    }
}

/// Fetch prices and FX now. Always answers with the view; per-symbol problems are
/// in `market.errors`, never an HTTP error, so the UI keeps the last good numbers.
async fn refresh(State(s): State<AppState>) -> Json<LedgerView> {
    let st = s.clone();
    if let Err(e) = tokio::task::spawn_blocking(move || st.refresh()).await {
        tracing::warn!("refresh task failed: {e}");
    }
    Json(view_of(&s))
}

#[derive(Deserialize)]
pub struct ParseHoldingsRequest {
    pub account: String,
    pub date: NaiveDate,
    pub text: String,
}

#[derive(Serialize)]
pub struct ParseHoldingsResponse {
    #[serde(flatten)]
    pub plan: importers::Plan,
    pub parsed_lines: usize,
    pub total_lines: usize,
}

/// Parse a pasted broker export against one account and propose mutations. Pure:
/// nothing is written; the UI sends `mutations` to `/api/apply` when the user confirms.
async fn parse_holdings(
    State(s): State<AppState>,
    Json(req): Json<ParseHoldingsRequest>,
) -> ApiResult<ParseHoldingsResponse> {
    let snap = s.snapshot();
    let Some(account) = snap.ledger.account(&req.account) else {
        return Err(ledger_error(ledger::LedgerError::UnknownAccount(req.account)));
    };
    let parsed = importers::parse_holdings(&req.text);
    let total_lines = req.text.lines().filter(|l| !l.trim().is_empty()).count();
    let plan = importers::reconcile(account, &parsed, req.date);
    Ok(Json(ParseHoldingsResponse { parsed_lines: parsed.rows.len(), total_lines, plan }))
}

/// Server-sent events: one `change` event with the new version whenever the
/// ledger changes (a save from the UI, a hand edit on disk, or fetched quotes).
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
        .route("/api/refresh", post(refresh))
        .route("/api/parse-holdings", post(parse_holdings))
        .route("/api/events", get(events))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use providers::QuoteCache;
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    #[test]
    fn example_view_has_deltas_history_and_portfolio() {
        let l = ledger::io::parse(include_str!("../../data/ledger.example.yaml")).unwrap();
        let snap = Snapshot { ledger: l, problem: None, quotes: QuoteCache::default(), version: 1 };
        let v = build_view(&snap, "x", ymd(2026, 11, 2), true);
        assert_eq!(v.series.len(), 3);
        let prev = v.deltas.previous.unwrap();
        assert_eq!(prev.from, ymd(2026, 10, 15));
        assert!(v.deltas.one_year.is_none());
        let tfsa = v.accounts.iter().find(|a| a.account.id == "ws-tfsa").unwrap();
        assert_eq!(tfsa.holdings.len(), 1);
        assert_eq!(tfsa.holdings[0].price, Some(dec!(34.12)));
        assert_eq!(tfsa.holdings[0].price_source, Some(derive::Source::Manual));
        assert_eq!(tfsa.native_value, tfsa.value);
        let condo = v.accounts.iter().find(|a| a.account.id == "condo").unwrap();
        assert_eq!(condo.history.len(), 3);
        assert_eq!(condo.days_stale, Some(32));
        assert_eq!(v.portfolio.securities.len(), 1);
        assert_eq!(v.portfolio.total, dec!(412.5) * dec!(34.12));
        assert_eq!(v.market.prices_as_of, None);
        assert_eq!(v.market.price_provider, "yahoo");
        assert_eq!(v.market.fx_provider, "bank_of_canada");
        assert!(v.market.offline);
        assert!(v.market.rates.is_empty()); // no account in USD, so nothing to apply
        assert_eq!(v.manual_fx["USD"], dec!(1.38));
    }

    #[test]
    fn fetched_quotes_show_as_of_and_rates() {
        let l = ledger::io::parse(include_str!("../../data/ledger.example.yaml")).unwrap();
        let mut quotes = QuoteCache::default();
        quotes.prices.insert(
            "XEQT.TO".into(),
            BTreeMap::from([(ymd(2026, 10, 30), dec!(34.00)), (ymd(2026, 11, 1), dec!(34.50))]),
        );
        quotes.fx.insert("USD".into(), BTreeMap::from([(ymd(2026, 11, 1), dec!(1.39))]));
        let mut l = l;
        l.accounts.push(Account {
            id: "usd".into(),
            name: "USD".into(),
            institution: None,
            kind: ledger::AccountKind::Chequing,
            currency: "USD".into(),
            tags: vec![],
            positions: None,
        });
        let snap = Snapshot { ledger: l, problem: None, quotes, version: 1 };
        let v = build_view(&snap, "x", ymd(2026, 11, 2), false);
        assert_eq!(v.market.prices_as_of, Some(ymd(2026, 11, 1)));
        assert_eq!(v.market.prices_age_days, Some(1));
        assert_eq!(v.market.rates[0].rate, Some(dec!(1.39)));
        assert_eq!(v.market.rates[0].source, Some(derive::Source::Fetched));
        let tfsa = v.accounts.iter().find(|a| a.account.id == "ws-tfsa").unwrap();
        assert_eq!(tfsa.holdings[0].price_source, Some(derive::Source::Fetched));
        // Day change is fetched Nov 1 vs fetched Oct 30; the typed Oct 31 price is not "yesterday".
        assert_eq!(v.portfolio.securities[0].day_change, Some(dec!(412.5) * dec!(0.50)));
    }
}
