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
