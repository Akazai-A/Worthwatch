//! Registered accounts added up by type: what all your TFSAs (or FHSAs, or RRSPs) are
//! worth together, how much you put in, what grew, and the room view from [`room`].
//! Pure.
//!
//! - Value: every account of the kind, valued today in the base currency.
//! - Put in: the total you gave at setup (`registered`) plus logged contributions since
//!   (withdrawals subtract). Unknown until you answer the two setup questions.
//! - Growth: value minus put in.
//! - Years: one cell per year of room for the strip on the Plan screen. TFSA: each year's
//!   limit from the start year, plus next year (estimated). FHSA: the five $8,000 years
//!   from the year it opened. Cells fill oldest first with the room you used.

use chrono::{Datelike, NaiveDate};
use ledger::cra::{tfsa_lifetime, tfsa_limit, FHSA_ANNUAL, FHSA_LIFETIME, FHSA_YEARS, TFSA_FIRST_YEAR};
use ledger::{Ledger, RoomKind};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::room::{room, RoomView};
use crate::{value_account, Market};

/// TFSA, FHSA, then RRSP: the order the Plan screen shows them in.
pub const ORDER: [RoomKind; 3] = [RoomKind::Tfsa, RoomKind::Fhsa, RoomKind::Rrsp];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct HeldAccount {
    pub account: String,
    pub name: String,
    /// Today, base currency.
    pub value: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct YearCell {
    pub year: i32,
    /// Room this year adds.
    pub limit: Decimal,
    /// How much of `limit` is used, 0..=limit.
    pub used: Decimal,
    /// A year past CRA's table: the limit is a guess.
    pub estimated: bool,
    /// A year that has not started yet.
    pub future: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RegisteredView {
    pub kind: RoomKind,
    pub label: String,
    /// All accounts of the kind, today, base currency.
    pub value: Decimal,
    /// Largest first.
    pub accounts: Vec<HeldAccount>,
    /// The year room started (TFSA) or the account opened (FHSA), once set up.
    pub since: Option<i32>,
    /// Deposits minus withdrawals, all accounts of the kind, ever. `None` until set up.
    pub put_in: Option<Decimal>,
    pub growth: Option<Decimal>,
    /// Simple return since you started: growth ÷ put in, percent (Wealthsimple's "simple return").
    pub return_pct: Option<Decimal>,
    /// All room there has been since `since` (TFSA), or $40,000 (FHSA).
    pub lifetime: Option<Decimal>,
    /// FHSA: what the lifetime limit still allows.
    pub lifetime_left: Option<Decimal>,
    /// FHSA: the last year the account can stay open.
    pub closes: Option<i32>,
    pub years: Vec<YearCell>,
    /// Room as [`room`] sees it; `None` when no room figure is known yet.
    pub room: Option<RoomView>,
}

/// One view per registered kind you hold an account of (or have room for).
pub fn registered(l: &Ledger, m: &Market, today: NaiveDate) -> Vec<RegisteredView> {
    let rooms = room(l, m, today);
    ORDER
        .iter()
        .filter(|k| l.accounts.iter().any(|a| a.kind == k.account_kind()) || l.room.contains_key(k))
        .map(|k| one(l, m, today, *k, rooms.iter().find(|r| r.kind == *k).cloned()))
        .collect()
}

fn one(l: &Ledger, m: &Market, today: NaiveDate, kind: RoomKind, room: Option<RoomView>) -> RegisteredView {
    let mut accounts: Vec<HeldAccount> = l
        .accounts
        .iter()
        .filter(|a| a.kind == kind.account_kind())
        .map(|a| HeldAccount {
            account: a.id.clone(),
            name: a.name.clone(),
            value: value_account(l, m, a, today).value,
        })
        .collect();
    accounts.sort_by(|a, b| b.value.cmp(&a.value).then_with(|| a.name.cmp(&b.name)));
    let value: Decimal = accounts.iter().map(|a| a.value).sum();

    let reg = l.registered.get(&kind);
    let put_in = reg.map(|g| {
        g.put_in
            + l.contributions
                .iter()
                .filter(|c| c.date >= g.as_of && c.date <= today)
                .filter(|c| accounts.iter().any(|a| a.account == c.account))
                .map(|c| {
                    let cur = l.account(&c.account).map_or("", |a| a.currency.as_str());
                    crate::to_base(l, m, c.amount, cur, c.date).unwrap_or(c.amount)
                })
                .sum::<Decimal>()
    });
    let since = reg.map(|g| g.since);
    let (lifetime, lifetime_left, closes, years) = match (kind, since) {
        (RoomKind::Tfsa, Some(since)) => {
            let since = since.max(TFSA_FIRST_YEAR);
            let lifetime = tfsa_lifetime(since, today.year());
            // Room used is all room minus what is left; withdrawals give room back.
            let used = room.as_ref().map_or(put_in.unwrap_or_default(), |r| lifetime - r.left);
            let mut pool = used.max(Decimal::ZERO);
            let years = (since..=today.year() + 1)
                .map(|year| {
                    let (limit, estimated) = tfsa_limit(year);
                    let future = year > today.year();
                    let u = if future { Decimal::ZERO } else { pool.min(limit) };
                    pool -= u;
                    YearCell { year, limit, used: u, estimated, future }
                })
                .collect();
            (Some(lifetime), None, None, years)
        }
        (RoomKind::Fhsa, Some(since)) => {
            let lifetime = Decimal::from(FHSA_LIFETIME);
            let put = put_in.unwrap_or_default().max(Decimal::ZERO);
            let mut pool = put;
            let annual = Decimal::from(FHSA_ANNUAL);
            let years = (0..FHSA_LIFETIME / FHSA_ANNUAL)
                .map(|i| {
                    let year = since + i as i32;
                    let u = pool.min(annual);
                    pool -= u;
                    YearCell { year, limit: annual, used: u, estimated: false, future: year > today.year() }
                })
                .collect();
            (Some(lifetime), Some((lifetime - put).max(Decimal::ZERO)), Some(since + FHSA_YEARS), years)
        }
        _ => (None, None, None, Vec::new()),
    };
    RegisteredView {
        kind,
        label: kind.label().into(),
        value,
        accounts,
        since,
        put_in,
        growth: put_in.map(|p| value - p),
        return_pct: put_in.and_then(|p| crate::simple_return(Decimal::ZERO, value, p)),
        lifetime,
        lifetime_left,
        closes,
        years,
        room,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{Account, AccountKind, Contribution, Mutation, Standing};
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn acct(id: &str, kind: AccountKind) -> Account {
        Account {
            id: id.into(),
            name: id.to_uppercase(),
            institution: None,
            kind,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        }
    }

    /// Eight TFSAs and two FHSAs, each with a balance on `day`.
    fn eight_tfsas(day: NaiveDate) -> Ledger {
        let mut l = Ledger::starter("CAD");
        let tfsa = [41200, 28950, 22400, 17300, 12850, 9600, 6200, 3800];
        for (i, v) in tfsa.iter().enumerate() {
            let id = format!("tfsa-{i}");
            l.accounts.push(acct(&id, AccountKind::Tfsa));
            l.snapshots.entry(day).or_default().insert(id, Decimal::from(*v));
        }
        for (id, v) in [("fhsa-a", 19700), ("fhsa-b", 8250)] {
            l.accounts.push(acct(id, AccountKind::Fhsa));
            l.snapshots.entry(day).or_default().insert(id.into(), Decimal::from(v));
        }
        l
    }

    #[test]
    fn eight_tfsas_add_up_and_maxed_out_means_no_room() {
        let today = ymd(2026, 11, 6);
        let l = eight_tfsas(today)
            .apply(Mutation::SetupRegistered {
                kind: RoomKind::Tfsa,
                since: 2005,
                standing: Standing::Maxed,
                as_of: today,
            })
            .unwrap();
        let v = registered(&l, &Market::from_ledger(&l), today);
        assert_eq!(v.iter().map(|x| x.kind).collect::<Vec<_>>(), vec![RoomKind::Tfsa, RoomKind::Fhsa]);
        let t = &v[0];
        assert_eq!(t.accounts.len(), 8);
        assert_eq!(t.accounts[0].value, dec!(41200));
        assert_eq!(t.value, dec!(142300));
        assert_eq!(t.put_in, Some(dec!(109000)));
        assert_eq!(t.growth, Some(dec!(33300)));
        assert_eq!(t.return_pct, Some(dec!(30.6)));
        assert_eq!(t.lifetime, Some(dec!(109000)));
        assert_eq!(t.room.as_ref().unwrap().left, dec!(0));
        // 2009..=2026 used in full, then 2027 to come.
        assert_eq!(t.years.len(), 19);
        assert!(t.years.iter().filter(|y| !y.future).all(|y| y.used == y.limit));
        assert!(t.years.last().unwrap().future && t.years.last().unwrap().estimated);
        // FHSA not set up: value only.
        assert_eq!(v[1].value, dec!(27950));
        assert_eq!(v[1].put_in, None);
        assert!(v[1].room.is_none() && v[1].years.is_empty());
    }

    #[test]
    fn jan_1_adds_the_new_limit_and_contributions_add_to_put_in() {
        let today = ymd(2026, 11, 6);
        let mut l = eight_tfsas(today)
            .apply(Mutation::SetupRegistered {
                kind: RoomKind::Tfsa,
                since: 2009,
                standing: Standing::Maxed,
                as_of: today,
            })
            .unwrap();
        l.contributions.push(Contribution {
            date: ymd(2027, 1, 5),
            account: "tfsa-3".into(),
            amount: dec!(2000),
            note: None,
        });
        let v = &registered(&l, &Market::from_ledger(&l), ymd(2027, 2, 1))[0];
        let r = v.room.as_ref().unwrap();
        assert_eq!(r.left, dec!(5000)); // 7,000 new room - 2,000 in
        assert_eq!(v.put_in, Some(dec!(111000)));
        assert_eq!(v.lifetime, Some(dec!(116000)));
    }

    #[test]
    fn fhsa_fills_five_years_and_tracks_the_lifetime_limit() {
        let today = ymd(2026, 11, 6);
        let l = eight_tfsas(today)
            .apply(Mutation::SetupRegistered {
                kind: RoomKind::Fhsa,
                since: 2024,
                standing: Standing::Maxed,
                as_of: today,
            })
            .unwrap();
        let v = registered(&l, &Market::from_ledger(&l), today);
        let f = v.iter().find(|x| x.kind == RoomKind::Fhsa).unwrap();
        assert_eq!(f.put_in, Some(dec!(24000)));
        assert_eq!(f.growth, Some(dec!(3950)));
        assert_eq!(f.lifetime_left, Some(dec!(16000)));
        assert_eq!(f.closes, Some(2039));
        let used: Vec<_> = f.years.iter().map(|y| (y.year, y.used, y.future)).collect();
        assert_eq!(
            used,
            vec![
                (2024, dec!(8000), false),
                (2025, dec!(8000), false),
                (2026, dec!(8000), false),
                (2027, dec!(0), true),
                (2028, dec!(0), true)
            ]
        );
        // Next year: $8,000 more room (nothing carries; it was all used).
        let next = registered(&l, &Market::from_ledger(&l), ymd(2027, 1, 2));
        let f = next.iter().find(|x| x.kind == RoomKind::Fhsa).unwrap();
        assert_eq!(f.room.as_ref().unwrap().left, dec!(8000));
    }

    #[test]
    fn fhsa_room_stops_at_the_lifetime_limit() {
        let as_of = ymd(2026, 6, 1);
        let base = eight_tfsas(as_of);
        let setup = |amount: Decimal| Mutation::SetupRegistered {
            kind: RoomKind::Fhsa,
            since: 2023,
            standing: Standing::PutIn { amount },
            as_of,
        };
        // 2023-2026 allow 32,000 at most, so 36,000 is rejected.
        assert!(base.apply(setup(dec!(36000))).is_err());
        let mut l = base.apply(setup(dec!(32000))).unwrap();
        // In 2027 only 8,000 of lifetime is left, even though 8,000 + carry would be more.
        l.contributions.push(Contribution {
            date: ymd(2027, 3, 1),
            account: "fhsa-a".into(),
            amount: dec!(3000),
            note: None,
        });
        let v = registered(&l, &Market::from_ledger(&l), ymd(2028, 1, 2));
        let f = v.iter().find(|x| x.kind == RoomKind::Fhsa).unwrap();
        assert_eq!(f.put_in, Some(dec!(35000)));
        assert_eq!(f.lifetime_left, Some(dec!(5000)));
        assert_eq!(f.room.as_ref().unwrap().left, dec!(5000));
    }
}
