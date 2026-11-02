//! `worthwatch seed`: write a realistic 14-month ledger, plus a `quotes.json` that
//! looks like three months of fetched daily closes and FX, so every screen can be
//! designed against real-looking data. Deterministic (no randomness crate).

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{Datelike, Months, NaiveDate, Weekday};
use ledger::{Account, AccountKind, Event, Ledger, LedgerError, Position, PositionChange};
use providers::QuoteCache;
use rust_decimal::Decimal;

fn acct(id: &str, name: &str, inst: Option<&str>, kind: AccountKind, cur: &str) -> Account {
    Account {
        id: id.into(),
        name: name.into(),
        institution: inst.map(str::to_string),
        kind,
        currency: cur.into(),
        tags: vec![],
        positions: None,
    }
}

fn d(cents: i64) -> Decimal {
    Decimal::new(cents, 2)
}

/// Tiny deterministic noise in [-1, 1).
fn wobble(seed: u64) -> Decimal {
    let x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    Decimal::new(((x >> 33) % 2000) as i64 - 1000, 3)
}

pub fn seed_ledger(today: NaiveDate) -> Ledger {
    let mut l = Ledger::starter("CAD");
    l.settings.fx.insert("USD".into(), d(138));
    let mut tfsa = acct("ws-tfsa", "Wealthsimple TFSA", Some("Wealthsimple"), AccountKind::Tfsa, "CAD");
    tfsa.positions = Some(vec![
        Position { symbol: "XEQT.TO".into(), units: d(42650), book_cost: Some(d(1124000)) },
        Position { symbol: "VFV.TO".into(), units: d(62000), book_cost: Some(d(7100000)) },
    ]);
    let mut rrsp = acct("qt-rrsp", "Questrade RRSP", Some("Questrade"), AccountKind::Rrsp, "USD");
    rrsp.positions =
        Some(vec![Position { symbol: "VTI".into(), units: d(31000), book_cost: Some(d(6200000)) }]);
    l.accounts = vec![
        acct("td-chq", "TD Chequing", Some("TD"), AccountKind::Chequing, "CAD"),
        acct("eq-sav", "EQ Savings", Some("EQ Bank"), AccountKind::Savings, "CAD"),
        acct("rbc-usd", "USD Chequing", Some("RBC"), AccountKind::Chequing, "USD"),
        tfsa,
        rrsp,
        acct("ws-fhsa", "FHSA", Some("Wealthsimple"), AccountKind::Fhsa, "CAD"),
        acct("condo", "Condo", None, AccountKind::Property, "CAD"),
        acct("td-mortgage", "Mortgage", Some("TD"), AccountKind::Mortgage, "CAD"),
        acct("td-visa", "TD Visa", Some("TD"), AccountKind::CreditCard, "CAD"),
    ];
    l.accounts.iter_mut().find(|a| a.id == "condo").unwrap().tags = vec!["illiquid".into()];

    // Month-ends from 14 months ago up to last month-end, then "today" partial.
    let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap();
    let mut months: Vec<NaiveDate> = (1..=14)
        .rev()
        .map(|n| first.checked_sub_months(Months::new(n - 1)).unwrap().pred_opt().unwrap())
        .collect();
    months.push(today);

    for (i, &date) in months.iter().enumerate() {
        let i64i = i as i64;
        let w = wobble(i as u64);
        let mut day = BTreeMap::new();
        day.insert(
            "td-chq".to_string(),
            (d(420000) + d(90000) * w + d(15000) * Decimal::from(i64i % 3)).round_dp(2),
        );
        day.insert(
            "eq-sav".to_string(),
            (d(3000000) + d(75000) * Decimal::from(i64i) + d(40000) * w).round_dp(2),
        );
        if i % 3 == 0 {
            day.insert("rbc-usd".to_string(), (d(210000) - d(2500) * Decimal::from(i64i)).round_dp(2));
        }
        if i % 2 == 0 {
            day.insert("ws-fhsa".to_string(), d(800000) + d(100000) * Decimal::from(i64i / 2));
        }
        if i == 0 || i == 7 {
            day.insert("condo".to_string(), if i == 0 { d(42000000) } else { d(42500000) });
        }
        day.insert("td-mortgage".to_string(), d(44600000) - d(80000) * Decimal::from(i64i));
        day.insert("td-visa".to_string(), (d(140000) + d(90000) * w).abs().round_dp(2));
        l.snapshots.insert(date, day);

        // Prices drift up with noise.
        let g = Decimal::ONE + Decimal::new(i64i * 12, 3);
        l.prices.entry("XEQT.TO".into()).or_default().insert(date, (d(2980) * g + d(60) * w).round_dp(2));
        l.prices.entry("VFV.TO".into()).or_default().insert(date, (d(12650) * g + d(200) * w).round_dp(2));
        l.prices.entry("VTI".into()).or_default().insert(date, (d(24100) * g + d(300) * w).round_dp(2));
    }
    // Monthly buys of XEQT in the TFSA (14 units each, mid-month).
    for date in months.iter().skip(1).take(13) {
        let mid = NaiveDate::from_ymd_opt(date.year(), date.month(), 15).unwrap();
        if mid <= today {
            l.position_changes.push(PositionChange {
                date: mid,
                account: "ws-tfsa".into(),
                symbol: "XEQT.TO".into(),
                units: d(1400),
                price: None,
                note: Some("monthly buy".into()),
            });
        }
    }
    l.events = vec![
        Event { date: months[7], note: "Condo reassessed".into() },
        Event { date: months[13], note: "Annual bonus".into() },
    ];
    l
}

/// Daily closes for the last ~90 weekdays for every seeded symbol, and a USD rate,
/// as if a refresh had run every day. Starts from the ledger's manual month-end prices.
pub fn seed_quotes(l: &Ledger, today: NaiveDate) -> QuoteCache {
    let mut q = QuoteCache::default();
    let start = today - chrono::Days::new(90);
    let mut i = 0u64;
    let mut day = start;
    while day <= today {
        if !matches!(day.weekday(), Weekday::Sat | Weekday::Sun) {
            let w = wobble(i.wrapping_mul(7919));
            let drift = Decimal::ONE + Decimal::new(i as i64 * 4, 4);
            for (sym, base) in [("XEQT.TO", d(3180)), ("VFV.TO", d(13500)), ("VTI", d(25700))] {
                let px = (base * drift + base * Decimal::new(12, 3) * w).round_dp(2);
                q.prices.entry(sym.into()).or_default().insert(day, px);
            }
            q.fx.entry("USD".into()).or_default().insert(day, (d(13880) + d(30) * w).round_dp(4));
            i += 1;
        }
        day = day.succ_opt().unwrap_or(day);
    }
    q.currencies = [("XEQT.TO", "CAD"), ("VFV.TO", "CAD"), ("VTI", "USD")]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    // Quotes are "as of" the last weekday, fetched that evening.
    let last = q.latest_price_date().unwrap_or(today);
    q.fetched_at = last.and_hms_opt(21, 5, 0).map(|t| t.and_utc());
    let _ = l;
    q
}

pub fn write_seed(data_dir: &Path, today: NaiveDate, force: bool) -> Result<std::path::PathBuf, LedgerError> {
    std::fs::create_dir_all(data_dir)?;
    let path = data_dir.join("ledger.yaml");
    if path.exists() && !force {
        return Err(LedgerError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            format!("{} exists; pass --force to overwrite it", path.display()),
        )));
    }
    let l = seed_ledger(today);
    ledger::io::save(&path, &l)?;
    seed_quotes(&l, today)
        .save(&data_dir.join("quotes.json"))
        .map_err(|e| LedgerError::Io(std::io::Error::other(e.to_string())))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_is_valid_and_has_a_year_of_history() {
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let l = seed_ledger(today);
        ledger::validate::validate(&l).unwrap();
        let m = derive::Market::from_ledger(&l);
        let s = derive::net_worth_series(&l, &m);
        assert!(s.len() >= 15);
        assert!(s.first().unwrap().date < today - chrono::Days::new(365));
        let t = derive::totals(&l, &m, today);
        assert!(t.net_worth > Decimal::ZERO);
        assert!(l.accounts.iter().all(|a| !derive::value_account(&l, &m, a, today).missing_fx));
        // With the seeded quotes, every position is priced within a few days and has a day change.
        let q = seed_quotes(&l, today);
        let m = derive::Market::merge(&l, &q.prices, &q.fx, &q.currencies);
        let p = derive::portfolio(&l, &m, today);
        assert!(p.unpriced.is_empty());
        assert!(p.day_change.is_some());
        assert!((today - m.latest_fetched_price_date().unwrap()).num_days() <= 3);
        assert_eq!(m.rate_at("CAD", "USD", today).unwrap().source, derive::Source::Fetched);
        // Round-trips through YAML.
        let again = ledger::io::parse(&ledger::io::to_yaml(&l).unwrap()).unwrap();
        assert_eq!(l, again);
    }
}
