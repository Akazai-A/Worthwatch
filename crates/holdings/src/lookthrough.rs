//! Positions × compositions, flattened. Pure: values in, companies and buckets out.
//!
//! `max_depth` 0 is the "as held" view (every position is a leaf); 3 is enough for a
//! fund of funds of funds. A held symbol with a snapshot is expanded; a constituent
//! whose ticker resolves to a fund we also have a snapshot for is expanded in turn;
//! everything else is a leaf keyed by ticker (else name) so the same company reached
//! through several funds merges into one entry that remembers each path.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::region::{country_from_symbol, region};
use crate::sector::{self, canonical};
use crate::{child_symbol, AssetClass, FundHoldings};

/// One position as the look-through sees it: already valued in base currency.
#[derive(Debug, Clone, PartialEq)]
pub struct Holding {
    pub symbol: String,
    pub name: Option<String>,
    pub value: Decimal,
    /// Currency it trades in, when known (from the quote provider or the account).
    pub currency: Option<String>,
}

/// What the ledger says about a security by hand (`securities:` in the YAML): fills
/// in or overrides what issuer files say. All optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_class: Option<AssetClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

/// How a company was reached: the chain of funds from what you hold down to it.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Via {
    pub path: Vec<String>,
    /// Percent of the top fund in `path` that this leaf represents (through every level).
    pub weight_pct: Decimal,
    pub value: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Company {
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    pub sector: String,
    /// Colour slot for `sector`.
    pub slot: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    pub region: String,
    pub asset_class: AssetClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    pub value: Decimal,
    /// Percent of the look-through total.
    pub pct: Decimal,
    pub undisclosed: bool,
    pub via: Vec<Via>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Bucket {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    pub value: Decimal,
    pub pct: Decimal,
}

/// One per position you hold.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FundStatus {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub value: Decimal,
    /// A snapshot exists and was expanded.
    pub resolved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub constituents: usize,
    /// Child funds that were expanded under it (`XEQT.TO` -> `XUS.TO`, ...).
    pub children: Vec<String>,
    /// Child tickers that look like funds but have no snapshot (kept as leaves).
    pub unresolved_children: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage_pct: Option<Decimal>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_holdings: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LookThrough {
    pub depth: u8,
    pub total: Decimal,
    /// Oldest snapshot that fed the numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub as_of: Option<NaiveDate>,
    pub funds: Vec<FundStatus>,
    pub companies: Vec<Company>,
    pub company_count: usize,
    /// Dollars inside funds whose files do not disclose where they are.
    pub undisclosed: Decimal,
    /// Dollars in positions with no snapshot (shown as themselves).
    pub unresolved: Decimal,
    pub by_sector: Vec<Bucket>,
    pub by_region: Vec<Bucket>,
    pub by_country: Vec<Bucket>,
    pub by_asset_class: Vec<Bucket>,
    pub by_currency: Vec<Bucket>,
    pub top10_pct: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_holdings: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SharedName {
    pub name: String,
    pub a_pct: Decimal,
    pub b_pct: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct OverlapCell {
    /// Shared weight: sum over every underlying name of min(weight in A, weight in B).
    pub pct: Decimal,
    /// Dollars of A that sit in names B also holds, at A's weights.
    pub shared_value: Decimal,
    pub shared: Vec<SharedName>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Overlap {
    pub symbols: Vec<String>,
    /// `cells[i][j]`; `None` when either fund has no snapshot.
    pub cells: Vec<Vec<Option<OverlapCell>>>,
    pub effective_holdings: Vec<Option<Decimal>>,
}

fn pct(part: Decimal, whole: Decimal, dp: u32) -> Decimal {
    if whole.is_zero() {
        Decimal::ZERO
    } else {
        (part / whole * Decimal::ONE_HUNDRED).round_dp(dp)
    }
}

/// "APPLE INC" -> "Apple Inc"; names that already have lowercase letters are kept.
pub(crate) fn pretty_name(s: &str) -> String {
    if s.chars().any(|c| c.is_ascii_lowercase()) {
        return s.trim().to_string();
    }
    const KEEP: &[&str] = &[
        "ETF", "REIT", "IMI", "EAFE", "USD", "CAD", "S&P", "TSX", "MSCI", "FTSE", "IBM", "AMD", "HSBC",
        "UBS", "SAP", "ASML", "TSMC", "CP", "CN", "RBC", "TD", "BMO", "CIBC", "AT&T", "3M", "GE", "UPS",
        "CVS", "ADR", "PLC", "AG", "SA", "NV", "SE", "ASA", "AB", "II", "III", "US", "UK",
    ];
    s.split_whitespace()
        .map(|w| {
            if KEEP.contains(&w) || w.contains('&') || w.chars().any(|c| c.is_ascii_digit()) {
                return w.to_string();
            }
            let mut cs = w.chars();
            match cs.next() {
                Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn leaf_key(ticker: Option<&str>, name: &str) -> String {
    match ticker.map(str::trim).filter(|t| !t.is_empty() && *t != "-") {
        Some(t) => t.to_uppercase(),
        None => name.to_lowercase().chars().filter(|c| c.is_alphanumeric()).collect(),
    }
}

struct Leaf {
    key: String,
    name: String,
    ticker: Option<String>,
    sector: Option<String>,
    country: Option<String>,
    asset_class: AssetClass,
    currency: Option<String>,
    undisclosed: bool,
}

/// Leaf, accumulated value, and per-path (weight, value).
type Entry = (Leaf, Decimal, BTreeMap<Vec<String>, (Decimal, Decimal)>);

struct Ctx<'a> {
    snapshots: &'a BTreeMap<String, FundHoldings>,
    meta: &'a BTreeMap<String, SecurityMeta>,
    max_depth: u8,
    out: BTreeMap<String, Entry>,
    undisclosed: Decimal,
    as_of: Option<NaiveDate>,
}

impl Ctx<'_> {
    fn add(&mut self, leaf: Leaf, value: Decimal, path: &[String], weight_pct: Decimal) {
        let e = self.out.entry(leaf.key.clone()).or_insert_with(|| (leaf, Decimal::ZERO, BTreeMap::new()));
        // Keep the first non-empty metadata seen; later funds only fill gaps.
        e.1 += value;
        let via = e.2.entry(path.to_vec()).or_default();
        via.0 += weight_pct;
        via.1 += value;
    }

    fn fill(&mut self, key: &str, sector: Option<&str>, country: Option<&str>, currency: Option<&str>) {
        if let Some((leaf, _, _)) = self.out.get_mut(key) {
            if leaf.sector.is_none() {
                leaf.sector = sector.map(Into::into);
            }
            if leaf.country.is_none() {
                leaf.country = country.map(Into::into);
            }
            if leaf.currency.is_none() {
                leaf.currency = currency.map(Into::into);
            }
        }
    }

    /// Expand `symbol` worth `value`. `path` is the chain of funds above it (empty at
    /// the top). Returns the child funds expanded directly under it and the fund-like
    /// children it could not expand.
    fn expand(
        &mut self,
        symbol: &str,
        value: Decimal,
        weight_pct: Decimal,
        path: &mut Vec<String>,
        depth: u8,
    ) -> (Vec<String>, Vec<String>) {
        let mut children = Vec::new();
        let mut unresolved = Vec::new();
        let snap = if depth < self.max_depth && !path.iter().any(|p| p == symbol) {
            self.snapshots.get(symbol)
        } else {
            None
        };
        let Some(snap) = snap else {
            // A leaf you hold directly: a stock, or a fund we know nothing about.
            let m = self.meta.get(symbol).cloned().unwrap_or_default();
            let known_fund = self.snapshots.get(symbol);
            let name = m
                .name
                .clone()
                .or_else(|| known_fund.and_then(|s| s.name.clone()))
                .unwrap_or_else(|| symbol.to_string());
            let sector = m.sector.clone().or_else(|| known_fund.map(|_| sector::FUNDS.label.to_string()));
            let asset_class = m
                .asset_class
                .or_else(|| known_fund.map(FundHoldings::dominant_asset_class))
                .unwrap_or(AssetClass::Equity);
            let country = m.country.clone().or_else(|| country_from_symbol(symbol).map(Into::into));
            self.add(
                Leaf {
                    key: symbol.to_uppercase(),
                    name,
                    ticker: Some(symbol.to_string()),
                    sector,
                    country,
                    asset_class,
                    currency: m.currency.clone(),
                    undisclosed: false,
                },
                value,
                &if path.is_empty() { vec![symbol.to_string()] } else { path.clone() },
                weight_pct,
            );
            return (children, unresolved);
        };
        self.as_of = Some(self.as_of.map_or(snap.as_of, |d| d.min(snap.as_of)));
        path.push(symbol.to_string());
        let snap = snap.clone();
        let mut coverage = Decimal::ZERO;
        for row in &snap.rows {
            if row.weight <= Decimal::ZERO {
                continue;
            }
            coverage += row.weight;
            let v = value * row.weight / Decimal::ONE_HUNDRED;
            let w = weight_pct * row.weight / Decimal::ONE_HUNDRED;
            let child = child_symbol(row).filter(|c| c != symbol);
            match child {
                Some(c) if self.snapshots.contains_key(&c) && depth + 1 < self.max_depth => {
                    if !children.contains(&c) {
                        children.push(c.clone());
                    }
                    self.expand(&c, v, w, path, depth + 1);
                }
                _ => {
                    if let Some(c) = &child {
                        // Looks like a fund with no snapshot: say so once.
                        if crate::is_fundish(row) && !unresolved.contains(c) {
                            unresolved.push(c.clone());
                        }
                    }
                    let key = leaf_key(row.ticker.as_deref(), &row.name);
                    self.add(
                        Leaf {
                            key: key.clone(),
                            name: pretty_name(&row.name),
                            ticker: row.ticker.clone(),
                            sector: row.sector.clone(),
                            country: row.country.clone(),
                            asset_class: row.asset_class,
                            currency: row.currency.clone(),
                            undisclosed: false,
                        },
                        v,
                        path,
                        w,
                    );
                    self.fill(&key, row.sector.as_deref(), row.country.as_deref(), row.currency.as_deref());
                }
            }
        }
        let remainder = Decimal::ONE_HUNDRED - coverage;
        if remainder > Decimal::new(5, 1) {
            let v = value * remainder / Decimal::ONE_HUNDRED;
            self.undisclosed += v;
            self.add(
                Leaf {
                    key: format!("__undisclosed__{symbol}"),
                    name: format!("Undisclosed in {symbol}"),
                    ticker: None,
                    sector: Some(sector::UNDISCLOSED.label.into()),
                    country: None,
                    asset_class: AssetClass::Other,
                    currency: None,
                    undisclosed: true,
                },
                v,
                path,
                weight_pct * remainder / Decimal::ONE_HUNDRED,
            );
        }
        path.pop();
        (children, unresolved)
    }
}

fn buckets<F: Fn(&Company) -> (String, Option<String>)>(
    companies: &[Company],
    total: Decimal,
    f: F,
) -> Vec<Bucket> {
    let mut m: BTreeMap<String, (Option<String>, Decimal)> = BTreeMap::new();
    for c in companies {
        let (label, slot) = f(c);
        let e = m.entry(label).or_insert((slot, Decimal::ZERO));
        e.1 += c.value;
    }
    let mut v: Vec<Bucket> = m
        .into_iter()
        .map(|(label, (slot, value))| Bucket {
            label,
            slot,
            value: value.round_dp(2),
            pct: pct(value, total, 1),
        })
        .collect();
    v.sort_by(|a, b| b.value.cmp(&a.value).then(a.label.cmp(&b.label)));
    v
}

fn effective(weights: impl Iterator<Item = Decimal>) -> Option<Decimal> {
    let s: Decimal = weights.map(|w| w * w).sum();
    (!s.is_zero()).then(|| (Decimal::ONE / s).round_dp(0))
}

pub fn lookthrough(
    holdings: &[Holding],
    snapshots: &BTreeMap<String, FundHoldings>,
    meta: &BTreeMap<String, SecurityMeta>,
    max_depth: u8,
) -> LookThrough {
    let mut ctx =
        Ctx { snapshots, meta, max_depth, out: BTreeMap::new(), undisclosed: Decimal::ZERO, as_of: None };
    let total: Decimal = holdings.iter().map(|h| h.value).sum();
    let mut funds = Vec::new();
    let mut unresolved_value = Decimal::ZERO;
    for h in holdings {
        let mut path = Vec::new();
        let (children, unresolved_children) =
            ctx.expand(&h.symbol, h.value, Decimal::ONE_HUNDRED, &mut path, 0);
        let snap = snapshots.get(&h.symbol);
        let resolved = max_depth > 0 && snap.is_some();
        if !resolved {
            unresolved_value += h.value;
        }
        // Per-fund effective holdings: expand it alone at 100 and count.
        let effective_holdings = snap.filter(|_| max_depth > 0).and_then(|_| {
            let mut solo = Ctx {
                snapshots,
                meta,
                max_depth,
                out: BTreeMap::new(),
                undisclosed: Decimal::ZERO,
                as_of: None,
            };
            solo.expand(&h.symbol, Decimal::ONE_HUNDRED, Decimal::ONE_HUNDRED, &mut Vec::new(), 0);
            let covered: Decimal =
                solo.out.values().filter(|(l, _, _)| !l.undisclosed).map(|(_, v, _)| *v).sum();
            effective(solo.out.values().filter(|(l, _, _)| !l.undisclosed).map(|(_, v, _)| *v / covered))
        });
        funds.push(FundStatus {
            symbol: h.symbol.clone(),
            name: snap.and_then(|s| s.name.clone()).or_else(|| h.name.clone()),
            value: h.value,
            resolved,
            as_of: snap.map(|s| s.as_of),
            source: snap.map(|s| s.source.clone()),
            constituents: snap.map_or(0, |s| s.rows.len()),
            children,
            unresolved_children,
            coverage_pct: snap.map(|s| s.coverage().round_dp(1)),
            effective_holdings,
        });
    }
    // Direct holdings get their trading currency when the issuer rows gave none.
    for h in holdings {
        if let Some((leaf, _, _)) = ctx.out.get_mut(&h.symbol.to_uppercase()) {
            if leaf.currency.is_none() {
                leaf.currency = h.currency.clone();
            }
        }
    }
    let mut companies: Vec<Company> = ctx
        .out
        .into_values()
        .map(|(leaf, value, via)| {
            let s = if leaf.undisclosed {
                sector::UNDISCLOSED
            } else {
                canonical(leaf.sector.as_deref(), leaf.asset_class)
            };
            let region = if leaf.undisclosed {
                "Undisclosed".to_string()
            } else {
                region(leaf.country.as_deref()).to_string()
            };
            let mut via: Vec<Via> = via
                .into_iter()
                .map(|(path, (w, v))| Via { path, weight_pct: w.round_dp(2), value: v.round_dp(2) })
                .collect();
            via.sort_by_key(|v| std::cmp::Reverse(v.value));
            Company {
                key: leaf.key,
                name: leaf.name,
                ticker: leaf.ticker,
                sector: s.label.to_string(),
                slot: s.slot.to_string(),
                country: leaf.country,
                region,
                asset_class: leaf.asset_class,
                currency: leaf.currency,
                value: value.round_dp(2),
                pct: pct(value, total, 2),
                undisclosed: leaf.undisclosed,
                via,
            }
        })
        .collect();
    companies.sort_by(|a, b| b.value.cmp(&a.value).then(a.name.cmp(&b.name)));
    let disclosed: Vec<&Company> = companies.iter().filter(|c| !c.undisclosed).collect();
    let top10: Decimal = disclosed.iter().take(10).map(|c| c.value).sum();
    let covered: Decimal = disclosed.iter().map(|c| c.value).sum();
    let effective_holdings =
        effective(
            disclosed.iter().map(|c| if covered.is_zero() { Decimal::ZERO } else { c.value / covered }),
        );
    let by_sector = buckets(&companies, total, |c| (c.sector.clone(), Some(c.slot.clone())));
    let by_region = buckets(&companies, total, |c| (c.region.clone(), None));
    let by_country = buckets(&companies, total, |c| {
        (
            if c.undisclosed {
                "Undisclosed".into()
            } else {
                c.country.clone().unwrap_or_else(|| "Unknown".into())
            },
            None,
        )
    });
    let by_asset_class = buckets(&companies, total, |c| {
        (if c.undisclosed { "Undisclosed".into() } else { c.asset_class.label().to_string() }, None)
    });
    let by_currency = buckets(&companies, total, |c| {
        (
            if c.undisclosed {
                "Undisclosed".into()
            } else {
                c.currency.clone().unwrap_or_else(|| "Unknown".into())
            },
            None,
        )
    });
    LookThrough {
        depth: max_depth,
        total: total.round_dp(2),
        as_of: ctx.as_of,
        funds,
        company_count: disclosed.len(),
        companies,
        undisclosed: ctx.undisclosed.round_dp(2),
        unresolved: unresolved_value,
        by_sector,
        by_region,
        by_country,
        by_asset_class,
        by_currency,
        top10_pct: pct(top10, total, 1),
        effective_holdings,
    }
}

impl LookThrough {
    /// Keep the biggest `keep` companies and fold the rest into one "N other companies"
    /// entry, for a payload the browser can draw. Buckets are already complete.
    pub fn cap(mut self, keep: usize) -> Self {
        let (undisclosed, mut rest): (Vec<Company>, Vec<Company>) =
            std::mem::take(&mut self.companies).into_iter().partition(|c| c.undisclosed);
        if rest.len() > keep + 1 {
            let tail = rest.split_off(keep);
            let value: Decimal = tail.iter().map(|c| c.value).sum();
            rest.push(Company {
                key: "__other__".into(),
                name: format!("{} other companies", tail.len()),
                ticker: None,
                sector: "Other".into(),
                slot: "other".into(),
                country: None,
                region: "Other".into(),
                asset_class: AssetClass::Other,
                currency: None,
                value: value.round_dp(2),
                pct: pct(value, self.total, 2),
                undisclosed: false,
                via: Vec::new(),
            });
        }
        rest.extend(undisclosed);
        rest.sort_by(|a, b| b.value.cmp(&a.value).then(a.name.cmp(&b.name)));
        self.companies = rest;
        self
    }
}

/// Pairwise shared weight between the funds you hold that have snapshots.
pub fn overlap(
    holdings: &[Holding],
    snapshots: &BTreeMap<String, FundHoldings>,
    max_depth: u8,
    top: usize,
) -> Overlap {
    let symbols: Vec<String> = holdings.iter().map(|h| h.symbol.clone()).collect();
    let empty = BTreeMap::new();
    // Each fund's leaf weights as a share of its disclosed part, so overlap(A, A) = 100.
    let weights: Vec<Option<BTreeMap<String, (String, Decimal)>>> = holdings
        .iter()
        .map(|h| {
            snapshots.get(&h.symbol)?;
            let mut ctx = Ctx {
                snapshots,
                meta: &empty,
                max_depth,
                out: BTreeMap::new(),
                undisclosed: Decimal::ZERO,
                as_of: None,
            };
            ctx.expand(&h.symbol, Decimal::ONE_HUNDRED, Decimal::ONE_HUNDRED, &mut Vec::new(), 0);
            let covered: Decimal =
                ctx.out.values().filter(|(l, _, _)| !l.undisclosed).map(|(_, v, _)| *v).sum();
            if covered.is_zero() {
                return None;
            }
            Some(
                ctx.out
                    .into_iter()
                    .filter(|(_, (l, _, _))| !l.undisclosed)
                    .map(|(k, (l, v, _))| (k, (l.name, v / covered * Decimal::ONE_HUNDRED)))
                    .collect(),
            )
        })
        .collect();
    let effective_holdings = weights
        .iter()
        .map(|w| w.as_ref().and_then(|w| effective(w.values().map(|(_, p)| *p / Decimal::ONE_HUNDRED))))
        .collect();
    let n = holdings.len();
    let mut cells = vec![vec![None; n]; n];
    for i in 0..n {
        for j in 0..n {
            let (Some(a), Some(b)) = (&weights[i], &weights[j]) else { continue };
            let mut shared: Vec<SharedName> = Vec::new();
            let mut sum = Decimal::ZERO;
            for (k, (name, wa)) in a {
                if let Some((_, wb)) = b.get(k) {
                    sum += (*wa).min(*wb);
                    shared.push(SharedName {
                        name: name.clone(),
                        a_pct: wa.round_dp(2),
                        b_pct: wb.round_dp(2),
                    });
                }
            }
            shared.sort_by_key(|x| std::cmp::Reverse(x.a_pct.min(x.b_pct)));
            shared.truncate(top);
            cells[i][j] = Some(OverlapCell {
                pct: sum.round_dp(1),
                shared_value: (holdings[i].value * sum / Decimal::ONE_HUNDRED).round_dp(2),
                shared,
            });
        }
    }
    Overlap { symbols, cells, effective_holdings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Constituent;
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn row(
        ticker: &str,
        name: &str,
        w: Decimal,
        sector: &str,
        country: &str,
        cur: &str,
        ex: Option<&str>,
    ) -> Constituent {
        Constituent {
            ticker: Some(ticker.into()),
            name: name.into(),
            weight: w,
            sector: Some(sector.into()),
            country: Some(country.into()),
            asset_class: AssetClass::Equity,
            currency: Some(cur.into()),
            exchange: ex.map(Into::into),
        }
    }

    fn snap(symbol: &str, rows: Vec<Constituent>) -> FundHoldings {
        FundHoldings {
            symbol: symbol.into(),
            name: Some(format!("{symbol} fund")),
            as_of: ymd(2026, 10, 30),
            source: "seed".into(),
            rows,
        }
    }

    /// XEQT-style: a fund of two funds; one of them lists Apple, both list Shopify.
    fn world() -> BTreeMap<String, FundHoldings> {
        let mut m = BTreeMap::new();
        m.insert(
            "XEQT.TO".into(),
            snap(
                "XEQT.TO",
                vec![
                    row(
                        "XUS",
                        "ISHARES CORE S&P 500 INDEX ETF",
                        dec!(60),
                        "-",
                        "Canada",
                        "CAD",
                        Some("Toronto Stock Exchange"),
                    ),
                    row(
                        "XIC",
                        "ISHARES S&P/TSX CAPPED COMPOSITE ETF",
                        dec!(39.5),
                        "-",
                        "Canada",
                        "CAD",
                        Some("Toronto Stock Exchange"),
                    ),
                    Constituent {
                        ticker: Some("CAD".into()),
                        name: "CAD CASH".into(),
                        weight: dec!(0.5),
                        sector: Some("Cash and/or Derivatives".into()),
                        country: Some("Canada".into()),
                        asset_class: AssetClass::Cash,
                        currency: Some("CAD".into()),
                        exchange: None,
                    },
                ],
            ),
        );
        m.insert(
            "XUS.TO".into(),
            snap(
                "XUS.TO",
                vec![
                    row(
                        "AAPL",
                        "APPLE INC",
                        dec!(70),
                        "Information Technology",
                        "United States",
                        "USD",
                        Some("NASDAQ"),
                    ),
                    row(
                        "SHOP",
                        "SHOPIFY INC",
                        dec!(30),
                        "Information Technology",
                        "Canada",
                        "USD",
                        Some("NASDAQ"),
                    ),
                ],
            ),
        );
        m.insert(
            "XIC.TO".into(),
            snap(
                "XIC.TO",
                vec![
                    row(
                        "RY",
                        "ROYAL BANK OF CANADA",
                        dec!(50),
                        "Financials",
                        "Canada",
                        "CAD",
                        Some("Toronto Stock Exchange"),
                    ),
                    row(
                        "SHOP",
                        "SHOPIFY INC",
                        dec!(50),
                        "Information Technology",
                        "Canada",
                        "CAD",
                        Some("Toronto Stock Exchange"),
                    ),
                ],
            ),
        );
        m
    }

    fn held(symbol: &str, value: Decimal) -> Holding {
        Holding { symbol: symbol.into(), name: None, value, currency: Some("CAD".into()) }
    }

    #[test]
    fn fund_of_funds_resolves_two_levels_and_sums_to_100() {
        let snaps = world();
        let lt = lookthrough(&[held("XEQT.TO", dec!(10000))], &snaps, &BTreeMap::new(), 3);
        assert_eq!(lt.total, dec!(10000));
        let sum: Decimal = lt.companies.iter().map(|c| c.value).sum();
        assert!((sum - dec!(10000)).abs() <= dec!(50), "sum {sum}"); // 100% ± 0.5%
        assert_eq!(lt.undisclosed, dec!(0));
        let apple = lt.companies.iter().find(|c| c.key == "AAPL").unwrap();
        assert_eq!(apple.name, "Apple Inc");
        assert_eq!(apple.value, dec!(4200)); // 60% × 70%
        assert_eq!(apple.via[0].path, vec!["XEQT.TO", "XUS.TO"]);
        assert_eq!(apple.via[0].weight_pct, dec!(42.00));
        let shop = lt.companies.iter().find(|c| c.key == "SHOP").unwrap();
        assert_eq!(shop.value, dec!(1800) + dec!(1975)); // via XUS and via XIC, merged
        assert_eq!(shop.via.len(), 2);
        assert_eq!(lt.funds[0].children, vec!["XUS.TO", "XIC.TO"]);
        assert!(lt.funds[0].resolved);
        assert_eq!(lt.by_sector[0].label, "Technology");
        assert_eq!(lt.by_sector[0].pct, dec!(79.8));
        assert_eq!(lt.by_region[0].label, "North America");
        assert_eq!(lt.by_region[0].pct, dec!(100.0));
        assert_eq!(lt.by_asset_class.iter().find(|b| b.label == "Cash").unwrap().value, dec!(50));
        assert_eq!(lt.company_count, 4);
    }

    #[test]
    fn as_held_treats_every_position_as_a_leaf() {
        let snaps = world();
        let lt = lookthrough(
            &[held("XEQT.TO", dec!(10000)), held("SHOP.TO", dec!(500))],
            &snaps,
            &BTreeMap::new(),
            0,
        );
        assert_eq!(lt.companies.len(), 2);
        let x = lt.companies.iter().find(|c| c.key == "XEQT.TO").unwrap();
        assert_eq!(x.sector, "Funds");
        assert_eq!(x.country.as_deref(), Some("Canada"));
        assert_eq!(x.asset_class, AssetClass::Equity);
        assert!(!lt.funds[0].resolved);
    }

    #[test]
    fn undisclosed_remainder_and_unknown_positions_are_never_dropped() {
        let mut snaps = world();
        snaps.insert(
            "TOP10.TO".into(),
            snap(
                "TOP10.TO",
                vec![row(
                    "AAPL",
                    "APPLE INC",
                    dec!(40),
                    "Information Technology",
                    "United States",
                    "USD",
                    None,
                )],
            ),
        );
        let mut meta = BTreeMap::new();
        meta.insert(
            "NOPE.TO".to_string(),
            SecurityMeta { sector: Some("Energy".into()), ..Default::default() },
        );
        let lt = lookthrough(&[held("TOP10.TO", dec!(1000)), held("NOPE.TO", dec!(100))], &snaps, &meta, 3);
        assert_eq!(lt.undisclosed, dec!(600));
        assert_eq!(lt.unresolved, dec!(100));
        let u = lt.companies.iter().find(|c| c.undisclosed).unwrap();
        assert_eq!(u.name, "Undisclosed in TOP10.TO");
        assert_eq!(u.value, dec!(600));
        let nope = lt.companies.iter().find(|c| c.key == "NOPE.TO").unwrap();
        assert_eq!(nope.sector, "Energy");
        let sum: Decimal = lt.companies.iter().map(|c| c.value).sum();
        assert_eq!(sum, dec!(1100));
        assert_eq!(lt.by_sector.iter().find(|b| b.label == "Undisclosed").unwrap().pct, dec!(54.5));
    }

    #[test]
    fn overlap_of_a_fund_with_itself_is_100() {
        let snaps = world();
        let hs = [held("XEQT.TO", dec!(10000)), held("XUS.TO", dec!(5000)), held("NOPE.TO", dec!(1))];
        let o = overlap(&hs, &snaps, 3, 10);
        let aa = o.cells[0][0].as_ref().unwrap();
        assert_eq!(aa.pct, dec!(100.0));
        assert_eq!(aa.shared_value, dec!(10000));
        let ab = o.cells[0][1].as_ref().unwrap();
        // XEQT leaves: AAPL 42, SHOP 37.75, RY 19.75, CAD 0.5; XUS: AAPL 70, SHOP 30 -> min sum 42 + 30 = 72
        assert_eq!(ab.pct, dec!(72.0));
        assert_eq!(o.cells[1][0].as_ref().unwrap().pct, dec!(72.0));
        assert_eq!(ab.shared[0].name, "Apple Inc");
        assert!(o.cells[0][2].is_none() && o.cells[2][2].is_none());
        assert_eq!(o.effective_holdings[1], Some(dec!(2))); // 1 / (0.49 + 0.09) = 1.72 -> 2
    }

    #[test]
    fn cap_folds_the_tail_but_keeps_undisclosed_separate() {
        let mut snaps = world();
        snaps.insert(
            "TOP10.TO".into(),
            snap(
                "TOP10.TO",
                vec![row(
                    "AAPL",
                    "APPLE INC",
                    dec!(40),
                    "Information Technology",
                    "United States",
                    "USD",
                    None,
                )],
            ),
        );
        let lt = lookthrough(
            &[held("XEQT.TO", dec!(10000)), held("TOP10.TO", dec!(1000))],
            &snaps,
            &BTreeMap::new(),
            3,
        )
        .cap(2);
        let names: Vec<&str> = lt.companies.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"2 other companies"), "{names:?}");
        assert!(names.contains(&"Undisclosed in TOP10.TO"));
        let sum: Decimal = lt.companies.iter().map(|c| c.value).sum();
        assert_eq!(sum, dec!(11000));
    }

    #[test]
    fn pretty_names() {
        assert_eq!(pretty_name("ROYAL BANK OF CANADA"), "Royal Bank Of Canada");
        assert_eq!(pretty_name("Apple Inc."), "Apple Inc.");
        assert_eq!(pretty_name("AT&T INC"), "AT&T Inc");
    }
}
