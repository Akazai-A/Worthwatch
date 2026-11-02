//! `data/holdings/`: fund compositions on disk.
//!
//! ```text
//! holdings/
//!   XEQT.TO/2026-10-30.json   snapshots the app wrote (one per fund per as-of date; append-only)
//!   XEQT.TO.csv               a file you downloaded from the issuer and dropped here (any supported format)
//!   status.json               last refresh time and per-fund errors
//! ```
//! A dropped-in file is parsed on every load and saved as a snapshot for its date if
//! that date is new; the original is left where it is. Snapshots are never rewritten,
//! so Stage 4 can diff them.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use holdings::{child_symbol, FundHoldings};
use serde::{Deserialize, Serialize};

use super::HoldingsProvider;
use crate::ProviderError;

/// How old a snapshot may be before the daily tick fetches again.
pub const STALE_DAYS: i64 = 30;
/// How far a fund of funds is followed when fetching children.
pub const MAX_DEPTH: usize = 3;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
struct Status {
    fetched_at: Option<DateTime<Utc>>,
    #[serde(default)]
    errors: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct HoldingsStore {
    pub dir: PathBuf,
    /// Newest snapshot per symbol, including children fetched for look-through.
    pub latest: BTreeMap<String, FundHoldings>,
    /// Snapshot dates on disk per symbol, oldest first.
    pub dates: BTreeMap<String, Vec<NaiveDate>>,
    pub fetched_at: Option<DateTime<Utc>>,
    /// symbol or file name -> what went wrong last time.
    pub errors: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct RefreshReport {
    pub fetched: Vec<String>,
    pub skipped_fresh: Vec<String>,
    pub not_funds: Vec<String>,
    pub errors: BTreeMap<String, String>,
}

fn snapshot_path(dir: &Path, symbol: &str, as_of: NaiveDate) -> PathBuf {
    dir.join(symbol).join(format!("{as_of}.json"))
}

fn mtime_date(p: &Path) -> Option<NaiveDate> {
    let t: DateTime<Utc> = std::fs::metadata(p).ok()?.modified().ok()?.into();
    Some(t.date_naive())
}

impl HoldingsStore {
    /// Read every snapshot and import any dropped-in files. A missing dir is empty.
    pub fn load(dir: &Path) -> Self {
        let mut s = HoldingsStore { dir: dir.to_path_buf(), ..Default::default() };
        if let Ok(text) = std::fs::read_to_string(dir.join("status.json")) {
            if let Ok(st) = serde_json::from_str::<Status>(&text) {
                s.fetched_at = st.fetched_at;
                s.errors = st.errors;
            }
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return s };
        let mut drop_ins = Vec::new();
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                let symbol = e.file_name().to_string_lossy().to_string();
                let mut dates: Vec<(NaiveDate, PathBuf)> = std::fs::read_dir(&p)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .filter_map(|f| {
                        let stem = f.path().file_stem()?.to_string_lossy().to_string();
                        Some((NaiveDate::parse_from_str(&stem, "%Y-%m-%d").ok()?, f.path()))
                    })
                    .collect();
                dates.sort();
                if let Some((_, newest)) = dates.last() {
                    match std::fs::read_to_string(newest)
                        .map_err(ProviderError::from)
                        .and_then(|t| Ok(serde_json::from_str::<FundHoldings>(&t)?))
                    {
                        Ok(h) => {
                            s.latest.insert(symbol.clone(), h);
                        }
                        Err(e) => {
                            s.errors.insert(symbol.clone(), format!("{}: {e}", newest.display()));
                        }
                    }
                }
                s.dates.insert(symbol, dates.into_iter().map(|(d, _)| d).collect());
            } else if p.extension().is_some_and(|x| {
                matches!(x.to_string_lossy().to_ascii_lowercase().as_str(), "csv" | "json" | "txt")
            }) {
                let stem = p.file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_default();
                if stem != "status" {
                    drop_ins.push((stem.to_uppercase(), p));
                }
            }
        }
        for (symbol, path) in drop_ins {
            let fallback = mtime_date(&path);
            match std::fs::read_to_string(&path)
                .map_err(ProviderError::from)
                .and_then(|t| Ok(holdings::parse_any(&symbol, &t, fallback)?))
            {
                Ok(mut h) => {
                    h.source = "file".into();
                    s.errors.remove(&symbol);
                    if let Err(e) = s.save(h) {
                        s.errors.insert(symbol, e.to_string());
                    }
                }
                Err(e) => {
                    s.errors.insert(
                        symbol,
                        format!("{}: {e}", path.file_name().unwrap_or_default().to_string_lossy()),
                    );
                }
            }
        }
        s
    }

    /// Write a snapshot unless one for that fund and date exists; keep it as latest if newest.
    pub fn save(&mut self, h: FundHoldings) -> Result<(), ProviderError> {
        let path = snapshot_path(&self.dir, &h.symbol, h.as_of);
        if path.exists() {
            // Disk is the truth for a date we already have; only fill an empty slot.
            if !self.latest.contains_key(&h.symbol) {
                self.latest.insert(h.symbol.clone(), h.clone());
            }
            let dates = self.dates.entry(h.symbol.clone()).or_default();
            if !dates.contains(&h.as_of) {
                dates.push(h.as_of);
                dates.sort();
            }
            return Ok(());
        }
        std::fs::create_dir_all(path.parent().expect("snapshot dir"))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(&h)?)?;
        std::fs::rename(&tmp, &path)?;
        let dates = self.dates.entry(h.symbol.clone()).or_default();
        if !dates.contains(&h.as_of) {
            dates.push(h.as_of);
            dates.sort();
        }
        let newer = self.latest.get(&h.symbol).is_none_or(|cur| h.as_of >= cur.as_of);
        if newer {
            self.latest.insert(h.symbol.clone(), h);
        }
        Ok(())
    }

    fn save_status(&self) {
        let st = Status { fetched_at: self.fetched_at, errors: self.errors.clone() };
        if std::fs::create_dir_all(&self.dir).is_ok() {
            if let Ok(text) = serde_json::to_string_pretty(&st) {
                let tmp = self.dir.join("status.json.tmp");
                if std::fs::write(&tmp, text).is_ok() {
                    let _ = std::fs::rename(tmp, self.dir.join("status.json"));
                }
            }
        }
    }

    /// Symbols among `held` with no snapshot or one older than [`STALE_DAYS`].
    pub fn stale(&self, held: &[String], today: NaiveDate) -> Vec<String> {
        held.iter()
            .filter(|s| self.latest.get(*s).is_none_or(|h| (today - h.as_of).num_days() > STALE_DAYS))
            .cloned()
            .collect()
    }

    /// Fetch the held funds and, breadth-first, the funds they hold. A symbol the
    /// provider does not know is simply not a fund. A failure records an error and keeps
    /// the old snapshot. `force` refetches funds that are still fresh.
    pub fn refresh(
        &mut self,
        provider: &dyn HoldingsProvider,
        held: &[String],
        today: NaiveDate,
        now: DateTime<Utc>,
        force: bool,
    ) -> RefreshReport {
        let mut report = RefreshReport::default();
        let mut queue: VecDeque<(String, usize)> = held.iter().map(|s| (s.clone(), 0)).collect();
        let mut seen: Vec<String> = Vec::new();
        let mut errors = BTreeMap::new();
        while let Some((symbol, depth)) = queue.pop_front() {
            if seen.contains(&symbol) {
                continue;
            }
            seen.push(symbol.clone());
            let fresh = !force
                && self.latest.get(&symbol).is_some_and(|h| (today - h.as_of).num_days() <= STALE_DAYS);
            let snapshot = if fresh {
                report.skipped_fresh.push(symbol.clone());
                self.latest.get(&symbol).cloned()
            } else {
                match provider.locate(&symbol) {
                    Ok(None) => {
                        if depth == 0 {
                            // Could be a stock; the look-through view says so if it is a fund.
                            report.not_funds.push(symbol.clone());
                        } else {
                            // Queued because a parent lists it as a fund, so its absence matters.
                            errors.insert(
                                symbol.clone(),
                                format!(
                                    "no built-in source for {symbol} (only iShares CA/US are looked up); set settings.holdings_urls or drop in {symbol}.csv"
                                ),
                            );
                        }
                        self.latest.get(&symbol).cloned()
                    }
                    Ok(Some(fund)) => match provider.fetch(&fund, today) {
                        Ok(h) => match self.save(h.clone()) {
                            Ok(()) => {
                                report.fetched.push(symbol.clone());
                                Some(h)
                            }
                            Err(e) => {
                                errors.insert(symbol.clone(), e.to_string());
                                None
                            }
                        },
                        Err(e) => {
                            errors.insert(symbol.clone(), e.to_string());
                            self.latest.get(&symbol).cloned()
                        }
                    },
                    Err(e) => {
                        errors.insert(symbol.clone(), e.to_string());
                        self.latest.get(&symbol).cloned()
                    }
                }
            };
            if let Some(h) = snapshot {
                if depth + 1 < MAX_DEPTH {
                    // A row is followed as a child fund when the issuer's product list
                    // knows its ticker and the row trades where that issuer lists funds
                    // (a bare ticker like CRBN is Corbion on Euronext and an iShares ETF on
                    // NYSE Arca), or when the row itself reads as a fund (so a Vanguard
                    // fund inside an iShares file is at least reported).
                    for row in &h.rows {
                        let venue_ok = holdings::trades_on_north_american_fund_venue(row);
                        if let Some(c) = child_symbol(row)
                            .filter(|c| (venue_ok && provider.knows(c)) || holdings::is_fundish(row))
                        {
                            queue.push_back((c, depth + 1));
                        }
                    }
                }
            }
        }
        // Errors are per refresh: a fund that now works drops its old error.
        for s in &seen {
            self.errors.remove(s);
        }
        self.errors.extend(errors.clone());
        report.errors = errors;
        self.fetched_at = Some(now);
        self.save_status();
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::holdings::ManualHoldings;
    use holdings::{AssetClass, Constituent};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn fund(symbol: &str, as_of: NaiveDate, rows: Vec<(&str, &str, &str)>) -> FundHoldings {
        FundHoldings {
            symbol: symbol.into(),
            name: None,
            as_of,
            source: "manual".into(),
            rows: rows
                .into_iter()
                .map(|(t, n, c)| Constituent {
                    ticker: Some(t.into()),
                    name: n.into(),
                    weight: dec!(50),
                    sector: None,
                    country: Some(c.into()),
                    asset_class: AssetClass::Equity,
                    currency: None,
                    exchange: None,
                })
                .collect(),
        }
    }

    #[test]
    fn refresh_follows_children_saves_snapshots_and_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = ManualHoldings::default();
        let d = ymd(2026, 10, 30);
        p.funds.insert(
            "XEQT.TO".into(),
            fund(
                "XEQT.TO",
                d,
                vec![
                    ("XUS", "ISHARES CORE S&P 500 INDEX ETF", "Canada"),
                    ("XIC", "ISHARES S&P/TSX ETF", "Canada"),
                    // Nothing in the name says fund; the provider's product list does.
                    ("XGRO", "GROWTH PORTFOLIO UNITS", "Canada"),
                ],
            ),
        );
        p.funds.insert(
            "XUS.TO".into(),
            fund(
                "XUS.TO",
                d,
                vec![("AAPL", "APPLE INC", "United States"), ("MSFT", "MICROSOFT CORP", "United States")],
            ),
        );
        p.funds.insert(
            "XGRO.TO".into(),
            fund("XGRO.TO", d, vec![("AAPL", "APPLE INC", "United States"), ("RY", "ROYAL BANK", "Canada")]),
        );
        // Corbion NV trades on Euronext under CRBN, which is also an iShares US ETF the
        // provider knows. The venue gate keeps it a stock.
        let mut corbion = fund("CRBN", d, vec![("AAPL", "APPLE INC", "United States")]);
        corbion.rows.clear();
        p.funds.insert("CRBN".into(), corbion);
        {
            let xus = p.funds.get_mut("XUS.TO").unwrap();
            xus.rows.push(Constituent {
                ticker: Some("CRBN".into()),
                name: "CORBION NV".into(),
                weight: dec!(1),
                sector: Some("Materials".into()),
                country: Some("Netherlands".into()),
                asset_class: AssetClass::Equity,
                currency: Some("EUR".into()),
                exchange: Some("Euronext Amsterdam".into()),
            });
        }
        let mut s = HoldingsStore::load(dir.path());
        let r = s.refresh(&p, &["XEQT.TO".into(), "SHOP.TO".into()], d, Utc::now(), false);
        assert_eq!(r.fetched, vec!["XEQT.TO".to_string(), "XUS.TO".to_string(), "XGRO.TO".to_string()]);
        assert!(!dir.path().join("CRBN").exists(), "a foreign stock sharing an ETF's ticker is not followed");
        assert_eq!(r.not_funds, vec!["SHOP.TO".to_string()]);
        // XIC.TO was listed by its parent as a fund but has no source: that is an error
        // worth showing, unlike SHOP.TO, which may simply be a stock.
        assert_eq!(r.errors.len(), 1, "{:?}", r.errors);
        assert!(r.errors["XIC.TO"].contains("holdings_urls"));
        assert!(dir.path().join("XEQT.TO/2026-10-30.json").exists());
        assert!(dir.path().join("XUS.TO/2026-10-30.json").exists());
        assert!(dir.path().join("status.json").exists());

        // Second refresh within 30 days fetches nothing; a forced one does, and the
        // existing snapshot file for that date is left untouched.
        let before = std::fs::read_to_string(dir.path().join("XEQT.TO/2026-10-30.json")).unwrap();
        let r2 = s.refresh(&p, &["XEQT.TO".into()], d + chrono::Days::new(3), Utc::now(), false);
        assert!(r2.fetched.is_empty());
        assert_eq!(
            r2.skipped_fresh,
            vec!["XEQT.TO".to_string(), "XUS.TO".to_string(), "XGRO.TO".to_string()]
        );
        p.funds.get_mut("XEQT.TO").unwrap().rows.pop();
        let r3 = s.refresh(&p, &["XEQT.TO".into()], d, Utc::now(), true);
        assert_eq!(r3.fetched.len(), 2);
        assert_eq!(std::fs::read_to_string(dir.path().join("XEQT.TO/2026-10-30.json")).unwrap(), before);
        assert_eq!(s.latest["XEQT.TO"].rows.len(), 3, "latest in memory stays what disk has for that date");

        // Reload from disk sees the same snapshots.
        let again = HoldingsStore::load(dir.path());
        assert_eq!(again.latest["XEQT.TO"].rows.len(), 3);
        assert_eq!(again.dates["XUS.TO"], vec![d]);
        assert!(again.fetched_at.is_some());
    }

    #[test]
    fn dropped_in_issuer_file_becomes_a_snapshot_and_a_bad_one_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("xeqt.to.csv"),
            include_str!("../../../holdings/fixtures/ishares_xeqt.csv"),
        )
        .unwrap();
        std::fs::write(dir.path().join("BAD.TO.csv"), "nothing,to,see\n").unwrap();
        let s = HoldingsStore::load(dir.path());
        assert_eq!(s.latest["XEQT.TO"].as_of, ymd(2026, 10, 30));
        assert_eq!(s.latest["XEQT.TO"].source, "file");
        assert!(dir.path().join("XEQT.TO/2026-10-30.json").exists());
        assert!(dir.path().join("xeqt.to.csv").exists(), "the user's file is left alone");
        assert!(s.errors["BAD.TO"].contains("BAD.TO.csv"), "{:?}", s.errors);
        assert_eq!(
            s.stale(&["XEQT.TO".into(), "VFV.TO".into()], ymd(2026, 11, 2)),
            vec!["VFV.TO".to_string()]
        );
        assert_eq!(s.stale(&["XEQT.TO".into()], ymd(2027, 1, 2)), vec!["XEQT.TO".to_string()]);
    }

    #[test]
    fn provider_failure_keeps_the_old_snapshot_and_records_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let d = ymd(2026, 10, 30);
        let mut s = HoldingsStore::load(dir.path());
        s.save(fund("XEQT.TO", d - chrono::Days::new(60), vec![("AAPL", "APPLE INC", "United States")]))
            .unwrap();
        let r = s.refresh(&crate::Off, &["XEQT.TO".into()], d, Utc::now(), false);
        assert_eq!(r.not_funds, vec!["XEQT.TO".to_string()]);
        assert!(s.latest.contains_key("XEQT.TO"));
        let mut failing = ManualHoldings::default();
        failing.funds.insert("XEQT.TO".into(), fund("XEQT.TO", d, vec![]));
        let f = Failing(failing);
        let r = s.refresh(&f, &["XEQT.TO".into()], d, Utc::now(), false);
        assert!(r.errors["XEQT.TO"].contains("boom"));
        assert_eq!(s.latest["XEQT.TO"].as_of, d - chrono::Days::new(60));
        assert!(s.errors.contains_key("XEQT.TO"));
    }

    struct Failing(ManualHoldings);
    impl HoldingsProvider for Failing {
        fn name(&self) -> &'static str {
            "failing"
        }
        fn locate(&self, symbol: &str) -> Result<Option<super::super::FundRef>, ProviderError> {
            self.0.locate(symbol)
        }
        fn fetch(&self, _fund: &super::super::FundRef, _d: NaiveDate) -> Result<FundHoldings, ProviderError> {
            Err(ProviderError::Http("boom".into()))
        }
    }
}
