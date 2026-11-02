//! Everything here is a pure function of a [`Ledger`], a [`Market`] and a date.
//! No IO, no clock.

pub mod diff;
pub mod market;
pub mod portfolio;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ledger::{Account, Group, Ledger, Symbol};
use rust_decimal::Decimal;
use serde::Serialize;

pub use diff::{changes, AccountDiff, Changes, HoldingDiff};
pub use market::{Market, Price, Rate, Source};
pub use portfolio::{portfolio, AccountSlice, Portfolio, SecurityRow, Slice};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Valuation {
    /// In base currency. Liabilities are positive here; sign is applied in totals.
    pub value: Decimal,
    pub as_of: Option<NaiveDate>,
    pub missing_fx: bool,
    pub missing_price: bool,
}

/// Convert `amount` in `currency` to the base currency on `date`. `None` if no rate.
pub fn to_base(l: &Ledger, m: &Market, amount: Decimal, currency: &str, date: NaiveDate) -> Option<Decimal> {
    m.rate_at(&l.settings.base_currency, currency, date).map(|r| amount * r.rate)
}

/// Latest balance snapshot for an account on or before `date`.
pub fn balance_at(l: &Ledger, account: &str, date: NaiveDate) -> Option<(NaiveDate, Decimal)> {
    l.snapshots.range(..=date).rev().find_map(|(d, day)| day.get(account).map(|v| (*d, *v)))
}

/// Units held of `symbol` in `account` on `date`: the current positions rolled
/// back through later trades, so history is valued with the units held then.
pub fn units_at(l: &Ledger, a: &Account, symbol: &str, date: NaiveDate) -> Decimal {
    let now = a
        .positions
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .find(|p| p.symbol == symbol)
        .map_or(Decimal::ZERO, |p| p.units);
    let later: Decimal = l
        .position_changes
        .iter()
        .filter(|c| c.account == a.id && c.symbol == symbol && c.date > date)
        .map(|c| c.units)
        .sum();
    now - later
}

/// Every symbol the account holds now or ever traded, sorted. A position sold to zero
/// leaves the account's list but still counts on dates it was held.
pub fn symbols_ever(l: &Ledger, a: &Account) -> Vec<Symbol> {
    let mut v: Vec<Symbol> = a.positions.as_deref().unwrap_or(&[]).iter().map(|p| p.symbol.clone()).collect();
    v.extend(l.position_changes.iter().filter(|c| c.account == a.id).map(|c| c.symbol.clone()));
    v.sort();
    v.dedup();
    v
}

/// Native-currency value of a position account on `date`: sum of units x latest price.
/// Returns (total, oldest price date, any holding without a price).
pub fn positions_native(
    l: &Ledger,
    m: &Market,
    a: &Account,
    date: NaiveDate,
) -> (Decimal, Option<NaiveDate>, bool) {
    let mut total = Decimal::ZERO;
    let mut oldest: Option<NaiveDate> = None;
    let mut missing = false;
    for symbol in symbols_ever(l, a) {
        let units = units_at(l, a, &symbol, date);
        if units.is_zero() {
            continue;
        }
        match m.price_at(&symbol, date) {
            Some(px) => {
                total += units * px.price;
                oldest = Some(oldest.map_or(px.date, |o| o.min(px.date)));
            }
            None => missing = true,
        }
    }
    (total, oldest, missing)
}

pub fn value_account(l: &Ledger, m: &Market, a: &Account, date: NaiveDate) -> Valuation {
    let (native, as_of, missing_price) = match &a.positions {
        Some(_) => positions_native(l, m, a, date),
        None => match balance_at(l, &a.id, date) {
            Some((d, v)) => (v, Some(d), false),
            None => (Decimal::ZERO, None, false),
        },
    };
    let (value, missing_fx) = match to_base(l, m, native, &a.currency, date) {
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
    /// Per group, in base currency. Debt is positive (amount owed).
    pub by_group: BTreeMap<Group, Decimal>,
}

pub fn totals(l: &Ledger, m: &Market, date: NaiveDate) -> Totals {
    let mut t = Totals {
        assets: Decimal::ZERO,
        liabilities: Decimal::ZERO,
        net_worth: Decimal::ZERO,
        by_group: BTreeMap::new(),
    };
    for a in &l.accounts {
        let v = value_account(l, m, a, date).value;
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
    pub by_group: BTreeMap<Group, Decimal>,
}

/// Every date on which anything changed (snapshot, price, FX rate or trade), sorted.
/// Only prices of symbols actually held and rates of currencies actually used count.
pub fn change_dates(l: &Ledger, m: &Market) -> Vec<NaiveDate> {
    let mut dates: Vec<NaiveDate> = l.snapshots.keys().copied().collect();
    for sym in l.symbols() {
        if let Some(days) = m.prices.get(&sym) {
            dates.extend(days.keys().copied());
        }
    }
    for cur in l.foreign_currencies() {
        if let Some(days) = m.fx.get(&cur) {
            dates.extend(days.keys().copied());
        }
    }
    dates.extend(l.position_changes.iter().map(|c| c.date));
    dates.sort();
    dates.dedup();
    dates
}

/// One point per change date, carrying values forward in between.
pub fn net_worth_series(l: &Ledger, m: &Market) -> Vec<Point> {
    change_dates(l, m)
        .into_iter()
        .map(|d| {
            let t = totals(l, m, d);
            Point {
                date: d,
                net_worth: t.net_worth,
                assets: t.assets,
                liabilities: t.liabilities,
                by_group: t.by_group,
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HistoryPoint {
    pub date: NaiveDate,
    pub value: Decimal,
}

/// One account's value (base currency) on each change date it existed for.
pub fn account_history(l: &Ledger, m: &Market, a: &Account) -> Vec<HistoryPoint> {
    change_dates(l, m)
        .into_iter()
        .filter_map(|d| {
            let v = value_account(l, m, a, d);
            v.as_of.map(|_| HistoryPoint { date: d, value: v.value })
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Delta {
    pub from: NaiveDate,
    pub amount: Decimal,
    /// Percent of the starting value; `None` when the start was zero.
    pub pct: Option<Decimal>,
}

/// Change in net worth between the series value at or before `from` and the latest point.
/// `None` when nothing was recorded at or before `from`.
pub fn delta_since(series: &[Point], from: NaiveDate) -> Option<Delta> {
    let last = series.last()?;
    let start = series.iter().rev().find(|p| p.date <= from)?;
    if start.date == last.date {
        return None;
    }
    let amount = last.net_worth - start.net_worth;
    let pct = if start.net_worth.is_zero() {
        None
    } else {
        Some((amount / start.net_worth.abs() * Decimal::ONE_HUNDRED).round_dp(1))
    };
    Some(Delta { from: start.date, amount, pct })
}

pub fn days_stale(l: &Ledger, m: &Market, a: &Account, today: NaiveDate) -> Option<i64> {
    value_account(l, m, a, today).as_of.map(|d| (today - d).num_days())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{AccountKind, Mutation, Position};
    use proptest::prelude::*;
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn example() -> Ledger {
        ledger::io::parse(include_str!("../../../data/ledger.example.yaml")).unwrap()
    }

    fn account(id: &str, kind: AccountKind, currency: &str) -> Account {
        Account {
            id: id.into(),
            name: id.into(),
            institution: None,
            kind,
            currency: currency.into(),
            tags: vec![],
            positions: None,
        }
    }

    #[test]
    fn net_worth_is_assets_minus_liabilities() {
        let l = example();
        let m = Market::from_ledger(&l);
        let t = totals(&l, &m, ymd(2026, 10, 31));
        assert_eq!(t.net_worth, t.assets - t.liabilities);
        // condo carried forward from Oct 1, visa updated Oct 31, TFSA = 412.5 * 34.12
        assert_eq!(t.assets, dec!(640000) + dec!(5102.10) + dec!(412.5) * dec!(34.12));
        assert_eq!(t.liabilities, dec!(640.12));
        assert_eq!(t.by_group[&Group::Debt], dec!(640.12));
        assert_eq!(t.by_group[&Group::Property], dec!(640000));
    }

    #[test]
    fn series_carries_forward_and_is_sorted() {
        let l = example();
        let m = Market::from_ledger(&l);
        let s = net_worth_series(&l, &m);
        assert!(s.len() >= 2);
        assert!(s.windows(2).all(|w| w[0].date < w[1].date));
        // Oct 15 (a trade, no snapshot) still carries the Oct 1 balances.
        let oct15 = s.iter().find(|p| p.date == ymd(2026, 10, 15)).unwrap();
        assert_eq!(oct15.by_group[&Group::Cash], dec!(4210.55));
    }

    #[test]
    fn positions_are_rolled_back_through_later_trades() {
        let l = example();
        let tfsa = l.account("ws-tfsa").unwrap();
        // 14 units bought Oct 15, so before that the account held 398.5.
        assert_eq!(units_at(&l, tfsa, "XEQT.TO", ymd(2026, 10, 1)), dec!(398.5));
        assert_eq!(units_at(&l, tfsa, "XEQT.TO", ymd(2026, 10, 15)), dec!(412.5));
    }

    #[test]
    fn usd_position_is_converted_with_manual_fx() {
        let mut l = Ledger::starter("CAD");
        l.settings.fx.insert("USD".into(), dec!(1.38));
        let mut tfsa = account("tfsa", AccountKind::Tfsa, "USD");
        tfsa.positions = Some(vec![Position { symbol: "VTI".into(), units: dec!(10), book_cost: None }]);
        l = l
            .apply(Mutation::UpsertAccount { account: tfsa })
            .unwrap()
            .apply(Mutation::SetPrice { symbol: "VTI".into(), date: ymd(2026, 1, 1), price: dec!(250) })
            .unwrap();
        let m = Market::from_ledger(&l);
        let v = value_account(&l, &m, l.account("tfsa").unwrap(), ymd(2026, 6, 1));
        assert_eq!(v.value, dec!(10) * dec!(250) * dec!(1.38));
        assert!(!v.missing_fx);

        l.settings.fx.clear();
        let m = Market::from_ledger(&l);
        let v = value_account(&l, &m, l.account("tfsa").unwrap(), ymd(2026, 6, 1));
        assert!(v.missing_fx);
        assert_eq!(v.value, Decimal::ZERO);
    }

    #[test]
    fn fetched_price_and_fx_are_used_and_manual_price_wins_on_its_date() {
        let mut l = Ledger::starter("CAD");
        l.settings.fx.insert("USD".into(), dec!(1.30)); // stale manual rate
        let mut rrsp = account("rrsp", AccountKind::Rrsp, "USD");
        rrsp.positions = Some(vec![Position { symbol: "VTI".into(), units: dec!(10), book_cost: None }]);
        l = l
            .apply(Mutation::UpsertAccount { account: rrsp })
            .unwrap()
            .apply(Mutation::SetPrice { symbol: "VTI".into(), date: ymd(2026, 3, 1), price: dec!(200) })
            .unwrap();
        let fetched_prices = BTreeMap::from([(
            "VTI".to_string(),
            BTreeMap::from([(ymd(2026, 3, 1), dec!(199)), (ymd(2026, 3, 2), dec!(210))]),
        )]);
        let fetched_fx =
            BTreeMap::from([("USD".to_string(), BTreeMap::from([(ymd(2026, 3, 2), dec!(1.40))]))]);
        let m = Market::merge(&l, &fetched_prices, &fetched_fx, &BTreeMap::new());
        let a = l.account("rrsp").unwrap();
        // Mar 1: manual 200 beats fetched 199 on the same date; no fetched FX yet -> manual 1.30.
        assert_eq!(m.price_at("VTI", ymd(2026, 3, 1)).unwrap().source, Source::Manual);
        assert_eq!(value_account(&l, &m, a, ymd(2026, 3, 1)).value, dec!(10) * dec!(200) * dec!(1.30));
        // Mar 2: fetched price and fetched rate.
        let v = value_account(&l, &m, a, ymd(2026, 3, 2));
        assert_eq!(v.value, dec!(10) * dec!(210) * dec!(1.40));
        assert_eq!(v.as_of, Some(ymd(2026, 3, 2)));
        assert_eq!(m.rate_at("CAD", "USD", ymd(2026, 3, 2)).unwrap().date, Some(ymd(2026, 3, 2)));
        // The series has a point for the fetched FX date and the fetched price date.
        let dates = change_dates(&l, &m);
        assert_eq!(dates, vec![ymd(2026, 3, 1), ymd(2026, 3, 2)]);
    }

    #[test]
    fn prices_of_symbols_not_held_do_not_add_series_points() {
        let l = example();
        let fetched = BTreeMap::from([("ZZZ".to_string(), BTreeMap::from([(ymd(2020, 1, 1), dec!(1))]))]);
        let m = Market::merge(&l, &fetched, &BTreeMap::new(), &BTreeMap::new());
        assert!(!change_dates(&l, &m).contains(&ymd(2020, 1, 1)));
    }

    #[test]
    fn missing_price_is_flagged_not_zeroed_silently() {
        let mut l = Ledger::starter("CAD");
        let mut tfsa = account("tfsa", AccountKind::Tfsa, "CAD");
        tfsa.positions = Some(vec![Position { symbol: "X".into(), units: dec!(1), book_cost: None }]);
        l = l.apply(Mutation::UpsertAccount { account: tfsa }).unwrap();
        let m = Market::from_ledger(&l);
        let v = value_account(&l, &m, l.account("tfsa").unwrap(), ymd(2026, 6, 1));
        assert!(v.missing_price);
        assert_eq!(v.as_of, None);
    }

    #[test]
    fn two_checkins_make_two_points() {
        let l = Ledger::starter("CAD")
            .apply_many([
                Mutation::UpsertAccount { account: account("chq", AccountKind::Chequing, "CAD") },
                Mutation::UpsertAccount { account: account("visa", AccountKind::CreditCard, "CAD") },
                Mutation::SetBalance { date: ymd(2026, 9, 30), account: "chq".into(), value: dec!(1000) },
                Mutation::SetBalance { date: ymd(2026, 9, 30), account: "visa".into(), value: dec!(300) },
                Mutation::SetBalance { date: ymd(2026, 10, 31), account: "chq".into(), value: dec!(1500) },
            ])
            .unwrap();
        let m = Market::from_ledger(&l);
        let s = net_worth_series(&l, &m);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].net_worth, dec!(700));
        assert_eq!(s[1].net_worth, dec!(1200)); // visa carried forward
        let d = delta_since(&s, ymd(2026, 10, 1)).unwrap();
        assert_eq!(d.from, ymd(2026, 9, 30));
        assert_eq!(d.amount, dec!(500));
        assert_eq!(d.pct, Some(dec!(71.4)));
        assert_eq!(delta_since(&s, ymd(2026, 1, 1)), None);
        assert_eq!(delta_since(&s, ymd(2026, 10, 31)), None);
    }

    #[test]
    fn account_history_starts_at_first_snapshot() {
        let l = example();
        let m = Market::from_ledger(&l);
        let h = account_history(&l, &m, l.account("visa").unwrap());
        assert_eq!(h.first().unwrap().date, ymd(2026, 10, 1));
        assert_eq!(h.last().unwrap().value, dec!(640.12));
        assert_eq!(h.len(), 3); // Oct 1, Oct 15 (carried), Oct 31
    }

    fn money() -> impl Strategy<Value = Decimal> {
        (0i64..10_000_000).prop_map(|c| Decimal::new(c, 2))
    }

    proptest! {
        #[test]
        fn invariants_hold_for_random_balances(
            chq in money(), tfsa in money(), visa in money(), mortgage in money()
        ) {
            let l = Ledger::starter("CAD")
                .apply_many([
                    Mutation::UpsertAccount { account: account("chq", AccountKind::Chequing, "CAD") },
                    Mutation::UpsertAccount { account: account("tfsa", AccountKind::Tfsa, "CAD") },
                    Mutation::UpsertAccount { account: account("visa", AccountKind::CreditCard, "CAD") },
                    Mutation::UpsertAccount { account: account("mort", AccountKind::Mortgage, "CAD") },
                    Mutation::SetBalance { date: ymd(2026, 1, 1), account: "chq".into(), value: chq },
                    Mutation::SetBalance { date: ymd(2026, 1, 1), account: "tfsa".into(), value: tfsa },
                    Mutation::SetBalance { date: ymd(2026, 1, 1), account: "visa".into(), value: visa },
                    Mutation::SetBalance { date: ymd(2026, 2, 1), account: "mort".into(), value: mortgage },
                ])
                .unwrap();
            let m = Market::from_ledger(&l);
            let t = totals(&l, &m, ymd(2026, 3, 1));
            prop_assert_eq!(t.assets, chq + tfsa);
            prop_assert_eq!(t.liabilities, visa + mortgage);
            prop_assert_eq!(t.net_worth, t.assets - t.liabilities);
            let groups: Decimal = t.by_group.iter()
                .map(|(g, v)| if *g == Group::Debt { -*v } else { *v }).sum();
            prop_assert_eq!(groups, t.net_worth);
            // Carry-forward: Feb 1 only added the mortgage.
            let jan = totals(&l, &m, ymd(2026, 1, 15));
            prop_assert_eq!(jan.net_worth, chq + tfsa - visa);
        }

        /// Valuation with a fetched price equals valuation with the same manual price:
        /// the source changes nothing about the arithmetic.
        #[test]
        fn fetched_and_manual_prices_value_identically(units in 1i64..100_000, cents in 1i64..1_000_000) {
            let units = Decimal::new(units, 2);
            let px = Decimal::new(cents, 2);
            let mut tfsa = account("tfsa", AccountKind::Tfsa, "CAD");
            tfsa.positions = Some(vec![Position { symbol: "X".into(), units, book_cost: None }]);
            let base = Ledger::starter("CAD").apply(Mutation::UpsertAccount { account: tfsa }).unwrap();
            let manual = base.apply(Mutation::SetPrice { symbol: "X".into(), date: ymd(2026, 1, 1), price: px }).unwrap();
            let fetched = BTreeMap::from([("X".to_string(), BTreeMap::from([(ymd(2026, 1, 1), px)]))]);
            let vm = value_account(&manual, &Market::from_ledger(&manual), manual.account("tfsa").unwrap(), ymd(2026, 2, 1));
            let vf = value_account(&base, &Market::merge(&base, &fetched, &BTreeMap::new(), &BTreeMap::new()), base.account("tfsa").unwrap(), ymd(2026, 2, 1));
            prop_assert_eq!(vm.value, vf.value);
            prop_assert_eq!(vm.value, units * px);
            prop_assert!(!vf.missing_price);
        }
    }
}
