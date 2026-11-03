use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::schema::*;
use crate::validate::{self, LedgerError};

/// Cards that can run in one week.
pub const MAX_CARDS_A_WEEK: usize = 2;

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
    /// Monthly limit for a category; `None` removes it.
    SetBudgetLimit {
        category: String,
        amount: Option<Decimal>,
    },
    /// Link a Wallet card to a credit card account (`None` unlinks). Linking a card to
    /// an account that only had statements switches it to Apple Pay + statement.
    LinkCard {
        card: String,
        account: Option<AccountId>,
    },
    SetTxnSource {
        account: AccountId,
        source: TxnSource,
    },
    /// Replace the categorization rules.
    SetCategoryRules {
        rules: Vec<CategoryRule>,
    },
    /// Replace the bills.
    SetBills {
        bills: Vec<Bill>,
    },
    SetFeed {
        feed: Feed,
    },
    /// The budget game's monthly jar goal; `None` turns the game off.
    SetGameGoal {
        goal: Option<Decimal>,
    },
    /// Play a challenge card for its week. Playing the same id again does nothing.
    PlayCard {
        card: PlayedCard,
    },
    /// Pass on an offered card so it is not offered again.
    PassCard {
        id: String,
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
                next.budget.cards.retain(|_, a| *a != id);
                next.budget.sources.remove(&id);
                next.budget.rules.retain(|r| r.account.as_deref() != Some(id.as_str()));
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
            Mutation::SetBudgetLimit { category, amount } => {
                let category = category.trim().to_lowercase();
                match amount {
                    Some(a) => {
                        next.budget.limits.insert(category, a);
                    }
                    None => {
                        next.budget.limits.remove(&category);
                    }
                }
            }
            Mutation::LinkCard { card, account } => {
                let previous = next.budget.cards.remove(&card);
                if let Some(id) = account {
                    if next.account(&id).is_none() {
                        return Err(LedgerError::UnknownAccount(id));
                    }
                    if !next.budget.source(&id).uses_taps() {
                        next.budget.sources.insert(id.clone(), TxnSource::ApplePayStatement);
                    }
                    next.budget.cards.insert(card, id);
                }
                // An account left with no card can only have statements.
                if let Some(old) = previous {
                    if !next.budget.cards.values().any(|a| *a == old) {
                        next.budget.sources.remove(&old);
                    }
                }
            }
            Mutation::SetTxnSource { account, source } => {
                if next.account(&account).is_none() {
                    return Err(LedgerError::UnknownAccount(account));
                }
                match source {
                    TxnSource::Statement => next.budget.sources.remove(&account),
                    s => next.budget.sources.insert(account, s),
                };
            }
            Mutation::SetCategoryRules { rules } => {
                next.budget.rules = rules
                    .into_iter()
                    .map(|r| CategoryRule {
                        contains: r.contains.trim().to_string(),
                        category: r.category.trim().to_lowercase(),
                        account: r.account,
                    })
                    .collect();
            }
            Mutation::SetBills { bills } => {
                next.budget.bills = bills;
            }
            Mutation::SetFeed { feed } => {
                next.budget.feed = feed;
            }
            Mutation::SetGameGoal { goal } => {
                next.budget.game.goal = goal;
            }
            Mutation::PlayCard { card } => {
                let game = &mut next.budget.game;
                if !game.cards.iter().any(|c| c.id == card.id) {
                    let in_week = game.cards.iter().filter(|c| c.week == card.week).count();
                    if in_week >= MAX_CARDS_A_WEEK {
                        return Err(LedgerError::BadCard(
                            card.id,
                            format!("{MAX_CARDS_A_WEEK} cards are already in play that week"),
                        ));
                    }
                    game.passed.retain(|p| *p != card.id);
                    game.cards.push(card);
                    game.cards.sort_by_key(|c| c.week);
                }
            }
            Mutation::PassCard { id } => {
                let game = &mut next.budget.game;
                if !game.passed.contains(&id) && !game.cards.iter().any(|c| c.id == id) {
                    game.passed.push(id);
                }
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
    fn linking_a_card_switches_the_source_and_unlinking_switches_it_back() {
        let amex = Account { id: "amex".into(), kind: AccountKind::CreditCard, ..chq() };
        let l = Ledger::starter("CAD")
            .apply_many([
                Mutation::UpsertAccount { account: chq() },
                Mutation::UpsertAccount { account: amex },
            ])
            .unwrap();
        // A Wallet card is a credit card: chequing is refused.
        let err = l.apply(Mutation::LinkCard { card: "Amex Cobalt".into(), account: Some("td-chq".into()) });
        assert!(matches!(err, Err(LedgerError::NotACard(..))));
        let l =
            l.apply(Mutation::LinkCard { card: "Amex Cobalt".into(), account: Some("amex".into()) }).unwrap();
        assert_eq!(l.budget.source("amex"), TxnSource::ApplePayStatement);
        let l =
            l.apply(Mutation::SetTxnSource { account: "amex".into(), source: TxnSource::ApplePay }).unwrap();
        // Statement-only accounts can't be set to a source that never sends anything.
        let err = l.apply(Mutation::SetTxnSource { account: "td-chq".into(), source: TxnSource::ApplePay });
        assert!(matches!(err, Err(LedgerError::NoWalletCard(..))));
        let l = l.apply(Mutation::LinkCard { card: "Amex Cobalt".into(), account: None }).unwrap();
        assert_eq!(l.budget.source("amex"), TxnSource::Statement);
        assert!(l.budget.is_default());
    }

    #[test]
    fn removing_an_account_drops_its_card_and_source() {
        let amex = Account { id: "amex".into(), kind: AccountKind::CreditCard, ..chq() };
        let l = Ledger::starter("CAD")
            .apply_many([
                Mutation::UpsertAccount { account: amex },
                Mutation::LinkCard { card: "Amex".into(), account: Some("amex".into()) },
                Mutation::SetBudgetLimit {
                    category: " Dining ".into(),
                    amount: Some(rust_decimal_macros::dec!(400)),
                },
            ])
            .unwrap();
        assert!(l.budget.limits.contains_key("dining"));
        let l = l.apply(Mutation::RemoveAccount { id: "amex".into() }).unwrap();
        assert!(l.budget.cards.is_empty() && l.budget.sources.is_empty());
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
