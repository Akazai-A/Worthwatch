//! One month of the budget page: spending by category against limits, the running
//! total by day, bills paid or due, and what needs your attention. Pure; amounts are
//! converted to the base currency with a rate you pass in.

use std::collections::BTreeMap;

use chrono::{Datelike, Days, Months, NaiveDate};
use ledger::{Ledger, TxnSource};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::category::{counts_as_spending, label, CATEGORIES};
use crate::reconcile::{similarity, Status, Txn};

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonthView {
    /// First day of the month.
    pub month: NaiveDate,
    pub days: u32,
    /// Day of the month today, when this is the current month; `days` for a past one.
    pub day: u32,
    pub base_currency: String,
    /// Spending (not transfers, not income), base currency.
    pub spent: Decimal,
    /// The part of `spent` not on a statement yet.
    pub provisional: Decimal,
    pub income: Decimal,
    /// Sum of the category limits.
    pub budget: Decimal,
    /// Where the budget says you'd be today: bills on their day, the rest evenly.
    pub pace: Decimal,
    /// The same, for every day of the month.
    pub pace_by_day: Vec<Decimal>,
    pub categories: Vec<CategoryLine>,
    /// Spending per day of the month (index 0 is the 1st); `None` after today.
    pub daily: Vec<Option<Decimal>>,
    /// Running total per day, and the part of it that is still provisional.
    pub cumulative: Vec<Option<Decimal>>,
    pub cumulative_provisional: Vec<Option<Decimal>>,
    pub bills: Vec<BillStatus>,
    /// This month's transactions, newest first, each with its base-currency amount.
    pub txns: Vec<MonthTxn>,
    /// Taps a week old with no statement row, any month.
    pub review: Vec<MonthTxn>,
    /// Wallet cards that sent taps but are not linked to an account.
    pub unlinked: Vec<UnlinkedCard>,
    /// Transactions whose currency has no rate (left out of the totals).
    pub missing_fx: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MonthTxn {
    #[serde(flatten)]
    pub txn: Txn,
    pub base_amount: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CategoryLine {
    pub category: String,
    pub label: String,
    pub spent: Decimal,
    pub provisional: Decimal,
    pub limit: Option<Decimal>,
    /// Spending in each of the five months before this one, oldest first.
    pub history: Vec<Decimal>,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BillStatus {
    pub name: String,
    pub amount: Decimal,
    pub category: String,
    /// The statement row that paid it this month.
    pub paid_on: Option<NaiveDate>,
    pub paid_amount: Option<Decimal>,
    pub due_day: Option<u32>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct UnlinkedCard {
    pub card: String,
    pub taps: usize,
    pub last: NaiveDate,
    pub total: Decimal,
}

pub fn first_of(d: NaiveDate) -> NaiveDate {
    d.with_day(1).unwrap_or(d)
}

/// Days in the month that starts on `month` (the 1st).
pub(crate) fn days_in(month: NaiveDate) -> u32 {
    let next = month + Months::new(1);
    (next - month).num_days() as u32
}

/// Build the view for the month containing `month`. `rate(currency, date)` gives units
/// of base currency per unit of `currency`.
pub fn month(
    l: &Ledger,
    txns: &[Txn],
    month: NaiveDate,
    today: NaiveDate,
    rate: &dyn Fn(&str, NaiveDate) -> Option<Decimal>,
) -> MonthView {
    let base = l.settings.base_currency.clone();
    let start = first_of(month);
    let days = days_in(start);
    let end = start + Days::new(u64::from(days));
    let day = if today < start {
        0
    } else if today >= end {
        days
    } else {
        today.day()
    };
    let to_base = |t: &Txn| -> Option<Decimal> {
        if t.currency == base {
            Some(t.amount)
        } else {
            rate(&t.currency, t.date).map(|r| (t.amount * r).round_dp(2))
        }
    };

    // Category totals for this month and the five before it.
    let hist_start = start - Months::new(5);
    let mut by_month: BTreeMap<(NaiveDate, String), Decimal> = BTreeMap::new();
    let mut lines: BTreeMap<String, CategoryLine> = BTreeMap::new();
    let mut daily = vec![Decimal::ZERO; days as usize];
    let mut daily_prov = vec![Decimal::ZERO; days as usize];
    let (mut spent, mut provisional, mut income) = (Decimal::ZERO, Decimal::ZERO, Decimal::ZERO);
    let mut missing_fx = 0;
    let mut month_txns = Vec::new();
    for t in txns {
        if t.date < hist_start || t.date >= end {
            continue;
        }
        let base_amount = to_base(t);
        if t.date >= start {
            month_txns.push(MonthTxn { txn: t.clone(), base_amount });
        }
        let Some(v) = base_amount else {
            if t.date >= start {
                missing_fx += 1;
            }
            continue;
        };
        if !counts_as_spending(&t.category) {
            if t.date >= start && t.category == "income" {
                income -= v;
            }
            continue;
        }
        *by_month.entry((first_of(t.date), t.category.clone())).or_default() += v;
        if t.date < start {
            continue;
        }
        let line = lines.entry(t.category.clone()).or_insert_with(|| CategoryLine {
            category: t.category.clone(),
            label: label(&t.category),
            spent: Decimal::ZERO,
            provisional: Decimal::ZERO,
            limit: None,
            history: Vec::new(),
            count: 0,
        });
        line.spent += v;
        line.count += 1;
        spent += v;
        let i = (t.date.day() - 1) as usize;
        daily[i] += v;
        if t.status.provisional() {
            line.provisional += v;
            provisional += v;
            daily_prov[i] += v;
        }
    }
    for (cat, limit) in &l.budget.limits {
        lines
            .entry(cat.clone())
            .or_insert_with(|| CategoryLine {
                category: cat.clone(),
                label: label(cat),
                spent: Decimal::ZERO,
                provisional: Decimal::ZERO,
                limit: None,
                history: Vec::new(),
                count: 0,
            })
            .limit = Some(*limit);
    }
    for line in lines.values_mut() {
        line.history = (1..=5)
            .rev()
            .map(|k| {
                by_month.get(&(start - Months::new(k), line.category.clone())).copied().unwrap_or_default()
            })
            .collect();
    }
    // Budgeted categories first in the house order, then the rest by amount.
    let order = |c: &str| CATEGORIES.iter().position(|(id, _)| *id == c).unwrap_or(usize::MAX);
    let mut categories: Vec<CategoryLine> = lines.into_values().collect();
    categories.sort_by(|a, b| {
        b.limit
            .is_some()
            .cmp(&a.limit.is_some())
            .then(order(&a.category).cmp(&order(&b.category)))
            .then(b.spent.cmp(&a.spent))
    });

    let budget: Decimal = l.budget.limits.values().copied().sum();
    // A bill is paid when a statement row this month names it and is within 10% of it.
    let bills: Vec<BillStatus> = l
        .budget
        .bills
        .iter()
        .map(|bill| {
            let needle = bill.matches.clone().unwrap_or_else(|| bill.name.clone());
            let paid = month_txns.iter().find(|m| {
                let t = &m.txn;
                let text = t.description.as_deref().unwrap_or(&t.merchant);
                let near =
                    m.base_amount.is_some_and(|v| (v - bill.amount).abs() <= bill.amount / Decimal::TEN);
                near && (text.to_uppercase().contains(&needle.to_uppercase())
                    || similarity(&needle, text) >= 0.99)
            });
            BillStatus {
                name: bill.name.clone(),
                amount: bill.amount,
                category: bill.category.clone(),
                paid_on: paid.map(|m| m.txn.date),
                paid_amount: paid.and_then(|m| m.base_amount),
                due_day: bill.day,
            }
        })
        .collect();

    // Pace: bills land on their day (paid, or due), the rest of the budget evenly, so
    // rent on the 1st doesn't read as "over pace" for the whole first week.
    let mut fixed_by_day = vec![Decimal::ZERO; days as usize];
    let mut fixed = Decimal::ZERO;
    for bill in &bills {
        if let Some(at) = bill.paid_on.map(|d| d.day()).or(bill.due_day.map(|d| d.clamp(1, days))) {
            let amount = bill.paid_amount.unwrap_or(bill.amount);
            fixed_by_day[(at - 1) as usize] += amount;
            fixed += amount;
        }
    }
    let flexible = (budget - fixed).max(Decimal::ZERO);
    let mut pace_by_day = Vec::with_capacity(days as usize);
    let mut so_far = Decimal::ZERO;
    for (i, f) in fixed_by_day.iter().enumerate() {
        so_far += *f;
        pace_by_day.push(
            (so_far.min(budget) + flexible * Decimal::from(i as u32 + 1) / Decimal::from(days)).round_dp(2),
        );
    }
    let pace = if day == 0 { Decimal::ZERO } else { pace_by_day[(day - 1) as usize] };
    let mut run = Decimal::ZERO;
    let mut run_prov = Decimal::ZERO;
    let mut cumulative = Vec::with_capacity(days as usize);
    let mut cumulative_provisional = Vec::with_capacity(days as usize);
    let mut daily_out = Vec::with_capacity(days as usize);
    for i in 0..days as usize {
        if (i as u32) < day {
            run += daily[i];
            run_prov += daily_prov[i];
            cumulative.push(Some(run));
            cumulative_provisional.push(Some(run_prov));
            daily_out.push(Some(daily[i]));
        } else {
            cumulative.push(None);
            cumulative_provisional.push(None);
            daily_out.push(None);
        }
    }

    let review = txns
        .iter()
        .filter(|t| t.status == Status::Review)
        .map(|t| MonthTxn { txn: t.clone(), base_amount: to_base(t) })
        .collect();
    let mut unlinked: BTreeMap<String, UnlinkedCard> = BTreeMap::new();
    for t in txns.iter().filter(|t| t.status == Status::Unlinked) {
        let card = t.card.clone().unwrap_or_default();
        let u = unlinked.entry(card.clone()).or_insert(UnlinkedCard {
            card,
            taps: 0,
            last: t.date,
            total: Decimal::ZERO,
        });
        u.taps += 1;
        u.last = u.last.max(t.date);
        u.total += to_base(t).unwrap_or_default();
    }
    // Statement-only accounts never get taps; nothing to show for them here.
    let _ = TxnSource::Statement;

    MonthView {
        month: start,
        days,
        day,
        base_currency: base,
        spent,
        provisional,
        income,
        budget,
        pace,
        pace_by_day,
        categories,
        daily: daily_out,
        cumulative,
        cumulative_provisional,
        bills,
        txns: month_txns,
        review,
        unlinked: unlinked.into_values().collect(),
        missing_fx,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::reconcile;
    use crate::reconcile::tests::{ledger, row, tap, ymd};
    use ledger::{Bill, Every, Mutation};
    use rust_decimal_macros::dec;

    fn no_fx(_: &str, _: NaiveDate) -> Option<Decimal> {
        None
    }

    #[test]
    fn totals_pace_running_line_and_bills() {
        let l = ledger()
            .apply_many([
                Mutation::SetBudgetLimit { category: "dining".into(), amount: Some(dec!(400)) },
                Mutation::SetBudgetLimit { category: "groceries".into(), amount: Some(dec!(800)) },
                Mutation::SetBills {
                    bills: vec![Bill {
                        name: "Rogers".into(),
                        amount: dec!(95),
                        every: Every::Month,
                        category: "bills".into(),
                        matches: None,
                        day: Some(4),
                    }],
                },
            ])
            .unwrap();
        let taps = [
            tap("Amex Cobalt", "Pai Northern Thai", dec!(48), "2026-11-03T19:00:00-05:00"),
            tap("Amex Cobalt", "Loblaws", dec!(52.18), "2026-11-10T12:00:00-05:00"),
        ];
        let rows = [
            row("amex", ymd(2026, 11, 4), dec!(56.40), "PAI NORTHERN THAI KITCH"),
            row("td-chq", ymd(2026, 11, 4), dec!(95.00), "ROGERS WIRELESS PAD"),
            row("td-chq", ymd(2026, 11, 5), dec!(-2850), "PAYROLL DEP"),
            row("td-chq", ymd(2026, 11, 16), dec!(1204.55), "AMEX BILL PYMT"),
            row("amex", ymd(2026, 10, 12), dec!(30), "SQ *SUSHI"),
        ];
        let today = ymd(2026, 11, 15);
        let txns = reconcile(&l, &taps, &rows, &BTreeMap::new(), today);
        let v = month(&l, &txns, ymd(2026, 11, 20), today, &no_fx);
        assert_eq!(v.month, ymd(2026, 11, 1));
        assert_eq!((v.days, v.day), (30, 15));
        assert_eq!(v.spent, dec!(56.40) + dec!(95) + dec!(52.18), "transfer and income left out");
        assert_eq!(v.provisional, dec!(52.18));
        assert_eq!(v.income, dec!(2850));
        assert_eq!(v.budget, dec!(1200));
        // Rogers lands on the 4th; the other 1105 spreads evenly.
        assert_eq!(v.pace, dec!(647.50));
        assert_eq!(v.pace_by_day[2], dec!(110.50));
        assert_eq!(v.pace_by_day[3], dec!(95) + dec!(147.33));
        assert_eq!(v.pace_by_day[29], dec!(1200));
        assert_eq!(v.cumulative[14], Some(v.spent - dec!(1204.55) + dec!(1204.55) - dec!(0)));
        assert_eq!(v.cumulative[15], None);
        assert_eq!(v.cumulative_provisional[14], Some(dec!(52.18)));
        let dining = v.categories.iter().find(|c| c.category == "dining").unwrap();
        assert_eq!((dining.spent, dining.limit), (dec!(56.40), Some(dec!(400))));
        assert_eq!(dining.history, vec![dec!(0), dec!(0), dec!(0), dec!(0), dec!(30)]);
        assert_eq!(v.categories[0].category, "groceries", "budgeted categories first, house order");
        assert_eq!(v.bills[0].paid_on, Some(ymd(2026, 11, 4)));
        assert_eq!(v.txns.len(), 5);
        assert!(v.review.is_empty());
    }

    #[test]
    fn foreign_amounts_convert_or_are_counted_as_missing() {
        let mut t = tap("TD Visa", "Shake Shack", dec!(10), "2026-11-03T12:00:00-05:00");
        t.currency = Some("USD".into());
        let txns = reconcile(&ledger(), &[t], &[], &BTreeMap::new(), ymd(2026, 11, 4));
        let v = month(&ledger(), &txns, ymd(2026, 11, 1), ymd(2026, 11, 4), &no_fx);
        assert_eq!((v.spent, v.missing_fx), (dec!(0), 1));
        let v = month(&ledger(), &txns, ymd(2026, 11, 1), ymd(2026, 11, 4), &|_, _| Some(dec!(1.39)));
        assert_eq!(v.spent, dec!(13.90));
    }

    #[test]
    fn review_and_unlinked_cards_surface() {
        let taps = [
            tap("Amex Cobalt", "Apple.com/bill", dec!(14.68), "2026-10-20T10:00:00-05:00"),
            tap("Scotia Gold", "Metro", dec!(30), "2026-11-02T10:00:00-05:00"),
            tap("Scotia Gold", "Metro", dec!(12), "2026-11-03T10:00:00-05:00"),
        ];
        let txns = reconcile(&ledger(), &taps, &[], &BTreeMap::new(), ymd(2026, 11, 4));
        let v = month(&ledger(), &txns, ymd(2026, 11, 1), ymd(2026, 11, 4), &no_fx);
        assert_eq!(v.review.len(), 1, "October's tap shows in November's review");
        assert_eq!(v.unlinked.len(), 1);
        assert_eq!((v.unlinked[0].taps, v.unlinked[0].total), (2, dec!(42)));
        assert_eq!(v.spent, dec!(42), "unlinked taps are still spending");
    }
}
