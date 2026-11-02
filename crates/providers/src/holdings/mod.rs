//! Fund composition at the boundary: where a fund's holdings file lives, how it is
//! fetched, and the on-disk snapshot store. Parsing is in the `holdings` crate.
//!
//! Two ways a snapshot arrives: the provider downloads it (iShares by product
//! screener lookup; any issuer through `settings.holdings_urls`), or the user drops the
//! issuer's own file into `data/holdings/<SYMBOL>.csv`. Both go through the same
//! parsers and land as `data/holdings/<SYMBOL>/<as-of>.json`, never overwritten.

pub mod ishares;
pub mod store;

use std::collections::BTreeMap;
use std::sync::Mutex;

use chrono::NaiveDate;
use holdings::FundHoldings;

use crate::{get, ProviderError};

pub use store::{HoldingsStore, RefreshReport as HoldingsReport};

/// A fund we know how to download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FundRef {
    pub symbol: String,
    /// `ishares`, `vanguard`, `url` (an address typed in the ledger).
    pub source: String,
    pub url: String,
}

pub trait HoldingsProvider {
    fn name(&self) -> &'static str;
    /// Is this symbol a fund this provider can download? `Ok(None)` means "not one of
    /// ours" (a stock, or an issuer we do not cover); an `Err` is a real failure.
    fn locate(&self, symbol: &str) -> Result<Option<FundRef>, ProviderError>;
    /// Download and parse. `fallback_as_of` dates a file that carries no date.
    fn fetch(&self, fund: &FundRef, fallback_as_of: NaiveDate) -> Result<FundHoldings, ProviderError>;
}

/// Fixed answers for tests and for offline runs.
#[derive(Debug, Clone, Default)]
pub struct ManualHoldings {
    pub funds: BTreeMap<String, FundHoldings>,
}

impl HoldingsProvider for ManualHoldings {
    fn name(&self) -> &'static str {
        "manual"
    }
    fn locate(&self, symbol: &str) -> Result<Option<FundRef>, ProviderError> {
        Ok(self.funds.contains_key(symbol).then(|| FundRef {
            symbol: symbol.to_string(),
            source: "manual".into(),
            url: format!("manual:{symbol}"),
        }))
    }
    fn fetch(&self, fund: &FundRef, _fallback_as_of: NaiveDate) -> Result<FundHoldings, ProviderError> {
        self.funds
            .get(&fund.symbol)
            .cloned()
            .ok_or_else(|| ProviderError::Unknown(fund.symbol.clone(), "manual"))
    }
}

impl HoldingsProvider for crate::Off {
    fn name(&self) -> &'static str {
        "off"
    }
    fn locate(&self, _symbol: &str) -> Result<Option<FundRef>, ProviderError> {
        Ok(None)
    }
    fn fetch(&self, fund: &FundRef, _fallback_as_of: NaiveDate) -> Result<FundHoldings, ProviderError> {
        Err(ProviderError::Unknown(fund.symbol.clone(), "off"))
    }
}

/// The real thing: typed URLs first, then iShares Canada and iShares US by ticker.
/// Vanguard Canada files parse (CSV export and the site's JSON) but their download
/// address is per fund and was not verifiable offline, so a Vanguard fund is fetched
/// only through `settings.holdings_urls`, or read from a dropped-in file.
#[derive(Debug, Default)]
pub struct Issuers {
    /// symbol -> URL, from `settings.holdings_urls`.
    pub overrides: BTreeMap<String, String>,
    screeners: Mutex<BTreeMap<ishares::Site, BTreeMap<String, String>>>,
}

impl Issuers {
    pub fn new(overrides: BTreeMap<String, String>) -> Self {
        Self { overrides, screeners: Mutex::new(BTreeMap::new()) }
    }

    fn screener(&self, site: ishares::Site) -> Result<BTreeMap<String, String>, ProviderError> {
        let mut cache = self.screeners.lock().expect("screener cache");
        if let Some(m) = cache.get(&site) {
            return Ok(m.clone());
        }
        let body = get(&ishares::screener_url(site))?;
        let m = ishares::parse_screener(&body)?;
        cache.insert(site, m.clone());
        Ok(m)
    }
}

impl HoldingsProvider for Issuers {
    fn name(&self) -> &'static str {
        "issuers"
    }

    fn locate(&self, symbol: &str) -> Result<Option<FundRef>, ProviderError> {
        if let Some(url) = self.overrides.get(symbol) {
            return Ok(Some(FundRef { symbol: symbol.to_string(), source: "url".into(), url: url.clone() }));
        }
        let (site, ticker) = ishares::site_for(symbol);
        let map = self.screener(site)?;
        Ok(map.get(ticker).map(|page| FundRef {
            symbol: symbol.to_string(),
            source: "ishares".into(),
            url: ishares::holdings_url(site, page, ticker),
        }))
    }

    fn fetch(&self, fund: &FundRef, fallback_as_of: NaiveDate) -> Result<FundHoldings, ProviderError> {
        let body = get(&fund.url)?;
        let mut h = holdings::parse_any(&fund.symbol, &body, Some(fallback_as_of))?;
        h.source = fund.source.clone();
        Ok(h)
    }
}
