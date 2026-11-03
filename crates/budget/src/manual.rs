//! Transactions added by hand: cash, or anything that never reaches a statement or
//! the Apple Pay feed. Stored beside taps and statement rows, append-only like them.
//!
//! ```text
//!  Budget "+ Add" ─┐
//!  Play "+ cash" ──┼─▶ POST /api/budget/spend ─▶ Manual ─▶ transactions/YYYY-MM.jsonl
//!  Chat approve ───┘                                         │
//!                      reconcile: on an account you import, a statement row 0–5 days
//!                      later with the same amount (or a tip) merges with it; otherwise
//!                      it counts as entered.
//! ```
//! Removing one appends a [`Void`]; nothing is ever rewritten.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Manual {
    pub id: String,
    pub date: NaiveDate,
    pub merchant: String,
    /// Positive out, negative in (a refund).
    pub amount: Decimal,
    /// The account it was paid from; `None` is cash outside any account.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Picked when it was added; `None` goes to the category rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    pub added: DateTime<Utc>,
}

impl Manual {
    /// A new entry. The id includes the moment it was added, so two identical coffees
    /// on the same day are two transactions.
    pub fn new(
        date: NaiveDate,
        merchant: &str,
        amount: Decimal,
        account: Option<String>,
        category: Option<String>,
        added: DateTime<Utc>,
        seq: usize,
    ) -> Self {
        let merchant = merchant.trim().to_string();
        let mut h = DefaultHasher::new();
        (date, &merchant, amount.normalize().to_string(), &account, added.timestamp_nanos_opt(), seq)
            .hash(&mut h);
        Manual {
            id: format!("man-{date}-{:012x}", h.finish() & 0xffff_ffff_ffff),
            date,
            merchant,
            amount,
            account,
            category: category.map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty()),
            added,
        }
    }
}

/// "Forget this one": hides a manual entry without rewriting the file it is in.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Void {
    pub id: String,
    pub target: String,
    /// The target's date, so the void lands in the same month file.
    pub date: NaiveDate,
    pub at: DateTime<Utc>,
}

impl Void {
    pub fn of(m: &Manual, at: DateTime<Utc>) -> Self {
        Void { id: format!("void-{}", m.id), target: m.id.clone(), date: m.date, at }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::tests::{ledger, row, ymd};
    use crate::reconcile::{reconcile_all, Status};
    use crate::store::{Record, TxnStore};
    use rust_decimal_macros::dec;
    use std::collections::BTreeMap;

    fn at(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn cash_counts_as_entered_with_the_category_you_picked() {
        let m = Manual::new(
            ymd(2026, 11, 18),
            " Farmers market ",
            dec!(12.50),
            None,
            Some("Groceries".into()),
            at("2026-11-18T15:00:00Z"),
            0,
        );
        assert!(m.id.starts_with("man-2026-11-18-"));
        assert_eq!(m.merchant, "Farmers market");
        let v =
            reconcile_all(&ledger(), &[], &[], std::slice::from_ref(&m), &BTreeMap::new(), ymd(2026, 11, 30));
        assert_eq!(v.len(), 1);
        assert_eq!(
            (v[0].status, v[0].category.as_str(), v[0].account.as_deref()),
            (Status::Manual, "groceries", None)
        );
        assert!(!v[0].status.provisional(), "never waits for a statement, never goes to Review");
        assert_eq!(v[0].manual_id.as_deref(), Some(m.id.as_str()));
        // Without a category the rules pick one, like any tap.
        let c = Manual::new(
            ymd(2026, 11, 18),
            "Tim Hortons",
            dec!(3.89),
            None,
            None,
            at("2026-11-18T15:00:00Z"),
            1,
        );
        let v = reconcile_all(&ledger(), &[], &[], &[c], &BTreeMap::new(), ymd(2026, 11, 18));
        assert_eq!(v[0].category, "coffee");
    }

    #[test]
    fn on_an_account_you_import_it_merges_with_the_statement_row() {
        let added = at("2026-11-18T15:00:00Z");
        let card = Manual::new(
            ymd(2026, 11, 18),
            "Pai",
            dec!(48),
            Some("amex".into()),
            Some("dining".into()),
            added,
            0,
        );
        let chq =
            Manual::new(ymd(2026, 11, 18), "Rent cheque", dec!(20), Some("td-chq".into()), None, added, 1);
        let rows = [
            row("amex", ymd(2026, 11, 20), dec!(56.64), "PAI NORTHERN THAI KITCH TORONTO"),
            row("td-chq", ymd(2026, 11, 19), dec!(20), "CHQ#0012"),
        ];
        let v = reconcile_all(
            &ledger(),
            &[],
            &rows,
            &[card.clone(), chq.clone()],
            &BTreeMap::new(),
            ymd(2026, 11, 21),
        );
        assert_eq!(v.len(), 2, "each counted once: {v:#?}");
        let pai = v.iter().find(|t| t.account.as_deref() == Some("amex")).unwrap();
        assert_eq!((pai.status, pai.amount, pai.merchant.as_str()), (Status::Confirmed, dec!(56.64), "Pai"));
        assert_eq!(
            (pai.tap_amount, pai.manual_id.as_deref(), pai.tap_id.as_deref()),
            (Some(dec!(48)), Some(card.id.as_str()), None)
        );
        assert_eq!(pai.category, "dining");
        assert_eq!(
            v.iter().find(|t| t.account.as_deref() == Some("td-chq")).unwrap().manual_id.as_deref(),
            Some(chq.id.as_str())
        );
        // On "Apple Pay only" (visa) nothing is imported, so it just counts.
        let visa = Manual::new(ymd(2026, 11, 18), "Esso", dec!(61), Some("visa".into()), None, added, 2);
        let v = reconcile_all(
            &ledger(),
            &[],
            &[row("visa", ymd(2026, 11, 19), dec!(61), "ESSO")],
            &[visa],
            &BTreeMap::new(),
            ymd(2026, 11, 21),
        );
        assert_eq!(v.iter().filter(|t| t.status == Status::Manual).count(), 1);
    }

    #[test]
    fn the_same_coffee_twice_is_two_entries_and_removing_one_is_appended() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = TxnStore::load(dir.path());
        let t = at("2026-11-18T15:00:00Z");
        let a = Manual::new(ymd(2026, 11, 18), "Coffee", dec!(6), None, None, t, 0);
        let b = Manual::new(ymd(2026, 11, 18), "Coffee", dec!(6), None, None, t, 1);
        assert_ne!(a.id, b.id);
        assert_eq!(s.append(vec![Record::Manual(a.clone()), Record::Manual(b.clone())]).unwrap(), 2);
        assert!(s.remove_manual(&a.id, t).unwrap());
        assert!(!s.remove_manual(&a.id, t).unwrap(), "already gone");
        assert_eq!(s.manual, vec![b.clone()]);
        let back = TxnStore::load(dir.path());
        assert_eq!(back.manual, vec![b]);
        let text = std::fs::read_to_string(dir.path().join("2026-11.jsonl")).unwrap();
        assert_eq!(text.lines().count(), 3, "two entries and one void, nothing rewritten");
        assert!(text.contains("\"source\":\"void\""));
    }
}
