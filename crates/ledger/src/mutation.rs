use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::schema::*;
use crate::validate::{self, LedgerError};

/// Every write to the ledger is one of these. The UI, the YAML watcher, the
/// chatbot and MCP all produce `Mutation`s; only [`Ledger::apply`] consumes them.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Mutation {
    UpsertAccount {
        account: Account,
    },
    RemoveAccount {
        id: AccountId,
    },
    /// Set one account's balance on a date. Idempotent.
    SetBalance {
        date: NaiveDate,
        account: AccountId,
        value: Decimal,
    },
    SetPrice {
        symbol: Symbol,
        date: NaiveDate,
        price: Decimal,
    },
    SetFx {
        currency: Currency,
        rate: Decimal,
    },
    RecordPositionChange {
        change: PositionChange,
    },
    /// Total cost of the position in the account's currency (what a broker export calls
    /// book cost, or average cost x units). `None` clears it.
    SetBookCost {
        account: AccountId,
        symbol: Symbol,
        book_cost: Option<Decimal>,
    },
    AddEvent {
        event: Event,
    },
    /// Replace every recurring rule for one account with `rules` (empty removes them).
    SetRecurring {
        account: AccountId,
        rules: Vec<Recurring>,
    },
    /// Room CRA reported for a kind of account; `None` forgets it.
    SetRoom {
        kind: RoomKind,
        room: Option<Room>,
    },
    RecordContribution {
        contribution: Contribution,
    },
    SetProjection {
        projection: Projection,
    },
}

impl Ledger {
    /// Apply a mutation to a copy, validate the result, return it. The caller
    /// (the server) persists it. Pure: no IO, no clock.
    pub fn apply(&self, m: Mutation) -> Result<Ledger, LedgerError> {
        let mut next = self.clone();
        match m {
            Mutation::UpsertAccount { account } => {
                match next.accounts.iter_mut().find(|a| a.id == account.id) {
                    Some(existing) => *existing = account,
                    None => next.accounts.push(account),
                }
            }
            Mutation::RemoveAccount { id } => {
                next.accounts.retain(|a| a.id != id);
                next.recurring.retain(|r| r.account != id);
                next.contributions.retain(|c| c.account != id);
                next.settings.projection.rates.remove(&id);
                for day in next.snapshots.values_mut() {
                    day.remove(&id);
                }
                next.snapshots.retain(|_, day| !day.is_empty());
                next.position_changes.retain(|c| c.account != id);
            }
            Mutation::SetBalance { date, account, value } => {
                next.snapshots.entry(date).or_default().insert(account, value);
            }
            Mutation::SetPrice { symbol, date, price } => {
                next.prices.entry(symbol).or_default().insert(date, price);
            }
            Mutation::SetFx { currency, rate } => {
                next.settings.fx.insert(currency, rate);
            }
            Mutation::RecordPositionChange { change } => {
                // Keep the account's current positions in sync with the trade log.
                let acct = next
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == change.account)
                    .ok_or_else(|| LedgerError::UnknownAccount(change.account.clone()))?;
                let positions = acct.positions.get_or_insert_with(Vec::new);
                match positions.iter_mut().find(|p| p.symbol == change.symbol) {
                    Some(p) => p.units += change.units,
                    None => positions.push(Position {
                        symbol: change.symbol.clone(),
                        units: change.units,
                        book_cost: None,
                    }),
                }
                positions.retain(|p| !p.units.is_zero());
                next.position_changes.push(change);
                next.position_changes.sort_by_key(|c| c.date);
            }
            Mutation::SetBookCost { account, symbol, book_cost } => {
                let acct = next
                    .accounts
                    .iter_mut()
                    .find(|a| a.id == account)
                    .ok_or_else(|| LedgerError::UnknownAccount(account.clone()))?;
                let p = acct
                    .positions
                    .as_deref_mut()
                    .and_then(|ps| ps.iter_mut().find(|p| p.symbol == symbol))
                    .ok_or_else(|| LedgerError::UnknownPosition(account.clone(), symbol.clone()))?;
                p.book_cost = book_cost;
            }
            Mutation::AddEvent { event } => {
                next.events.push(event);
                next.events.sort_by_key(|e| e.date);
            }
            Mutation::SetRecurring { account, rules } => {
                if next.account(&account).is_none() {
                    return Err(LedgerError::UnknownAccount(account));
                }
                next.recurring.retain(|r| r.account != account);
                next.recurring.extend(rules.into_iter().map(|r| Recurring { account: account.clone(), ..r }));
            }
            Mutation::SetRoom { kind, room } => match room {
                Some(r) => {
                    next.room.insert(kind, r);
                }
                None => {
                    next.room.remove(&kind);
                }
            },
            Mutation::RecordContribution { contribution } => {
                next.contributions.push(contribution);
                next.contributions.sort_by_key(|c| c.date);
            }
            Mutation::SetProjection { projection } => {
                next.settings.projection = projection;
            }
        }
        validate::validate(&next)?;
        Ok(next)
    }

    pub fn apply_many(&self, ms: impl IntoIterator<Item = Mutation>) -> Result<Ledger, LedgerError> {
        ms.into_iter().try_fold(self.clone(), |l, m| l.apply(m))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn chq() -> Account {
        Account {
            id: "td-chq".into(),
            name: "TD Chequing".into(),
            institution: None,
            kind: AccountKind::Chequing,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        }
    }

    #[test]
    fn set_balance_then_read_back() {
        let l = Ledger::starter("CAD")
            .apply(Mutation::UpsertAccount { account: chq() })
            .unwrap()
            .apply(Mutation::SetBalance {
                date: NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
                account: "td-chq".into(),
                value: dec!(5102.10),
            })
            .unwrap();
        assert_eq!(l.snapshots.len(), 1);
    }

    #[test]
    fn book_cost_needs_an_existing_position() {
        let l = Ledger::starter("CAD").apply(Mutation::UpsertAccount { account: chq() }).unwrap();
        let err = l
            .apply(Mutation::SetBookCost {
                account: "td-chq".into(),
                symbol: "X".into(),
                book_cost: Some(dec!(1)),
            })
            .unwrap_err();
        assert!(matches!(err, LedgerError::UnknownPosition(..)));
        let l = l
            .apply(Mutation::RecordPositionChange {
                change: PositionChange {
                    date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                    account: "td-chq".into(),
                    symbol: "X".into(),
                    units: dec!(2),
                    price: None,
                    note: None,
                },
            })
            .unwrap()
            .apply(Mutation::SetBookCost {
                account: "td-chq".into(),
                symbol: "X".into(),
                book_cost: Some(dec!(50)),
            })
            .unwrap();
        assert_eq!(l.account("td-chq").unwrap().positions.as_ref().unwrap()[0].book_cost, Some(dec!(50)));
    }

    #[test]
    fn recurring_rules_replace_per_account_and_go_with_the_account() {
        let rule = |amount| Recurring { account: String::new(), amount, every: Every::Month, note: None };
        let l = Ledger::starter("CAD")
            .apply(Mutation::UpsertAccount { account: chq() })
            .unwrap()
            .apply(Mutation::SetRecurring { account: "td-chq".into(), rules: vec![rule(dec!(100))] })
            .unwrap()
            .apply(Mutation::SetRecurring { account: "td-chq".into(), rules: vec![rule(dec!(250))] })
            .unwrap();
        assert_eq!(l.recurring.len(), 1);
        assert_eq!(l.recurring[0].account, "td-chq");
        assert_eq!(l.recurring[0].amount, dec!(250));
        let l = l.apply(Mutation::RemoveAccount { id: "td-chq".into() }).unwrap();
        assert!(l.recurring.is_empty());
    }

    #[test]
    fn contributions_go_to_registered_accounts_only() {
        let date = NaiveDate::from_ymd_opt(2026, 3, 1).unwrap();
        let c = Contribution { date, account: "td-chq".into(), amount: dec!(500), note: None };
        let l = Ledger::starter("CAD").apply(Mutation::UpsertAccount { account: chq() }).unwrap();
        let err = l.apply(Mutation::RecordContribution { contribution: c.clone() }).unwrap_err();
        assert!(matches!(err, LedgerError::NotRegistered(_)));
        let tfsa = Account { id: "tfsa".into(), kind: AccountKind::Tfsa, ..chq() };
        let l = l
            .apply(Mutation::UpsertAccount { account: tfsa })
            .unwrap()
            .apply(Mutation::RecordContribution {
                contribution: Contribution { account: "tfsa".into(), ..c },
            })
            .unwrap();
        assert_eq!(l.contributions.len(), 1);
    }

    #[test]
    fn balance_for_unknown_account_is_rejected() {
        let err = Ledger::starter("CAD")
            .apply(Mutation::SetBalance {
                date: NaiveDate::from_ymd_opt(2026, 10, 31).unwrap(),
                account: "nope".into(),
                value: dec!(1),
            })
            .unwrap_err();
        assert!(matches!(err, LedgerError::UnknownAccount(_)));
    }
}
