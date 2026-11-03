//! Added vs grown: how much of the change in net worth came from you (saving,
//! contributions, debt paid down) and how much from markets, FX and property values.
//! Pure. Built by summing `changes` over every pair of neighbouring points, so each
//! step uses the finest split the ledger has.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate};
use ledger::{Group, Ledger};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{changes, Changes, Market, Point};

/// One point of the view. `added` is the running total since the first point;
/// whatever else moved net worth (`net_worth - start - added`) is growth.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GrowthPoint {
    pub date: NaiveDate,
    pub net_worth: Decimal,
    pub added: Decimal,
}

/// The part of one period's change that you put in: units bought or sold, cash
/// balances you typed, and debt balances paid down. Property values, prices and FX
/// are left to growth.
pub fn added(c: &Changes) -> Decimal {
    let cash: Decimal = c
        .accounts
        .iter()
        .filter(|a| !a.liability && a.group != Group::Property)
        .filter_map(|a| a.balance)
        .sum();
    c.trades + cash + c.debt
}

pub fn growth(l: &Ledger, m: &Market, series: &[Point]) -> Vec<GrowthPoint> {
    let mut out = Vec::with_capacity(series.len());
    let mut total = Decimal::ZERO;
    for (i, p) in series.iter().enumerate() {
        if i > 0 {
            total += added(&changes(l, m, series[i - 1].date, p.date));
        }
        out.push(GrowthPoint { date: p.date, net_worth: p.net_worth, added: total });
    }
    out
}

/// One calendar month of one account: its change split into what you added and what grew.
/// Signs are contributions to net worth, so paying down a debt is a positive `added`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonthSplit {
    /// `YYYY-MM`.
    pub month: String,
    pub added: Decimal,
    pub grown: Decimal,
}

/// Month by month for one account, oldest first; months with no recorded change are left out.
/// A property's value changes are growth; everything else typed as a balance, and every
/// trade, is added.
pub fn account_months(l: &Ledger, m: &Market, series: &[Point], account: &str) -> Vec<MonthSplit> {
    let mut months: BTreeMap<(i32, u32), (Decimal, Decimal)> = BTreeMap::new();
    for w in series.windows(2) {
        let c = changes(l, m, w[0].date, w[1].date);
        let Some(a) = c.accounts.iter().find(|a| a.account == account) else { continue };
        let added = if a.group == Group::Property {
            Decimal::ZERO
        } else {
            a.trades.unwrap_or_default() + a.balance.unwrap_or_default()
        };
        let e = months.entry((w[1].date.year(), w[1].date.month())).or_default();
        e.0 += added;
        e.1 += a.change - added;
    }
    months
        .into_iter()
        .map(|((y, mo), (added, grown))| MonthSplit { month: format!("{y}-{mo:02}"), added, grown })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net_worth_series;
    use ledger::{Account, AccountKind, Mutation, Position};
    use rust_decimal_macros::dec;
    use std::collections::BTreeMap;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn account(id: &str, kind: AccountKind) -> Account {
        Account {
            id: id.into(),
            name: id.into(),
            institution: None,
            kind,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        }
    }

    #[test]
    fn saving_and_paydown_are_added_while_property_and_prices_are_growth() {
        let mut tfsa = account("tfsa", AccountKind::Tfsa);
        tfsa.positions = Some(vec![Position { symbol: "XEQT.TO".into(), units: dec!(10), book_cost: None }]);
        let (d0, d1) = (ymd(2026, 1, 31), ymd(2026, 2, 28));
        let mut ms = vec![
            Mutation::UpsertAccount { account: account("chq", AccountKind::Chequing) },
            Mutation::UpsertAccount { account: account("visa", AccountKind::CreditCard) },
            Mutation::UpsertAccount { account: account("condo", AccountKind::Property) },
            Mutation::UpsertAccount { account: tfsa },
        ];
        for (id, date, value) in [
            ("chq", d0, dec!(1000)),
            ("visa", d0, dec!(500)),
            ("condo", d0, dec!(400000)),
            ("chq", d1, dec!(1300)),
            ("visa", d1, dec!(200)),
            ("condo", d1, dec!(410000)),
        ] {
            ms.push(Mutation::SetBalance { date, account: id.into(), value });
        }
        let l = Ledger::starter("CAD").apply_many(ms).unwrap();
        let prices =
            BTreeMap::from([("XEQT.TO".to_string(), BTreeMap::from([(d0, dec!(30)), (d1, dec!(33))]))]);
        let m = Market::merge(&l, &prices, &BTreeMap::new(), &BTreeMap::new());
        let g = growth(&l, &m, &net_worth_series(&l, &m));
        let (first, last) = (g.first().unwrap(), g.last().unwrap());
        assert_eq!(first.added, Decimal::ZERO);
        // Chequing +300 and the card paid down 300 are added; the condo +10,000 and
        // XEQT +30 are growth.
        assert_eq!(last.added, dec!(600));
        assert_eq!(last.net_worth - first.net_worth - last.added, dec!(10030));

        let visa = account_months(&l, &m, &net_worth_series(&l, &m), "visa");
        assert_eq!(
            visa,
            vec![MonthSplit { month: "2026-02".into(), added: dec!(300), grown: Decimal::ZERO }]
        );
        let condo = account_months(&l, &m, &net_worth_series(&l, &m), "condo");
        assert_eq!(condo[0].added, Decimal::ZERO);
        assert_eq!(condo[0].grown, dec!(10000));
        let tfsa = account_months(&l, &m, &net_worth_series(&l, &m), "tfsa");
        assert_eq!((tfsa[0].added, tfsa[0].grown), (Decimal::ZERO, dec!(30)));
    }
}
