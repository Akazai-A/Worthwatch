//! What changed between two dates: net worth, every account and every holding,
//! with each investment move split into market, trades and FX. Pure: a ledger, a
//! market and two dates in; exact decimals out. The three parts of a holding's
//! change add up to the change itself, so nothing is ever "unexplained".

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ledger::{Account, Group, Ledger, Symbol};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{balance_at, symbols_ever, units_at, value_account, Market};

fn pct(part: Decimal, whole: Decimal) -> Option<Decimal> {
    (!whole.is_zero()).then(|| (part / whole.abs() * Decimal::ONE_HUNDRED).round_dp(1))
}

fn add(a: &mut Option<Decimal>, v: Decimal) {
    *a = Some(a.unwrap_or_default() + v);
}

/// One account then and now. Values are in base currency; `change` is the signed
/// contribution to net worth, so paying down a liability is positive.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AccountDiff {
    pub account: String,
    pub name: String,
    pub group: Group,
    pub liability: bool,
    pub currency: String,
    /// `None` when the account had no value yet on the start date.
    pub from_value: Option<Decimal>,
    pub to_value: Option<Decimal>,
    pub change: Decimal,
    /// Percent of the starting value, one decimal.
    pub pct: Option<Decimal>,
    /// Balance accounts: the typed balance moved (at the end rate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<Decimal>,
    /// Position accounts: prices moved on the units already held.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market: Option<Decimal>,
    /// Position accounts: units bought or sold, valued at the end price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trades: Option<Decimal>,
    /// Foreign-currency accounts: the rate moved on the starting value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fx: Option<Decimal>,
    /// Part of `change` the parts above do not cover (a holding priced only at one
    /// end, or an FX rate missing at one end). Zero for a fully priced account.
    pub residual: Decimal,
    pub missing_price: bool,
    pub missing_fx: bool,
}

/// One security across every account that holds it, then and now.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HoldingDiff {
    pub symbol: Symbol,
    pub currency: String,
    pub accounts: Vec<String>,
    pub from_units: Decimal,
    pub to_units: Decimal,
    pub from_price: Option<Decimal>,
    pub to_price: Option<Decimal>,
    pub from_price_date: Option<NaiveDate>,
    pub to_price_date: Option<NaiveDate>,
    pub from_value: Option<Decimal>,
    pub to_value: Option<Decimal>,
    /// Trades recorded in (from, to].
    pub trade_count: usize,
    pub market: Option<Decimal>,
    pub trades: Option<Decimal>,
    pub fx: Option<Decimal>,
    /// `None` when the holding could not be valued at one end.
    pub change: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Changes {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub days: i64,
    pub from_net_worth: Decimal,
    pub to_net_worth: Decimal,
    pub change: Decimal,
    pub pct: Option<Decimal>,
    /// Sum of the account-level parts, in base currency.
    pub market: Decimal,
    pub trades: Decimal,
    pub fx: Decimal,
    /// Balance moves on asset accounts you type (cash, property).
    pub balances: Decimal,
    /// Liabilities paid down (positive) or grown (negative).
    pub debt: Decimal,
    pub residual: Decimal,
    /// Sorted by the size of the change, largest first.
    pub accounts: Vec<AccountDiff>,
    pub holdings: Vec<HoldingDiff>,
    /// Holdings without a price at one end, so their move is in `residual`.
    pub unpriced: Vec<Symbol>,
}

fn rate(l: &Ledger, m: &Market, currency: &str, date: NaiveDate) -> Option<Decimal> {
    m.rate_at(&l.settings.base_currency, currency, date).map(|r| r.rate)
}

/// Per-position parts for one account between two dates. Everything in base currency.
struct Parts {
    market: Option<Decimal>,
    trades: Option<Decimal>,
    fx: Option<Decimal>,
}

fn position_parts(
    l: &Ledger,
    m: &Market,
    a: &Account,
    from: NaiveDate,
    to: NaiveDate,
    rows: &mut BTreeMap<Symbol, HoldingDiff>,
) -> Parts {
    let mut parts = Parts { market: None, trades: None, fx: None };
    let r0 = rate(l, m, &a.currency, from);
    let r1 = rate(l, m, &a.currency, to);
    // Current positions plus anything the trade log mentions: a position sold to zero
    // is dropped from the account but still moved money in the period.
    for symbol in &symbols_ever(l, a) {
        let u0 = units_at(l, a, symbol, from);
        let u1 = units_at(l, a, symbol, to);
        if u0.is_zero() && u1.is_zero() {
            continue;
        }
        let px0 = m.price_at(symbol, from);
        let px1 = m.price_at(symbol, to);
        let trade_count = l
            .position_changes
            .iter()
            .filter(|c| c.account == a.id && &c.symbol == symbol && c.date > from && c.date <= to)
            .count();
        let row = rows.entry(symbol.clone()).or_insert_with(|| HoldingDiff {
            symbol: symbol.clone(),
            currency: a.currency.clone(),
            accounts: Vec::new(),
            from_units: Decimal::ZERO,
            to_units: Decimal::ZERO,
            from_price: px0.map(|x| x.price),
            to_price: px1.map(|x| x.price),
            from_price_date: px0.map(|x| x.date),
            to_price_date: px1.map(|x| x.date),
            from_value: None,
            to_value: None,
            trade_count: 0,
            market: None,
            trades: None,
            fx: None,
            change: None,
        });
        row.accounts.push(a.name.clone());
        row.from_units += u0;
        row.to_units += u1;
        row.trade_count += trade_count;
        // A position that did not exist at the start needs no start price: it is all trades.
        let p0 = if u0.is_zero() { Some(Decimal::ZERO) } else { px0.map(|x| x.price) };
        // A position sold to zero is valued at the end price (carried forward); with
        // no price at all it is taken out at its start price, so the sale is all trades.
        let p1 = px1.map(|x| x.price).or(if u1.is_zero() { p0 } else { None });
        let r0 = if u0.is_zero() { Some(Decimal::ZERO) } else { r0 };
        if let (Some(v0), Some(r0)) = (px0.map(|x| u0 * x.price), r0) {
            add(&mut row.from_value, v0 * r0);
        }
        if let (Some(v1), Some(r1)) = (px1.map(|x| u1 * x.price), r1) {
            add(&mut row.to_value, v1 * r1);
        }
        let (Some(p0), Some(p1), Some(r0), Some(r1)) = (p0, p1, r0, r1) else { continue };
        let trades = (u1 - u0) * p1 * r1;
        let market = u0 * (p1 - p0) * r1;
        let fx = u0 * p0 * (r1 - r0);
        add(&mut parts.trades, trades);
        add(&mut parts.market, market);
        add(&mut parts.fx, fx);
        add(&mut row.trades, trades);
        add(&mut row.market, market);
        add(&mut row.fx, fx);
    }
    parts
}

/// Everything that changed between `from` and `to`, both inclusive of what was
/// recorded on them. Accounts with no value on either date are left out.
pub fn changes(l: &Ledger, m: &Market, from: NaiveDate, to: NaiveDate) -> Changes {
    let mut accounts = Vec::new();
    let mut rows: BTreeMap<Symbol, HoldingDiff> = BTreeMap::new();
    let mut total = Changes {
        from,
        to,
        days: (to - from).num_days(),
        from_net_worth: Decimal::ZERO,
        to_net_worth: Decimal::ZERO,
        change: Decimal::ZERO,
        pct: None,
        market: Decimal::ZERO,
        trades: Decimal::ZERO,
        fx: Decimal::ZERO,
        balances: Decimal::ZERO,
        debt: Decimal::ZERO,
        residual: Decimal::ZERO,
        accounts: Vec::new(),
        holdings: Vec::new(),
        unpriced: Vec::new(),
    };
    for a in &l.accounts {
        let v0 = value_account(l, m, a, from);
        let v1 = value_account(l, m, a, to);
        if v0.as_of.is_none() && v1.as_of.is_none() {
            continue;
        }
        let sign = if a.kind.is_liability() { -Decimal::ONE } else { Decimal::ONE };
        let from_value = v0.as_of.map(|_| v0.value);
        let to_value = v1.as_of.map(|_| v1.value);
        let change = sign * (v1.value - v0.value);
        let mut d = AccountDiff {
            account: a.id.clone(),
            name: a.name.clone(),
            group: a.kind.group(),
            liability: a.kind.is_liability(),
            currency: a.currency.clone(),
            from_value,
            to_value,
            change,
            pct: pct(v1.value - v0.value, v0.value),
            balance: None,
            market: None,
            trades: None,
            fx: None,
            residual: Decimal::ZERO,
            missing_price: v0.missing_price || v1.missing_price,
            missing_fx: v0.missing_fx || v1.missing_fx,
        };
        let explained = match &a.positions {
            Some(_) => {
                let p = position_parts(l, m, a, from, to, &mut rows);
                d.market = p.market.map(|x| x * sign);
                d.trades = p.trades.map(|x| x * sign);
                d.fx = p.fx.map(|x| x * sign);
                p.market.unwrap_or_default() + p.trades.unwrap_or_default() + p.fx.unwrap_or_default()
            }
            None => {
                let b0 = balance_at(l, &a.id, from).map_or(Decimal::ZERO, |(_, b)| b);
                let b1 = balance_at(l, &a.id, to).map_or(Decimal::ZERO, |(_, b)| b);
                let r0 = if b0.is_zero() { Some(Decimal::ZERO) } else { rate(l, m, &a.currency, from) };
                let r1 = rate(l, m, &a.currency, to);
                match (r0, r1) {
                    (Some(r0), Some(r1)) => {
                        let balance = (b1 - b0) * r1;
                        let fx = b0 * (r1 - r0);
                        d.balance = Some(balance * sign);
                        if a.currency != l.settings.base_currency {
                            d.fx = Some(fx * sign);
                        }
                        balance + fx
                    }
                    _ => Decimal::ZERO,
                }
            }
        };
        d.residual = change - sign * explained;
        total.from_net_worth += sign * v0.value;
        total.to_net_worth += sign * v1.value;
        total.market += d.market.unwrap_or_default();
        total.trades += d.trades.unwrap_or_default();
        total.fx += d.fx.unwrap_or_default();
        if d.liability {
            total.debt += d.balance.unwrap_or_default();
        } else {
            total.balances += d.balance.unwrap_or_default();
        }
        total.residual += d.residual;
        accounts.push(d);
    }
    accounts.sort_by(|a, b| b.change.abs().cmp(&a.change.abs()).then(a.name.cmp(&b.name)));
    let mut holdings: Vec<HoldingDiff> = rows.into_values().collect();
    for h in &mut holdings {
        h.change = match (h.from_value, h.to_value, h.from_units.is_zero(), h.to_units.is_zero()) {
            (Some(a), Some(b), _, _) => Some(b - a),
            (None, Some(b), true, _) => Some(b),
            (Some(a), None, _, true) => Some(-a),
            _ => None,
        };
        if h.change.is_none() {
            total.unpriced.push(h.symbol.clone());
        }
    }
    holdings.sort_by(|a, b| {
        b.change
            .unwrap_or_default()
            .abs()
            .cmp(&a.change.unwrap_or_default().abs())
            .then(a.symbol.cmp(&b.symbol))
    });
    total.change = total.to_net_worth - total.from_net_worth;
    total.pct = pct(total.change, total.from_net_worth);
    total.accounts = accounts;
    total.holdings = holdings;
    total
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{AccountKind, Mutation, Position};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
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
    fn example_ledger_change_matches_the_series() {
        let l = ledger::io::parse(include_str!("../../../data/ledger.example.yaml")).unwrap();
        let m = Market::from_ledger(&l);
        let c = changes(&l, &m, ymd(2026, 10, 1), ymd(2026, 10, 31));
        let s = crate::net_worth_series(&l, &m);
        assert_eq!(c.from_net_worth, s[0].net_worth);
        assert_eq!(c.to_net_worth, s.last().unwrap().net_worth);
        assert_eq!(c.change, c.to_net_worth - c.from_net_worth);
        // The parts cover the change: chequing +891.55, visa paid down 1179.88, condo flat,
        // TFSA: 14 units bought (trades) and no price before Oct 31 (residual).
        let visa = c.accounts.iter().find(|a| a.account == "visa").unwrap();
        assert!(visa.liability);
        assert_eq!(visa.change, dec!(1179.88));
        assert_eq!(visa.balance, Some(dec!(1179.88)));
        assert_eq!(c.debt, dec!(1179.88));
        let chq = c.accounts.iter().find(|a| a.account == "td-chq").unwrap();
        assert_eq!(chq.change, dec!(891.55));
        assert_eq!(chq.pct, Some(dec!(21.2)));
        let tfsa = c.accounts.iter().find(|a| a.account == "ws-tfsa").unwrap();
        assert_eq!(tfsa.from_value, None, "no price on Oct 1 means no value yet");
        assert_eq!(c.unpriced, vec!["XEQT.TO".to_string()]);
        assert_eq!(c.residual, tfsa.residual);
        assert_eq!(
            c.change,
            c.market + c.trades + c.fx + c.balances + c.debt + c.residual,
            "parts always sum to the change"
        );
        assert_eq!(c.accounts[0].account, "ws-tfsa", "sorted by size of change");
    }

    #[test]
    fn holding_change_splits_into_market_trades_and_fx_exactly() {
        let mut l = Ledger::starter("CAD");
        let mut rrsp = account("rrsp", AccountKind::Rrsp, "USD");
        rrsp.positions = Some(vec![Position { symbol: "VTI".into(), units: dec!(10), book_cost: None }]);
        l = l
            .apply_many([
                Mutation::UpsertAccount { account: rrsp },
                Mutation::RecordPositionChange {
                    change: ledger::PositionChange {
                        date: ymd(2026, 2, 10),
                        account: "rrsp".into(),
                        symbol: "VTI".into(),
                        units: dec!(2),
                        price: None,
                        note: None,
                    },
                },
            ])
            .unwrap();
        let prices = BTreeMap::from([(
            "VTI".to_string(),
            BTreeMap::from([(ymd(2026, 1, 31), dec!(200)), (ymd(2026, 2, 28), dec!(210))]),
        )]);
        let fx = BTreeMap::from([(
            "USD".to_string(),
            BTreeMap::from([(ymd(2026, 1, 31), dec!(1.30)), (ymd(2026, 2, 28), dec!(1.40))]),
        )]);
        let m = Market::merge(&l, &prices, &fx, &BTreeMap::new());
        let c = changes(&l, &m, ymd(2026, 1, 31), ymd(2026, 2, 28));
        let h = &c.holdings[0];
        assert_eq!((h.from_units, h.to_units), (dec!(10), dec!(12)));
        assert_eq!(h.trade_count, 1);
        // from 10*200*1.30 = 2600; to 12*210*1.40 = 3528
        assert_eq!(h.from_value, Some(dec!(2600)));
        assert_eq!(h.to_value, Some(dec!(3528)));
        assert_eq!(h.trades, Some(dec!(2) * dec!(210) * dec!(1.40)));
        assert_eq!(h.market, Some(dec!(10) * dec!(10) * dec!(1.40)));
        assert_eq!(h.fx, Some(dec!(10) * dec!(200) * dec!(0.10)));
        assert_eq!(h.change, Some(dec!(928)));
        assert_eq!(h.market.unwrap() + h.trades.unwrap() + h.fx.unwrap(), h.change.unwrap());
        let a = &c.accounts[0];
        assert_eq!((a.market, a.trades, a.fx), (h.market, h.trades, h.fx));
        assert_eq!(a.residual, Decimal::ZERO);
        assert_eq!(c.change, dec!(928));
        assert_eq!(c.pct, Some(dec!(35.7)));
        assert_eq!(c.days, 28);
    }

    #[test]
    fn new_position_is_all_trades_and_closed_position_is_all_trades() {
        let mut l = Ledger::starter("CAD");
        let mut tfsa = account("tfsa", AccountKind::Tfsa, "CAD");
        tfsa.positions = Some(vec![
            Position { symbol: "NEW".into(), units: dec!(0), book_cost: None },
            Position { symbol: "GONE".into(), units: dec!(3), book_cost: None },
        ]);
        l = l
            .apply_many([
                Mutation::UpsertAccount { account: tfsa },
                Mutation::RecordPositionChange {
                    change: ledger::PositionChange {
                        date: ymd(2026, 3, 5),
                        account: "tfsa".into(),
                        symbol: "NEW".into(),
                        units: dec!(5),
                        price: None,
                        note: None,
                    },
                },
                Mutation::RecordPositionChange {
                    change: ledger::PositionChange {
                        date: ymd(2026, 3, 6),
                        account: "tfsa".into(),
                        symbol: "GONE".into(),
                        units: dec!(-3),
                        price: None,
                        note: None,
                    },
                },
            ])
            .unwrap();
        let prices = BTreeMap::from([
            ("NEW".to_string(), BTreeMap::from([(ymd(2026, 3, 31), dec!(10))])),
            ("GONE".to_string(), BTreeMap::from([(ymd(2026, 2, 28), dec!(7))])),
        ]);
        let m = Market::merge(&l, &prices, &BTreeMap::new(), &BTreeMap::new());
        let c = changes(&l, &m, ymd(2026, 2, 28), ymd(2026, 3, 31));
        let new = c.holdings.iter().find(|h| h.symbol == "NEW").unwrap();
        assert_eq!(new.from_value, None);
        assert_eq!(new.trades, Some(dec!(50)));
        assert_eq!(new.market, Some(Decimal::ZERO));
        assert_eq!(new.change, Some(dec!(50)));
        let gone = c.holdings.iter().find(|h| h.symbol == "GONE").unwrap();
        assert_eq!(gone.from_units, dec!(3));
        assert_eq!(gone.to_units, Decimal::ZERO);
        assert_eq!(gone.change, Some(dec!(-21)));
        assert_eq!(gone.trades, Some(dec!(-21)), "sold at the end price (carried forward)");
        assert!(c.unpriced.is_empty());
        assert_eq!(c.change, dec!(29));
        assert_eq!(c.trades, dec!(29));
    }

    #[test]
    fn foreign_balance_account_splits_balance_and_fx() {
        let mut l = Ledger::starter("CAD");
        l = l
            .apply_many([
                Mutation::UpsertAccount { account: account("usd", AccountKind::Chequing, "USD") },
                Mutation::SetBalance { date: ymd(2026, 1, 1), account: "usd".into(), value: dec!(1000) },
                Mutation::SetBalance { date: ymd(2026, 2, 1), account: "usd".into(), value: dec!(1100) },
            ])
            .unwrap();
        let fx = BTreeMap::from([(
            "USD".to_string(),
            BTreeMap::from([(ymd(2026, 1, 1), dec!(1.30)), (ymd(2026, 2, 1), dec!(1.35))]),
        )]);
        let m = Market::merge(&l, &BTreeMap::new(), &fx, &BTreeMap::new());
        let c = changes(&l, &m, ymd(2026, 1, 1), ymd(2026, 2, 1));
        let a = &c.accounts[0];
        assert_eq!(a.balance, Some(dec!(100) * dec!(1.35)));
        assert_eq!(a.fx, Some(dec!(1000) * dec!(0.05)));
        assert_eq!(a.change, dec!(1485) - dec!(1300));
        assert_eq!(a.residual, Decimal::ZERO);
        assert_eq!(c.balances + c.fx, c.change);
    }

    #[test]
    fn accounts_without_values_are_skipped_and_new_ones_have_no_start() {
        let l = Ledger::starter("CAD")
            .apply_many([
                Mutation::UpsertAccount { account: account("a", AccountKind::Savings, "CAD") },
                Mutation::UpsertAccount { account: account("empty", AccountKind::Savings, "CAD") },
                Mutation::SetBalance { date: ymd(2026, 5, 1), account: "a".into(), value: dec!(500) },
            ])
            .unwrap();
        let m = Market::from_ledger(&l);
        let c = changes(&l, &m, ymd(2026, 4, 1), ymd(2026, 5, 31));
        assert_eq!(c.accounts.len(), 1);
        assert_eq!(c.accounts[0].from_value, None);
        assert_eq!(c.accounts[0].change, dec!(500));
        assert_eq!(c.accounts[0].pct, None);
        assert_eq!(c.from_net_worth, Decimal::ZERO);
    }
}
