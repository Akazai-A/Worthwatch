//! `quotes.json`: everything the app fetched. Lives beside `ledger.yaml`, is never
//! hand-edited, and can be deleted; the next refresh rebuilds it.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::{FxProvider, PriceProvider, ProviderError, Span};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct QuoteCache {
    pub fetched_at: Option<DateTime<Utc>>,
    /// symbol -> date -> close in the symbol's currency
    #[serde(default)]
    pub prices: BTreeMap<String, BTreeMap<NaiveDate, Decimal>>,
    /// symbol -> currency the provider quoted it in
    #[serde(default)]
    pub currencies: BTreeMap<String, String>,
    /// currency -> date -> units of base per 1 unit
    #[serde(default)]
    pub fx: BTreeMap<String, BTreeMap<NaiveDate, Decimal>>,
    /// What went wrong on the last refresh, per symbol or currency. Empty means all good.
    #[serde(default)]
    pub errors: BTreeMap<String, String>,
}

/// What one refresh did, for the log and the UI.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct RefreshReport {
    pub prices_ok: Vec<String>,
    pub fx_ok: Vec<String>,
    pub errors: BTreeMap<String, String>,
}

impl QuoteCache {
    pub fn load(path: &Path) -> Result<Self, ProviderError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        Ok(serde_json::from_str(&std::fs::read_to_string(path)?)?)
    }

    /// Atomic: temp file + rename, like the ledger.
    pub fn save(&self, path: &Path) -> Result<(), ProviderError> {
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    }

    /// Most recent date any price was fetched for.
    pub fn latest_price_date(&self) -> Option<NaiveDate> {
        self.prices.values().filter_map(|m| m.keys().next_back().copied()).max()
    }

    /// Fetch every symbol and currency and merge into the cache. A failure for one
    /// symbol keeps its old data and records the error; the rest still update.
    pub fn refresh(
        &mut self,
        prices: &dyn PriceProvider,
        fx: &dyn FxProvider,
        base: &str,
        symbols: &[String],
        currencies: &[String],
        now: DateTime<Utc>,
    ) -> RefreshReport {
        let mut report = RefreshReport::default();
        let (backfill, topup): (Vec<String>, Vec<String>) =
            symbols.iter().cloned().partition(|s| !self.prices.contains_key(s));
        for (list, span) in [(backfill, Span::Year), (topup, Span::Recent)] {
            if list.is_empty() {
                continue;
            }
            for (sym, r) in list.iter().zip(prices.prices(&list, span)) {
                match r {
                    Ok(h) => {
                        self.prices.entry(sym.clone()).or_default().extend(h.closes);
                        if let Some(c) = h.currency {
                            self.currencies.insert(sym.clone(), c);
                        }
                        report.prices_ok.push(sym.clone());
                    }
                    Err(e) => {
                        report.errors.insert(sym.clone(), e.to_string());
                    }
                }
            }
        }
        let (backfill, topup): (Vec<String>, Vec<String>) =
            currencies.iter().cloned().partition(|c| !self.fx.contains_key(c));
        for (list, span) in [(backfill, Span::Year), (topup, Span::Recent)] {
            if list.is_empty() {
                continue;
            }
            for (cur, r) in list.iter().zip(fx.rates(base, &list, span)) {
                match r {
                    Ok(h) => {
                        self.fx.entry(cur.clone()).or_default().extend(h.rates);
                        report.fx_ok.push(cur.clone());
                    }
                    Err(e) => {
                        report.errors.insert(cur.clone(), e.to_string());
                    }
                }
            }
        }
        self.errors = report.errors.clone();
        self.fetched_at = Some(now);
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Manual, Off, PriceHistory, RateHistory};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn manual() -> Manual {
        let mut m = Manual::default();
        m.prices.insert(
            "XEQT.TO".into(),
            PriceHistory {
                symbol: "XEQT.TO".into(),
                currency: Some("CAD".into()),
                closes: BTreeMap::from([(ymd(2026, 10, 31), dec!(34.12))]),
            },
        );
        m.rates.insert(
            "USD".into(),
            RateHistory {
                currency: "USD".into(),
                rates: BTreeMap::from([(ymd(2026, 10, 31), dec!(1.3912))]),
            },
        );
        m
    }

    #[test]
    fn refresh_merges_keeps_old_data_on_failure_and_records_errors() {
        let mut c = QuoteCache::default();
        c.prices.insert("VTI".into(), BTreeMap::from([(ymd(2026, 10, 1), dec!(240))]));
        let now = Utc::now();
        let m = manual();
        let r =
            c.refresh(&m, &m, "CAD", &["XEQT.TO".into(), "VTI".into()], &["USD".into(), "EUR".into()], now);
        assert_eq!(r.prices_ok, vec!["XEQT.TO".to_string()]);
        assert_eq!(r.fx_ok, vec!["USD".to_string()]);
        assert!(r.errors.contains_key("VTI"));
        assert!(r.errors.contains_key("EUR"));
        assert_eq!(c.prices["VTI"][&ymd(2026, 10, 1)], dec!(240)); // kept
        assert_eq!(c.currencies["XEQT.TO"], "CAD");
        assert_eq!(c.fetched_at, Some(now));
        assert_eq!(c.latest_price_date(), Some(ymd(2026, 10, 31)));
    }

    #[test]
    fn off_provider_fetches_nothing() {
        let mut c = QuoteCache::default();
        let r = c.refresh(&Off, &Off, "CAD", &["X".into()], &["USD".into()], Utc::now());
        assert!(r.prices_ok.is_empty() && r.fx_ok.is_empty());
        assert_eq!(r.errors.len(), 2);
    }

    #[test]
    fn round_trips_through_disk_and_missing_file_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("quotes.json");
        assert_eq!(QuoteCache::load(&p).unwrap(), QuoteCache::default());
        let mut c = QuoteCache::default();
        let m = manual();
        c.refresh(&m, &m, "CAD", &["XEQT.TO".into()], &["USD".into()], Utc::now());
        c.save(&p).unwrap();
        assert!(!p.with_extension("json.tmp").exists());
        assert_eq!(QuoteCache::load(&p).unwrap(), c);
    }
}
