//! CRA limits for registered accounts, and the arithmetic that turns "when did room
//! start" plus "where do you stand today" into a room figure. Pure; no clock.
//!
//! - TFSA: room starts in 2009 or the year you turned 18 (or became resident), if later.
//!   Each year adds that year's dollar limit. Limits after the table's last year are
//!   assumed equal to it and flagged as estimated.
//! - FHSA: room starts the year you open your first FHSA. Each year adds $8,000, plus
//!   last year's unused room up to $8,000, never past $40,000 contributed in a lifetime.
//!   The account must close by the end of the 15th year after it opened.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::schema::RoomKind;

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
/// The first year anyone had TFSA room.
pub const TFSA_FIRST_YEAR: i32 = 2009;
pub const FHSA_FIRST_YEAR: i32 = 2023;
pub const FHSA_ANNUAL: i64 = 8000;
/// Unused FHSA room carries into the next year, up to this much.
pub const FHSA_CARRY: i64 = 8000;
pub const FHSA_LIFETIME: i64 = 40000;
/// Years an FHSA can stay open after the year it opened.
pub const FHSA_YEARS: i32 = 15;

/// The TFSA limit for `year`, and whether it is a guess (a year past the table).
pub fn tfsa_limit(year: i32) -> (Decimal, bool) {
    let last = TFSA_LIMITS.last().copied().unwrap_or((TFSA_FIRST_YEAR, 5000));
    match TFSA_LIMITS.iter().find(|(y, _)| *y == year) {
        Some((_, v)) => (Decimal::from(*v), false),
        None => (Decimal::from(last.1), year > last.0),
    }
}

/// All TFSA room from `since` (2009 at the earliest) through `through`, inclusive.
pub fn tfsa_lifetime(since: i32, through: i32) -> Decimal {
    (since.max(TFSA_FIRST_YEAR)..=through).map(|y| tfsa_limit(y).0).sum()
}

/// What an FHSA opened in `opened` lets you put in through `through`, if you used it all.
pub fn fhsa_lifetime(opened: i32, through: i32) -> Decimal {
    let years = i64::from((through - opened + 1).max(0));
    Decimal::from((FHSA_ANNUAL * years).min(FHSA_LIFETIME))
}

/// FHSA room in one year: $8,000 plus last year's unused room (at most $8,000), capped by
/// what is left of the lifetime limit.
pub fn fhsa_year_room(unused_last_year: Decimal, put_in_before: Decimal) -> Decimal {
    let carry = unused_last_year.max(Decimal::ZERO).min(Decimal::from(FHSA_CARRY));
    let lifetime_left = (Decimal::from(FHSA_LIFETIME) - put_in_before).max(Decimal::ZERO);
    (Decimal::from(FHSA_ANNUAL) + carry).min(lifetime_left)
}

/// FHSA room left in `year` if `put_in` went in as early as the rules allowed since
/// `opened`. Exact when you contributed as soon as you could; otherwise CRA's figure is.
pub fn fhsa_room_left(opened: i32, put_in: Decimal, year: i32) -> Decimal {
    let mut unplaced = put_in.max(Decimal::ZERO);
    let mut placed = Decimal::ZERO;
    let mut unused = Decimal::ZERO;
    for _ in opened..=year {
        let room = fhsa_year_room(unused, placed);
        let c = unplaced.min(room);
        unplaced -= c;
        placed += c;
        unused = room - c;
    }
    unused
}

/// Where you stand today, across every account of one kind.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Standing {
    /// No room left: you put in all you could.
    Maxed,
    /// Deposits minus withdrawals, all accounts of the kind, ever.
    PutIn { amount: Decimal },
    /// The room CRA My Account shows.
    Left { amount: Decimal },
}

/// Why a setup cannot be worked out.
#[derive(Debug, Clone, PartialEq)]
pub enum SetupError {
    /// Only TFSA and FHSA room follow from a start year.
    NotYearBased,
    /// The start year is in the future, or before the account type existed.
    BadYear {
        first: i32,
        this: i32,
    },
    /// More put in than the rules allow since the start year.
    TooMuch {
        most: Decimal,
    },
    Negative,
}

/// Room left and total put in on `year`, from a start year and where you stand.
/// Returns `(room_left, put_in)`.
pub fn resolve(
    kind: RoomKind,
    since: i32,
    standing: &Standing,
    year: i32,
) -> Result<(Decimal, Decimal), SetupError> {
    let first = match kind {
        RoomKind::Tfsa => TFSA_FIRST_YEAR,
        RoomKind::Fhsa => FHSA_FIRST_YEAR,
        RoomKind::Rrsp => return Err(SetupError::NotYearBased),
    };
    // A TFSA start year before 2009 means "2009 or earlier": all the room there is.
    let since = if kind == RoomKind::Tfsa { since.max(first) } else { since };
    if since < first || since > year {
        return Err(SetupError::BadYear { first, this: year });
    }
    let lifetime = match kind {
        RoomKind::Tfsa => tfsa_lifetime(since, year),
        _ => fhsa_lifetime(since, year),
    };
    match standing {
        Standing::Maxed => Ok((Decimal::ZERO, lifetime)),
        Standing::PutIn { amount } | Standing::Left { amount } if amount.is_sign_negative() => {
            Err(SetupError::Negative)
        }
        Standing::PutIn { amount } => {
            if *amount > lifetime {
                return Err(SetupError::TooMuch { most: lifetime });
            }
            let left = match kind {
                RoomKind::Tfsa => lifetime - amount,
                _ => fhsa_room_left(since, *amount, year),
            };
            Ok((left, *amount))
        }
        // CRA's figure is the truth for room; put in is what the rules say is used.
        Standing::Left { amount } => Ok((*amount, (lifetime - amount).max(Decimal::ZERO))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn tfsa_room_since_2009_is_109k_in_2026() {
        assert_eq!(tfsa_lifetime(2009, 2026), dec!(109000));
        assert_eq!(tfsa_lifetime(1990, 2026), dec!(109000));
        assert_eq!(tfsa_lifetime(2020, 2026), dec!(45500));
        assert_eq!(tfsa_lifetime(2026, 2026), dec!(7000));
    }

    #[test]
    fn a_year_past_the_table_is_a_guess() {
        assert_eq!(tfsa_limit(2015), (dec!(10000), false));
        assert_eq!(tfsa_limit(2027), (dec!(7000), true));
    }

    #[test]
    fn fhsa_lifetime_stops_at_40k() {
        assert_eq!(fhsa_lifetime(2024, 2026), dec!(24000));
        assert_eq!(fhsa_lifetime(2023, 2030), dec!(40000));
    }

    #[test]
    fn fhsa_carry_forward_is_capped_at_8k() {
        // Opened 2024, nothing in: 8k, then 16k, then still 16k (only 8k carries).
        assert_eq!(fhsa_room_left(2024, dec!(0), 2024), dec!(8000));
        assert_eq!(fhsa_room_left(2024, dec!(0), 2025), dec!(16000));
        assert_eq!(fhsa_room_left(2024, dec!(0), 2026), dec!(16000));
        // 24k in since 2024: none left in 2026.
        assert_eq!(fhsa_room_left(2024, dec!(24000), 2026), dec!(0));
        // 36k in by 2027: the lifetime cap leaves 4k for 2028.
        assert_eq!(fhsa_room_left(2023, dec!(36000), 2028), dec!(4000));
    }

    #[test]
    fn maxed_out_means_no_room_and_everything_put_in() {
        assert_eq!(resolve(RoomKind::Tfsa, 2001, &Standing::Maxed, 2026), Ok((dec!(0), dec!(109000))));
        assert_eq!(resolve(RoomKind::Fhsa, 2024, &Standing::Maxed, 2026), Ok((dec!(0), dec!(24000))));
    }

    #[test]
    fn put_in_and_cra_left_work_out_the_other_figure() {
        assert_eq!(
            resolve(RoomKind::Tfsa, 2009, &Standing::PutIn { amount: dec!(100000) }, 2026),
            Ok((dec!(9000), dec!(100000)))
        );
        assert_eq!(
            resolve(RoomKind::Tfsa, 2009, &Standing::Left { amount: dec!(9000) }, 2026),
            Ok((dec!(9000), dec!(100000)))
        );
        assert_eq!(
            resolve(RoomKind::Fhsa, 2024, &Standing::PutIn { amount: dec!(8000) }, 2026),
            Ok((dec!(16000), dec!(8000)))
        );
    }

    #[test]
    fn impossible_setups_say_why() {
        assert_eq!(
            resolve(RoomKind::Tfsa, 2009, &Standing::PutIn { amount: dec!(200000) }, 2026),
            Err(SetupError::TooMuch { most: dec!(109000) })
        );
        assert_eq!(
            resolve(RoomKind::Fhsa, 2021, &Standing::Maxed, 2026),
            Err(SetupError::BadYear { first: 2023, this: 2026 })
        );
        assert_eq!(
            resolve(RoomKind::Tfsa, 2030, &Standing::Maxed, 2026),
            Err(SetupError::BadYear { first: 2009, this: 2026 })
        );
        assert_eq!(resolve(RoomKind::Rrsp, 2020, &Standing::Maxed, 2026), Err(SetupError::NotYearBased));
    }
}
