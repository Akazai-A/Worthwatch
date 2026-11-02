//! The Portfolio screen's numbers: every position across every account, grouped
//! by security, in base currency, with weight, gain against book cost and day change.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ledger::{Ledger, Symbol};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{units_at, Market, Source};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AccountSlice {
    pub account: String,
    pub name: String,
    pub units: Decimal,
    /// Base currency.
    pub value: Option<Decimal>,
    /// Account currency.
    pub book_cost: Option<Decimal>,
    /// Base currency: value minus book cost.
    pub gain: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SecurityRow {
    pub symbol: Symbol,
    /// Currency the holding accounts are denominated in (the first one, if they differ).
    pub currency: String,
    pub units: Decimal,
    pub price: Option<Decimal>,
    pub price_date: Option<NaiveDate>,
    pub price_source: Option<Source>,
    /// Base currency. `None` when there is no price or no FX rate.
    pub value: Option<Decimal>,
    /// Percent of the portfolio total, one decimal.
    pub weight_pct: Option<Decimal>,
    pub gain: Option<Decimal>,
    pub gain_pct: Option<Decimal>,
    /// Base currency: units x (latest fetched close - previous fetched close). `None`
    /// until two fetched closes exist; typed prices never feed it.
    pub day_change: Option<Decimal>,
    pub missing_fx: bool,
    pub accounts: Vec<AccountSlice>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Slice {
    pub key: String,
    pub label: String,
    pub value: Decimal,
    pub pct: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Portfolio {
    /// Base currency, every priced position.
    pub total: Decimal,
    pub day_change: Option<Decimal>,
    pub book_cost: Option<Decimal>,
    pub gain: Option<Decimal>,
    pub gain_pct: Option<Decimal>,
    pub by_currency: Vec<Slice>,
    pub by_account: Vec<Slice>,
    pub securities: Vec<SecurityRow>,
    /// Symbols without any price.
    pub unpriced: Vec<Symbol>,
}

fn pct(part: Decimal, whole: Decimal) -> Option<Decimal> {
    (!whole.is_zero()).then(|| (part / whole * Decimal::ONE_HUNDRED).round_dp(1))
}

fn slices(parts: BTreeMap<String, (String, Decimal)>, total: Decimal) -> Vec<Slice> {
    let mut v: Vec<Slice> = parts
        .into_iter()
        .map(|(key, (label, value))| Slice { key, label, value, pct: pct(value, total).unwrap_or_default() })
        .collect();
    v.sort_by_key(|s| std::cmp::Reverse(s.value));
    v
}

pub fn portfolio(l: &Ledger, m: &Market, today: NaiveDate) -> Portfolio {
    let base = &l.settings.base_currency;
    let mut rows: BTreeMap<Symbol, SecurityRow> = BTreeMap::new();
    let mut by_currency: BTreeMap<String, (String, Decimal)> = BTreeMap::new();
    let mut by_account: BTreeMap<String, (String, Decimal)> = BTreeMap::new();
    let mut total = Decimal::ZERO;
    let mut day_change: Option<Decimal> = None;
    let mut book_total: Option<Decimal> = None;
    let mut unpriced = Vec::new();

    for a in &l.accounts {
        let Some(positions) = &a.positions else { continue };
        let rate = m.rate_at(base, &a.currency, today);
        for p in positions {
            let units = units_at(l, a, &p.symbol, today);
            if units.is_zero() {
                continue;
            }
            let px = m.price_at(&p.symbol, today);
            let row = rows.entry(p.symbol.clone()).or_insert_with(|| SecurityRow {
                symbol: p.symbol.clone(),
                currency: a.currency.clone(),
                units: Decimal::ZERO,
                price: px.map(|x| x.price),
                price_date: px.map(|x| x.date),
                price_source: px.map(|x| x.source),
                value: None,
                weight_pct: None,
                gain: None,
                gain_pct: None,
                day_change: None,
                missing_fx: false,
                accounts: Vec::new(),
            });
            row.units += units;
            let native = px.map(|x| units * x.price);
            let value = match (native, rate) {
                (Some(n), Some(r)) => Some(n * r.rate),
                _ => None,
            };
            if px.is_none() && !unpriced.contains(&p.symbol) {
                unpriced.push(p.symbol.clone());
            }
            if px.is_some() && rate.is_none() {
                row.missing_fx = true;
            }
            let gain = match (native, p.book_cost, rate) {
                (Some(n), Some(b), Some(r)) => Some((n - b) * r.rate),
                _ => None,
            };
            if let Some(v) = value {
                total += v;
                row.value = Some(row.value.unwrap_or_default() + v);
                let e = by_currency
                    .entry(a.currency.clone())
                    .or_insert_with(|| (a.currency.clone(), Decimal::ZERO));
                e.1 += v;
                let e = by_account.entry(a.id.clone()).or_insert_with(|| (a.name.clone(), Decimal::ZERO));
                e.1 += v;
                if let (Some(b), Some(r)) = (p.book_cost, rate) {
                    book_total = Some(book_total.unwrap_or_default() + b * r.rate);
                }
                if let (Some(prev), Some(cur), Some(r)) = (
                    m.previous_fetched_price(&p.symbol, today),
                    m.latest_fetched_price(&p.symbol, today),
                    rate,
                ) {
                    let d = units * (cur.price - prev.price) * r.rate;
                    day_change = Some(day_change.unwrap_or_default() + d);
                    row.day_change = Some(row.day_change.unwrap_or_default() + d);
                }
            }
            if let Some(g) = gain {
                row.gain = Some(row.gain.unwrap_or_default() + g);
            }
            row.accounts.push(AccountSlice {
                account: a.id.clone(),
                name: a.name.clone(),
                units,
                value,
                book_cost: p.book_cost,
                gain,
            });
        }
    }

    let mut securities: Vec<SecurityRow> = rows.into_values().collect();
    for r in &mut securities {
        r.weight_pct = r.value.and_then(|v| pct(v, total));
        let book: Option<Decimal> = {
            let bs: Vec<Decimal> = r.accounts.iter().filter_map(|s| s.book_cost).collect();
            (!bs.is_empty() && bs.len() == r.accounts.len()).then(|| bs.iter().sum())
        };
        r.gain_pct = match (r.gain, book, m.rate_at(base, &r.currency, today)) {
            (Some(g), Some(b), Some(rate)) if !b.is_zero() => pct(g, b * rate.rate),
            _ => None,
        };
    }
    securities.sort_by(|a, b| {
        b.value.unwrap_or_default().cmp(&a.value.unwrap_or_default()).then(a.symbol.cmp(&b.symbol))
    });

    let gain = book_total.map(|b| total - b);
    Portfolio {
        total,
        day_change: day_change.map(|d| d.round_dp(2)),
        book_cost: book_total,
        gain,
        gain_pct: match (gain, book_total) {
            (Some(g), Some(b)) if !b.is_zero() => pct(g, b),
            _ => None,
        },
        by_currency: slices(by_currency, total),
        by_account: slices(by_account, total),
        securities,
        unpriced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{Account, AccountKind, Position};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn acct(id: &str, cur: &str, positions: Vec<Position>) -> Account {
        Account {
            id: id.into(),
            name: id.to_uppercase(),
            institution: None,
            kind: AccountKind::Tfsa,
            currency: cur.into(),
            tags: vec![],
            positions: Some(positions),
        }
    }

    fn pos(symbol: &str, units: Decimal, book: Option<Decimal>) -> Position {
        Position { symbol: symbol.into(), units, book_cost: book }
    }

    #[test]
    fn merges_the_same_symbol_across_accounts_and_converts_usd() {
        let mut l = Ledger::starter("CAD");
        l.settings.fx.insert("USD".into(), dec!(1.40));
        l.accounts = vec![
            acct(
                "tfsa",
                "CAD",
                vec![pos("XEQT.TO", dec!(100), Some(dec!(3000))), pos("NOPX", dec!(5), None)],
            ),
            acct("fhsa", "CAD", vec![pos("XEQT.TO", dec!(50), Some(dec!(1700)))]),
            acct("rrsp", "USD", vec![pos("VTI", dec!(10), Some(dec!(2000)))]),
        ];
        let fetched = BTreeMap::from([
            (
                "XEQT.TO".to_string(),
                BTreeMap::from([(ymd(2026, 1, 1), dec!(33)), (ymd(2026, 1, 2), dec!(34))]),
            ),
            ("VTI".to_string(), BTreeMap::from([(ymd(2026, 1, 2), dec!(250))])),
        ]);
        let m = Market::merge(&l, &fetched, &BTreeMap::new(), &BTreeMap::new());
        let p = portfolio(&l, &m, ymd(2026, 1, 2));
        // 150 * 34 = 5100 CAD; 10 * 250 * 1.40 = 3500 CAD
        assert_eq!(p.total, dec!(5100) + dec!(3500));
        assert_eq!(p.unpriced, vec!["NOPX".to_string()]);
        assert_eq!(p.securities[0].symbol, "XEQT.TO");
        assert_eq!(p.securities[0].units, dec!(150));
        assert_eq!(p.securities[0].accounts.len(), 2);
        assert_eq!(p.securities[0].weight_pct, Some(dec!(59.3)));
        assert_eq!(p.securities[0].gain, Some(dec!(5100) - dec!(4700)));
        assert_eq!(p.securities[0].day_change, Some(dec!(150)));
        assert_eq!(p.day_change, Some(dec!(150)));
        let vti = p.securities.iter().find(|s| s.symbol == "VTI").unwrap();
        assert_eq!(vti.gain, Some((dec!(2500) - dec!(2000)) * dec!(1.40)));
        assert_eq!(vti.gain_pct, Some(dec!(25.0)));
        assert_eq!(p.by_currency[0].key, "CAD");
        assert_eq!(p.by_currency[1].pct, dec!(40.7));
        assert_eq!(p.by_account.len(), 3);
        assert_eq!(p.gain, Some(dec!(8600) - (dec!(4700) + dec!(2800))));
    }

    #[test]
    fn day_change_ignores_typed_prices() {
        let mut l = Ledger::starter("CAD");
        l.accounts = vec![acct("tfsa", "CAD", vec![pos("XEQT.TO", dec!(100), None)])];
        // Fetched: 33 on Jan 1, 34 on Jan 2. Typed month-end between them, way off.
        l.prices.insert("XEQT.TO".into(), BTreeMap::from([(ymd(2026, 1, 1), dec!(20))]));
        let fetched = BTreeMap::from([(
            "XEQT.TO".to_string(),
            BTreeMap::from([(ymd(2025, 12, 31), dec!(33)), (ymd(2026, 1, 2), dec!(34))]),
        )]);
        let m = Market::merge(&l, &fetched, &BTreeMap::new(), &BTreeMap::new());
        let p = portfolio(&l, &m, ymd(2026, 1, 2));
        assert_eq!(p.securities[0].day_change, Some(dec!(100)));
        // A typed price as the latest point: no day change rather than a wrong one.
        let p = portfolio(&l, &m, ymd(2026, 1, 1));
        assert_eq!(p.securities[0].price, Some(dec!(20)));
        assert_eq!(p.securities[0].day_change, None);
    }

    #[test]
    fn missing_fx_is_flagged_and_excluded_never_zero() {
        let mut l = Ledger::starter("CAD");
        l.accounts = vec![acct("rrsp", "USD", vec![pos("VTI", dec!(10), None)])];
        let fetched = BTreeMap::from([("VTI".to_string(), BTreeMap::from([(ymd(2026, 1, 2), dec!(250))]))]);
        let m = Market::merge(&l, &fetched, &BTreeMap::new(), &BTreeMap::new());
        let p = portfolio(&l, &m, ymd(2026, 1, 2));
        assert!(p.securities[0].missing_fx);
        assert_eq!(p.securities[0].value, None);
        assert_eq!(p.total, Decimal::ZERO);
    }
}
