//! The budget game. Pure: the ledger, the reconciled transactions and today in, a
//! [`GameView`] out.
//!
//! ```text
//!  limits − bills ─▶ the month's allowance ─▶ split by weekday (your own habits) ─▶ each day
//!  each closed day: allowance − what you spent ─▶ into (or out of) the jar
//!  jar at month end vs your goal ─▶ hit or missed ─▶ three hits in a row offer a bigger goal
//!  last 8 weeks of spending ─▶ deal() ─▶ weekly challenge cards you play or pass
//! ```
//!
//! Bills never count against a day: rent on the 1st is not a choice you make that day.
//! Income and transfers never count either.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};
use ledger::{CardRule, Ledger, PlayedCard};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::category::{counts_as_spending, label};
use crate::month::first_of;
use crate::reconcile::Txn;

/// Coins a day can show: one per fifth of the allowance left over.
pub const MAX_COINS: u8 = 5;
/// Weeks of history used to deal cards.
const DEAL_WEEKS: u64 = 8;
/// Months of history used to weight weekdays.
const WEIGHT_MONTHS: u32 = 3;
/// Months hit in a row before a bigger goal is offered.
const STEP_UP_AFTER: usize = 3;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct GameView {
    pub month: NaiveDate,
    pub days: u32,
    /// Today's day of the month; 0 before the month, `days` + 1 after it.
    pub day: u32,
    pub goal: Option<Decimal>,
    /// The month's budget once bills are out: what the days share.
    pub allowance: Decimal,
    /// How a weekday's allowance compares with an average day (Monday first). 1 when
    /// there isn't enough history.
    pub weights: [Decimal; 7],
    pub tiles: Vec<Tile>,
    /// What closed days put in, less what they took out.
    pub jar: Decimal,
    pub today: Option<Today>,
    /// Day before today, once closed, for the morning recap.
    pub yesterday: Option<Tile>,
    pub hand: Vec<Card>,
    pub offers: Vec<Card>,
    /// Cards won this year, newest first.
    pub shelf: Vec<Card>,
    /// The month is over: how it went.
    pub closed: Option<MonthClose>,
    /// Goals to pick from when there is none.
    pub suggestions: Vec<Decimal>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TileState {
    /// Closed at or under its allowance.
    Green,
    /// Closed over it.
    Red,
    Today,
    Ahead,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Tile {
    pub day: u32,
    pub date: NaiveDate,
    pub allowance: Decimal,
    pub spent: Decimal,
    /// Allowance − spent once the day is closed; 0 before.
    pub net: Decimal,
    pub state: TileState,
    pub coins: u8,
    /// Bills due that day.
    pub bills: Vec<String>,
    pub payday: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Today {
    pub allowance: Decimal,
    pub spent: Decimal,
    pub left: Decimal,
    /// Today's purchases, oldest first.
    pub taps: Vec<TodayTap>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TodayTap {
    pub merchant: String,
    pub amount: Decimal,
    pub category: String,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardStatus {
    Offer,
    Playing,
    Won,
    Lost,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CardArt {
    /// A cup that fills as you spend toward the cap.
    Cup,
    /// One square per day, ticked off as it passes clean.
    Days,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Card {
    pub id: String,
    pub week: NaiveDate,
    /// Set when it starts mid-week.
    pub from: Option<NaiveDate>,
    pub rule: CardRule,
    pub title: String,
    pub status: CardStatus,
    pub art: CardArt,
    /// The category it is about, for the icon.
    pub category: Option<String>,
    /// Cap cards: spent so far this week against the cap.
    pub spent: Decimal,
    /// No-spend cards: days passed clean, and how many it asks for.
    pub days_done: u32,
    pub days_total: u32,
    /// What you usually spend on this in a week (cap) or on these days (no-spend).
    pub usual: Decimal,
    /// Usual − spent, once won.
    pub saved: Decimal,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonthClose {
    pub jar: Decimal,
    pub goal: Option<Decimal>,
    pub hit: bool,
    pub green: u32,
    pub days: u32,
    pub cards_won: usize,
    pub cards_played: usize,
    /// Months in a row the jar reached the goal, this one included.
    pub streak: usize,
    /// A goal to offer for next month, when it differs from this one.
    pub next_goal: Option<Decimal>,
}

/// Money in the base currency, given a rate. Statement rows and taps carry their own.
pub type Rate<'a> = &'a dyn Fn(&str, NaiveDate) -> Option<Decimal>;

/// Spending that counts in the game, per date, base currency.
fn day_spend<'t>(l: &Ledger, txns: &'t [Txn], rate: Rate) -> BTreeMap<NaiveDate, Vec<(Decimal, &'t Txn)>> {
    let base = &l.settings.base_currency;
    let mut out: BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>> = BTreeMap::new();
    for t in txns {
        if !counts(&t.category) {
            continue;
        }
        let v = if t.currency == *base {
            Some(t.amount)
        } else {
            rate(&t.currency, t.date).map(|r| (t.amount * r).round_dp(2))
        };
        if let Some(v) = v {
            out.entry(t.date).or_default().push((v, t));
        }
    }
    out
}

fn counts(category: &str) -> bool {
    counts_as_spending(category) && category != "bills"
}

fn spent_on(
    by_day: &BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>>,
    d: NaiveDate,
    category: Option<&str>,
) -> Decimal {
    by_day
        .get(&d)
        .map(|v| v.iter().filter(|(_, t)| category.is_none_or(|c| t.category == c)).map(|(a, _)| *a).sum())
        .unwrap_or_default()
}

fn days_in(month: NaiveDate) -> u32 {
    ((month + Months::new(1)) - month).num_days() as u32
}

pub fn monday(d: NaiveDate) -> NaiveDate {
    d - Days::new(u64::from(d.weekday().num_days_from_monday()))
}

/// The month's budget without bills: the sum of limits, less the bills category's limit
/// (or, without one, the bills you listed).
pub fn monthly_allowance(l: &Ledger) -> Decimal {
    let b = &l.budget;
    let total: Decimal = b.limits.values().copied().sum();
    let bills = b.limits.get("bills").copied().unwrap_or_else(|| b.bills.iter().map(|x| x.amount).sum());
    (total - bills).max(Decimal::ZERO)
}

/// Weekday weights from the months before `month`: how a Saturday compares with your
/// average day. Clamped so no day is starved or doubled.
pub fn weekday_weights(by_day: &BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>>, month: NaiveDate) -> [Decimal; 7] {
    let from = month - Months::new(WEIGHT_MONTHS);
    let first_data = by_day.keys().next().copied();
    let start = match first_data {
        Some(f) if f > from => f,
        _ => from,
    };
    let mut total = [Decimal::ZERO; 7];
    let mut count = [0u32; 7];
    let mut d = start;
    while d < month {
        let w = d.weekday().num_days_from_monday() as usize;
        total[w] += spent_on(by_day, d, None);
        count[w] += 1;
        d = d + Days::new(1);
    }
    let days: u32 = count.iter().sum();
    let sum: Decimal = total.iter().copied().sum();
    if days < 28 || sum <= Decimal::ZERO {
        return [Decimal::ONE; 7];
    }
    let avg: Vec<Decimal> = (0..7)
        .map(|i| if count[i] == 0 { Decimal::ZERO } else { total[i] / Decimal::from(count[i]) })
        .collect();
    let mean = avg.iter().copied().sum::<Decimal>() / Decimal::from(7);
    let mut out = [Decimal::ONE; 7];
    for i in 0..7 {
        out[i] = (avg[i] / mean).clamp(dec("0.5"), dec("2")).round_dp(2);
    }
    out
}

fn dec(s: &str) -> Decimal {
    Decimal::from_str(s).expect("constant")
}

/// Each day's share of `allowance` for the month, by weekday weight. Sums to `allowance`.
pub fn split(allowance: Decimal, month: NaiveDate, weights: &[Decimal; 7]) -> Vec<Decimal> {
    let n = days_in(month);
    let w: Vec<Decimal> = (0..n)
        .map(|i| weights[(month + Days::new(u64::from(i))).weekday().num_days_from_monday() as usize])
        .collect();
    let total: Decimal = w.iter().copied().sum();
    if total <= Decimal::ZERO {
        return vec![Decimal::ZERO; n as usize];
    }
    let mut out: Vec<Decimal> = w.iter().map(|x| (allowance * x / total).round_dp(2)).collect();
    // Rounding leftovers go on the last day so the month adds up.
    let drift = allowance - out.iter().copied().sum::<Decimal>();
    if let Some(last) = out.last_mut() {
        *last += drift;
    }
    out
}

fn coins(net: Decimal, allowance: Decimal) -> u8 {
    if net <= Decimal::ZERO || allowance <= Decimal::ZERO {
        return 0;
    }
    let f = (net / allowance * Decimal::from(MAX_COINS)).ceil();
    f.to_u8().unwrap_or(MAX_COINS).clamp(1, MAX_COINS)
}

/// Days of the month money usually comes in: income of $500 or more in the two months
/// before `today`.
fn paydays(l: &Ledger, txns: &[Txn], today: NaiveDate) -> BTreeSet<u32> {
    let from = today - Days::new(62);
    let base = &l.settings.base_currency;
    txns.iter()
        .filter(|t| t.category == "income" && t.date >= from && t.date <= today)
        .filter(|t| t.currency == *base && -t.amount >= Decimal::from(500))
        .map(|t| t.date.day())
        .collect()
}

/// Jar for a month that is fully closed, with the same rules as the live view.
fn month_jar(
    l: &Ledger,
    by_day: &BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>>,
    month: NaiveDate,
) -> (Decimal, u32) {
    let weights = weekday_weights(by_day, month);
    let shares = split(monthly_allowance(l), month, &weights);
    let mut jar = Decimal::ZERO;
    let mut green = 0;
    for (i, a) in shares.iter().enumerate() {
        let net = *a - spent_on(by_day, month + Days::new(i as u64), None);
        jar += net;
        if net >= Decimal::ZERO {
            green += 1;
        }
    }
    (jar, green)
}

fn nice_floor(v: Decimal) -> Decimal {
    let step = if v < Decimal::from(100) { Decimal::from(5) } else { Decimal::from(10) };
    ((v / step).floor() * step).max(step)
}

const WEEKDAY: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];

/// Cards on offer for the week starting `week`, from the eight weeks before it. Always
/// the same for the same history, so a reload deals the same hand. Dealt mid-week, a
/// card covers `start` to Sunday, with the cap cut to the days left.
pub fn deal(
    by_day: &BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>>,
    week: NaiveDate,
    start: NaiveDate,
) -> Vec<PlayedCard> {
    let start = start.clamp(week, week + Days::new(6));
    let first = start.weekday().num_days_from_monday() as u8;
    let share = Decimal::from(7 - first) / Decimal::from(7);
    let from_day = (start > week).then_some(start);
    let from = week - Days::new(7 * DEAL_WEEKS);
    let weeks = Decimal::from(DEAL_WEEKS);
    let mut per_cat: BTreeMap<String, Decimal> = BTreeMap::new();
    let mut weekday_days: BTreeMap<String, u32> = BTreeMap::new();
    let mut per_weekday = [Decimal::ZERO; 7];
    let mut d = from;
    while d < week {
        let w = d.weekday().num_days_from_monday() as usize;
        let mut cats_today = BTreeSet::new();
        for (v, t) in by_day.get(&d).into_iter().flatten() {
            *per_cat.entry(t.category.clone()).or_default() += *v;
            per_weekday[w] += *v;
            cats_today.insert(t.category.clone());
        }
        if w < 5 {
            for c in cats_today {
                *weekday_days.entry(c).or_default() += 1;
            }
        }
        d = d + Days::new(1);
    }
    let spend_of = per_cat.clone();
    let mut cards = Vec::new();
    // Caps on the two everyday categories you spend most on, at about 70% of usual.
    let mut cats: Vec<(String, Decimal)> = per_cat
        .into_iter()
        .filter(|(c, _)| !matches!(c.as_str(), "other" | "fees" | "travel" | "health"))
        .map(|(c, v)| (c, v / weeks))
        .filter(|(_, v)| *v >= Decimal::from(15))
        .collect();
    cats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    // Small, frequent categories make the best caps; skip groceries-sized ones first.
    cats.sort_by_key(|(_, v)| *v > Decimal::from(150));
    for (c, avg) in cats.iter().take(2) {
        let amount = nice_floor(*avg * dec("0.7") * share);
        cards.push(PlayedCard {
            id: format!("{start}-cap-{c}"),
            week,
            from: from_day,
            rule: CardRule::Cap { category: c.clone(), amount },
        });
    }
    // Weekdays without the category you most often buy on a weekday.
    if let Some((c, n)) = weekday_days
        .iter()
        .filter(|(c, _)| matches!(c.as_str(), "dining" | "coffee" | "shopping" | "entertainment"))
        .max_by(|a, b| {
            let v = |c: &String| spend_of.get(c).copied().unwrap_or_default();
            a.1.cmp(b.1).then(v(a.0).cmp(&v(b.0))).then(b.0.cmp(a.0))
        })
    {
        let weekdays: Vec<u8> = (first..5).collect();
        if *n >= 2 * DEAL_WEEKS as u32 && weekdays.len() >= 2 {
            cards.push(PlayedCard {
                id: format!("{start}-weekdays-{c}"),
                week,
                from: from_day,
                rule: CardRule::NoSpend { category: Some(c.clone()), weekdays },
            });
        }
    }
    // A whole day with no spending: the weekend day you spend least on.
    let day = if per_weekday[6] <= per_weekday[5] && first <= 6 { 6 } else { 5 };
    if day >= first {
        cards.push(PlayedCard {
            id: format!("{start}-nospend-{day}"),
            week,
            from: from_day,
            rule: CardRule::NoSpend { category: None, weekdays: vec![day] },
        });
    }
    cards
}

fn usual(by_day: &BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>>, card: &PlayedCard) -> Decimal {
    let from = card.week - Days::new(7 * DEAL_WEEKS);
    let first = card.start().weekday().num_days_from_monday() as u8;
    let mut sum = Decimal::ZERO;
    let mut d = from;
    while d < card.week {
        let w = d.weekday().num_days_from_monday() as u8;
        match &card.rule {
            CardRule::Cap { category, .. } if w >= first => sum += spent_on(by_day, d, Some(category)),
            CardRule::Cap { .. } => {}
            CardRule::NoSpend { category, weekdays } if weekdays.contains(&w) => {
                sum += spent_on(by_day, d, category.as_deref())
            }
            CardRule::NoSpend { .. } => {}
        }
        d = d + Days::new(1);
    }
    (sum / Decimal::from(DEAL_WEEKS)).round_dp(2)
}

fn title(rule: &CardRule) -> String {
    match rule {
        CardRule::Cap { category, amount } => format!("{} under ${}", label(category), amount.normalize()),
        CardRule::NoSpend { category, weekdays } => {
            let days = if weekdays.len() >= 2 && weekdays.windows(2).all(|w| w[1] == w[0] + 1) {
                format!(
                    "{}–{}",
                    &WEEKDAY[weekdays[0] as usize][..3],
                    &WEEKDAY[weekdays[weekdays.len() - 1] as usize][..3]
                )
            } else if weekdays.len() == 1 {
                WEEKDAY[weekdays[0] as usize].to_string()
            } else {
                weekdays.iter().map(|d| &WEEKDAY[*d as usize][..3]).collect::<Vec<_>>().join(", ")
            };
            match category {
                Some(c) => format!("No {} {days}", label(c).to_lowercase()),
                None => format!("No-spend {days}"),
            }
        }
    }
}

/// How a card is doing as of `today`.
pub fn score(
    by_day: &BTreeMap<NaiveDate, Vec<(Decimal, &Txn)>>,
    card: &PlayedCard,
    played: bool,
    today: NaiveDate,
) -> Card {
    let end = card.week + Days::new(7);
    let over = today >= end;
    let mut c = Card {
        id: card.id.clone(),
        week: card.week,
        from: card.from,
        rule: card.rule.clone(),
        title: title(&card.rule),
        status: CardStatus::Offer,
        art: CardArt::Days,
        category: None,
        spent: Decimal::ZERO,
        days_done: 0,
        days_total: 0,
        usual: usual(by_day, card),
        saved: Decimal::ZERO,
    };
    let lost = match &card.rule {
        CardRule::Cap { category, amount } => {
            c.art = CardArt::Cup;
            c.category = Some(category.clone());
            let first = (card.start() - card.week).num_days() as u64;
            c.spent = (first..7).map(|i| spent_on(by_day, card.week + Days::new(i), Some(category))).sum();
            c.spent > *amount
        }
        CardRule::NoSpend { category, weekdays } => {
            c.category = category.clone();
            c.days_total = weekdays.len() as u32;
            let mut broke = false;
            for w in weekdays {
                let d = card.week + Days::new(u64::from(*w));
                let s = spent_on(by_day, d, category.as_deref());
                c.spent += s;
                if s > Decimal::ZERO && d <= today {
                    broke = true;
                } else if d < today {
                    c.days_done += 1;
                }
            }
            broke
        }
    };
    if played {
        c.status = if lost {
            CardStatus::Lost
        } else if over {
            CardStatus::Won
        } else {
            CardStatus::Playing
        };
    }
    if c.status == CardStatus::Won {
        c.saved = (c.usual - c.spent).max(Decimal::ZERO);
    }
    c
}

/// The game for the month containing `month`, as of `today`.
pub fn view(l: &Ledger, txns: &[Txn], month: NaiveDate, today: NaiveDate, rate: Rate) -> GameView {
    let start = first_of(month);
    let n = days_in(start);
    let by_day = day_spend(l, txns, rate);
    let day = if today < start {
        0
    } else if today >= start + Months::new(1) {
        n + 1
    } else {
        today.day()
    };
    let allowance = monthly_allowance(l);
    let weights = weekday_weights(&by_day, start);
    let shares = split(allowance, start, &weights);
    let pays = paydays(l, txns, today);
    let mut tiles = Vec::with_capacity(n as usize);
    let mut jar = Decimal::ZERO;
    for i in 0..n {
        let date = start + Days::new(u64::from(i));
        let d = i + 1;
        let a = shares[i as usize];
        let spent = spent_on(&by_day, date, None);
        let state = if d < day {
            if spent <= a {
                TileState::Green
            } else {
                TileState::Red
            }
        } else if d == day {
            TileState::Today
        } else {
            TileState::Ahead
        };
        let closed = matches!(state, TileState::Green | TileState::Red);
        let net = if closed { a - spent } else { Decimal::ZERO };
        jar += net;
        let bills = l
            .budget
            .bills
            .iter()
            .filter(|b| b.day.map(|x| x.min(n)) == Some(d))
            .map(|b| b.name.clone())
            .collect();
        tiles.push(Tile {
            day: d,
            date,
            allowance: a,
            spent: if d <= day { spent } else { Decimal::ZERO },
            net,
            state,
            coins: coins(net, a),
            bills,
            payday: pays.contains(&d) || (d == n && pays.iter().any(|p| *p > n)),
        });
    }
    let today_view = tiles.iter().find(|t| t.state == TileState::Today).map(|t| Today {
        allowance: t.allowance,
        spent: t.spent,
        left: t.allowance - t.spent,
        taps: by_day
            .get(&t.date)
            .map(|v| {
                v.iter()
                    .rev()
                    .map(|(a, x)| TodayTap {
                        merchant: x.merchant.clone(),
                        amount: *a,
                        category: x.category.clone(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    });
    let yesterday = tiles.iter().rev().find(|t| t.day + 1 == day && t.state != TileState::Ahead).cloned();

    // Cards: everything played, scored; offers for this week when looking at it.
    let game = &l.budget.game;
    let this_week = monday(today);
    let scored: Vec<Card> = game.cards.iter().map(|c| score(&by_day, c, true, today)).collect();
    let in_month = |c: &Card| c.week + Days::new(6) >= start && c.week < start + Months::new(1);
    let hand: Vec<Card> = scored.iter().filter(|c| c.week == this_week && in_month(c)).cloned().collect();
    let offers: Vec<Card> = if day >= 1 && day <= n && hand.len() < ledger::mutation::MAX_CARDS_A_WEEK {
        deal(&by_day, this_week, today)
            .into_iter()
            .filter(|c| !game.passed.contains(&c.id) && !game.cards.iter().any(|p| p.id == c.id))
            // Mid-week, a card you have already broken isn't worth offering.
            .filter(|c| score(&by_day, c, true, today).status != CardStatus::Lost)
            .take(3)
            .map(|c| score(&by_day, &c, false, today))
            .collect()
    } else {
        Vec::new()
    };
    let mut shelf: Vec<Card> = scored
        .iter()
        .filter(|c| c.status == CardStatus::Won && c.week.year() == today.year())
        .cloned()
        .collect();
    shelf.reverse();

    let closed = (day > n).then(|| {
        let played: Vec<&Card> = scored.iter().filter(|c| in_month(c)).collect();
        let hit_at = |m: NaiveDate| game.goal.is_some_and(|g| month_jar(l, &by_day, m).0 >= g);
        let hit = game.goal.is_some_and(|g| jar >= g);
        let mut streak = 0;
        if hit {
            streak = 1;
            let mut m = start;
            while streak < 12 {
                m = m - Months::new(1);
                if !by_day.keys().next().is_some_and(|f| *f < m) || !hit_at(m) {
                    break;
                }
                streak += 1;
            }
        }
        let missed_two = !hit && game.goal.is_some() && !hit_at(start - Months::new(1));
        let next_goal = game.goal.and_then(|g| {
            let step = Decimal::from(10);
            if streak >= STEP_UP_AFTER {
                Some(((g * dec("1.05")) / step).ceil() * step)
            } else if missed_two {
                Some(((g * dec("0.95")) / step).floor() * step)
            } else {
                None
            }
            .filter(|n| *n != g)
        });
        MonthClose {
            jar,
            goal: game.goal,
            hit,
            green: tiles.iter().filter(|t| t.state == TileState::Green).count() as u32,
            days: n,
            cards_won: played.iter().filter(|c| c.status == CardStatus::Won).count(),
            cards_played: played.len(),
            streak,
            next_goal,
        }
    });

    let step = Decimal::from(50);
    let suggestions = if allowance > Decimal::ZERO {
        [dec("0.1"), dec("0.2"), dec("0.3")]
            .iter()
            .map(|f| ((allowance * f) / step).round() * step)
            .filter(|v| *v > Decimal::ZERO)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    } else {
        Vec::new()
    };

    GameView {
        month: start,
        days: n,
        day,
        goal: game.goal,
        allowance,
        weights,
        tiles,
        jar,
        today: today_view,
        yesterday,
        hand,
        offers,
        shelf,
        closed,
        suggestions,
    }
}

fn money(v: Decimal) -> String {
    let v = v.round_dp(0);
    let neg = v.is_sign_negative();
    let digits = v.abs().trunc().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    format!("{}${out}", if neg { "-" } else { "" })
}

fn cents(v: Decimal) -> String {
    let s = format!("{:.2}", v.round_dp(2));
    format!("${s}")
}

fn jar_line(g: &GameView) -> String {
    match g.goal {
        Some(goal) if g.jar >= goal => format!("Jar {} · past the {} goal", money(g.jar), money(goal)),
        Some(goal) => format!("Jar {} of {}", money(g.jar.max(Decimal::ZERO)), money(goal)),
        None => format!("Jar {}", money(g.jar.max(Decimal::ZERO))),
    }
}

/// The notification body after a tap: what's left today, the jar, and a card it touched.
/// The Shortcut puts the merchant and amount in the title, so they aren't repeated here.
pub fn tap_line(g: &GameView, category: Option<&str>) -> String {
    let Some(t) = &g.today else {
        return jar_line(g);
    };
    let head = if t.left >= Decimal::ZERO {
        format!("{} left today", money(t.left))
    } else {
        format!("{} over today, out of the jar", money(-t.left))
    };
    let card = category.and_then(|cat| {
        g.hand.iter().find(|c| c.status == CardStatus::Playing && c.category.as_deref() == Some(cat))
    });
    let card = card.map(|c| match &c.rule {
        CardRule::Cap { amount, .. } if c.spent <= *amount => {
            format!(" · {}: {} left", c.title, cents(*amount - c.spent))
        }
        _ => format!(" · {}: lost", c.title),
    });
    format!("{head}\n{}{}", jar_line(g), card.unwrap_or_default())
}

/// The same, before a tap is known (iCloud mode: the phone reads it from a file).
pub fn left_line(g: &GameView) -> String {
    match &g.today {
        Some(t) if t.left >= Decimal::ZERO => {
            format!("{} left today before this tap · {}", money(t.left), jar_line(g))
        }
        Some(t) => format!("{} over today before this tap · {}", money(-t.left), jar_line(g)),
        None => jar_line(g),
    }
}

/// The morning recap: how yesterday closed.
pub fn recap_line(g: &GameView) -> Option<String> {
    let Some(y) = g.yesterday.as_ref() else {
        // The 1st: nothing has closed yet this month.
        let t = g.today.as_ref()?;
        return Some(format!("New month: {} to spend today\n{}", money(t.allowance), jar_line(g)));
    };
    let name = WEEKDAY[y.date.weekday().num_days_from_monday() as usize];
    let how = if y.net >= Decimal::ZERO {
        format!("{name} closed: {} into the jar", money(y.net))
    } else {
        format!("{name} closed {} over, out of the jar", money(-y.net))
    };
    let today = g.today.as_ref().map(|t| format!(" · today: {}", money(t.allowance))).unwrap_or_default();
    Some(format!("{how}\n{}{today}", jar_line(g)))
}

/// Weekday name, for the UI and tests.
pub fn weekday_name(w: Weekday) -> &'static str {
    WEEKDAY[w.num_days_from_monday() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::Status;
    use ledger::{Bill, Every, Game};
    use rust_decimal_macros::dec;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn txn(date: &str, amount: Decimal, category: &str, merchant: &str) -> Txn {
        Txn {
            id: format!("{date}-{merchant}-{amount}"),
            account: Some("visa".into()),
            card: None,
            date: d(date),
            amount,
            currency: "CAD".into(),
            merchant: merchant.into(),
            description: None,
            category: category.into(),
            status: Status::Posted,
            tap_id: None,
            manual_id: None,
            tap_amount: None,
            tap_currency: None,
            tap_date: None,
        }
    }

    fn ledger(goal: Option<Decimal>) -> Ledger {
        let mut l = ledger::io::parse("version: 1\nsettings: { base_currency: CAD }\n").unwrap();
        let b = &mut l.budget;
        b.limits.insert("dining".into(), dec!(300));
        b.limits.insert("coffee".into(), dec!(60));
        b.limits.insert("bills".into(), dec!(2000));
        b.bills = vec![Bill {
            name: "Rent".into(),
            amount: dec!(2000),
            every: Every::Month,
            category: "bills".into(),
            matches: None,
            day: Some(1),
        }];
        b.game = Game { goal, ..Default::default() };
        l
    }

    const NO_FX: &dyn Fn(&str, NaiveDate) -> Option<Decimal> = &|_, _| None;

    #[test]
    fn allowance_leaves_bills_out_and_splits_evenly_without_history() {
        let l = ledger(None);
        assert_eq!(monthly_allowance(&l), dec!(360));
        let g = view(&l, &[], d("2026-11-01"), d("2026-11-10"), NO_FX);
        assert_eq!(g.weights, [Decimal::ONE; 7]);
        assert_eq!(g.tiles.iter().map(|t| t.allowance).sum::<Decimal>(), dec!(360));
        assert_eq!(g.tiles[0].allowance, dec!(12));
        assert_eq!(g.tiles[0].bills, vec!["Rent".to_string()]);
    }

    #[test]
    fn closed_days_fill_the_jar_and_overspending_takes_from_it() {
        let l = ledger(Some(dec!(100)));
        let txns = vec![
            txn("2026-11-01", dec!(2000), "bills", "Landlord"), // never counts
            txn("2026-11-02", dec!(5), "coffee", "Pilot"),
            txn("2026-11-03", dec!(30), "dining", "Pai"),
            txn("2026-11-04", dec!(4), "coffee", "Tim Hortons"), // today, not closed
        ];
        let g = view(&l, &txns, d("2026-11-01"), d("2026-11-04"), NO_FX);
        let s: Vec<TileState> = g.tiles.iter().take(5).map(|t| t.state).collect();
        assert_eq!(
            s,
            [TileState::Green, TileState::Green, TileState::Red, TileState::Today, TileState::Ahead]
        );
        // 12 + 7 − 18 = 1
        assert_eq!(g.jar, dec!(1));
        assert_eq!(g.tiles[0].coins, MAX_COINS);
        assert_eq!(g.tiles[1].coins, 3);
        let t = g.today.as_ref().unwrap();
        assert_eq!((t.spent, t.left), (dec!(4), dec!(8)));
        assert_eq!(g.yesterday.as_ref().unwrap().net, dec!(-18));
        assert!(g.closed.is_none());
    }

    #[test]
    fn weekends_get_more_when_you_spend_more_on_weekends() {
        let l = ledger(None);
        let mut txns = vec![];
        let mut day = d("2026-08-01");
        while day < d("2026-11-01") {
            let amt = if day.weekday().num_days_from_monday() >= 5 { dec!(30) } else { dec!(10) };
            txns.push(txn(&day.to_string(), amt, "dining", "X"));
            day = day + Days::new(1);
        }
        let g = view(&l, &txns, d("2026-11-01"), d("2026-11-10"), NO_FX);
        assert!(g.weights[5] > dec!(1.5) && g.weights[0] < dec!(0.8), "{:?}", g.weights);
        let sat = g.tiles.iter().find(|t| t.date.weekday() == Weekday::Sat).unwrap();
        let mon = g.tiles.iter().find(|t| t.date.weekday() == Weekday::Mon).unwrap();
        assert!(sat.allowance > mon.allowance * dec!(2));
        assert_eq!(g.tiles.iter().map(|t| t.allowance).sum::<Decimal>(), dec!(360));
    }

    #[test]
    fn a_cap_card_is_won_at_week_end_and_lost_the_moment_it_goes_over() {
        let mut l = ledger(None);
        let week = d("2026-11-16");
        let card = PlayedCard {
            id: "c".into(),
            week,
            from: None,
            rule: CardRule::Cap { category: "coffee".into(), amount: dec!(25) },
        };
        l.budget.game.cards.push(card.clone());
        let mut txns = vec![txn("2026-11-17", dec!(18.75), "coffee", "Pilot")];
        let g = view(&l, &txns, week, d("2026-11-18"), NO_FX);
        assert_eq!(g.hand[0].status, CardStatus::Playing);
        assert_eq!(g.hand[0].title, "Coffee under $25");
        let line = tap_line(&g, Some("coffee"));
        assert!(line.contains("Coffee under $25: $6.25 left"), "{line}");
        let g = view(&l, &txns, week, d("2026-11-23"), NO_FX);
        assert_eq!(g.shelf[0].status, CardStatus::Won);
        txns.push(txn("2026-11-19", dec!(7), "coffee", "Pilot"));
        let g = view(&l, &txns, week, d("2026-11-19"), NO_FX);
        assert_eq!(g.hand[0].status, CardStatus::Lost);
    }

    #[test]
    fn no_spend_days_count_down_and_break_on_any_spending() {
        let mut l = ledger(None);
        let week = d("2026-11-16");
        l.budget.game.cards.push(PlayedCard {
            id: "w".into(),
            week,
            from: None,
            rule: CardRule::NoSpend { category: Some("dining".into()), weekdays: vec![0, 1, 2, 3, 4] },
        });
        let txns = vec![txn("2026-11-21", dec!(40), "dining", "Weekend brunch")];
        let g = view(&l, &txns, week, d("2026-11-19"), NO_FX);
        assert_eq!(
            (g.hand[0].days_done, g.hand[0].days_total, g.hand[0].status),
            (3, 5, CardStatus::Playing)
        );
        assert_eq!(g.hand[0].title, "No dining Mon–Fri");
        let txns = vec![txn("2026-11-18", dec!(12), "dining", "Lunch")];
        let g = view(&l, &txns, week, d("2026-11-19"), NO_FX);
        assert_eq!(g.hand[0].status, CardStatus::Lost);
    }

    #[test]
    fn deals_the_same_cards_from_the_same_history_and_skips_passed_ones() {
        let mut l = ledger(None);
        let mut txns = vec![];
        let mut day = d("2026-09-14");
        while day < d("2026-11-16") {
            txns.push(txn(&day.to_string(), dec!(5.5), "coffee", "Pilot"));
            if day.weekday().num_days_from_monday() < 5 {
                txns.push(txn(&day.to_string(), dec!(14), "dining", "Lunch"));
            }
            day = day + Days::new(1);
        }
        let g = view(&l, &txns, d("2026-11-01"), d("2026-11-16"), NO_FX);
        let titles: Vec<&str> = g.offers.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(titles, ["Dining under $45", "Coffee under $25", "No dining Mon–Fri"]);
        assert!(g.offers.iter().all(|c| c.status == CardStatus::Offer));
        l.budget.game.passed.push(g.offers[0].id.clone());
        let g2 = view(&l, &txns, d("2026-11-01"), d("2026-11-16"), NO_FX);
        assert_eq!(g2.offers.len(), 3);
        assert_eq!(g2.offers[2].title, "No-spend Sunday");
        assert_eq!(g2.offers[0].id, g.offers[1].id);
    }

    #[test]
    fn dealt_mid_week_a_card_covers_the_days_left_with_a_smaller_cap() {
        let l = ledger(None);
        let mut txns = vec![];
        let mut day = d("2026-09-14");
        while day < d("2026-11-16") {
            txns.push(txn(&day.to_string(), dec!(5.5), "coffee", "Pilot"));
            day = day + Days::new(1);
        }
        // Thursday: four days left, so about 4/7 of the Monday cap.
        let g = view(&l, &txns, d("2026-11-01"), d("2026-11-19"), NO_FX);
        let cap = g.offers.iter().find(|c| c.title.starts_with("Coffee")).unwrap();
        assert_eq!(cap.title, "Coffee under $15");
        assert_eq!(cap.id, "2026-11-19-cap-coffee");
        assert!(g.offers.iter().all(|c| !c.title.contains("Mon")), "no weekday card after Thursday starts");
        // Coffee bought on Monday doesn't count against a card played on Thursday.
        let mut l = l;
        let card = PlayedCard {
            id: cap.id.clone(),
            week: cap.week,
            from: Some(d("2026-11-19")),
            rule: cap.rule.clone(),
        };
        l.budget.game.cards.push(card);
        txns.push(txn("2026-11-16", dec!(40), "coffee", "Big Monday"));
        let g = view(&l, &txns, d("2026-11-01"), d("2026-11-19"), NO_FX);
        assert_eq!(g.hand[0].status, CardStatus::Playing);
    }

    #[test]
    fn a_closed_month_reports_the_goal_and_offers_a_bigger_one_after_three_hits() {
        let l = ledger(Some(dec!(100)));
        // Spend $5 a day for four months: $12 − $5 ≈ $7 a day into the jar, ~$210 a month.
        let mut txns = vec![];
        let mut day = d("2026-07-01");
        while day < d("2026-11-01") {
            txns.push(txn(&day.to_string(), dec!(5), "coffee", "Pilot"));
            day = day + Days::new(1);
        }
        let g = view(&l, &txns, d("2026-10-01"), d("2026-11-03"), NO_FX);
        let c = g.closed.unwrap();
        assert!(c.hit && c.jar > dec!(100));
        assert_eq!((c.green, c.days), (31, 31));
        assert!(c.streak >= 3, "{c:?}");
        assert_eq!(c.next_goal, Some(dec!(110)));
    }

    #[test]
    fn notification_lines_read_plainly() {
        let l = ledger(Some(dec!(600)));
        let txns = vec![
            txn("2026-11-02", dec!(2), "coffee", "Pilot"),
            txn("2026-11-03", dec!(4.6), "transport", "Presto"),
        ];
        let g = view(&l, &txns, d("2026-11-01"), d("2026-11-03"), NO_FX);
        assert_eq!(tap_line(&g, None), "$7 left today\nJar $22 of $600");
        assert_eq!(recap_line(&g).unwrap(), "Monday closed: $10 into the jar\nJar $22 of $600 · today: $12");
        assert_eq!(left_line(&g), "$7 left today before this tap · Jar $22 of $600");
        let first = view(&l, &txns, d("2026-11-01"), d("2026-11-01"), NO_FX);
        assert_eq!(recap_line(&first).unwrap(), "New month: $12 to spend today\nJar $0 of $600");
        let tuesday = view(&l, &txns, d("2026-11-01"), d("2026-11-04"), NO_FX);
        assert!(recap_line(&tuesday).unwrap().starts_with("Tuesday closed"), "{:?}", recap_line(&tuesday));
    }
}
