//! Sample budget for `worthwatch seed`: six months of TD Visa and chequing statement
//! rows, Apple Pay taps (most matched, the newest still waiting, one never posted), and
//! a card nobody has linked yet. Deterministic, so screenshots and tests are stable.

use budget::store::Record;
use budget::{CsvRow, Tap};
use chrono::{DateTime, Datelike, Days, FixedOffset, Months, NaiveDate, TimeZone, Utc};
use ledger::{Bill, Budget, Every, Feed, FeedMode, TxnSource};
use rust_decimal::Decimal;

pub const CARD: &str = "TD Aeroplan Visa";

fn d(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

pub fn seed_budget_settings() -> Budget {
    let mut b = Budget::default();
    for (c, v) in [
        ("groceries", 950),
        ("dining", 400),
        ("coffee", 110),
        ("transport", 250),
        ("shopping", 450),
        ("entertainment", 150),
        ("health", 150),
        ("bills", 2200),
    ] {
        b.limits.insert(c.into(), Decimal::from(v));
    }
    b.cards.insert(CARD.into(), "td-visa".into());
    b.sources.insert("td-visa".into(), TxnSource::ApplePayStatement);
    let bill = |name: &str, cents: i64, day: u32, matches: Option<&str>| Bill {
        name: name.into(),
        amount: d(cents),
        every: Every::Month,
        category: "bills".into(),
        matches: matches.map(str::to_string),
        day: Some(day),
    };
    b.bills = vec![
        bill("Rent", 195000, 1, Some("E-TFR")),
        bill("Rogers", 9500, 4, None),
        bill("Hydro", 8000, 22, Some("HYDRO")),
    ];
    b.rules =
        vec![ledger::CategoryRule { contains: "LANDLORD".into(), category: "bills".into(), account: None }];
    b.feed = Feed { mode: FeedMode::Icloud, ..Default::default() };
    b
}

/// (tap merchant, statement text, cents, tipped)
const SHOPS: &[(&str, &str, i64, bool)] = &[
    ("Loblaws", "LOBLAWS #1012 TORONTO ON", 6218, false),
    ("Blue Bottle Coffee", "SQ *BLUE BOTTLE TORONTO ON", 725, false),
    ("Presto", "PRESTO FARE/TORONTO", 460, false),
    ("Pai Northern Thai", "PAI NORTHERN THAI KITCH TORONTO", 4800, true),
    ("No Frills", "NO FRILLS #3121 TORONTO ON", 4433, false),
    ("Tim Hortons", "TIM HORTONS #2231", 389, false),
    ("Shoppers Drug Mart", "SHOPPERS DRUG MART #1234", 2410, false),
    ("Uber", "UBER CANADA/UBERTRIP", 1840, false),
    ("Farm Boy", "FARM BOY #18 TORONTO", 3875, false),
    ("Sushi Gen", "SUSHI GEN TORONTO ON", 3600, true),
    ("Indigo", "INDIGO 0921 TORONTO", 3164, false),
    ("Cineplex", "CINEPLEX #7032", 2650, false),
    ("Starbucks", "STARBUCKS 04512", 615, false),
    ("Metro", "METRO 412 TORONTO", 5120, false),
    ("Esso", "ESSO CIRCLE K TORONTO", 6100, false),
];

fn at(day: NaiveDate, hour: u32) -> DateTime<FixedOffset> {
    let tz = FixedOffset::west_opt(5 * 3600).expect("offset");
    tz.from_local_datetime(&day.and_hms_opt(hour, 12, 0).expect("time")).single().expect("time")
}

fn tap(merchant: &str, cents: i64, when: DateTime<FixedOffset>, card: &str) -> Tap {
    Tap {
        id: format!("tap-seed-{}-{}", when.format("%Y%m%d%H"), merchant.replace(' ', "")),
        at: when,
        merchant: merchant.into(),
        name: None,
        amount: d(cents),
        currency: None,
        card: card.into(),
        received: when.to_utc().min(Utc::now()),
    }
}

fn row(account: &str, date: NaiveDate, cents: i64, text: &str, n: usize) -> CsvRow {
    CsvRow {
        id: format!("csv-seed-{account}-{date}-{n}"),
        account: account.into(),
        date,
        amount: d(cents),
        description: text.into(),
        imported: Utc::now(),
    }
}

pub fn seed_transactions(today: NaiveDate) -> Vec<Record> {
    let mut out = Vec::new();
    let this = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).expect("first");
    let mut n = 0;
    for back in (0..6u32).rev() {
        let start = this - Months::new(back);
        let next = start + Months::new(1);
        let end = if back == 0 { today } else { next.pred_opt().expect("day") };
        // Chequing: pay, rent, phone, hydro, the card bill.
        let chq = |day: u32, cents: i64, text: &str, out: &mut Vec<Record>, n: &mut usize| {
            if let Some(date) = start.with_day(day).filter(|x| *x <= end) {
                *n += 1;
                out.push(Record::Csv(row("td-chq", date, cents, text, *n)));
            }
        };
        chq(1, 195000, "SEND E-TFR ***Lk2 LANDLORD", &mut out, &mut n);
        chq(4, 9500, "ROGERS WIRELESS PAD", &mut out, &mut n);
        chq(15, -285000, "PAYROLL DEP ACME CORP", &mut out, &mut n);
        chq(22, 7800 + i64::from(back) * 250, "TORONTO HYDRO PAD", &mut out, &mut n);
        chq(30, -285000, "PAYROLL DEP ACME CORP", &mut out, &mut n);
        chq(16, 120455 - i64::from(back) * 3100, "TD VISA PAYMENT", &mut out, &mut n);
        // Card: two or three purchases most days; past months posted, recent ones taps.
        let mut day = start;
        let mut i = (back as usize) * 7;
        while day <= end {
            let per_day = 1 + (day.day() as usize + back as usize) % 3;
            for k in 0..per_day {
                i += 1;
                let (merchant, text, base, tipped) = SHOPS[(i * 7 + k * 3) % SHOPS.len()];
                // Spend varies a little by month and slot; dining runs high in the current month.
                let cents = base + ((i as i64 * 37) % 9) * 50 + if back == 0 && tipped { 900 } else { 0 };
                let hour = 8 + ((i * 5) % 12) as u32;
                let when = at(day, hour);
                let age = (today - day).num_days();
                let posts = day + Days::new(1 + (i % 2) as u64);
                // Every 4th purchase is a card swipe or online: statement only.
                let tapped = !i.is_multiple_of(4);
                if tapped {
                    out.push(Record::Tap(tap(merchant, cents, when, CARD)));
                }
                // The statement for the current month was imported 3 days ago; newer taps wait.
                let imported = back > 0 || age > 3;
                if imported && posts <= today {
                    let posted = if tipped && tapped { cents + cents * 18 / 100 } else { cents };
                    n += 1;
                    out.push(Record::Csv(row("td-visa", posts, posted, text, n)));
                }
            }
            day = day.succ_opt().expect("day");
        }
    }
    // A tap twelve days ago that never posted (refunded at the counter).
    out.push(Record::Tap(tap("Apple.com/bill", 1468, at(today - Days::new(12), 10), CARD)));
    // A new card, tapped twice this week, not linked to an account yet.
    for (k, (m, c)) in [("Pilot Coffee", 650), ("LCBO", 3295)].into_iter().enumerate() {
        let day = today - Days::new(k as u64 * 2);
        out.push(Record::Tap(tap(m, c, at(day, 17), "Amex Cobalt")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn the_sample_month_has_every_kind_of_row() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let mut l = crate::seed::seed_ledger(today);
        l.budget = seed_budget_settings();
        ledger::validate::validate(&l).unwrap();
        let recs = seed_transactions(today);
        let taps: Vec<Tap> =
            recs.iter().filter_map(|r| if let Record::Tap(t) = r { Some(t.clone()) } else { None }).collect();
        let rows: Vec<CsvRow> =
            recs.iter().filter_map(|r| if let Record::Csv(c) = r { Some(c.clone()) } else { None }).collect();
        let txns = budget::reconcile(&l, &taps, &rows, &BTreeMap::new(), today);
        let v = budget::month(&l, &txns, today, today, &|_, _| None);
        let has = |s: budget::Status| v.txns.iter().any(|t| t.txn.status == s);
        assert!(
            has(budget::Status::Confirmed) && has(budget::Status::Provisional) && has(budget::Status::Posted)
        );
        assert_eq!(v.review.len(), 1);
        assert_eq!(v.unlinked.len(), 1);
        assert!(v.txns.iter().any(|t| t.txn.tap_amount.is_some()), "a tipped meal");
        assert!(v.bills.iter().filter(|b| b.paid_on.is_some()).count() >= 2);
        assert!(v.spent > Decimal::from(1000) && v.spent < v.budget, "{}", v.spent);
        assert!(v.categories[0].history.iter().all(|h| *h > Decimal::ZERO));
    }
}
