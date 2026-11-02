//! Everything here is a pure function of a [`Ledger`] and a date. No IO, no clock.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ledger::{Account, Group, Ledger};
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Valuation {
    /// In base currency. Liabilities are positive here; sign is applied in totals.
    pub value: Decimal,
    pub as_of: Option<NaiveDate>,
    pub missing_fx: bool,
    pub missing_price: bool,
}

/// Convert `amount` in `currency` to the base currency. `None` if no rate.
pub fn to_base(l: &Ledger, amount: Decimal, currency: &str) -> Option<Decimal> {
    if currency == l.settings.base_currency {
        return Some(amount);
    }
    l.settings.fx.get(currency).map(|rate| amount * rate)
}

/// Latest price for a symbol on or before `date`.
pub fn price_at(l: &Ledger, symbol: &str, date: NaiveDate) -> Option<(NaiveDate, Decimal)> {
    l.prices.get(symbol)?.range(..=date).next_back().map(|(d, p)| (*d, *p))
}

/// Latest balance snapshot for an account on or before `date`.
pub fn balance_at(l: &Ledger, account: &str, date: NaiveDate) -> Option<(NaiveDate, Decimal)> {
    l.snapshots.range(..=date).rev().find_map(|(d, day)| day.get(account).map(|v| (*d, *v)))
}

pub fn value_account(l: &Ledger, a: &Account, date: NaiveDate) -> Valuation {
    let (native, as_of, missing_price) = match &a.positions {
        Some(positions) => {
            let mut total = Decimal::ZERO;
            let mut oldest: Option<NaiveDate> = None;
            let mut missing = false;
            for p in positions {
                match price_at(l, &p.symbol, date) {
                    Some((d, px)) => {
                        total += p.units * px;
                        oldest = Some(oldest.map_or(d, |o| o.min(d)));
                    }
                    None => missing = true,
                }
            }
            (total, oldest, missing)
        }
        None => match balance_at(l, &a.id, date) {
            Some((d, v)) => (v, Some(d), false),
            None => (Decimal::ZERO, None, false),
        },
    };
    let (value, missing_fx) = match to_base(l, native, &a.currency) {
        Some(v) => (v, false),
        None => (Decimal::ZERO, true),
    };
    Valuation { value, as_of, missing_fx, missing_price }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Totals {
    pub assets: Decimal,
    pub liabilities: Decimal,
    pub net_worth: Decimal,
    pub by_group: BTreeMap<Group, Decimal>,
}

pub fn totals(l: &Ledger, date: NaiveDate) -> Totals {
    let mut t = Totals {
        assets: Decimal::ZERO,
        liabilities: Decimal::ZERO,
        net_worth: Decimal::ZERO,
        by_group: BTreeMap::new(),
    };
    for a in &l.accounts {
        let v = value_account(l, a, date).value;
        if a.kind.is_liability() {
            t.liabilities += v;
        } else {
            t.assets += v;
        }
        *t.by_group.entry(a.kind.group()).or_default() += v;
    }
    t.net_worth = t.assets - t.liabilities;
    t
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Point {
    pub date: NaiveDate,
    pub net_worth: Decimal,
    pub assets: Decimal,
    pub liabilities: Decimal,
}

/// One point per date on which anything changed (snapshot, price or trade),
/// carrying values forward in between.
pub fn net_worth_series(l: &Ledger) -> Vec<Point> {
    let mut dates: Vec<NaiveDate> = l.snapshots.keys().copied().collect();
    dates.extend(l.prices.values().flat_map(|m| m.keys().copied()));
    dates.extend(l.position_changes.iter().map(|c| c.date));
    dates.sort();
    dates.dedup();
    dates
        .into_iter()
        .map(|d| {
            let t = totals(l, d);
            Point { date: d, net_worth: t.net_worth, assets: t.assets, liabilities: t.liabilities }
        })
        .collect()
}

pub fn days_stale(l: &Ledger, a: &Account, today: NaiveDate) -> Option<i64> {
    value_account(l, a, today).as_of.map(|d| (today - d).num_days())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn example() -> Ledger {
        ledger::io::parse(include_str!("../../../data/ledger.example.yaml")).unwrap()
    }

    #[test]
    fn net_worth_is_assets_minus_liabilities() {
        let l = example();
        let t = totals(&l, NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
        assert_eq!(t.net_worth, t.assets - t.liabilities);
        // condo carried forward from Oct 1, visa updated Oct 31, TFSA = 412.5 * 34.12
        assert_eq!(t.assets, dec!(640000) + dec!(5102.10) + dec!(412.5) * dec!(34.12));
        assert_eq!(t.liabilities, dec!(640.12));
    }

    #[test]
    fn series_carries_forward() {
        let l = example();
        let s = net_worth_series(&l);
        assert!(s.len() >= 2);
        assert!(s.windows(2).all(|w| w[0].date < w[1].date));
    }
}
