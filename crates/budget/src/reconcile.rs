//! Which Apple Pay tap and which statement row are the same purchase.
//!
//! A tap and a row match when they are on the same account, the row is dated 0 to 5
//! days after the tap, and the amounts agree: equal, or the row up to 30% higher at a
//! restaurant or café (the tip), or any amount when the tap was in another currency.
//! When more than one row fits, the closest merchant name wins ("Blue Bottle" vs
//! "SQ *BLUE BOTTLE TORONTO ON"), then the nearest date. The merged transaction keeps
//! the bank's amount and date and the tap's clean merchant name.

use std::collections::{BTreeMap, HashMap};

use chrono::{Days, NaiveDate};
use ledger::{Ledger, TxnSource};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::category::{categorize, words, TIPPED};
use crate::statement::CsvRow;
use crate::tap::Tap;

/// How long a tap waits for its statement row before it is flagged for a look.
pub const REVIEW_AFTER_DAYS: i64 = 7;
/// How many days after the tap a statement may post it.
pub const POST_WINDOW_DAYS: u64 = 5;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// An Apple Pay tap waiting for its statement row.
    Provisional,
    /// A tap matched to its statement row.
    Confirmed,
    /// A statement row with no tap (bills, e-transfers, card swipes, online).
    Posted,
    /// A tap still unmatched after a week: declined, refunded, or a card you don't export.
    Review,
    /// A tap on an account set to "Apple Pay only": the tap is the record.
    TapOnly,
    /// A tap from a Wallet card not linked to an account yet.
    Unlinked,
}

impl Status {
    /// Not confirmed by a statement (yet).
    pub fn provisional(self) -> bool {
        matches!(self, Status::Provisional | Status::Review | Status::Unlinked)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Txn {
    /// The statement row's id when there is one, else the tap's.
    pub id: String,
    pub account: Option<String>,
    pub card: Option<String>,
    pub date: NaiveDate,
    /// Positive out, negative in.
    pub amount: Decimal,
    pub currency: String,
    /// Clean name: the tap's merchant, or the statement text tidied up.
    pub merchant: String,
    /// The bank's own text, when a statement row is involved.
    pub description: Option<String>,
    pub category: String,
    pub status: Status,
    pub tap_id: Option<String>,
    /// What was tapped, when it differs from what posted (tip, FX).
    pub tap_amount: Option<Decimal>,
    pub tap_currency: Option<String>,
    pub tap_date: Option<NaiveDate>,
}

const STOP: &[&str] = &[
    "SQ",
    "TST",
    "SP",
    "PP",
    "PAYPAL",
    "POS",
    "PURCHASE",
    "THE",
    "AND",
    "INC",
    "LTD",
    "CO",
    "CA",
    "CANADA",
    "ON",
    "QC",
    "BC",
    "AB",
    "MB",
    "SK",
    "NS",
    "NB",
    "NL",
    "PE",
    "YT",
    "NT",
    "NU",
    "TORONTO",
    "MONTREAL",
    "VANCOUVER",
    "OTTAWA",
    "CALGARY",
    "WWW",
    "COM",
];

fn tokens(s: &str) -> Vec<String> {
    words(s)
        .into_iter()
        .filter(|w| w.len() >= 2 && !w.chars().all(|c| c.is_ascii_digit()) && !STOP.contains(&w.as_str()))
        .collect()
}

/// 0..1: how much of the shorter name appears in the longer one.
pub fn similarity(a: &str, b: &str) -> f64 {
    let (ta, tb) = (tokens(a), tokens(b));
    if ta.is_empty() || tb.is_empty() {
        return 0.0;
    }
    let (ja, jb) = (ta.concat(), tb.concat());
    if ja.len().min(jb.len()) >= 4 && (ja.contains(&jb) || jb.contains(&ja)) {
        return 1.0;
    }
    let close = |w: &String, v: &String| {
        w == v || (w.len() >= 4 && v.starts_with(w.as_str())) || (v.len() >= 4 && w.starts_with(v.as_str()))
    };
    let (short, long) = if ta.len() <= tb.len() { (&ta, &tb) } else { (&tb, &ta) };
    let hits = short.iter().filter(|w| long.iter().any(|v| close(w, v))).count();
    hits as f64 / short.len() as f64
}

/// "SQ *BLUE BOTTLE TORONTO ON" -> "Blue Bottle Toronto On". Good enough for a list.
pub fn tidy(description: &str) -> String {
    let mut s = description.trim();
    for prefix in ["SQ *", "SQ*", "TST*", "TST-", "SP *", "SP*", "PAYPAL *", "PP*"] {
        // `get` and not `[..]`: a merchant like "CAFÉ" has no char boundary at byte 4.
        if s.len() > prefix.len() && s.get(..prefix.len()).is_some_and(|h| h.eq_ignore_ascii_case(prefix)) {
            s = s[prefix.len()..].trim_start();
        }
    }
    s.split_whitespace()
        .map(|w| {
            if w.chars().any(|c| c.is_ascii_lowercase()) {
                return w.to_string();
            }
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_string() + &c.as_str().to_lowercase(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct Score {
    exact: bool,
    similarity: f64,
    near: i64,
}

/// Merge taps and statement rows into one list, newest first. Pure.
/// `overrides` maps a transaction id (or a tap id) to a category you picked by hand.
pub fn reconcile(
    l: &Ledger,
    taps: &[Tap],
    rows: &[CsvRow],
    overrides: &BTreeMap<String, String>,
    today: NaiveDate,
) -> Vec<Txn> {
    let b = &l.budget;
    let currency_of = |id: &str| {
        l.account(id).map(|a| a.currency.clone()).unwrap_or_else(|| l.settings.base_currency.clone())
    };
    let liability = |id: &str| l.account(id).is_some_and(|a| a.kind.is_liability());
    let category = |text: &str, account: Option<&str>, amount: Decimal| {
        categorize(text, account, amount, account.is_some_and(liability), &b.rules)
    };

    // Taps that take part, with the account their card is linked to.
    let taps: Vec<(&Tap, Option<&str>)> = taps
        .iter()
        .map(|t| (t, b.card_account(&t.card).map(String::as_str)))
        .filter(|(_, a)| a.is_none_or(|a| b.source(a).uses_taps()))
        .collect();

    // Every (tap, row) pair that could be the same purchase.
    let mut pairs: Vec<(Score, usize, usize)> = Vec::new();
    for (ti, (t, account)) in taps.iter().enumerate() {
        let Some(account) = account else { continue };
        if b.source(account) != TxnSource::ApplePayStatement {
            continue;
        }
        let acct_currency = currency_of(account);
        let foreign = t.currency.as_ref().is_some_and(|c| *c != acct_currency);
        let tap_date = t.at.date_naive();
        let last = tap_date + Days::new(POST_WINDOW_DAYS);
        let tap_text = format!("{} {}", t.merchant, t.name.as_deref().unwrap_or(""));
        let tipped = TIPPED.contains(&category(&tap_text, Some(account), t.amount).as_str());
        for (ri, r) in rows.iter().enumerate() {
            if r.account != *account || r.date < tap_date || r.date > last {
                continue;
            }
            let sim = similarity(&tap_text, &r.description);
            let exact = !foreign && (r.amount - t.amount).abs() <= Decimal::new(1, 2);
            let tip = !foreign
                && tipped
                && r.amount > t.amount
                && r.amount <= t.amount * Decimal::new(130, 2)
                && sim > 0.0;
            let fx = foreign && r.amount.is_sign_positive() && sim >= 0.5;
            if exact || tip || fx {
                pairs.push((Score { exact, similarity: sim, near: -(r.date - tap_date).num_days() }, ti, ri));
            }
        }
    }
    pairs.sort_by(|a, b| {
        b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2))
    });
    let mut tap_to_row: HashMap<usize, usize> = HashMap::new();
    let mut row_taken = vec![false; rows.len()];
    for (_, ti, ri) in pairs {
        if tap_to_row.contains_key(&ti) || row_taken[ri] {
            continue;
        }
        tap_to_row.insert(ti, ri);
        row_taken[ri] = true;
    }
    let row_to_tap: HashMap<usize, usize> = tap_to_row.iter().map(|(t, r)| (*r, *t)).collect();

    let pick = |ids: &[&str], fallback: String| {
        ids.iter().find_map(|id| overrides.get(*id).cloned()).unwrap_or(fallback)
    };
    let mut out = Vec::with_capacity(rows.len() + taps.len());
    for (ri, r) in rows.iter().enumerate() {
        let currency = currency_of(&r.account);
        let txn = match row_to_tap.get(&ri) {
            Some(&ti) => {
                let t = taps[ti].0;
                let tap_text = format!("{} {}", t.merchant, t.name.as_deref().unwrap_or(""));
                let mut cat = category(&tap_text, Some(&r.account), r.amount);
                if cat == "other" {
                    cat = category(&r.description, Some(&r.account), r.amount);
                }
                let tap_currency = t.currency.clone().unwrap_or_else(|| currency.clone());
                let differs = t.amount != r.amount || tap_currency != currency;
                Txn {
                    id: r.id.clone(),
                    account: Some(r.account.clone()),
                    card: Some(t.card.clone()),
                    date: r.date,
                    amount: r.amount,
                    currency,
                    merchant: t.merchant.clone(),
                    description: Some(r.description.clone()),
                    category: pick(&[&r.id, &t.id], cat),
                    status: Status::Confirmed,
                    tap_id: Some(t.id.clone()),
                    tap_amount: differs.then_some(t.amount),
                    tap_currency: differs.then_some(tap_currency),
                    tap_date: Some(t.at.date_naive()),
                }
            }
            None => Txn {
                id: r.id.clone(),
                account: Some(r.account.clone()),
                card: None,
                date: r.date,
                amount: r.amount,
                currency,
                merchant: tidy(&r.description),
                description: Some(r.description.clone()),
                category: pick(&[&r.id], category(&r.description, Some(&r.account), r.amount)),
                status: Status::Posted,
                tap_id: None,
                tap_amount: None,
                tap_currency: None,
                tap_date: None,
            },
        };
        out.push(txn);
    }
    for (ti, (t, account)) in taps.iter().enumerate() {
        if tap_to_row.contains_key(&ti) {
            continue;
        }
        let date = t.at.date_naive();
        let status = match account {
            None => Status::Unlinked,
            Some(a) if b.source(a) == TxnSource::ApplePay => Status::TapOnly,
            Some(_) if (today - date).num_days() > REVIEW_AFTER_DAYS => Status::Review,
            Some(_) => Status::Provisional,
        };
        let currency = t
            .currency
            .clone()
            .unwrap_or_else(|| account.map_or_else(|| l.settings.base_currency.clone(), currency_of));
        let tap_text = format!("{} {}", t.merchant, t.name.as_deref().unwrap_or(""));
        out.push(Txn {
            id: t.id.clone(),
            account: account.map(str::to_string),
            card: Some(t.card.clone()),
            date,
            amount: t.amount,
            currency,
            merchant: t.merchant.clone(),
            description: None,
            category: pick(&[&t.id], category(&tap_text, *account, t.amount)),
            status,
            tap_id: Some(t.id.clone()),
            tap_amount: None,
            tap_currency: None,
            tap_date: Some(date),
        });
    }
    out.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.id.cmp(&b.id)));
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use chrono::{DateTime, Utc};
    use ledger::{Account, AccountKind, Mutation};
    use rust_decimal_macros::dec;

    pub fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    pub fn acct(id: &str, kind: AccountKind) -> Account {
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

    /// Amex (Apple Pay + statement), Visa (Apple Pay only), chequing (statement).
    pub fn ledger() -> Ledger {
        Ledger::starter("CAD")
            .apply_many([
                Mutation::UpsertAccount { account: acct("amex", AccountKind::CreditCard) },
                Mutation::UpsertAccount { account: acct("visa", AccountKind::CreditCard) },
                Mutation::UpsertAccount { account: acct("td-chq", AccountKind::Chequing) },
                Mutation::LinkCard { card: "Amex Cobalt".into(), account: Some("amex".into()) },
                Mutation::LinkCard { card: "TD Visa".into(), account: Some("visa".into()) },
                Mutation::SetTxnSource { account: "visa".into(), source: TxnSource::ApplePay },
            ])
            .unwrap()
    }

    pub fn tap(card: &str, merchant: &str, amount: Decimal, at: &str) -> Tap {
        let at = DateTime::parse_from_rfc3339(at).unwrap();
        Tap {
            id: format!("tap-{merchant}-{at}"),
            at,
            merchant: merchant.into(),
            name: None,
            amount,
            currency: None,
            card: card.into(),
            received: Utc::now(),
        }
    }

    pub fn row(account: &str, date: NaiveDate, amount: Decimal, description: &str) -> CsvRow {
        CsvRow {
            id: format!("csv-{account}-{date}-{description}"),
            account: account.into(),
            date,
            amount,
            description: description.into(),
            imported: Utc::now(),
        }
    }

    fn run(taps: &[Tap], rows: &[CsvRow], today: NaiveDate) -> Vec<Txn> {
        reconcile(&ledger(), taps, rows, &BTreeMap::new(), today)
    }

    #[test]
    fn a_tap_is_provisional_until_the_statement_confirms_it() {
        let t = tap("Amex Cobalt", "Blue Bottle", dec!(7.25), "2026-11-03T08:12:00-05:00");
        let v = run(std::slice::from_ref(&t), &[], ymd(2026, 11, 3));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].status, Status::Provisional);
        assert_eq!(v[0].category, "coffee");

        let r = row("amex", ymd(2026, 11, 4), dec!(7.25), "SQ *BLUE BOTTLE TORONTO ON");
        let v = run(std::slice::from_ref(&t), std::slice::from_ref(&r), ymd(2026, 11, 5));
        assert_eq!(v.len(), 1, "no duplicate");
        assert_eq!(v[0].status, Status::Confirmed);
        assert_eq!(v[0].id, r.id);
        assert_eq!(v[0].merchant, "Blue Bottle");
        assert_eq!(v[0].tap_amount, None);
        // Importing the same rows again (deduped by the store) changes nothing.
        assert_eq!(run(&[t], &[r], ymd(2026, 11, 5)), v);
    }

    #[test]
    fn a_tipped_restaurant_tap_matches_its_higher_posted_amount() {
        let t = tap("Amex Cobalt", "Pai Northern Thai Kitchen", dec!(48.00), "2026-11-03T19:40:00-05:00");
        let rows = [
            row("amex", ymd(2026, 11, 4), dec!(56.40), "PAI NORTHERN THAI KITCH TORONTO"),
            row("amex", ymd(2026, 11, 4), dec!(70.00), "PAI NORTHERN THAI KITCH TORONTO"), // over 30%
        ];
        let v = run(&[t], &rows, ymd(2026, 11, 5));
        let c = v.iter().find(|x| x.status == Status::Confirmed).unwrap();
        assert_eq!(c.amount, dec!(56.40));
        assert_eq!(c.tap_amount, Some(dec!(48.00)));
        assert_eq!(v.len(), 2);
    }

    #[test]
    fn a_tip_never_matches_a_different_merchant() {
        let t = tap("Amex Cobalt", "Pai Northern Thai", dec!(48.00), "2026-11-03T19:40:00-05:00");
        let v = run(&[t], &[row("amex", ymd(2026, 11, 4), dec!(52.00), "SUSHI GEN")], ymd(2026, 11, 5));
        assert!(v.iter().all(|x| x.status != Status::Confirmed));
    }

    #[test]
    fn ties_go_to_the_closest_merchant_name_then_the_nearest_date() {
        let taps = [
            tap("Amex Cobalt", "Blue Bottle", dec!(5.00), "2026-11-03T08:00:00-05:00"),
            tap("Amex Cobalt", "Presto", dec!(5.00), "2026-11-03T09:00:00-05:00"),
        ];
        let rows = [
            row("amex", ymd(2026, 11, 4), dec!(5.00), "PRESTO FARE/TORONTO"),
            row("amex", ymd(2026, 11, 4), dec!(5.00), "SQ *BLUE BOTTLE"),
        ];
        let v = run(&taps, &rows, ymd(2026, 11, 5));
        assert_eq!(v.len(), 2);
        for x in &v {
            assert_eq!(x.status, Status::Confirmed);
            assert!(similarity(&x.merchant, x.description.as_deref().unwrap()) > 0.9, "{x:?}");
        }
    }

    #[test]
    fn outside_the_window_or_on_another_account_does_not_match() {
        let t = tap("Amex Cobalt", "Loblaws", dec!(52.18), "2026-11-03T12:00:00-05:00");
        let rows = [
            row("amex", ymd(2026, 11, 2), dec!(52.18), "LOBLAWS"), // before the tap
            row("amex", ymd(2026, 11, 9), dec!(52.18), "LOBLAWS"), // 6 days later
            row("td-chq", ymd(2026, 11, 4), dec!(52.18), "LOBLAWS"),
        ];
        let v = run(&[t], &rows, ymd(2026, 11, 9));
        assert_eq!(v.iter().filter(|x| x.status == Status::Provisional).count(), 1);
        assert_eq!(v.iter().filter(|x| x.status == Status::Posted).count(), 3);
    }

    #[test]
    fn an_unmatched_tap_older_than_a_week_needs_a_look() {
        let t = tap("Amex Cobalt", "Apple.com/bill", dec!(14.68), "2026-11-05T10:00:00-05:00");
        assert_eq!(run(std::slice::from_ref(&t), &[], ymd(2026, 11, 12))[0].status, Status::Provisional);
        assert_eq!(run(&[t], &[], ymd(2026, 11, 13))[0].status, Status::Review);
    }

    #[test]
    fn a_foreign_tap_matches_the_cad_row_by_name() {
        let mut t = tap("Amex Cobalt", "Shake Shack", dec!(14.20), "2026-11-03T12:00:00-05:00");
        t.currency = Some("USD".into());
        let v = run(
            &[t],
            &[row("amex", ymd(2026, 11, 5), dec!(19.87), "SHAKE SHACK NEW YORK NY")],
            ymd(2026, 11, 6),
        );
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].amount, dec!(19.87));
        assert_eq!(v[0].currency, "CAD");
        assert_eq!(v[0].tap_currency.as_deref(), Some("USD"));
    }

    #[test]
    fn apple_pay_only_unlinked_and_statement_only_cards() {
        let taps = [
            tap("TD Visa", "Presto", dec!(4.60), "2026-11-01T08:00:00-05:00"),
            tap("Scotia Gold", "Metro", dec!(30.00), "2026-11-01T08:00:00-05:00"),
        ];
        let v = run(&taps, &[], ymd(2026, 11, 20));
        assert_eq!(v.iter().find(|x| x.card.as_deref() == Some("TD Visa")).unwrap().status, Status::TapOnly);
        let u = v.iter().find(|x| x.card.as_deref() == Some("Scotia Gold")).unwrap();
        assert_eq!(u.status, Status::Unlinked);
        assert_eq!(u.account, None);
        // Set the Amex to statements only: its taps are ignored.
        let l = ledger()
            .apply(Mutation::SetTxnSource { account: "amex".into(), source: TxnSource::Statement })
            .unwrap();
        let t = tap("Amex Cobalt", "Loblaws", dec!(1), "2026-11-01T08:00:00-05:00");
        assert!(reconcile(&l, &[t], &[], &BTreeMap::new(), ymd(2026, 11, 2)).is_empty());
    }

    #[test]
    fn paying_the_card_is_a_transfer_on_both_sides() {
        let rows = [
            row("td-chq", ymd(2026, 11, 16), dec!(1204.55), "AMEX BILL PYMT"),
            row("amex", ymd(2026, 11, 17), dec!(-1204.55), "PAYMENT RECEIVED - THANK YOU"),
        ];
        let v = run(&[], &rows, ymd(2026, 11, 18));
        assert!(v.iter().all(|x| x.category == "transfer"), "{v:?}");
    }

    #[test]
    fn a_hand_picked_category_sticks_through_the_match() {
        let t = tap("Amex Cobalt", "Costco", dec!(215.90), "2026-11-03T12:00:00-05:00");
        let r = row("amex", ymd(2026, 11, 4), dec!(215.90), "COSTCO WHOLESALE W535");
        let o = BTreeMap::from([(t.id.clone(), "shopping".to_string())]);
        let v = reconcile(&ledger(), std::slice::from_ref(&t), &[], &o, ymd(2026, 11, 3));
        assert_eq!(v[0].category, "shopping");
        let v = reconcile(&ledger(), &[t], &[r], &o, ymd(2026, 11, 5));
        assert_eq!(v[0].status, Status::Confirmed);
        assert_eq!(v[0].category, "shopping");
    }

    #[test]
    fn names() {
        assert_eq!(similarity("Blue Bottle", "SQ *BLUE BOTTLE TORONTO ON"), 1.0);
        assert_eq!(similarity("BlueBottle", "SQ *BLUE BOTTLE"), 1.0);
        assert_eq!(similarity("Loblaws", "PRESTO FARE"), 0.0);
        assert_eq!(tidy("SQ *BLUE BOTTLE TORONTO ON"), "Blue Bottle Toronto On");
        // Accented merchants used to panic and take the Budget page down with them.
        for d in ["CAFÉ OLIMPICO MONTREAL", "CRÊPERIE", "Sé", "S€x", "日本料理 TORONTO", "SQ *CAFÉ VITA"]
        {
            let _ = tidy(d);
        }
        assert_eq!(tidy("SQ *CAFÉ VITA"), "Café Vita");
    }
}
