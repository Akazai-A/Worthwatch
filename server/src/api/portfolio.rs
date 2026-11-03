//! Portfolio screens: look-through, exposure and overlap (`GET /api/lookthrough`),
//! Changes (`GET /api/changes`), growth and per-account months.

use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Local, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::state::{AppState, Snapshot};

use super::{bad, ApiResult};

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

pub async fn get_changes(State(s): State<AppState>, Query(q): Query<ChangesQuery>) -> ApiResult<ChangesView> {
    // Old holdings files are read from disk; keep that off the async runtime.
    let snap = s.snapshot();
    let today = Local::now().date_naive();
    match tokio::task::spawn_blocking(move || build_changes(&snap, &q, today)).await {
        Ok(Ok(v)) => Ok(Json(v)),
        Ok(Err(message)) => Err(bad(StatusCode::UNPROCESSABLE_ENTITY, message)),
        Err(e) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
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
pub async fn get_growth(State(s): State<AppState>) -> Json<Vec<derive::GrowthPoint>> {
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
pub async fn get_account_months(
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

pub async fn get_lookthrough(State(s): State<AppState>) -> Json<LookThroughView> {
    Json(lookthrough_of(&s))
}

/// Re-read `holdings/` and fetch issuer files for every held fund now (forced, so a
/// fresh file is fetched too). Always answers with the view; failures are in `errors`.
pub async fn refresh_holdings(State(s): State<AppState>) -> Json<LookThroughView> {
    let st = s.clone();
    if let Err(e) = tokio::task::spawn_blocking(move || st.refresh_holdings(true)).await {
        tracing::warn!("holdings refresh task failed: {e}");
    }
    Json(lookthrough_of(&s))
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
    fn changes_default_to_the_last_checkin_and_diff_fund_files_on_disk() {
        let dir = tempfile::tempdir().unwrap();
        let today = ymd(2026, 11, 2);
        let l = crate::seed::seed_ledger(today);
        let quotes = crate::seed::seed_quotes(&l, today);
        let mut store = providers::HoldingsStore::load(&dir.path().join("holdings"));
        // Two files per fund a month apart; the later one moves one sleeve of XEQT.
        for f in crate::seed::funds::seed_holdings(ymd(2026, 9, 30)) {
            store.save(f).unwrap();
        }
        for mut f in crate::seed::funds::seed_holdings(ymd(2026, 10, 30)) {
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
}
