//! Contribution room for TFSA, RRSP and FHSA: what CRA reported on a date, plus the
//! new room each January 1 adds, minus what you logged as contributed since. Pure.
//!
//! Rules applied (and only these):
//! - TFSA: each Jan 1 after `as_of` adds that year's limit, and adds back what you
//!   withdrew in earlier years. Limits after the last known year are assumed equal to
//!   it and flagged as estimated.
//! - FHSA: each Jan 1 after `as_of` adds $8,000. Carry-forward caps and the $40,000
//!   lifetime limit are not modelled; the note says so.
//! - RRSP: new room depends on your income, so nothing is added; once `as_of` is in an
//!   earlier year the view asks for the figure from your notice of assessment.
//! - Planned: the monthly equivalent of recurring rules into those accounts, for the
//!   whole months left this year. Never counted as used.

use chrono::{Datelike, Months, NaiveDate};
use ledger::{Ledger, RoomKind};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{to_base, Market};

/// TFSA annual dollar limit by year (CRA). The last entry is used for later years.
pub const TFSA_LIMITS: &[(i32, i64)] = &[
    (2009, 5000),
    (2010, 5000),
    (2011, 5000),
    (2012, 5000),
    (2013, 5500),
    (2014, 5500),
    (2015, 10000),
    (2016, 5500),
    (2017, 5500),
    (2018, 5500),
    (2019, 6000),
    (2020, 6000),
    (2021, 6000),
    (2022, 6000),
    (2023, 6500),
    (2024, 7000),
    (2025, 7000),
    (2026, 7000),
];
pub const FHSA_ANNUAL: i64 = 8000;

fn tfsa_limit(year: i32) -> (Decimal, bool) {
    match TFSA_LIMITS.iter().find(|(y, _)| *y == year) {
        Some((_, v)) => (Decimal::from(*v), false),
        None => (
            Decimal::from(TFSA_LIMITS.last().map_or(7000, |x| x.1)),
            year > TFSA_LIMITS.last().map_or(0, |x| x.0),
        ),
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RoomAccount {
    pub account: String,
    pub name: String,
    /// Logged since `as_of` (deposits only), base currency.
    pub used: Decimal,
    pub monthly_plan: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RoomView {
    pub kind: RoomKind,
    pub label: String,
    pub as_of: NaiveDate,
    /// What CRA reported on `as_of`.
    pub reported: Decimal,
    /// New-year room and re-added withdrawals since `as_of`.
    pub added: Decimal,
    /// Deposits logged since `as_of`.
    pub used: Decimal,
    /// reported + added - used. Negative means over-contributed.
    pub left: Decimal,
    /// Recurring rules for the rest of this year.
    pub planned: Decimal,
    pub monthly_plan: Decimal,
    /// When the plan uses up what is left, if it does this year.
    pub runs_out: Option<NaiveDate>,
    pub over: bool,
    /// An added amount relied on a guessed limit.
    pub estimated: bool,
    /// Something to tell the user ("update from your notice of assessment").
    pub note: Option<String>,
    pub accounts: Vec<RoomAccount>,
}

/// One view per kind that has a `room` entry in the ledger, in TFSA, RRSP, FHSA order.
pub fn room(l: &Ledger, m: &Market, today: NaiveDate) -> Vec<RoomView> {
    RoomKind::ALL
        .iter()
        .filter_map(|k| l.room.get(k).map(|r| (*k, r)))
        .map(|(kind, r)| one(l, m, today, kind, r))
        .collect()
}

fn one(l: &Ledger, m: &Market, today: NaiveDate, kind: RoomKind, r: &ledger::Room) -> RoomView {
    let base =
        |amount: Decimal, cur: &str, date: NaiveDate| to_base(l, m, amount, cur, date).unwrap_or(amount);
    let accounts: Vec<&ledger::Account> =
        l.accounts.iter().filter(|a| a.kind == kind.account_kind()).collect();
    let months_left = 12 - today.month();

    let mut used = Decimal::ZERO;
    let mut withdrawn_by_year: std::collections::BTreeMap<i32, Decimal> = Default::default();
    let mut rows = Vec::new();
    for a in &accounts {
        let mut a_used = Decimal::ZERO;
        for c in l.contributions.iter().filter(|c| c.account == a.id && c.date >= r.as_of && c.date <= today)
        {
            let v = base(c.amount, &a.currency, c.date);
            if v > Decimal::ZERO {
                a_used += v;
            } else {
                *withdrawn_by_year.entry(c.date.year()).or_default() -= v;
            }
        }
        let monthly: Decimal = l
            .recurring
            .iter()
            .filter(|x| x.account == a.id)
            .map(|x| {
                base(x.amount * Decimal::from(x.every.per_year()) / Decimal::from(12), &a.currency, today)
            })
            .sum::<Decimal>()
            .round_dp(2);
        used += a_used;
        rows.push(RoomAccount {
            account: a.id.clone(),
            name: a.name.clone(),
            used: a_used,
            monthly_plan: monthly,
        });
    }

    let mut added = Decimal::ZERO;
    let mut estimated = false;
    let mut note = None;
    let first_new_year = r.as_of.year() + 1;
    match kind {
        RoomKind::Tfsa => {
            for year in first_new_year..=today.year() {
                let (limit, guess) = tfsa_limit(year);
                added += limit;
                estimated |= guess;
                added += withdrawn_by_year.get(&(year - 1)).copied().unwrap_or_default();
            }
            if estimated {
                note = Some(format!(
                    "Assumes the {} limit is {}; check CRA My Account.",
                    today.year(),
                    tfsa_limit(today.year()).0
                ));
            }
        }
        RoomKind::Fhsa => {
            let years = (today.year() - r.as_of.year()).max(0);
            added = Decimal::from(FHSA_ANNUAL * years as i64);
            if years > 0 {
                estimated = true;
                note = Some(
                    "Adds $8,000 a year; carry-forward caps and the $40,000 lifetime limit are not checked."
                        .into(),
                );
            }
        }
        RoomKind::Rrsp => {
            if r.as_of.year() < today.year() {
                note = Some(format!(
                    "RRSP room for {} comes from your notice of assessment; update it.",
                    today.year()
                ));
            }
        }
    }

    let monthly_plan: Decimal = rows.iter().map(|x| x.monthly_plan).sum();
    let planned = monthly_plan * Decimal::from(months_left);
    let left = r.amount + added - used;
    let runs_out = if monthly_plan > Decimal::ZERO && left > Decimal::ZERO && planned > left {
        let months = (left / monthly_plan).ceil();
        let months: u32 = months.try_into().unwrap_or(12);
        today.checked_add_months(Months::new(months))
    } else {
        None
    };
    RoomView {
        kind,
        label: kind.label().into(),
        as_of: r.as_of,
        reported: r.amount,
        added,
        used,
        left,
        planned,
        monthly_plan,
        runs_out,
        over: left < Decimal::ZERO,
        estimated,
        note,
        accounts: rows,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{Account, AccountKind, Contribution, Every, Recurring, Room};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn ledger_with(kind: AccountKind) -> Ledger {
        let mut l = Ledger::starter("CAD");
        l.accounts.push(Account {
            id: "reg".into(),
            name: "Registered".into(),
            institution: None,
            kind,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        });
        l
    }

    fn put(l: &mut Ledger, date: NaiveDate, amount: Decimal) {
        l.contributions.push(Contribution { date, account: "reg".into(), amount, note: None });
    }

    #[test]
    fn tfsa_room_left_is_reported_minus_deposits() {
        let mut l = ledger_with(AccountKind::Tfsa);
        l.room.insert(RoomKind::Tfsa, Room { as_of: ymd(2026, 1, 1), amount: dec!(14500) });
        put(&mut l, ymd(2026, 2, 1), dec!(5000));
        put(&mut l, ymd(2026, 6, 1), dec!(1500));
        put(&mut l, ymd(2025, 12, 1), dec!(9999)); // before as_of: already counted by CRA
        let v = &room(&l, &Market::from_ledger(&l), ymd(2026, 11, 3))[0];
        assert_eq!(v.used, dec!(6500));
        assert_eq!(v.left, dec!(8000));
        assert!(!v.over);
        assert_eq!(v.added, Decimal::ZERO);
    }

    #[test]
    fn a_new_year_adds_the_limit_and_last_years_withdrawals() {
        let mut l = ledger_with(AccountKind::Tfsa);
        l.room.insert(RoomKind::Tfsa, Room { as_of: ymd(2025, 1, 1), amount: dec!(10000) });
        put(&mut l, ymd(2025, 3, 1), dec!(8000));
        put(&mut l, ymd(2025, 9, 1), dec!(-2000)); // withdrawal: back on Jan 1 2026
        let v = &room(&l, &Market::from_ledger(&l), ymd(2026, 2, 1))[0];
        assert_eq!(v.added, dec!(9000)); // 7000 limit + 2000 re-added
        assert_eq!(v.left, dec!(11000));
        assert!(!v.estimated);
        // A year past the table is a guess, and says so.
        let v = &room(&l, &Market::from_ledger(&l), ymd(2027, 2, 1))[0];
        assert!(v.estimated && v.note.is_some());
    }

    #[test]
    fn over_contribution_is_flagged() {
        let mut l = ledger_with(AccountKind::Tfsa);
        l.room.insert(RoomKind::Tfsa, Room { as_of: ymd(2026, 1, 1), amount: dec!(1000) });
        put(&mut l, ymd(2026, 2, 1), dec!(1500));
        let v = &room(&l, &Market::from_ledger(&l), ymd(2026, 3, 1))[0];
        assert!(v.over);
        assert_eq!(v.left, dec!(-500));
    }

    #[test]
    fn the_plan_says_when_room_runs_out_this_year() {
        let mut l = ledger_with(AccountKind::Tfsa);
        l.room.insert(RoomKind::Tfsa, Room { as_of: ymd(2026, 1, 1), amount: dec!(2000) });
        l.recurring.push(Recurring {
            account: "reg".into(),
            amount: dec!(500),
            every: Every::Month,
            note: None,
        });
        let v = &room(&l, &Market::from_ledger(&l), ymd(2026, 3, 15))[0];
        assert_eq!(v.monthly_plan, dec!(500));
        assert_eq!(v.planned, dec!(4500)); // 9 whole months left
        assert_eq!(v.runs_out, Some(ymd(2026, 7, 15)));
        // The plan is never counted as used.
        assert_eq!(v.used, Decimal::ZERO);
    }

    #[test]
    fn rrsp_adds_nothing_and_asks_for_the_new_figure() {
        let mut l = ledger_with(AccountKind::Rrsp);
        l.room.insert(RoomKind::Rrsp, Room { as_of: ymd(2025, 5, 1), amount: dec!(21000) });
        let v = &room(&l, &Market::from_ledger(&l), ymd(2026, 2, 1))[0];
        assert_eq!(v.added, Decimal::ZERO);
        assert!(v.note.as_deref().unwrap().contains("notice of assessment"));
    }

    #[test]
    fn fhsa_adds_eight_thousand_a_year() {
        let mut l = ledger_with(AccountKind::Fhsa);
        l.room.insert(RoomKind::Fhsa, Room { as_of: ymd(2025, 1, 1), amount: dec!(8000) });
        put(&mut l, ymd(2025, 4, 1), dec!(8000));
        let v = &room(&l, &Market::from_ledger(&l), ymd(2026, 1, 10))[0];
        assert_eq!(v.left, dec!(8000));
    }

    #[test]
    fn kinds_without_a_room_entry_are_left_out() {
        let l = ledger_with(AccountKind::Tfsa);
        assert!(room(&l, &Market::from_ledger(&l), ymd(2026, 1, 1)).is_empty());
    }
}
