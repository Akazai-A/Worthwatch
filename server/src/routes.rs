use std::collections::BTreeMap;
use std::convert::Infallible;

use axum::extract::{Path, Query, State};
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
    /// How the chat panel is set up (settings.assistant).
    pub assistant: crate::chat::AssistantView,
    /// Proposals from MCP waiting for Approve or Discard, oldest first.
    pub pending: Vec<crate::chat::Pending>,
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
        assistant: crate::chat::assistant_view(l, offline),
        pending: snap.pending.clone(),
    }
}

/// Everything the Portfolio › Look-through, Exposure and Overlap tabs need. Its own
/// view because the company list is big; fetched when those tabs open.
#[derive(Serialize)]
pub struct LookThroughView {
    pub version: u64,
    pub as_of: NaiveDate,
    pub base_currency: String,
    /// `data/holdings`, where a downloaded issuer file can be dropped.
    pub holdings_dir: String,
    pub offline: bool,
    pub fetched_at: Option<DateTime<Utc>>,
    /// Positions as valued on the Portfolio screen (base currency), in the same order
    /// as `overlap.symbols`.
    pub holdings: Vec<HoldingInput>,
    /// Resolved through funds, up to three levels; companies capped to the top 300.
    pub lookthrough: holdings::LookThrough,
    /// Every position as its own leaf.
    pub as_held: holdings::LookThrough,
    pub overlap: holdings::Overlap,
    /// symbol or file -> what went wrong last refresh or load.
    pub errors: BTreeMap<String, String>,
    pub snapshots: Vec<SnapshotInfo>,
}

#[derive(Serialize)]
pub struct HoldingInput {
    pub symbol: String,
    pub value: Decimal,
    pub currency: Option<String>,
}

#[derive(Serialize)]
pub struct SnapshotInfo {
    pub symbol: String,
    pub name: Option<String>,
    pub as_of: NaiveDate,
    pub source: String,
    pub constituents: usize,
    /// How many dated snapshots are on disk for it.
    pub dates: usize,
}

const COMPANY_CAP: usize = 300;
const DEPTH: u8 = 3;

pub fn build_lookthrough(
    snap: &Snapshot,
    holdings_dir: &str,
    today: NaiveDate,
    offline: bool,
) -> LookThroughView {
    let l = &snap.ledger;
    let m = snap.market();
    let p = derive::portfolio(l, &m, today);
    let inputs: Vec<holdings::Holding> = p
        .securities
        .iter()
        .filter_map(|r| {
            Some(holdings::Holding {
                symbol: r.symbol.clone(),
                name: None,
                value: r.value?,
                currency: snap.quotes.currencies.get(&r.symbol).cloned().or_else(|| Some(r.currency.clone())),
            })
        })
        .collect();
    let meta = security_meta(l);
    let snaps = &snap.holdings.latest;
    LookThroughView {
        version: snap.version,
        as_of: today,
        base_currency: l.settings.base_currency.clone(),
        holdings_dir: holdings_dir.to_string(),
        offline,
        fetched_at: snap.holdings.fetched_at,
        holdings: inputs
            .iter()
            .map(|h| HoldingInput { symbol: h.symbol.clone(), value: h.value, currency: h.currency.clone() })
            .collect(),
        lookthrough: holdings::lookthrough(&inputs, snaps, &meta, DEPTH).cap(COMPANY_CAP),
        as_held: holdings::lookthrough(&inputs, snaps, &meta, 0),
        overlap: holdings::overlap(&inputs, snaps, DEPTH, 12),
        errors: snap.holdings.errors.clone(),
        snapshots: snap
            .holdings
            .latest
            .values()
            .map(|h| SnapshotInfo {
                symbol: h.symbol.clone(),
                name: h.name.clone(),
                as_of: h.as_of,
                source: h.source.clone(),
                constituents: h.rows.len(),
                dates: snap.holdings.dates.get(&h.symbol).map_or(1, Vec::len),
            })
            .collect(),
    }
}

/// Everything the Changes screen needs for one period: `GET /api/changes?from&to&threshold`.
#[derive(Serialize)]
pub struct ChangesView {
    pub version: u64,
    pub as_of: NaiveDate,
    pub base_currency: String,
    pub from: NaiveDate,
    pub to: NaiveDate,
    /// Dates a check-in was recorded on, oldest first: the period picker's stops.
    pub checkins: Vec<NaiveDate>,
    /// Earliest date anything is known, if any.
    pub first_date: Option<NaiveDate>,
    /// Percentage points a constituent must move to make the feed.
    pub threshold: Decimal,
    pub changes: derive::Changes,
    /// Through funds (depth 3) and as held (depth 0), both ends valued on their own date.
    pub drift: holdings::Drift,
    pub as_held_drift: holdings::Drift,
    /// Funds that resolved at either end: how many had a file on `from`, on `to`.
    pub funds_resolved_from: usize,
    pub funds_resolved_to: usize,
    /// One per fund reached at `to` whose file changed inside the period, biggest first.
    pub funds: Vec<FundFeed>,
    /// Funds whose file on `to` is the same one that was in effect on `from`.
    pub same_file: Vec<String>,
    /// Funds with a file on `to` but none old enough for `from`.
    pub no_earlier_file: Vec<String>,
    pub errors: BTreeMap<String, String>,
}

#[derive(Serialize)]
pub struct FundFeed {
    #[serde(flatten)]
    pub diff: holdings::FundDiff,
    /// Held directly (else reached inside another fund).
    pub held: bool,
    /// Changes above the threshold left out of `changes` to keep the payload small.
    pub hidden: usize,
}

#[derive(Deserialize, Default)]
pub struct ChangesQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub threshold: Option<Decimal>,
}

const FEED_CAP: usize = 20;
pub const DEFAULT_THRESHOLD: Decimal = Decimal::from_parts(5, 0, 0, false, 1);

fn holdings_inputs(snap: &Snapshot, m: &derive::Market, date: NaiveDate) -> Vec<holdings::Holding> {
    derive::portfolio(&snap.ledger, m, date)
        .securities
        .iter()
        .filter_map(|r| {
            Some(holdings::Holding {
                symbol: r.symbol.clone(),
                name: None,
                value: r.value?,
                currency: snap.quotes.currencies.get(&r.symbol).cloned().or_else(|| Some(r.currency.clone())),
            })
        })
        .collect()
}

fn security_meta(l: &ledger::Ledger) -> BTreeMap<String, holdings::SecurityMeta> {
    l.securities
        .iter()
        .map(|(sym, sec)| {
            (
                sym.clone(),
                holdings::SecurityMeta {
                    name: sec.name.clone(),
                    sector: sec.sector.clone(),
                    country: sec.country.clone(),
                    asset_class: sec.asset_class.as_deref().map(holdings::AssetClass::parse),
                    currency: sec.currency.clone(),
                },
            )
        })
        .collect()
}

/// Default start of the period: the last check-in before `to`, else the first date
/// anything is known, else `to` itself.
pub fn default_from(l: &ledger::Ledger, m: &derive::Market, to: NaiveDate) -> NaiveDate {
    l.snapshots
        .range(..to)
        .next_back()
        .map(|(d, _)| *d)
        .or_else(|| derive::change_dates(l, m).into_iter().find(|d| *d < to))
        .unwrap_or(to)
}

pub fn build_changes(snap: &Snapshot, q: &ChangesQuery, today: NaiveDate) -> Result<ChangesView, String> {
    let l = &snap.ledger;
    let m = snap.market();
    let to = q.to.unwrap_or(today);
    let from = q.from.unwrap_or_else(|| default_from(l, &m, to));
    if from > to {
        return Err(format!("from ({from}) is after to ({to})"));
    }
    let threshold = q.threshold.unwrap_or(DEFAULT_THRESHOLD);
    if threshold < Decimal::ZERO {
        return Err("threshold must be zero or more".into());
    }
    let meta = security_meta(l);
    let inputs_from = holdings_inputs(snap, &m, from);
    let inputs_to = holdings_inputs(snap, &m, to);
    let snaps_from = snap.holdings.snapshots_at(from);
    let snaps_to = snap.holdings.snapshots_at(to);
    let lt_from = holdings::lookthrough(&inputs_from, &snaps_from, &meta, DEPTH);
    let lt_to = holdings::lookthrough(&inputs_to, &snaps_to, &meta, DEPTH);
    let drift = holdings::drift(&lt_from, &lt_to);
    // As held never expands a fund; the newest files only tell it which symbols are
    // funds, so both ends are classified the same way.
    let as_held_drift = holdings::drift(
        &holdings::lookthrough(&inputs_from, &snaps_to, &meta, 0),
        &holdings::lookthrough(&inputs_to, &snaps_to, &meta, 0),
    );
    let dollars = holdings::fund_dollars(&inputs_to, &snaps_to, DEPTH);
    let held: Vec<&str> = inputs_to.iter().map(|h| h.symbol.as_str()).collect();
    let mut funds = Vec::new();
    let mut same_file = Vec::new();
    let mut no_earlier_file = Vec::new();
    for (symbol, value) in &dollars {
        let Some(file_to) = snaps_to.get(symbol) else { continue };
        match snap.holdings.at(symbol, from) {
            None => no_earlier_file.push(symbol.clone()),
            Some(file_from) if file_from.as_of == file_to.as_of => same_file.push(symbol.clone()),
            Some(file_from) => {
                let mut diff = holdings::diff_fund(&file_from, file_to, threshold, Some(*value));
                let hidden = diff.changes.len().saturating_sub(FEED_CAP);
                diff.changes.truncate(FEED_CAP);
                funds.push(FundFeed { diff, held: held.contains(&symbol.as_str()), hidden });
            }
        }
    }
    // Biggest move in your dollars first; funds with nothing above the threshold last.
    funds.sort_by(|a, b| {
        let key = |f: &FundFeed| {
            f.diff.changes.first().and_then(|c| c.your_value).map_or(Decimal::ZERO, |v| v.abs())
        };
        key(b)
            .cmp(&key(a))
            .then(b.diff.turnover_pct.cmp(&a.diff.turnover_pct))
            .then(a.diff.symbol.cmp(&b.diff.symbol))
    });
    let dates = derive::change_dates(l, &m);
    Ok(ChangesView {
        version: snap.version,
        as_of: today,
        base_currency: l.settings.base_currency.clone(),
        from,
        to,
        checkins: l.snapshots.keys().copied().collect(),
        first_date: dates.first().copied(),
        threshold,
        changes: derive::changes(l, &m, from, to),
        funds_resolved_from: lt_from.funds.iter().filter(|f| f.resolved).count(),
        funds_resolved_to: lt_to.funds.iter().filter(|f| f.resolved).count(),
        drift,
        as_held_drift,
        funds,
        same_file,
        no_earlier_file,
        errors: snap.holdings.errors.clone(),
    })
}

async fn get_changes(State(s): State<AppState>, Query(q): Query<ChangesQuery>) -> ApiResult<ChangesView> {
    // Old holdings files are read from disk; keep that off the async runtime.
    let snap = s.snapshot();
    let today = Local::now().date_naive();
    match tokio::task::spawn_blocking(move || build_changes(&snap, &q, today)).await {
        Ok(Ok(v)) => Ok(Json(v)),
        Ok(Err(message)) => Err((StatusCode::UNPROCESSABLE_ENTITY, Json(ApiError { message, line: None }))),
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { message: e.to_string(), line: None })))
        }
    }
}

fn lookthrough_of(s: &AppState) -> LookThroughView {
    build_lookthrough(
        &s.snapshot(),
        &s.holdings_dir.display().to_string(),
        Local::now().date_naive(),
        s.offline,
    )
}

/// Added vs grown for every point of the net worth series: `GET /api/growth`.
async fn get_growth(State(s): State<AppState>) -> Json<Vec<derive::GrowthPoint>> {
    let snap = s.snapshot();
    let points = tokio::task::spawn_blocking(move || {
        let m = snap.market();
        derive::growth(&snap.ledger, &m, &derive::net_worth_series(&snap.ledger, &m))
    })
    .await
    .unwrap_or_default();
    Json(points)
}

/// One account month by month, added vs grown: `GET /api/accounts/{id}/months`.
async fn get_account_months(
    State(s): State<AppState>,
    Path(id): Path<String>,
) -> Json<Vec<derive::MonthSplit>> {
    let snap = s.snapshot();
    let months = tokio::task::spawn_blocking(move || {
        let m = snap.market();
        derive::account_months(&snap.ledger, &m, &derive::net_worth_series(&snap.ledger, &m), &id)
    })
    .await
    .unwrap_or_default();
    Json(months)
}

async fn get_lookthrough(State(s): State<AppState>) -> Json<LookThroughView> {
    Json(lookthrough_of(&s))
}

/// Re-read `holdings/` and fetch issuer files for every held fund now (forced, so a
/// fresh file is fetched too). Always answers with the view; failures are in `errors`.
async fn refresh_holdings(State(s): State<AppState>) -> Json<LookThroughView> {
    let st = s.clone();
    if let Err(e) = tokio::task::spawn_blocking(move || st.refresh_holdings(true)).await {
        tracing::warn!("holdings refresh task failed: {e}");
    }
    Json(lookthrough_of(&s))
}

/// The Plan screen: projection under the ledger's assumptions (or the slider's), contribution
/// room and the recurring rules behind both.
#[derive(Serialize)]
pub struct PlanView {
    pub projection: derive::projection::Projection,
    pub room: Vec<derive::RoomView>,
    pub recurring: Vec<RecurringView>,
    /// The ledger's own assumptions, so the screen can show where the slider started.
    pub settings: ledger::Projection,
}

#[derive(Serialize)]
pub struct RecurringView {
    #[serde(flatten)]
    pub rule: ledger::Recurring,
    pub name: String,
    pub liability: bool,
    /// Monthly equivalent in base currency.
    pub monthly: Option<Decimal>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlanQuery {
    pub years: Option<u32>,
    pub return_pct: Option<Decimal>,
    pub extra: Option<Decimal>,
    pub target: Option<Decimal>,
}

pub fn build_plan(snap: &Snapshot, q: &PlanQuery, today: NaiveDate) -> PlanView {
    let l = &snap.ledger;
    let m = snap.market();
    let mut a = derive::Assumptions::from_ledger(l);
    a.years = q.years.unwrap_or(30).clamp(1, 50);
    if let Some(r) = q.return_pct {
        a.return_pct = r.clamp(Decimal::from(-20), Decimal::from(30));
    }
    if let Some(x) = q.extra {
        a.extra_monthly = x.max(Decimal::ZERO);
    }
    if q.target.is_some() {
        a.target = q.target.filter(|t| *t > Decimal::ZERO);
    }
    let recurring = l
        .recurring
        .iter()
        .map(|r| {
            let acct = l.account(&r.account);
            let per_month = r.amount * Decimal::from(r.every.per_year()) / Decimal::from(12);
            RecurringView {
                rule: r.clone(),
                name: acct.map_or_else(|| r.account.clone(), |a| a.name.clone()),
                liability: acct.is_some_and(|a| a.kind.is_liability()),
                monthly: acct
                    .and_then(|a| derive::to_base(l, &m, per_month, &a.currency, today))
                    .map(|v| v.round_dp(2)),
            }
        })
        .collect();
    PlanView {
        projection: derive::project(l, &m, today, &a),
        room: derive::room(l, &m, today),
        recurring,
        settings: l.settings.projection.clone(),
    }
}

async fn get_plan(State(s): State<AppState>, Query(q): Query<PlanQuery>) -> Json<PlanView> {
    Json(build_plan(&s.snapshot(), &q, Local::now().date_naive()))
}

async fn list_backups(State(s): State<AppState>) -> Json<Vec<crate::backup::BackupInfo>> {
    Json(crate::backup::list(&s.data_dir()))
}

async fn create_backup(State(s): State<AppState>) -> ApiResult<Vec<crate::backup::BackupInfo>> {
    let dir = s.data_dir();
    let made = tokio::task::spawn_blocking(move || crate::backup::create(&dir, Local::now())).await;
    match made {
        Ok(Ok(_)) => Ok(Json(crate::backup::list(&s.data_dir()))),
        Ok(Err(e)) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { message: e.to_string(), line: None })))
        }
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { message: e.to_string(), line: None })))
        }
    }
}

async fn download_backup(State(s): State<AppState>, Path(name): Path<String>) -> axum::response::Response {
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

/// Turn a message into a proposal. Writes nothing: the UI sends `plan.mutations` to
/// `/api/apply` when the user approves. With a cloud model and `send: false`, nothing
/// leaves the machine either; the reply carries the exact prompt as a preview.
async fn chat(
    State(s): State<AppState>,
    Json(req): Json<crate::chat::ChatRequest>,
) -> ApiResult<crate::chat::ChatReply> {
    let snap = s.snapshot();
    let offline = s.offline;
    let today = Local::now().date_naive();
    match tokio::task::spawn_blocking(move || crate::chat::chat(&snap, offline, &req, today)).await {
        Ok(r) => Ok(Json(r)),
        Err(e) => {
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ApiError { message: e.to_string(), line: None })))
        }
    }
}

/// What MCP clients read: the assistant's context (accounts with values, headline
/// figures). Same numbers as the dashboard, assembled server-side.
async fn get_context(State(s): State<AppState>) -> Json<assistant::Context> {
    Json(crate::chat::context(&s.snapshot(), Local::now().date_naive()))
}

#[derive(Deserialize)]
pub struct ProposeRequest {
    #[serde(default = "default_origin")]
    pub origin: String,
    /// What the user said, for the card.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub proposal: Option<assistant::Proposal>,
}

fn default_origin() -> String {
    "mcp".into()
}

#[derive(Serialize)]
pub struct ProposeReply {
    /// Set when the plan has something to approve and was queued.
    pub pending: Option<crate::chat::Pending>,
    pub plan: assistant::Plan,
}

/// Queue a proposal for the user (the MCP `propose_update` path). A structured
/// proposal is resolved as given; text alone goes through the shorthand parser. The
/// plan is returned either way so the caller sees rejections at once.
async fn propose(State(s): State<AppState>, Json(req): Json<ProposeRequest>) -> Json<ProposeReply> {
    let snap = s.snapshot();
    let today = Local::now().date_naive();
    let ctx = crate::chat::context(&snap, today);
    let p = req.proposal.unwrap_or_else(|| assistant::shorthand::parse(&req.text, &ctx));
    let plan = assistant::resolve(&snap.ledger, &ctx, &p);
    let pending = (!plan.mutations.is_empty()).then(|| s.propose(&req.origin, &req.text, plan.clone()));
    Json(ProposeReply { pending, plan })
}

#[derive(Serialize)]
pub struct PendingStatus {
    pub id: u64,
    /// pending | approved | discarded | expired | unknown
    pub status: String,
}

async fn pending_status(State(s): State<AppState>, Path(id): Path<u64>) -> Json<PendingStatus> {
    let snap = s.snapshot();
    let status = if snap.pending.iter().any(|p| p.id == id) {
        "pending".to_string()
    } else {
        snap.decided.get(&id).cloned().unwrap_or_else(|| "unknown".into())
    };
    Json(PendingStatus { id, status })
}

async fn decide(State(s): State<AppState>, Path((id, action)): Path<(u64, String)>) -> ApiResult<LedgerView> {
    let approve = match action.as_str() {
        "approve" => true,
        "discard" => false,
        _ => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ApiError { message: format!("unknown action {action}"), line: None }),
            ))
        }
    };
    match s.decide(id, approve) {
        Ok(Some(_)) => Ok(Json(view_of(&s))),
        Ok(None) => Err((
            StatusCode::NOT_FOUND,
            Json(ApiError { message: format!("no pending proposal {id}"), line: None }),
        )),
        Err(e) => Err(ledger_error(e)),
    }
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
        .route("/api/lookthrough", get(get_lookthrough))
        .route("/api/changes", get(get_changes))
        .route("/api/growth", get(get_growth))
        .route("/api/accounts/{id}/months", get(get_account_months))
        .route("/api/refresh-holdings", post(refresh_holdings))
        .route("/api/events", get(events))
        .route("/api/plan", get(get_plan))
        .route("/api/backups", get(list_backups).post(create_backup))
        .route("/api/backups/{name}", get(download_backup))
        .route("/api/chat", post(chat))
        .route("/api/context", get(get_context))
        .route("/api/propose", post(propose))
        .route("/api/pending/{id}", get(pending_status))
        .route("/api/pending/{id}/{action}", post(decide))
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
        let snap = Snapshot {
            ledger: l,
            problem: None,
            quotes: QuoteCache::default(),
            holdings: Default::default(),
            version: 1,
            pending: vec![],
            decided: Default::default(),
            next_id: 1,
        };
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
    fn changes_default_to_the_last_checkin_and_diff_fund_files_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let today = ymd(2026, 11, 2);
        let l = crate::seed::seed_ledger(today);
        let quotes = crate::seed::seed_quotes(&l, today);
        let mut store = providers::HoldingsStore::load(&dir.path().join("holdings"));
        // Two files per fund a month apart; the later one moves one sleeve of XEQT.
        for f in crate::seed_holdings::seed_holdings(ymd(2026, 9, 30)) {
            store.save(f).unwrap();
        }
        for mut f in crate::seed_holdings::seed_holdings(ymd(2026, 10, 30)) {
            if f.symbol == "XEQT.TO" {
                for r in &mut f.rows {
                    match r.ticker.as_deref() {
                        Some("XEC") => r.weight += dec!(1),
                        Some("XEF") => r.weight -= dec!(1),
                        _ => {}
                    }
                }
            }
            store.save(f).unwrap();
        }
        let snap = Snapshot {
            ledger: l,
            problem: None,
            quotes,
            holdings: store,
            version: 3,
            pending: vec![],
            decided: Default::default(),
            next_id: 1,
        };
        let v = build_changes(&snap, &ChangesQuery::default(), today).unwrap();
        assert_eq!(v.to, today);
        assert_eq!(v.from, *snap.ledger.snapshots.keys().rev().nth(1).unwrap(), "last check-in before today");
        assert_eq!(v.changes.from, v.from);
        assert_eq!(v.threshold, dec!(0.5));
        assert!(v.changes.accounts.len() >= 9);
        assert!(!v.changes.holdings.is_empty());
        assert_eq!(v.checkins.len(), 15);
        assert_eq!(v.funds_resolved_to, 3);
        // The period (Oct 31 -> Nov 2) sees the same Oct 30 file at both ends: nothing to diff.
        assert!(v.funds.is_empty());
        assert!(v.same_file.contains(&"XEQT.TO".to_string()));

        let q = ChangesQuery { from: Some(ymd(2026, 10, 1)), to: Some(today), threshold: Some(dec!(0.5)) };
        let v = build_changes(&snap, &q, today).unwrap();
        // Every fund got a new file in the period; only XEQT's moved anything, so it leads.
        assert_eq!(v.funds.len(), 8);
        assert!(v.funds[1..].iter().all(|f| f.diff.changes.is_empty() && f.diff.turnover_pct.is_zero()));
        let xeqt = &v.funds[0];
        assert_eq!(xeqt.diff.symbol, "XEQT.TO");
        assert!(xeqt.held);
        assert_eq!(xeqt.diff.changes.len(), 2);
        assert_eq!(xeqt.diff.changes[0].delta.abs(), dec!(1));
        let your = xeqt.diff.changes[0].your_value.unwrap().abs();
        assert_eq!(your, (xeqt.diff.your_value.unwrap() / Decimal::ONE_HUNDRED).round_dp(2));
        assert!(v.same_file.is_empty());
        assert!(v.drift.by_sector.len() > 3);
        assert_eq!(v.drift.from_as_of, Some(ymd(2026, 9, 30)));
        assert_eq!(v.drift.to_as_of, Some(ymd(2026, 10, 30)));
        let sum: Decimal = v.drift.by_region.iter().map(|b| b.delta).sum();
        assert!(sum.abs() <= dec!(0.3), "rounded points net to about zero: {sum}");
        assert!(v.as_held_drift.by_currency.iter().any(|b| b.label == "USD"));

        // A threshold of 2 points hides both moves; a backwards range is refused.
        let q = ChangesQuery { from: Some(ymd(2026, 10, 1)), to: Some(today), threshold: Some(dec!(2)) };
        let v = build_changes(&snap, &q, today).unwrap();
        assert_eq!(v.funds[0].diff.changes.len(), 0);
        assert_eq!(v.funds[0].diff.minor_reweighted, 2);
        let q = ChangesQuery { from: Some(today), to: Some(ymd(2026, 10, 1)), threshold: None };
        assert!(build_changes(&snap, &q, today).is_err());

        // Empty ledger: a view with nothing in it, never an error.
        let empty = Snapshot {
            ledger: ledger::Ledger::starter("CAD"),
            problem: None,
            quotes: QuoteCache::default(),
            holdings: Default::default(),
            version: 1,
            pending: vec![],
            decided: Default::default(),
            next_id: 1,
        };
        let v = build_changes(&empty, &ChangesQuery::default(), today).unwrap();
        assert_eq!(v.from, today);
        assert!(v.changes.accounts.is_empty());
        assert_eq!(v.first_date, None);
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
        let snap = Snapshot {
            ledger: l,
            problem: None,
            quotes,
            holdings: Default::default(),
            version: 1,
            pending: vec![],
            decided: Default::default(),
            next_id: 1,
        };
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
