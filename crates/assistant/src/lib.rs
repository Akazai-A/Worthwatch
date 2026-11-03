//! Natural language -> a proposal the app checks against the ledger.
//!
//! The model (or the built-in shorthand parser, which needs no model) only ever
//! produces a [`Proposal`]: a list of plain items that name accounts and amounts.
//! [`resolve`] turns a proposal into a [`Plan`]: every item is matched against the
//! ledger, turned into a `ledger::Mutation`, and the whole list is applied to a copy
//! of the ledger and validated. An item that does not resolve is rejected with a
//! reason, and a list that does not apply cleanly is rejected as a whole. Nothing in
//! this crate writes anything; the server's one write path takes `Plan::mutations`
//! only after the user approves.

pub mod anthropic;
pub mod llm;
pub mod names;
pub mod ollama;
pub mod shorthand;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use ledger::{Account, AccountKind, Event, Ledger, Mutation, PositionChange};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub use anthropic::Anthropic;
pub use ledger::AssistantProvider;
pub use llm::{build_prompt, parse_reply, Llm, LlmError, Manual, Off, Prompt};
pub use ollama::Ollama;

/// What the assistant knows about the ledger: enough to match names and show a diff.
/// Only `today`, `base_currency` and the ids, names, kinds, currencies and symbols
/// are ever put in a prompt; values and balances stay on this side of the boundary.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Context {
    pub today: NaiveDate,
    pub base_currency: String,
    pub accounts: Vec<AccountCtx>,
    /// currency -> units of base per 1 unit, as valued today (for the net effect only).
    pub rates: BTreeMap<String, Decimal>,
    pub summary: Summary,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AccountCtx {
    pub id: String,
    pub name: String,
    pub institution: Option<String>,
    pub kind: AccountKind,
    pub currency: String,
    pub liability: bool,
    /// Native-currency figure today: the balance, or the value of the positions.
    pub value: Option<Decimal>,
    pub as_of: Option<NaiveDate>,
    /// `Some` for an account valued from holdings, even when empty.
    pub positions: Option<Vec<PositionCtx>>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PositionCtx {
    pub symbol: String,
    pub units: Decimal,
    pub price: Option<Decimal>,
}

/// Headline figures, base currency, for read-only answers.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Summary {
    pub net_worth: Decimal,
    pub assets: Decimal,
    pub liabilities: Decimal,
    /// "Cash", "Investments", "Property", "Debt" -> total.
    pub by_group: BTreeMap<String, Decimal>,
    pub ytd_change: Option<Decimal>,
    pub ytd_pct: Option<Decimal>,
    pub previous_change: Option<Decimal>,
    pub previous_from: Option<NaiveDate>,
}

/// The model's whole job: this JSON. Account and symbol fields may be ids or the
/// user's own words; [`resolve`] matches them. `source` is the phrase the item came
/// from, shown on the diff card.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Proposal {
    #[serde(default)]
    pub items: Vec<Item>,
    /// Dates every item; today when omitted.
    #[serde(default)]
    pub date: Option<NaiveDate>,
    /// A read-only question the app answers from the ledger: "net worth", "cash",
    /// "debt", "investments", "property", or an account name.
    #[serde(default)]
    pub query: Option<String>,
    /// Free text to show when there is nothing to propose or look up.
    #[serde(default)]
    pub answer: Option<String>,
    /// Phrases the parser or model could not read; each becomes a rejection.
    #[serde(default)]
    pub unparsed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    /// The account's new balance in its own currency (a liability: the amount owed).
    Balance {
        account: String,
        value: Decimal,
        #[serde(default)]
        source: Option<String>,
    },
    /// Relative: `delta` added to the current balance. For a liability, paying it down
    /// is a negative delta.
    Adjust {
        account: String,
        delta: Decimal,
        #[serde(default)]
        source: Option<String>,
    },
    /// Signed units: bought +, sold -. `account` may be omitted when only one holds it.
    Trade {
        #[serde(default)]
        account: Option<String>,
        symbol: String,
        units: Decimal,
        #[serde(default)]
        price: Option<Decimal>,
        #[serde(default)]
        source: Option<String>,
    },
    Price {
        symbol: String,
        price: Decimal,
        #[serde(default)]
        source: Option<String>,
    },
    NewAccount {
        name: String,
        account_kind: AccountKind,
        #[serde(default)]
        currency: Option<String>,
        #[serde(default)]
        value: Option<Decimal>,
        #[serde(default)]
        source: Option<String>,
    },
    Note {
        note: String,
        #[serde(default)]
        source: Option<String>,
    },
    /// Money spent, for the budget, not a balance: "coffee 6 cash", "lunch 18 on visa".
    /// `paid_with` is "cash" or an account; omitted means cash.
    Spend {
        what: String,
        amount: Decimal,
        #[serde(default)]
        paid_with: Option<String>,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        source: Option<String>,
    },
}

impl Item {
    pub fn source(&self) -> Option<&str> {
        match self {
            Item::Balance { source, .. }
            | Item::Adjust { source, .. }
            | Item::Trade { source, .. }
            | Item::Price { source, .. }
            | Item::NewAccount { source, .. }
            | Item::Note { source, .. }
            | Item::Spend { source, .. } => source.as_deref(),
        }
    }
}

/// A proposal checked against the ledger: what the diff card shows and what
/// `/api/apply` receives on approval. `mutations` is empty whenever anything about
/// the proposal failed to apply, so a half-valid proposal can never be approved.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub date: NaiveDate,
    pub rows: Vec<Row>,
    pub mutations: Vec<Mutation>,
    /// Spending to add to the budget on approval. These are not ledger mutations: they go
    /// to the transaction store, so they never change a balance.
    #[serde(default)]
    pub spends: Vec<Spend>,
    pub rejected: Vec<Rejected>,
    /// Estimated change in net worth, base currency, when every row could be valued.
    pub net_change: Option<Decimal>,
    pub net_worth_after: Option<Decimal>,
    pub answer: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RowKind {
    Balance,
    Trade,
    Price,
    NewAccount,
    Note,
    Spend,
}

/// One purchase to add to the budget, as the card shows it and as
/// `POST /api/budget/spend` takes it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Spend {
    pub date: NaiveDate,
    pub merchant: String,
    /// Positive out.
    pub amount: Decimal,
    /// `None` is cash.
    #[serde(default)]
    pub account: Option<String>,
    /// `None` until the server fills it from the category rules.
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Row {
    pub kind: RowKind,
    pub account: Option<String>,
    pub account_name: String,
    /// "XEQT.TO" for a trade or price; the note for a note.
    pub detail: Option<String>,
    /// The phrase this came from.
    pub source: Option<String>,
    pub liability: bool,
    pub currency: String,
    /// Balance, units or price before; `None` when there was nothing.
    pub from: Option<Decimal>,
    pub to: Option<Decimal>,
    pub delta: Option<Decimal>,
    /// Price used for a trade.
    pub price: Option<Decimal>,
    /// Signed effect on net worth, base currency.
    pub effect: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Rejected {
    pub source: Option<String>,
    pub reason: String,
    pub hint: Option<String>,
}

fn rate(ctx: &Context, currency: &str) -> Option<Decimal> {
    if currency == ctx.base_currency {
        Some(Decimal::ONE)
    } else {
        ctx.rates.get(currency).copied()
    }
}

fn to_base(ctx: &Context, amount: Decimal, currency: &str) -> Option<Decimal> {
    rate(ctx, currency).map(|r| (amount * r).round_dp(2))
}

/// kebab-case id from a display name, unique against existing ids.
pub fn slugify(name: &str, existing: &[String]) -> String {
    let mut base = String::new();
    let mut dash = false;
    for c in name.to_ascii_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            base.push(c);
            dash = false;
        } else if !dash && !base.is_empty() {
            base.push('-');
            dash = true;
        }
    }
    let base = base.trim_end_matches('-').to_string();
    let base = if base.is_empty() { "account".to_string() } else { base };
    let mut id = base.clone();
    let mut n = 2;
    while existing.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

/// Match every item against the ledger and validate the result by applying it to a
/// copy. Pure: no IO, no clock.
pub fn resolve(ledger: &Ledger, ctx: &Context, p: &Proposal) -> Plan {
    let mut plan = Plan { date: p.date.unwrap_or(ctx.today), ..Default::default() };
    if plan.date > ctx.today {
        plan.rejected.push(Rejected {
            source: None,
            reason: format!("{} is in the future; dated today instead", plan.date),
            hint: None,
        });
        plan.date = ctx.today;
    }
    let date = plan.date;
    let mut new_ids: Vec<String> = ledger.accounts.iter().map(|a| a.id.clone()).collect();
    for phrase in &p.unparsed {
        plan.rejected.push(Rejected {
            source: Some(phrase.clone()),
            reason: format!("could not read \"{phrase}\""),
            hint: Some("try \"td chq 5102\", \"visa down 200\", or \"sold 20 XEQT in the TFSA\"".into()),
        });
    }
    for item in &p.items {
        let source = item.source().map(str::to_string);
        let reject = |plan: &mut Plan, reason: String, hint: Option<String>| {
            plan.rejected.push(Rejected { source: source.clone(), reason, hint });
        };
        match item {
            Item::Balance { account, value, .. } | Item::Adjust { account, delta: value, .. } => {
                let relative = matches!(item, Item::Adjust { .. });
                let a = match names::match_account(&ctx.accounts, account) {
                    Ok(a) => a,
                    Err((reason, hint)) => {
                        reject(&mut plan, reason, hint);
                        continue;
                    }
                };
                if a.positions.is_some() {
                    reject(
                        &mut plan,
                        format!("{} is valued from its holdings, not a balance", a.name),
                        Some(format!(
                            "say \"sold 20 XEQT in the {}\" or paste its holdings on Check-in",
                            a.name
                        )),
                    );
                    continue;
                }
                let to = if relative {
                    match a.value {
                        Some(v) => v + *value,
                        None => {
                            reject(
                                &mut plan,
                                format!(
                                    "{} has no balance yet, so \"up/down\" has nothing to start from",
                                    a.name
                                ),
                                Some("give the full amount".into()),
                            );
                            continue;
                        }
                    }
                } else {
                    *value
                };
                let from = a.value;
                let delta = from.map(|f| to - f);
                let sign = if a.liability { Decimal::NEGATIVE_ONE } else { Decimal::ONE };
                let effect = delta.and_then(|d| to_base(ctx, d * sign, &a.currency));
                if delta != Some(Decimal::ZERO) {
                    plan.mutations.push(Mutation::SetBalance { date, account: a.id.clone(), value: to });
                }
                plan.rows.push(Row {
                    kind: RowKind::Balance,
                    account: Some(a.id.clone()),
                    account_name: a.name.clone(),
                    detail: None,
                    source: source.clone(),
                    liability: a.liability,
                    currency: a.currency.clone(),
                    from,
                    to: Some(to),
                    delta,
                    price: None,
                    effect,
                });
            }
            Item::Trade { account, symbol, units, price, .. } => {
                if units.is_zero() {
                    reject(&mut plan, "a trade of zero units".into(), None);
                    continue;
                }
                let a = match account {
                    Some(q) => match names::match_account(&ctx.accounts, q) {
                        Ok(a) => a,
                        Err((reason, hint)) => {
                            reject(&mut plan, reason, hint);
                            continue;
                        }
                    },
                    None => match names::accounts_holding(&ctx.accounts, symbol) {
                        (Some(sym), holders) if holders.len() == 1 => {
                            let _ = sym;
                            holders[0]
                        }
                        (_, holders) if holders.len() > 1 => {
                            let list: Vec<&str> = holders.iter().map(|h| h.name.as_str()).collect();
                            reject(
                                &mut plan,
                                format!("{symbol} is held in more than one account"),
                                Some(format!("say which: {}", list.join(" or "))),
                            );
                            continue;
                        }
                        _ => {
                            reject(
                                &mut plan,
                                format!("no account holds {symbol}"),
                                Some("name the account: \"bought 10 VFV in the TFSA\"".into()),
                            );
                            continue;
                        }
                    },
                };
                let Some(positions) = &a.positions else {
                    reject(
                        &mut plan,
                        format!("{} tracks a balance, not holdings", a.name),
                        Some(format!("add symbols under positions for {} in ledger.yaml first", a.name)),
                    );
                    continue;
                };
                // The ledger's spelling where it is held anywhere; else a new symbol as typed.
                let sym = names::match_symbol(positions.iter().map(|p| p.symbol.as_str()), symbol)
                    .map(str::to_string)
                    .or_else(|| names::accounts_holding(&ctx.accounts, symbol).0)
                    .unwrap_or_else(|| names::canonical_symbol(symbol, &a.currency));
                let pos = positions.iter().find(|p| p.symbol == sym);
                let from = pos.map(|p| p.units).unwrap_or(Decimal::ZERO);
                let to = from + *units;
                if to < Decimal::ZERO {
                    let held: Vec<&str> = positions.iter().map(|p| p.symbol.as_str()).collect();
                    let reason = if pos.is_none() {
                        format!(
                            "{} holds no {sym}{}",
                            a.name,
                            if held.is_empty() {
                                String::new()
                            } else {
                                format!(" (it holds {})", held.join(", "))
                            }
                        )
                    } else {
                        format!("{} holds only {from} {sym}", a.name)
                    };
                    let hint = names::accounts_holding(&ctx.accounts, symbol)
                        .1
                        .iter()
                        .find(|h| h.id != a.id)
                        .map(|h| format!("did you mean the {}?", h.name));
                    reject(&mut plan, reason, hint);
                    continue;
                }
                let px = price.or_else(|| pos.and_then(|p| p.price));
                let effect = px.and_then(|px| to_base(ctx, *units * px, &a.currency)).map(|e| {
                    if a.liability {
                        -e
                    } else {
                        e
                    }
                });
                plan.mutations.push(Mutation::RecordPositionChange {
                    change: PositionChange {
                        date,
                        account: a.id.clone(),
                        symbol: sym.clone(),
                        units: *units,
                        price: *price,
                        note: source.clone(),
                    },
                });
                plan.rows.push(Row {
                    kind: RowKind::Trade,
                    account: Some(a.id.clone()),
                    account_name: a.name.clone(),
                    detail: Some(sym),
                    source: source.clone(),
                    liability: a.liability,
                    currency: a.currency.clone(),
                    from: pos.map(|p| p.units),
                    to: Some(to),
                    delta: Some(*units),
                    price: px,
                    effect,
                });
            }
            Item::Price { symbol, price, .. } => {
                if *price <= Decimal::ZERO {
                    reject(&mut plan, format!("a price for {symbol} must be positive"), None);
                    continue;
                }
                let (sym, holders) = names::accounts_holding(&ctx.accounts, symbol);
                let Some(sym) = sym else {
                    reject(
                        &mut plan,
                        format!("nothing holds {symbol}"),
                        Some("prices are kept for held symbols only".into()),
                    );
                    continue;
                };
                let mut from = None;
                let mut effect = Some(Decimal::ZERO);
                for h in &holders {
                    if let Some(p) = h.positions.as_ref().and_then(|ps| ps.iter().find(|p| p.symbol == sym)) {
                        from = from.or(p.price);
                        let e = p.price.and_then(|old| to_base(ctx, p.units * (*price - old), &h.currency));
                        effect = match (effect, e) {
                            (Some(acc), Some(e)) => Some(acc + e),
                            _ => None,
                        };
                    }
                }
                plan.mutations.push(Mutation::SetPrice { symbol: sym.clone(), date, price: *price });
                plan.rows.push(Row {
                    kind: RowKind::Price,
                    account: None,
                    account_name: holders.iter().map(|h| h.name.as_str()).collect::<Vec<_>>().join(", "),
                    detail: Some(sym),
                    source: source.clone(),
                    liability: false,
                    currency: holders.first().map(|h| h.currency.clone()).unwrap_or_default(),
                    from,
                    to: Some(*price),
                    delta: from.map(|f| *price - f),
                    price: None,
                    effect,
                });
            }
            Item::NewAccount { name, account_kind: kind, currency, value, .. } => {
                let name = name.trim();
                if name.is_empty() {
                    reject(&mut plan, "a new account needs a name".into(), None);
                    continue;
                }
                if let Ok(existing) = names::match_account(&ctx.accounts, name) {
                    if existing.name.eq_ignore_ascii_case(name) {
                        reject(
                            &mut plan,
                            format!("{} already exists", existing.name),
                            Some(format!(
                                "to update it, say \"{} {}\"",
                                existing.name,
                                value.unwrap_or_default()
                            )),
                        );
                        continue;
                    }
                }
                let id = slugify(name, &new_ids);
                new_ids.push(id.clone());
                let currency =
                    currency.clone().unwrap_or_else(|| ctx.base_currency.clone()).to_ascii_uppercase();
                let liability = kind.is_liability();
                plan.mutations.push(Mutation::UpsertAccount {
                    account: Account {
                        id: id.clone(),
                        name: name.to_string(),
                        institution: None,
                        kind: *kind,
                        currency: currency.clone(),
                        tags: vec![],
                        positions: if kind.group() == ledger::Group::Investments {
                            Some(vec![])
                        } else {
                            None
                        },
                    },
                });
                let mut effect = Some(Decimal::ZERO);
                if let Some(v) = value {
                    if kind.group() == ledger::Group::Investments {
                        reject(
                            &mut plan,
                            format!("{name} will be valued from holdings; paste them on Check-in"),
                            None,
                        );
                    } else {
                        plan.mutations.push(Mutation::SetBalance { date, account: id.clone(), value: *v });
                        let sign = if liability { Decimal::NEGATIVE_ONE } else { Decimal::ONE };
                        effect = to_base(ctx, *v * sign, &currency);
                    }
                }
                plan.rows.push(Row {
                    kind: RowKind::NewAccount,
                    account: Some(id),
                    account_name: name.to_string(),
                    detail: Some(format!("new {:?} account", kind).to_ascii_lowercase().replace('_', " ")),
                    source: source.clone(),
                    liability,
                    currency,
                    from: None,
                    to: *value,
                    delta: *value,
                    price: None,
                    effect,
                });
            }
            Item::Note { note, .. } => {
                let note = note.trim();
                if note.is_empty() {
                    continue;
                }
                plan.mutations.push(Mutation::AddEvent { event: Event { date, note: note.to_string() } });
                plan.rows.push(Row {
                    kind: RowKind::Note,
                    account: None,
                    account_name: "Note".into(),
                    detail: Some(note.to_string()),
                    source: source.clone(),
                    liability: false,
                    currency: ctx.base_currency.clone(),
                    from: None,
                    to: None,
                    delta: None,
                    price: None,
                    effect: Some(Decimal::ZERO),
                });
            }
            Item::Spend { what, amount, paid_with, category, .. } => {
                let what = what.trim();
                if what.is_empty() || amount.is_zero() {
                    reject(
                        &mut plan,
                        "a purchase needs what it was and an amount".into(),
                        Some("try \"coffee 6 cash\"".into()),
                    );
                    continue;
                }
                let account = match paid_with
                    .as_deref()
                    .map(str::trim)
                    .filter(|w| !w.is_empty() && !w.eq_ignore_ascii_case("cash"))
                {
                    None => None,
                    Some(w) => match names::match_account(&ctx.accounts, w) {
                        Ok(a) if a.positions.is_none() && a.kind.pays() => Some(a),
                        Ok(a) => {
                            reject(
                                &mut plan,
                                format!("{} isn't something you pay with", a.name),
                                Some("name a card or a bank account, or say cash".into()),
                            );
                            continue;
                        }
                        Err((reason, hint)) => {
                            reject(&mut plan, reason, hint);
                            continue;
                        }
                    },
                };
                let mut chars = what.chars();
                let merchant = chars
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
                    .unwrap_or_default();
                let currency = account.map_or_else(|| ctx.base_currency.clone(), |a| a.currency.clone());
                plan.spends.push(Spend {
                    date,
                    merchant: merchant.clone(),
                    amount: *amount,
                    account: account.map(|a| a.id.clone()),
                    category: category.as_deref().map(|c| c.trim().to_lowercase()).filter(|c| !c.is_empty()),
                    source: source.clone(),
                });
                plan.rows.push(Row {
                    kind: RowKind::Spend,
                    account: account.map(|a| a.id.clone()),
                    account_name: account.map_or_else(|| "Cash".to_string(), |a| a.name.clone()),
                    detail: Some(merchant),
                    source: source.clone(),
                    liability: false,
                    currency,
                    from: None,
                    to: Some(*amount),
                    delta: None,
                    price: None,
                    // Spending is budget, not a balance: net worth moves when you next
                    // update the account it came from.
                    effect: Some(Decimal::ZERO),
                });
            }
        }
    }
    // The gate: the whole list must apply to a copy of the ledger, or none of it does.
    if let Err(e) = ledger.apply_many(plan.mutations.clone()) {
        plan.rejected.push(Rejected {
            source: None,
            reason: format!("the proposal does not apply cleanly: {e}"),
            hint: Some("nothing was changed".into()),
        });
        plan.mutations.clear();
    }
    if !plan.rows.is_empty() {
        let effects: Option<Vec<Decimal>> = plan.rows.iter().map(|r| r.effect).collect();
        plan.net_change = effects.map(|v| v.iter().sum());
        plan.net_worth_after = plan.net_change.map(|c| ctx.summary.net_worth + c);
    }
    if let Some(q) = &p.query {
        plan.answer = Some(answer_query(ctx, q));
    } else if let Some(a) = &p.answer {
        plan.answer = Some(a.clone());
    }
    plan
}

fn money(v: Decimal) -> String {
    let neg = v.is_sign_negative();
    let s = format!("{:.0}", v.abs().round_dp(0));
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{}${out}", if neg { "−" } else { "" })
}

fn signed(v: Decimal) -> String {
    if v > Decimal::ZERO {
        format!("+{}", money(v))
    } else {
        money(v)
    }
}

/// Read-only answers from the context: net worth, a group, or one account.
pub fn answer_query(ctx: &Context, q: &str) -> String {
    let s = &ctx.summary;
    let ql = q.to_ascii_lowercase();
    let ytd = match (s.ytd_change, s.ytd_pct) {
        (Some(c), Some(p)) => {
            format!(" · {} ({}{p:.1}%) since Jan 1", signed(c), if p >= Decimal::ZERO { "+" } else { "" })
        }
        (Some(c), None) => format!(" · {} since Jan 1", signed(c)),
        _ => String::new(),
    };
    if ql.contains("net worth") || ql.contains("worth") || ql.contains("total") {
        return format!(
            "Net worth {}{ytd} · assets {} · debt {}",
            money(s.net_worth),
            money(s.assets),
            money(s.liabilities)
        );
    }
    for (label, key) in
        [("Cash", "cash"), ("Investments", "invest"), ("Property", "propert"), ("Debt", "debt")]
    {
        if ql.contains(key) || (key == "debt" && (ql.contains("owe") || ql.contains("liabilit"))) {
            let v = s.by_group.get(label).copied().unwrap_or_default();
            return format!("{label} {} · {:.1}% of assets", money(v), pct(v, s.assets));
        }
    }
    match names::match_account(&ctx.accounts, &ql) {
        Ok(a) => match a.value {
            Some(v) => format!(
                "{} {} {}{}",
                a.name,
                money(v),
                a.currency,
                a.as_of.map(|d| format!(" as of {d}")).unwrap_or_default()
            ),
            None => format!("{} has no balance yet", a.name),
        },
        Err(_) => format!(
            "Net worth {}{ytd}. Ask about cash, investments, property, debt, or an account by name.",
            money(s.net_worth)
        ),
    }
}

fn pct(part: Decimal, whole: Decimal) -> Decimal {
    if whole.is_zero() {
        Decimal::ZERO
    } else {
        (part / whole * Decimal::ONE_HUNDRED).round_dp(1)
    }
}

/// Build a context from a ledger plus the figures the caller valued (the server has
/// the market; this crate does not).
impl Context {
    pub fn account(&self, id: &str) -> Option<&AccountCtx> {
        self.accounts.iter().find(|a| a.id == id)
    }
}

#[cfg(test)]
pub(crate) mod testctx {
    use super::*;
    use rust_decimal_macros::dec;

    pub fn ledger() -> Ledger {
        ledger::io::parse(TEST_LEDGER).unwrap()
    }

    pub const TEST_LEDGER: &str = r#"
version: 1
settings:
  base_currency: CAD
  fx: { USD: 1.38 }
accounts:
  - { id: td-chq, name: TD Chequing, institution: TD, kind: chequing, currency: CAD }
  - { id: eq-sav, name: EQ Savings, institution: EQ Bank, kind: savings, currency: CAD }
  - { id: rbc-usd, name: USD Chequing, institution: RBC, kind: chequing, currency: USD }
  - id: ws-tfsa
    name: Wealthsimple TFSA
    institution: Wealthsimple
    kind: tfsa
    currency: CAD
    positions:
      - { symbol: XEQT.TO, units: 426.5 }
      - { symbol: VFV.TO, units: 620 }
  - id: qt-rrsp
    name: Questrade RRSP
    institution: Questrade
    kind: rrsp
    currency: USD
    positions:
      - { symbol: VTI, units: 310 }
  - { id: condo, name: Condo, kind: property, currency: CAD }
  - { id: td-mortgage, name: Mortgage, institution: TD, kind: mortgage, currency: CAD }
  - { id: td-visa, name: TD Visa, institution: TD, kind: credit_card, currency: CAD }
prices:
  XEQT.TO: { 2026-10-31: 34.87 }
  VFV.TO: { 2026-10-31: 128.40 }
  VTI: { 2026-10-31: 290.10 }
snapshots:
  2026-10-31:
    td-chq: 4273.20
    eq-sav: 40399.20
    rbc-usd: 1800
    condo: 610000
    td-mortgage: 412000
    td-visa: 1240
"#;

    pub fn ctx() -> Context {
        let l = ledger();
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let snap = l.snapshots.values().next_back().unwrap();
        let accounts = l
            .accounts
            .iter()
            .map(|a| AccountCtx {
                id: a.id.clone(),
                name: a.name.clone(),
                institution: a.institution.clone(),
                kind: a.kind,
                currency: a.currency.clone(),
                liability: a.kind.is_liability(),
                value: match &a.positions {
                    Some(ps) => Some(
                        ps.iter()
                            .map(|p| p.units * l.prices[&p.symbol].values().next_back().copied().unwrap())
                            .sum(),
                    ),
                    None => snap.get(&a.id).copied(),
                },
                as_of: Some(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()),
                positions: a.positions.as_ref().map(|ps| {
                    ps.iter()
                        .map(|p| PositionCtx {
                            symbol: p.symbol.clone(),
                            units: p.units,
                            price: l.prices[&p.symbol].values().next_back().copied(),
                        })
                        .collect()
                }),
            })
            .collect();
        Context {
            today,
            base_currency: "CAD".into(),
            accounts,
            rates: BTreeMap::from([("USD".to_string(), dec!(1.38))]),
            summary: Summary {
                net_worth: dec!(412950),
                assets: dec!(826190),
                liabilities: dec!(413240),
                by_group: BTreeMap::from([
                    ("Cash".to_string(), dec!(47156.40)),
                    ("Debt".to_string(), dec!(413240)),
                ]),
                ytd_change: Some(dec!(18920)),
                ytd_pct: Some(dec!(4.8)),
                previous_change: None,
                previous_from: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use testctx::{ctx, ledger};

    fn balance(account: &str, value: Decimal) -> Item {
        Item::Balance { account: account.into(), value, source: Some(account.into()) }
    }

    #[test]
    fn purchases_go_to_spends_not_mutations_and_never_move_net_worth() {
        let p = shorthand::parse("coffee 6 cash, lunch 18.40 on visa, td chq 5102", &ctx());
        let plan = resolve(&ledger(), &ctx(), &p);
        assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
        assert_eq!(plan.mutations.len(), 1, "only the balance touches the ledger");
        let got: Vec<(&str, Decimal, Option<&str>)> =
            plan.spends.iter().map(|s| (s.merchant.as_str(), s.amount, s.account.as_deref())).collect();
        assert_eq!(got, vec![("Coffee", dec!(6), None), ("Lunch", dec!(18.40), Some("td-visa"))]);
        assert_eq!(plan.spends[0].date, ctx().today);
        let kinds: Vec<RowKind> = plan.rows.iter().map(|r| r.kind).collect();
        assert_eq!(kinds, vec![RowKind::Spend, RowKind::Spend, RowKind::Balance]);
        assert_eq!((plan.rows[0].account_name.as_str(), plan.rows[0].effect), ("Cash", Some(Decimal::ZERO)));
        assert_eq!(plan.net_change, plan.rows[2].effect, "spending is not a balance change");

        let bad = Proposal {
            items: vec![Item::Spend {
                what: "coffee".into(),
                amount: dec!(6),
                paid_with: Some("tfsa".into()),
                category: None,
                source: None,
            }],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &bad);
        assert!(plan.spends.is_empty());
        assert!(plan.rejected[0].reason.contains("isn't something you pay with"), "{:?}", plan.rejected);
    }

    #[test]
    fn balances_resolve_by_id_or_words_and_value_the_effect() {
        let p = Proposal {
            items: vec![
                balance("td-chq", dec!(5102)),
                balance("visa", dec!(640)),
                balance("usd chequing", dec!(2000)),
            ],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
        assert_eq!(plan.mutations.len(), 3);
        assert_eq!(plan.rows[0].delta, Some(dec!(828.80)));
        assert_eq!(plan.rows[0].effect, Some(dec!(828.80)));
        // Paying the card down is a gain for net worth.
        assert!(plan.rows[1].liability);
        assert_eq!(plan.rows[1].delta, Some(dec!(-600)));
        assert_eq!(plan.rows[1].effect, Some(dec!(600)));
        // USD converted at the context rate.
        assert_eq!(plan.rows[2].effect, Some(dec!(276)));
        assert_eq!(plan.net_change, Some(dec!(1704.80)));
        assert_eq!(plan.net_worth_after, Some(dec!(414654.80)));
    }

    #[test]
    fn adjust_is_relative_and_needs_a_starting_balance() {
        let p = Proposal {
            items: vec![
                Item::Adjust { account: "visa".into(), delta: dec!(-200), source: None },
                Item::Adjust { account: "eq sav".into(), delta: dec!(500), source: None },
            ],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rows[0].to, Some(dec!(1040)));
        assert_eq!(plan.rows[1].to, Some(dec!(40899.20)));
        let mut c = ctx();
        c.accounts.iter_mut().find(|a| a.id == "td-visa").unwrap().value = None;
        let plan = resolve(&ledger(), &c, &p);
        assert_eq!(plan.rows.len(), 1);
        assert!(plan.rejected[0].reason.contains("no balance yet"));
    }

    #[test]
    fn unchanged_balance_is_a_row_but_not_a_mutation() {
        let p = Proposal { items: vec![balance("td chq", dec!(4273.20))], ..Default::default() };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rows.len(), 1);
        assert_eq!(plan.rows[0].delta, Some(Decimal::ZERO));
        assert!(plan.mutations.is_empty());
    }

    #[test]
    fn trades_find_the_account_check_units_and_carry_the_price() {
        let p = Proposal {
            items: vec![
                Item::Trade {
                    account: None,
                    symbol: "xeqt".into(),
                    units: dec!(-20),
                    price: None,
                    source: None,
                },
                Item::Trade {
                    account: Some("rrsp".into()),
                    symbol: "VTI".into(),
                    units: dec!(5),
                    price: Some(dec!(291)),
                    source: None,
                },
                Item::Trade {
                    account: Some("tfsa".into()),
                    symbol: "ZAG".into(),
                    units: dec!(100),
                    price: None,
                    source: None,
                },
            ],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
        assert_eq!(plan.rows[0].account.as_deref(), Some("ws-tfsa"));
        assert_eq!(plan.rows[0].detail.as_deref(), Some("XEQT.TO"));
        assert_eq!(plan.rows[0].to, Some(dec!(406.5)));
        assert_eq!(plan.rows[0].price, Some(dec!(34.87)));
        assert_eq!(plan.rows[0].effect, Some(dec!(-697.40)));
        assert_eq!(plan.rows[1].effect, Some((dec!(5) * dec!(291) * dec!(1.38)).round_dp(2)));
        // A new symbol in a position account: bare ticker gets the venue suffix, no price yet.
        assert_eq!(plan.rows[2].detail.as_deref(), Some("ZAG.TO"));
        assert_eq!(plan.rows[2].from, None);
        assert_eq!(plan.rows[2].effect, None);
        assert_eq!(plan.net_change, None);
        assert_eq!(plan.mutations.len(), 3);
    }

    #[test]
    fn selling_what_an_account_does_not_hold_is_rejected_with_a_hint() {
        let p = Proposal {
            items: vec![Item::Trade {
                account: Some("rrsp".into()),
                symbol: "XEQT".into(),
                units: dec!(-20),
                price: None,
                source: Some("sold 20 XEQT in the RRSP".into()),
            }],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert!(plan.rows.is_empty());
        assert!(plan.mutations.is_empty());
        let r = &plan.rejected[0];
        assert!(r.reason.contains("Questrade RRSP holds no XEQT.TO (it holds VTI)"), "{}", r.reason);
        assert_eq!(r.hint.as_deref(), Some("did you mean the Wealthsimple TFSA?"));
        assert_eq!(r.source.as_deref(), Some("sold 20 XEQT in the RRSP"));
    }

    #[test]
    fn a_balance_for_a_holdings_account_and_a_trade_for_a_balance_account_are_rejected() {
        let p = Proposal {
            items: vec![
                balance("tfsa", dec!(100000)),
                Item::Trade {
                    account: Some("td chq".into()),
                    symbol: "XEQT".into(),
                    units: dec!(1),
                    price: None,
                    source: None,
                },
            ],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rejected.len(), 2);
        assert!(plan.rejected[0].reason.contains("valued from its holdings"));
        assert!(plan.rejected[1].reason.contains("tracks a balance"));
        assert!(plan.mutations.is_empty());
    }

    #[test]
    fn price_updates_every_holder_and_values_the_move() {
        let p = Proposal {
            items: vec![Item::Price { symbol: "xeqt".into(), price: dec!(35.87), source: None }],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rows[0].from, Some(dec!(34.87)));
        assert_eq!(plan.rows[0].effect, Some(dec!(426.50)));
        assert!(matches!(plan.mutations[0], Mutation::SetPrice { .. }));
        let p = Proposal {
            items: vec![Item::Price { symbol: "AAPL".into(), price: dec!(1), source: None }],
            ..Default::default()
        };
        assert!(resolve(&ledger(), &ctx(), &p).rejected[0].reason.contains("nothing holds AAPL"));
    }

    #[test]
    fn new_account_gets_a_unique_id_and_an_opening_balance() {
        let p = Proposal {
            items: vec![
                Item::NewAccount {
                    name: "Tangerine Savings".into(),
                    account_kind: AccountKind::Savings,
                    currency: None,
                    value: Some(dec!(1500)),
                    source: None,
                },
                Item::NewAccount {
                    name: "TD Visa".into(),
                    account_kind: AccountKind::CreditCard,
                    currency: None,
                    value: None,
                    source: None,
                },
            ],
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rows.len(), 1);
        assert_eq!(plan.rows[0].account.as_deref(), Some("tangerine-savings"));
        assert_eq!(plan.rows[0].effect, Some(dec!(1500)));
        assert_eq!(plan.mutations.len(), 2);
        assert!(plan.rejected[0].reason.contains("already exists"));
        assert_eq!(slugify("TD Chequing", &["td-chequing".into()]), "td-chequing-2");
        assert_eq!(slugify("  ", &[]), "account");
    }

    #[test]
    fn a_list_that_does_not_apply_is_rejected_as_a_whole() {
        // Two items each fine alone; together they overdraw the position.
        let sell = |n: Decimal| Item::Trade {
            account: None,
            symbol: "VTI".into(),
            units: -n,
            price: None,
            source: None,
        };
        let p = Proposal { items: vec![sell(dec!(200)), sell(dec!(200))], ..Default::default() };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rows.len(), 2, "rows shown so the user sees what was tried");
        assert!(plan.mutations.is_empty(), "but nothing can be approved");
        assert!(plan.rejected.iter().any(|r| r.reason.contains("does not apply cleanly")));
    }

    #[test]
    fn unparsed_phrases_and_future_dates_are_rejected_not_dropped() {
        let p = Proposal {
            unparsed: vec!["lol idk".into()],
            date: Some(NaiveDate::from_ymd_opt(2030, 1, 1).unwrap()),
            ..Default::default()
        };
        let plan = resolve(&ledger(), &ctx(), &p);
        assert_eq!(plan.rejected.len(), 2);
        assert_eq!(plan.date, ctx().today);
    }

    #[test]
    fn queries_are_answered_from_the_context() {
        let c = ctx();
        assert_eq!(
            answer_query(&c, "net worth"),
            "Net worth $412,950 · +$18,920 (+4.8%) since Jan 1 · assets $826,190 · debt $413,240"
        );
        assert!(answer_query(&c, "how much cash").starts_with("Cash $47,156"));
        assert!(answer_query(&c, "what do I owe").starts_with("Debt $413,240"));
        assert_eq!(answer_query(&c, "mortgage"), "Mortgage $412,000 CAD as of 2026-10-31");
        assert!(answer_query(&c, "weather").starts_with("Net worth"));
        let p = Proposal { query: Some("eq savings".into()), ..Default::default() };
        assert_eq!(
            resolve(&ledger(), &c, &p).answer.as_deref(),
            Some("EQ Savings $40,399 CAD as of 2026-10-31")
        );
    }

    #[test]
    fn proposal_json_round_trips_with_lenient_fields() {
        let j = r#"{"items":[{"kind":"balance","account":"td-chq","value":5102.1},{"kind":"trade","symbol":"XEQT","units":-20}],"date":"2026-11-02"}"#;
        let p: Proposal = serde_json::from_str(j).unwrap();
        assert_eq!(p.items.len(), 2);
        assert!(matches!(&p.items[1], Item::Trade { account: None, price: None, .. }));
        assert!(serde_json::from_str::<Proposal>(r#"{"items":[{"kind":"wat"}]}"#).is_err());
    }
}
