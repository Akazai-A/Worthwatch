//! One fund or company (`GET /api/security`): its price history and what you have had in
//! it at each check-in. Everything else on that page (what's inside, overlap, lineage)
//! the UI already has from the view and the look-through.

use std::collections::BTreeMap;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{Duration, Local, NaiveDate, Utc};
use providers::{PriceHistory, Span};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::portfolio::{holdings_inputs, security_meta, DEPTH};
use super::{bad, ApiResult};
use crate::state::{providers_for, AppState, Snapshot};

#[derive(Deserialize)]
pub struct SecurityQuery {
    /// The quote symbol: `VTI`, `XEQT.TO`, `NVDA`, `RY.TO`.
    pub symbol: String,
    /// A company's look-through key, when the page is for a company inside your funds.
    pub key: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Close {
    pub date: NaiveDate,
    pub close: Decimal,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Yours {
    pub date: NaiveDate,
    /// Your dollars in it that day, in the base currency, through every fund.
    pub value: Decimal,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct SecurityView {
    pub symbol: String,
    pub base_currency: String,
    /// The currency `prices` are quoted in.
    pub currency: Option<String>,
    /// Daily closes, about a year. Empty when nothing is cached and fetching is off.
    pub prices: Vec<Close>,
    /// `held` (refreshed with your prices), `fetched` (looked up for this page), or `none`.
    pub price_source: &'static str,
    /// Why there is no chart, in words, when `prices` is empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price_note: Option<String>,
    /// One point per check-in, then today.
    pub yours: Vec<Yours>,
    /// Points dated before this (the first holdings file) use that file's weights.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_before: Option<NaiveDate>,
}

/// Lookups older than this are fetched again.
const LOOKUP_TTL_HOURS: i64 = 12;

/// A quote symbol a provider URL can carry: letters, digits and `.-^=`, at most 20.
fn valid_symbol(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_alphanumeric() || ".-^=".contains(c))
}

/// Your dollars in `symbol` (a fund, held or inside one) or company `key` on `date`.
fn yours_on(
    snap: &Snapshot,
    m: &derive::Market,
    symbol: &str,
    key: Option<&str>,
    date: NaiveDate,
) -> Decimal {
    let inputs = holdings_inputs(snap, m, date);
    let snaps = snap.holdings.snapshots_near(date);
    match key {
        Some(k) => holdings::lookthrough(&inputs, &snaps, &security_meta(&snap.ledger), DEPTH)
            .companies
            .iter()
            .find(|c| c.key == k)
            .map_or(Decimal::ZERO, |c| c.value),
        None => {
            let held: Decimal = inputs.iter().filter(|h| h.symbol == symbol).map(|h| h.value).sum();
            if held > Decimal::ZERO {
                return held;
            }
            holdings::fund_dollars(&inputs, &snaps, DEPTH).get(symbol).copied().unwrap_or(Decimal::ZERO)
        }
    }
}

/// Everything but the network: cached prices (or a lookup passed in) and your dollars over time.
pub fn build_security(
    snap: &Snapshot,
    q: &SecurityQuery,
    lookup: Option<&PriceHistory>,
    offline: bool,
    today: NaiveDate,
) -> SecurityView {
    let m = snap.market();
    let cached = snap.quotes.prices.get(&q.symbol);
    let (closes, currency, source) = match (cached, lookup) {
        (Some(c), _) => (c.clone(), snap.quotes.currencies.get(&q.symbol).cloned(), "held"),
        (None, Some(h)) => (h.closes.clone(), h.currency.clone(), "fetched"),
        (None, None) => (BTreeMap::new(), None, "none"),
    };
    let price_note = closes.is_empty().then(|| {
        if offline {
            "Offline, so prices for things you don't hold directly aren't fetched.".to_string()
        } else {
            format!("No price history found for {}.", q.symbol)
        }
    });
    let mut dates: Vec<NaiveDate> = snap.ledger.snapshots.keys().copied().filter(|d| *d <= today).collect();
    if dates.last() != Some(&today) {
        dates.push(today);
    }
    let mut yours: Vec<Yours> = dates
        .into_iter()
        .map(|date| Yours { date, value: yours_on(snap, &m, &q.symbol, q.key.as_deref(), date).round_dp(2) })
        .collect();
    // Start the line where you first had some.
    let first = yours.iter().position(|y| !y.value.is_zero()).unwrap_or(yours.len().saturating_sub(1));
    yours.drain(..first);
    // A fund you hold is valued from its units; anything inside a fund leans on its files.
    let through_files = q.key.is_some() || !snap.quotes.prices.contains_key(&q.symbol);
    let estimated_before =
        snap.holdings.earliest().filter(|e| through_files && yours.first().is_some_and(|y| y.date < *e));
    SecurityView {
        symbol: q.symbol.clone(),
        base_currency: snap.ledger.settings.base_currency.clone(),
        currency,
        prices: closes.into_iter().map(|(date, close)| Close { date, close }).collect(),
        price_source: source,
        price_note,
        yours,
        estimated_before,
    }
}

pub async fn get_security(
    State(s): State<AppState>,
    Query(q): Query<SecurityQuery>,
) -> ApiResult<SecurityView> {
    if !valid_symbol(&q.symbol) {
        return Err(bad(StatusCode::BAD_REQUEST, "symbol: letters, digits and . - ^ = only"));
    }
    let snap = s.snapshot();
    let today = Local::now().date_naive();
    let mut lookup = None;
    if !snap.quotes.prices.contains_key(&q.symbol) && !s.offline {
        let fresh = s
            .lookups
            .read()
            .expect("lookups lock")
            .get(&q.symbol)
            .filter(|(at, _)| Utc::now() - *at < Duration::hours(LOOKUP_TTL_HOURS))
            .map(|(_, h)| h.clone());
        lookup = match fresh {
            Some(h) => Some(h),
            None => {
                let (provider, _) = providers_for(&snap.ledger, s.offline);
                let sym = q.symbol.clone();
                let got = tokio::task::spawn_blocking(move || provider.prices(&[sym], Span::Year).pop())
                    .await
                    .ok()
                    .flatten();
                match got {
                    Some(Ok(h)) => {
                        s.lookups
                            .write()
                            .expect("lookups lock")
                            .insert(q.symbol.clone(), (Utc::now(), h.clone()));
                        Some(h)
                    }
                    Some(Err(e)) => {
                        tracing::info!("price lookup for {}: {e}", q.symbol);
                        None
                    }
                    None => None,
                }
            }
        };
    }
    let offline = s.offline;
    // Old holdings files are read from disk for each check-in; keep that off the runtime.
    match tokio::task::spawn_blocking(move || build_security(&snap, &q, lookup.as_ref(), offline, today))
        .await
    {
        Ok(v) => Ok(Json(v)),
        Err(e) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn seeded(dir: &std::path::Path, today: NaiveDate) -> Snapshot {
        let l = crate::seed::seed_ledger(today);
        let quotes = crate::seed::seed_quotes(&l, today);
        let mut store = providers::HoldingsStore::load(&dir.join("holdings"));
        for f in crate::seed::funds::seed_holdings(ymd(2026, 9, 30)) {
            store.save(f).unwrap();
        }
        Snapshot {
            ledger: l,
            problem: None,
            quotes,
            holdings: store,
            version: 1,
            pending: vec![],
            decided: Default::default(),
            next_id: 1,
        }
    }

    #[test]
    fn a_held_fund_has_its_cached_closes_and_a_value_per_checkin() {
        let dir = tempfile::tempdir().unwrap();
        let today = ymd(2026, 11, 2);
        let snap = seeded(dir.path(), today);
        let v = build_security(&snap, &SecurityQuery { symbol: "VTI".into(), key: None }, None, true, today);
        assert_eq!(v.price_source, "held");
        assert!(v.prices.len() > 200, "about a year of closes");
        assert_eq!(v.currency.as_deref(), Some("USD"));
        assert!(v.price_note.is_none());
        assert!(v.estimated_before.is_none(), "a held fund is valued from its units");
        assert_eq!(v.yours.last().unwrap().date, today);
        assert!(v.yours.iter().all(|y| y.value > Decimal::ZERO));
    }

    #[test]
    fn a_company_inside_funds_is_followed_through_them_and_has_no_chart_offline() {
        let dir = tempfile::tempdir().unwrap();
        let today = ymd(2026, 11, 2);
        let snap = seeded(dir.path(), today);
        let lt = holdings::lookthrough(
            &holdings_inputs(&snap, &snap.market(), today),
            &snap.holdings.snapshots_at(today),
            &security_meta(&snap.ledger),
            DEPTH,
        );
        let top = &lt.companies[0];
        let q = SecurityQuery { symbol: top.ticker.clone().unwrap(), key: Some(top.key.clone()) };
        let v = build_security(&snap, &q, None, true, today);
        assert_eq!(v.price_source, "none");
        assert!(v.price_note.as_deref().unwrap().starts_with("Offline"));
        assert_eq!(v.yours.last().unwrap().value, top.value.round_dp(2));
        // Check-ins before the first file (Sep 30) lean on it, and say so.
        assert!(v.yours.first().unwrap().date < ymd(2026, 9, 30));
        assert_eq!(v.estimated_before, Some(ymd(2026, 9, 30)));
        // A lookup passed in is used and labelled as such.
        let h = PriceHistory {
            symbol: q.symbol.clone(),
            currency: Some("USD".into()),
            closes: BTreeMap::from([(today, Decimal::ONE)]),
        };
        let v = build_security(&snap, &q, Some(&h), false, today);
        assert_eq!((v.price_source, v.prices.len()), ("fetched", 1));
    }

    #[test]
    fn only_plain_quote_symbols_reach_a_provider() {
        assert!(valid_symbol("XEQT.TO") && valid_symbol("BRK-B") && valid_symbol("^GSPC"));
        assert!(
            !valid_symbol("")
                && !valid_symbol("a/b")
                && !valid_symbol("x?y=1&z")
                && !valid_symbol(&"A".repeat(21))
        );
    }
}
