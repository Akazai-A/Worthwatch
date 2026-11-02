//! Two holdings files for the same fund, compared; two look-throughs, compared.
//! Pure. A constituent is matched by ticker (else by name), the same key the
//! look-through merges on, so a company that moved from one line to two still
//! reads as one change.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::lookthrough::{leaf_key, pretty_name};
use crate::{child_symbol, Bucket, FundHoldings, Holding, LookThrough};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Reweighted,
}

/// One constituent whose weight in the fund moved.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ConstituentChange {
    pub kind: ChangeKind,
    pub key: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    /// Percent of the fund before and after (0 when absent).
    pub from_weight: Decimal,
    pub to_weight: Decimal,
    /// Percentage points, signed.
    pub delta: Decimal,
    /// Your dollars that moved with it: what you hold in the fund x delta. `None`
    /// when the caller gave no exposure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub your_value: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FundDiff {
    pub symbol: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub from_constituents: usize,
    pub to_constituents: usize,
    /// Threshold in percentage points the caller asked for.
    pub threshold: Decimal,
    /// Moves of at least `threshold`, largest first.
    pub changes: Vec<ConstituentChange>,
    /// Moves under the threshold, by kind: (added, removed, reweighted).
    pub minor_added: usize,
    pub minor_removed: usize,
    pub minor_reweighted: usize,
    /// Half the sum of absolute weight moves: how much of the fund changed hands.
    pub turnover_pct: Decimal,
    /// Your dollars in the fund, through every level, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub your_value: Option<Decimal>,
}

fn weights(h: &FundHoldings) -> BTreeMap<String, (String, Option<String>, Decimal)> {
    let mut m: BTreeMap<String, (String, Option<String>, Decimal)> = BTreeMap::new();
    for r in &h.rows {
        if r.weight <= Decimal::ZERO {
            continue;
        }
        let key = leaf_key(r.ticker.as_deref(), &r.name);
        let e = m.entry(key).or_insert_with(|| (pretty_name(&r.name), r.ticker.clone(), Decimal::ZERO));
        e.2 += r.weight;
    }
    m
}

/// Compare two files of the same fund. `your_value` is what you hold in the fund in
/// base currency, through every level, and prices each move in your dollars.
pub fn diff_fund(
    from: &FundHoldings,
    to: &FundHoldings,
    threshold: Decimal,
    your_value: Option<Decimal>,
) -> FundDiff {
    let a = weights(from);
    let b = weights(to);
    let mut changes = Vec::new();
    let (mut minor_added, mut minor_removed, mut minor_reweighted) = (0, 0, 0);
    let mut gross = Decimal::ZERO;
    let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let (name, ticker, w0) = a.get(key).cloned().unwrap_or_else(|| {
            let (n, t, _) = &b[key];
            (n.clone(), t.clone(), Decimal::ZERO)
        });
        let w1 = b.get(key).map_or(Decimal::ZERO, |x| x.2);
        let delta = (w1 - w0).round_dp(2);
        if delta.is_zero() {
            continue;
        }
        gross += delta.abs();
        let kind = if w0.is_zero() {
            ChangeKind::Added
        } else if w1.is_zero() {
            ChangeKind::Removed
        } else {
            ChangeKind::Reweighted
        };
        if delta.abs() < threshold {
            match kind {
                ChangeKind::Added => minor_added += 1,
                ChangeKind::Removed => minor_removed += 1,
                ChangeKind::Reweighted => minor_reweighted += 1,
            }
            continue;
        }
        changes.push(ConstituentChange {
            kind,
            key: key.clone(),
            name: b.get(key).map_or(name, |x| x.0.clone()),
            ticker: b.get(key).and_then(|x| x.1.clone()).or(ticker),
            from_weight: w0.round_dp(2),
            to_weight: w1.round_dp(2),
            delta,
            your_value: your_value.map(|v| (v * delta / Decimal::ONE_HUNDRED).round_dp(2)),
        });
    }
    changes.sort_by(|x, y| y.delta.abs().cmp(&x.delta.abs()).then(x.name.cmp(&y.name)));
    FundDiff {
        symbol: to.symbol.clone(),
        name: to.name.clone().or_else(|| from.name.clone()),
        from: from.as_of,
        to: to.as_of,
        from_constituents: a.len(),
        to_constituents: b.len(),
        threshold,
        changes,
        minor_added,
        minor_removed,
        minor_reweighted,
        turnover_pct: (gross / Decimal::TWO).round_dp(2),
        your_value,
    }
}

/// Your dollars in every fund the look-through reaches: a held fund at its value,
/// a fund inside it at the parent's value x its weight, down to `max_depth`.
pub fn fund_dollars(
    holdings: &[Holding],
    snapshots: &BTreeMap<String, FundHoldings>,
    max_depth: u8,
) -> BTreeMap<String, Decimal> {
    fn walk(
        symbol: &str,
        value: Decimal,
        snapshots: &BTreeMap<String, FundHoldings>,
        path: &mut Vec<String>,
        depth: u8,
        max_depth: u8,
        out: &mut BTreeMap<String, Decimal>,
    ) {
        let Some(snap) = snapshots.get(symbol) else { return };
        if path.iter().any(|p| p == symbol) {
            return;
        }
        *out.entry(symbol.to_string()).or_default() += value;
        if depth + 1 >= max_depth {
            return;
        }
        path.push(symbol.to_string());
        for row in &snap.rows {
            if row.weight <= Decimal::ZERO {
                continue;
            }
            if let Some(child) = child_symbol(row).filter(|c| c != symbol && snapshots.contains_key(c)) {
                walk(
                    &child,
                    value * row.weight / Decimal::ONE_HUNDRED,
                    snapshots,
                    path,
                    depth + 1,
                    max_depth,
                    out,
                );
            }
        }
        path.pop();
    }
    let mut out = BTreeMap::new();
    for h in holdings {
        walk(&h.symbol, h.value, snapshots, &mut Vec::new(), 0, max_depth, &mut out);
    }
    for v in out.values_mut() {
        *v = v.round_dp(2);
    }
    out
}

/// One exposure bucket then and now, in percentage points of the total.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BucketDelta {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    pub from_pct: Decimal,
    pub to_pct: Decimal,
    pub delta: Decimal,
    pub from_value: Decimal,
    pub to_value: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Drift {
    pub from_total: Decimal,
    pub to_total: Decimal,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_as_of: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_as_of: Option<NaiveDate>,
    pub by_sector: Vec<BucketDelta>,
    pub by_region: Vec<BucketDelta>,
    pub by_country: Vec<BucketDelta>,
    pub by_asset_class: Vec<BucketDelta>,
    pub by_currency: Vec<BucketDelta>,
}

fn bucket_deltas(a: &[Bucket], b: &[Bucket]) -> Vec<BucketDelta> {
    let mut m: BTreeMap<&str, (Option<String>, Decimal, Decimal, Decimal, Decimal)> = BTreeMap::new();
    for x in a {
        let e = m.entry(&x.label).or_insert((
            x.slot.clone(),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ));
        e.1 = x.pct;
        e.3 = x.value;
    }
    for x in b {
        let e = m.entry(&x.label).or_insert((
            x.slot.clone(),
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
            Decimal::ZERO,
        ));
        e.2 = x.pct;
        e.4 = x.value;
        if e.0.is_none() {
            e.0 = x.slot.clone();
        }
    }
    let mut v: Vec<BucketDelta> = m
        .into_iter()
        .map(|(label, (slot, p0, p1, v0, v1))| BucketDelta {
            label: label.to_string(),
            slot,
            from_pct: p0,
            to_pct: p1,
            delta: (p1 - p0).round_dp(1),
            from_value: v0,
            to_value: v1,
        })
        .collect();
    // Largest share first, so the list reads like the Exposure tab; the UI sorts by move if asked.
    v.sort_by(|x, y| y.to_pct.max(y.from_pct).cmp(&x.to_pct.max(x.from_pct)).then(x.label.cmp(&y.label)));
    v
}

/// Percentage-point drift between two look-throughs of the same portfolio.
pub fn drift(from: &LookThrough, to: &LookThrough) -> Drift {
    Drift {
        from_total: from.total,
        to_total: to.total,
        from_as_of: from.as_of,
        to_as_of: to.as_of,
        by_sector: bucket_deltas(&from.by_sector, &to.by_sector),
        by_region: bucket_deltas(&from.by_region, &to.by_region),
        by_country: bucket_deltas(&from.by_country, &to.by_country),
        by_asset_class: bucket_deltas(&from.by_asset_class, &to.by_asset_class),
        by_currency: bucket_deltas(&from.by_currency, &to.by_currency),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lookthrough, parse_any, AssetClass, Constituent};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn row(t: &str, name: &str, w: Decimal) -> Constituent {
        Constituent {
            ticker: Some(t.into()),
            name: name.into(),
            weight: w,
            sector: None,
            country: Some("United States".into()),
            asset_class: AssetClass::Equity,
            currency: Some("USD".into()),
            exchange: Some("NYSE Arca".into()),
        }
    }

    fn fund(as_of: NaiveDate, rows: Vec<Constituent>) -> FundHoldings {
        FundHoldings { symbol: "F".into(), name: Some("Fund".into()), as_of, source: "test".into(), rows }
    }

    #[test]
    fn diff_reports_added_removed_and_reweighted_above_the_threshold() {
        let a = fund(
            ymd(2026, 9, 30),
            vec![
                row("AAPL", "APPLE INC", dec!(6.0)),
                row("NVDA", "NVIDIA CORP", dec!(7.0)),
                row("OLD", "OLD CO", dec!(0.3)),
                row("X", "X", dec!(1.0)),
            ],
        );
        let b = fund(
            ymd(2026, 10, 31),
            vec![
                row("AAPL", "APPLE INC", dec!(5.4)),
                row("NVDA", "NVIDIA CORP", dec!(7.9)),
                row("NEW", "NEW CO", dec!(0.8)),
                row("X", "X", dec!(1.1)),
            ],
        );
        let d = diff_fund(&a, &b, dec!(0.5), Some(dec!(10000)));
        assert_eq!((d.from, d.to), (ymd(2026, 9, 30), ymd(2026, 10, 31)));
        let kinds: Vec<(ChangeKind, &str, Decimal)> =
            d.changes.iter().map(|c| (c.kind, c.key.as_str(), c.delta)).collect();
        assert_eq!(
            kinds,
            vec![
                (ChangeKind::Reweighted, "NVDA", dec!(0.90)),
                (ChangeKind::Added, "NEW", dec!(0.80)),
                (ChangeKind::Reweighted, "AAPL", dec!(-0.60)),
            ]
        );
        assert_eq!(d.changes[0].your_value, Some(dec!(90)));
        assert_eq!(d.changes[0].name, "Nvidia Corp");
        assert_eq!((d.minor_added, d.minor_removed, d.minor_reweighted), (0, 1, 1));
        // gross = 0.9 + 0.8 + 0.6 + 0.3 + 0.1 = 2.7; turnover = 1.35
        assert_eq!(d.turnover_pct, dec!(1.35));
        assert_eq!((d.from_constituents, d.to_constituents), (4, 4));

        // A higher threshold hides more; a zero threshold shows everything.
        assert_eq!(diff_fund(&a, &b, dec!(1.0), None).changes.len(), 0);
        let all = diff_fund(&a, &b, Decimal::ZERO, None);
        assert_eq!(all.changes.len(), 5);
        assert!(all.changes.iter().all(|c| c.your_value.is_none()));
    }

    #[test]
    fn identical_files_have_no_changes_and_two_lines_for_one_ticker_merge() {
        let a = fund(
            ymd(2026, 9, 30),
            vec![row("GOOGL", "ALPHABET A", dec!(2.0)), row("GOOGL", "ALPHABET A", dec!(1.0))],
        );
        let b = fund(ymd(2026, 10, 31), vec![row("GOOGL", "ALPHABET A", dec!(3.0))]);
        let d = diff_fund(&a, &b, dec!(0.1), None);
        assert!(d.changes.is_empty());
        assert_eq!(d.turnover_pct, Decimal::ZERO);
        assert_eq!(d.from_constituents, 1);
    }

    #[test]
    fn two_fixture_files_diff_as_expected() {
        let xeqt = parse_any("XEQT.TO", include_str!("../fixtures/ishares_xeqt.csv"), None).unwrap();
        let mut later = xeqt.clone();
        later.as_of = ymd(2026, 11, 30);
        // XEC up 0.5, XEF down 0.5, cash gone, a new line.
        for r in &mut later.rows {
            match r.ticker.as_deref() {
                Some("XEC") => r.weight += dec!(0.6),
                Some("XEF") => r.weight -= dec!(0.5),
                _ => {}
            }
        }
        later.rows.retain(|r| r.ticker.as_deref() != Some("CAD"));
        later.rows.push(row("XFH", "ISHARES CORE MSCI EAFE IMI HEDGED", dec!(0.24)));
        let d = diff_fund(&xeqt, &later, dec!(0.5), Some(dec!(20000)));
        assert_eq!(d.changes.len(), 2);
        assert_eq!(d.changes[0].key, "XEC");
        assert_eq!(d.changes[0].kind, ChangeKind::Reweighted);
        assert_eq!(d.changes[0].from_weight, dec!(7.37));
        assert_eq!(d.changes[0].to_weight, dec!(7.97));
        assert_eq!(d.changes[0].your_value, Some(dec!(120)));
        assert_eq!(d.changes[1].delta, dec!(-0.50));
        assert_eq!((d.minor_added, d.minor_removed), (1, 1));
    }

    #[test]
    fn fund_dollars_follow_funds_inside_funds() {
        let xeqt = parse_any("XEQT.TO", include_str!("../fixtures/ishares_xeqt.csv"), None).unwrap();
        let xic = parse_any("XIC.TO", include_str!("../fixtures/ishares_xic.csv"), None).unwrap();
        let snaps = BTreeMap::from([("XEQT.TO".to_string(), xeqt), ("XIC.TO".to_string(), xic)]);
        let held = vec![
            Holding { symbol: "XEQT.TO".into(), name: None, value: dec!(10000), currency: None },
            Holding { symbol: "XIC.TO".into(), name: None, value: dec!(1000), currency: None },
            Holding { symbol: "AAPL".into(), name: None, value: dec!(500), currency: None },
        ];
        let d = fund_dollars(&held, &snaps, 3);
        assert_eq!(d["XEQT.TO"], dec!(10000));
        // 24.64% of XEQT plus the direct 1000.
        assert_eq!(d["XIC.TO"], dec!(2464) + dec!(1000));
        assert!(!d.contains_key("AAPL"));
        assert_eq!(fund_dollars(&held, &snaps, 1)["XIC.TO"], dec!(1000), "depth 1 stops at what you hold");
    }

    #[test]
    fn drift_is_in_percentage_points_and_keeps_buckets_that_vanished() {
        let mk = |rows: Vec<Constituent>| {
            let f = FundHoldings {
                symbol: "F".into(),
                name: None,
                as_of: ymd(2026, 1, 1),
                source: "t".into(),
                rows,
            };
            let snaps = BTreeMap::from([("F".to_string(), f)]);
            let held = vec![Holding { symbol: "F".into(), name: None, value: dec!(1000), currency: None }];
            lookthrough(&held, &snaps, &BTreeMap::new(), 3)
        };
        let sec = |t: &str, s: &str, w: Decimal| {
            let mut r = row(t, t, w);
            r.sector = Some(s.into());
            r
        };
        let a = mk(vec![sec("A", "Information Technology", dec!(60)), sec("B", "Financials", dec!(40))]);
        let b = mk(vec![sec("A", "Information Technology", dec!(70)), sec("C", "Energy", dec!(30))]);
        let d = drift(&a, &b);
        let tech = d.by_sector.iter().find(|x| x.label == "Technology").unwrap();
        assert_eq!((tech.from_pct, tech.to_pct, tech.delta), (dec!(60.0), dec!(70.0), dec!(10.0)));
        let fin = d.by_sector.iter().find(|x| x.label == "Financials").unwrap();
        assert_eq!((fin.to_pct, fin.delta), (Decimal::ZERO, dec!(-40.0)));
        assert_eq!(fin.to_value, Decimal::ZERO);
        let energy = d.by_sector.iter().find(|x| x.label == "Energy").unwrap();
        assert_eq!(energy.from_pct, Decimal::ZERO);
        assert_eq!(d.by_sector[0].label, "Technology", "largest share first");
        let sum: Decimal = d.by_sector.iter().map(|x| x.delta).sum();
        assert_eq!(sum, Decimal::ZERO);
    }
}
