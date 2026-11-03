//! Where net worth goes from here: today's balances grown at assumed rates, plus the
//! recurring rules, month by month. Pure: the start date and assumptions come in.
//!
//! Model, kept deliberately simple so the numbers can be checked by hand:
//! - Every account compounds monthly at its annual rate / 12 (nominal), then takes its
//!   recurring amounts converted to a monthly equivalent (biweekly = 26/12 a month).
//! - A debt grows at its interest rate and each payment lowers it; it stops at zero and
//!   the month it gets there is its payoff date. Money freed by a paid-off debt is not
//!   redirected anywhere (no guessing what you would do with it).
//! - `extra_monthly` is new saving on top of the rules, invested at the investment return.
//! - The band is the same run at the investment return ± `BAND_PCT`.

use chrono::{Months, NaiveDate};
use ledger::{Account, Group, Ledger};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{to_base, value_account, Market};

/// Width of the low/high band around the investment return, in percentage points.
pub const BAND_PCT: Decimal = Decimal::from_parts(2, 0, 0, false, 0);

/// Inputs a caller may change from the screen without touching the ledger.
#[derive(Debug, Clone, PartialEq)]
pub struct Assumptions {
    pub years: u32,
    pub return_pct: Decimal,
    pub extra_monthly: Decimal,
    pub target: Option<Decimal>,
}

impl Assumptions {
    pub fn from_ledger(l: &Ledger) -> Self {
        Self {
            years: 20,
            return_pct: l.settings.projection.return_pct,
            extra_monthly: Decimal::ZERO,
            target: l.settings.projection.target,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProjPoint {
    pub date: NaiveDate,
    pub net_worth: Decimal,
    /// The same run at return - BAND_PCT and return + BAND_PCT.
    pub low: Decimal,
    pub high: Decimal,
    /// Cumulative new money put in (rules into assets + extra), so the chart can show
    /// how much of the growth is saving and how much is return.
    pub contributed: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Milestone {
    pub years: u32,
    pub date: NaiveDate,
    pub net_worth: Decimal,
    pub low: Decimal,
    pub high: Decimal,
    pub contributed: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DebtPayoff {
    pub account: String,
    pub name: String,
    /// Owed today, base currency.
    pub balance: Decimal,
    pub rate_pct: Decimal,
    /// Monthly equivalent of the account's payment rules, base currency.
    pub monthly_payment: Decimal,
    /// `None`: not paid off within the horizon (or no payment rule at all).
    pub payoff: Option<NaiveDate>,
    pub months: Option<u32>,
    /// Interest charged until payoff (or the horizon).
    pub interest: Decimal,
    /// Owed at the end of each projected year, for the payoff bar's shape.
    pub yearly: Vec<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProjAccount {
    pub account: String,
    pub name: String,
    pub group: Group,
    pub rate_pct: Decimal,
    pub monthly: Decimal,
    pub start: Decimal,
    pub end: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Projection {
    pub start: NaiveDate,
    pub start_net_worth: Decimal,
    pub return_pct: Decimal,
    pub band_pct: Decimal,
    pub extra_monthly: Decimal,
    /// Monthly equivalent of every rule into an asset, base currency.
    pub monthly_saving: Decimal,
    pub points: Vec<ProjPoint>,
    pub milestones: Vec<Milestone>,
    pub debts: Vec<DebtPayoff>,
    pub accounts: Vec<ProjAccount>,
    pub target: Option<Decimal>,
    /// First month the central line reaches `target`.
    pub target_date: Option<NaiveDate>,
    /// Rules or accounts that could not be converted (no FX rate); left out of the run.
    pub skipped: Vec<String>,
}

/// One account going into the run: value today, annual rate, monthly inflow.
#[derive(Debug, Clone)]
struct Lane {
    id: String,
    name: String,
    group: Group,
    debt: bool,
    /// Uses the investment return (so it moves with the band and the slider).
    market: bool,
    rate_pct: Decimal,
    monthly: Decimal,
    start: Decimal,
}

fn rate_for(l: &Ledger, a: &Account) -> (Decimal, bool) {
    let p = &l.settings.projection;
    if let Some(r) = p.rates.get(&a.id) {
        return (*r, false);
    }
    match a.kind.group() {
        Group::Investments => (p.return_pct, true),
        Group::Cash => (p.cash_pct, false),
        Group::Property => (p.property_pct, false),
        Group::Debt => (Decimal::ZERO, false),
    }
}

fn lanes(l: &Ledger, m: &Market, start: NaiveDate, skipped: &mut Vec<String>) -> Vec<Lane> {
    let mut out = Vec::new();
    for a in &l.accounts {
        let v = value_account(l, m, a, start);
        if v.missing_fx {
            skipped.push(format!("{} (no FX rate for {})", a.name, a.currency));
            continue;
        }
        let mut monthly = Decimal::ZERO;
        for r in l.recurring.iter().filter(|r| r.account == a.id) {
            let per_month = r.amount * Decimal::from(r.every.per_year()) / Decimal::from(12);
            match to_base(l, m, per_month, &a.currency, start) {
                Some(b) => monthly += b,
                None => skipped.push(format!("rule on {} (no FX rate)", a.name)),
            }
        }
        let (rate_pct, market) = rate_for(l, a);
        out.push(Lane {
            id: a.id.clone(),
            name: a.name.clone(),
            group: a.kind.group(),
            debt: a.kind.is_liability(),
            market,
            rate_pct,
            monthly: monthly.round_dp(2),
            start: v.value.round_dp(2),
        });
    }
    out
}

fn monthly_rate(pct: Decimal) -> Decimal {
    pct / Decimal::from(1200)
}

/// One month forward for one lane. Returns (new balance, interest charged on a debt).
fn step(balance: Decimal, rate_pct: Decimal, monthly: Decimal, debt: bool) -> (Decimal, Decimal) {
    let growth = (balance * monthly_rate(rate_pct)).round_dp(2);
    if debt {
        if balance <= Decimal::ZERO {
            return (Decimal::ZERO, Decimal::ZERO);
        }
        let next = balance + growth - monthly;
        (next.max(Decimal::ZERO), growth)
    } else {
        (balance + growth + monthly, Decimal::ZERO)
    }
}

/// Net worth per month for one return assumption. Also fills debt payoff details when
/// `debts` is given (only the central run needs them).
fn run(
    lanes: &[Lane],
    return_pct: Decimal,
    extra: Decimal,
    months: u32,
    mut debts: Option<&mut Vec<DebtPayoff>>,
) -> (Vec<Decimal>, Vec<Decimal>, Vec<Decimal>) {
    let mut bal: Vec<Decimal> = lanes.iter().map(|x| x.start).collect();
    let mut extra_bal = Decimal::ZERO;
    let mut contributed = Decimal::ZERO;
    let mut nw = Vec::with_capacity(months as usize);
    let mut contrib = Vec::with_capacity(months as usize);
    for month in 1..=months {
        for (i, lane) in lanes.iter().enumerate() {
            let rate = if lane.market { return_pct } else { lane.rate_pct };
            let was = bal[i];
            let (next, interest) = step(was, rate, lane.monthly, lane.debt);
            bal[i] = next;
            if !lane.debt {
                contributed += lane.monthly;
            }
            if let Some(ds) = debts.as_deref_mut() {
                if let Some(d) = ds.iter_mut().find(|d| d.account == lane.id) {
                    if d.months.is_none() {
                        d.interest += interest;
                        if was > Decimal::ZERO && next.is_zero() {
                            d.months = Some(month);
                        }
                    }
                    if month % 12 == 0 {
                        d.yearly.push(next);
                    }
                }
            }
        }
        extra_bal = extra_bal + (extra_bal * monthly_rate(return_pct)).round_dp(2) + extra;
        contributed += extra;
        let total: Decimal =
            lanes.iter().zip(&bal).map(|(lane, v)| if lane.debt { -*v } else { *v }).sum::<Decimal>()
                + extra_bal;
        nw.push(total);
        contrib.push(contributed);
    }
    (nw, contrib, bal)
}

pub fn project(l: &Ledger, m: &Market, start: NaiveDate, a: &Assumptions) -> Projection {
    let mut skipped = Vec::new();
    let lanes = lanes(l, m, start, &mut skipped);
    let months = a.years.max(1) * 12;
    let mut debts: Vec<DebtPayoff> = lanes
        .iter()
        .filter(|x| x.debt && x.start > Decimal::ZERO)
        .map(|x| DebtPayoff {
            account: x.id.clone(),
            name: x.name.clone(),
            balance: x.start,
            rate_pct: x.rate_pct,
            monthly_payment: x.monthly,
            payoff: None,
            months: None,
            interest: Decimal::ZERO,
            yearly: Vec::new(),
        })
        .collect();
    let (mid, contributed, end) = run(&lanes, a.return_pct, a.extra_monthly, months, Some(&mut debts));
    let (low, _, _) = run(&lanes, a.return_pct - BAND_PCT, a.extra_monthly, months, None);
    let (high, _, _) = run(&lanes, a.return_pct + BAND_PCT, a.extra_monthly, months, None);
    let date_at = |month: u32| start.checked_add_months(Months::new(month)).unwrap_or(start);
    for d in &mut debts {
        d.payoff = d.months.map(date_at);
    }
    let points: Vec<ProjPoint> = (0..months as usize)
        .map(|i| ProjPoint {
            date: date_at(i as u32 + 1),
            net_worth: mid[i],
            low: low[i],
            high: high[i],
            contributed: contributed[i],
        })
        .collect();
    let milestones = [1u32, 5, 10, 20]
        .into_iter()
        .filter(|y| *y <= a.years.max(1))
        .map(|y| {
            let p = &points[(y * 12 - 1) as usize];
            Milestone {
                years: y,
                date: p.date,
                net_worth: p.net_worth,
                low: p.low,
                high: p.high,
                contributed: p.contributed,
            }
        })
        .collect();
    let start_net_worth: Decimal = lanes.iter().map(|x| if x.debt { -x.start } else { x.start }).sum();
    let target_date = a.target.and_then(|t| {
        if start_net_worth >= t {
            return Some(start);
        }
        points.iter().find(|p| p.net_worth >= t).map(|p| p.date)
    });
    let monthly_saving = lanes.iter().filter(|x| !x.debt).map(|x| x.monthly).sum();
    let accounts = lanes
        .iter()
        .zip(&end)
        .map(|(x, e)| ProjAccount {
            account: x.id.clone(),
            name: x.name.clone(),
            group: x.group,
            rate_pct: if x.market { a.return_pct } else { x.rate_pct },
            monthly: x.monthly,
            start: x.start,
            end: *e,
        })
        .collect();
    Projection {
        start,
        start_net_worth,
        return_pct: a.return_pct,
        band_pct: BAND_PCT,
        extra_monthly: a.extra_monthly,
        monthly_saving,
        points,
        milestones,
        debts,
        accounts,
        target: a.target,
        target_date,
        skipped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{AccountKind, Every, Mutation, Recurring};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn acct(id: &str, kind: AccountKind) -> Account {
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

    fn rule(account: &str, amount: Decimal, every: Every) -> Recurring {
        Recurring { account: account.into(), amount, every, note: None }
    }

    /// One account with a balance on `start`, optional rule, optional rate override.
    fn one(kind: AccountKind, balance: Decimal, r: Option<Recurring>, rate: Option<Decimal>) -> Ledger {
        let mut l = Ledger::starter("CAD")
            .apply(Mutation::UpsertAccount { account: acct("a", kind) })
            .unwrap()
            .apply(Mutation::SetBalance { date: ymd(2026, 1, 1), account: "a".into(), value: balance })
            .unwrap();
        if let Some(r) = r {
            l.recurring.push(r);
        }
        if let Some(rate) = rate {
            l.settings.projection.rates.insert("a".into(), rate);
        }
        l
    }

    fn assume(years: u32, return_pct: Decimal) -> Assumptions {
        Assumptions { years, return_pct, extra_monthly: Decimal::ZERO, target: None }
    }

    #[test]
    fn zero_rate_is_linear_in_the_rules() {
        let l = one(AccountKind::Savings, dec!(1000), Some(rule("a", dec!(100), Every::Month)), None);
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(1, dec!(5)));
        assert_eq!(p.points.len(), 12);
        assert_eq!(p.points[11].net_worth, dec!(2200));
        assert_eq!(p.points[11].contributed, dec!(1200));
        assert_eq!(p.points[0].date, ymd(2026, 2, 1));
        assert_eq!(p.milestones[0].years, 1);
    }

    #[test]
    fn biweekly_and_yearly_rules_convert_to_a_monthly_equivalent() {
        let mut l = one(AccountKind::Savings, dec!(0), Some(rule("a", dec!(600), Every::TwoWeeks)), None);
        l.recurring.push(rule("a", dec!(1200), Every::Year));
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(1, dec!(0)));
        // 600 x 26 / 12 = 1300, plus 100.
        assert_eq!(p.monthly_saving, dec!(1400));
        assert_eq!(p.points[11].net_worth, dec!(16800));
    }

    #[test]
    fn savings_match_the_future_value_of_an_annuity() {
        // 500 a month at 6% nominal, monthly compounding, 10 years:
        // FV = 500 * ((1.005^120 - 1) / 0.005) = 81,939.67
        let l = one(AccountKind::Tfsa, dec!(0), Some(rule("a", dec!(500), Every::Month)), None);
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(10, dec!(6)));
        let fv = p.milestones.iter().find(|m| m.years == 10).unwrap().net_worth;
        assert!((fv - dec!(81939.67)).abs() < dec!(1), "got {fv}");
    }

    #[test]
    fn a_lump_sum_compounds_monthly() {
        // 10,000 at 6% for 5 years: 10000 * 1.005^60 = 13,488.50
        let l = one(AccountKind::NonRegistered, dec!(10000), None, None);
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(5, dec!(6)));
        let v = p.milestones.last().unwrap().net_worth;
        assert!((v - dec!(13488.50)).abs() < dec!(1), "got {v}");
    }

    #[test]
    fn a_mortgage_pays_off_on_its_amortization_schedule() {
        // 300,000 at 6% with the 25-year payment of 1,932.90 pays off in month 300.
        let l = one(
            AccountKind::Mortgage,
            dec!(300000),
            Some(rule("a", dec!(1932.90), Every::Month)),
            Some(dec!(6)),
        );
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(30, dec!(5)));
        let d = &p.debts[0];
        let months = d.months.expect("paid off");
        assert!((299..=301).contains(&months), "paid off in month {months}");
        assert_eq!(d.payoff, Some(ymd(2026, 1, 1).checked_add_months(Months::new(months)).unwrap()));
        // Total paid ≈ 300 x 1932.90 = 579,870, so interest ≈ 279,870.
        assert!((d.interest - dec!(279870)).abs() < dec!(2000), "interest {}", d.interest);
        // Net worth never goes above zero from a debt alone, and stops at zero.
        assert_eq!(p.points.last().unwrap().net_worth, Decimal::ZERO);
        assert_eq!(d.yearly.len(), 30);
        assert!(d.yearly.windows(2).all(|w| w[1] <= w[0]));
    }

    #[test]
    fn a_debt_without_payments_grows_and_never_pays_off() {
        let l = one(AccountKind::CreditCard, dec!(1000), None, Some(dec!(20)));
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(1, dec!(5)));
        assert_eq!(p.debts[0].payoff, None);
        assert!(p.points[11].net_worth < dec!(-1200));
    }

    #[test]
    fn band_brackets_the_central_line_and_extra_saving_lifts_it() {
        let l = one(AccountKind::Tfsa, dec!(50000), Some(rule("a", dec!(500), Every::Month)), None);
        let m = Market::from_ledger(&l);
        let base = project(&l, &m, ymd(2026, 1, 1), &assume(20, dec!(5)));
        for p in &base.points {
            assert!(p.low <= p.net_worth && p.net_worth <= p.high);
        }
        let more = project(
            &l,
            &m,
            ymd(2026, 1, 1),
            &Assumptions { extra_monthly: dec!(250), ..assume(20, dec!(5)) },
        );
        assert!(more.points[239].net_worth > base.points[239].net_worth);
        assert_eq!(more.points[239].contributed - base.points[239].contributed, dec!(250) * dec!(240));
        assert_eq!(base.milestones.iter().map(|m| m.years).collect::<Vec<_>>(), vec![1, 5, 10, 20]);
    }

    #[test]
    fn target_date_is_the_first_month_at_or_above_target() {
        let l = one(AccountKind::Savings, dec!(0), Some(rule("a", dec!(1000), Every::Month)), None);
        let a = Assumptions { target: Some(dec!(10000)), ..assume(2, dec!(0)) };
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &a);
        assert_eq!(p.target_date, Some(ymd(2026, 11, 1)));
        let a = Assumptions { target: Some(dec!(1_000_000)), ..assume(2, dec!(0)) };
        assert_eq!(project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &a).target_date, None);
    }

    #[test]
    fn rate_overrides_beat_group_defaults() {
        let l = one(AccountKind::Savings, dec!(10000), None, Some(dec!(12)));
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(1, dec!(5)));
        assert_eq!(p.points[0].net_worth, dec!(10100));
        assert_eq!(p.accounts[0].rate_pct, dec!(12));
    }

    #[test]
    fn an_account_with_no_fx_rate_is_skipped_not_zeroed() {
        let mut l = one(AccountKind::Savings, dec!(100), None, None);
        let mut usd = acct("u", AccountKind::Savings);
        usd.currency = "USD".into();
        l = l.apply(Mutation::UpsertAccount { account: usd }).unwrap();
        let p = project(&l, &Market::from_ledger(&l), ymd(2026, 1, 1), &assume(1, dec!(0)));
        assert_eq!(p.accounts.len(), 1);
        assert_eq!(p.skipped.len(), 1);
    }
}
