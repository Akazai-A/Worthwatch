use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

pub type AccountId = String;
pub type Symbol = String;
pub type Currency = String;

pub const SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ledger {
    pub version: u8,
    pub settings: Settings,
    #[serde(default)]
    pub accounts: Vec<Account>,
    /// symbol -> date -> price in the symbol's currency
    #[serde(default)]
    pub prices: BTreeMap<Symbol, BTreeMap<NaiveDate, Decimal>>,
    /// date -> account id -> balance (liabilities: positive amount owed)
    #[serde(default)]
    pub snapshots: BTreeMap<NaiveDate, BTreeMap<AccountId, Decimal>>,
    #[serde(default)]
    pub position_changes: Vec<PositionChange>,
    #[serde(default)]
    pub events: Vec<Event>,
    /// symbol -> hand-typed metadata for look-through (sector, country, ...).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub securities: BTreeMap<Symbol, Security>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub base_currency: Currency,
    /// currency -> units of base currency per 1 unit of that currency. Typed by hand;
    /// used when no fetched rate exists for a date.
    #[serde(default)]
    pub fx: BTreeMap<Currency, Decimal>,
    /// Where market data comes from. Omitted in the file means the defaults.
    #[serde(default, skip_serializing_if = "Providers::is_default")]
    pub providers: Providers,
    /// symbol -> URL of that fund's holdings file (iShares-style CSV or Vanguard JSON),
    /// for a fund the built-in lookup cannot find. Fetched data lands in `holdings/`.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub holdings_urls: BTreeMap<Symbol, String>,
    /// How the chat panel turns a message into a proposal. Omitted means the defaults
    /// (a local Ollama, falling back to the built-in shorthand parser when it is not running).
    #[serde(default, skip_serializing_if = "Assistant::is_default")]
    pub assistant: Assistant,
}

/// Chat settings. The model only ever proposes; the API key for the cloud option
/// comes from the `ANTHROPIC_API_KEY` environment variable, never from this file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Assistant {
    #[serde(default)]
    pub provider: AssistantProvider,
    /// Ollama's base URL; default `http://127.0.0.1:11434`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ollama_url: Option<String>,
    /// Model name for the chosen provider; defaults per provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Assistant {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// `ollama`: local model, nothing leaves the machine. `cloud`: Anthropic's API, opt-in,
/// with a preview of what is sent. `shorthand`: no model at all, the built-in parser.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssistantProvider {
    #[default]
    Ollama,
    Cloud,
    Shorthand,
}

/// What you know about a security by hand: fills in or overrides what issuer files
/// say about a stock you hold directly, or a fund without a holdings file.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Security {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// GICS-style wording ("Information Technology", "Financials", ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// equity | fixed_income | cash | other
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

/// Market-data sources. Fetched data never lands in this file; it goes to
/// `quotes.json` beside it, which can be deleted and rebuilt.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Providers {
    #[serde(default)]
    pub prices: PriceSource,
    #[serde(default)]
    pub fx: FxSource,
}

impl Providers {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PriceSource {
    #[default]
    Yahoo,
    Stooq,
    Off,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FxSource {
    #[default]
    BankOfCanada,
    Off,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub institution: Option<String>,
    pub kind: AccountKind,
    pub currency: Currency,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Present => valued from positions (units x price), not from a balance snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub positions: Option<Vec<Position>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AccountKind {
    Chequing,
    Savings,
    Tfsa,
    Rrsp,
    Fhsa,
    Resp,
    NonRegistered,
    Crypto,
    Property,
    Vehicle,
    OtherAsset,
    CreditCard,
    LineOfCredit,
    Mortgage,
    Loan,
    OtherLiability,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    Cash,
    Investments,
    Property,
    Debt,
}

impl Group {
    pub fn label(self) -> &'static str {
        match self {
            Group::Cash => "Cash",
            Group::Investments => "Investments",
            Group::Property => "Property",
            Group::Debt => "Debt",
        }
    }
}

impl AccountKind {
    pub fn is_liability(self) -> bool {
        matches!(
            self,
            Self::CreditCard | Self::LineOfCredit | Self::Mortgage | Self::Loan | Self::OtherLiability
        )
    }

    pub fn is_registered(self) -> bool {
        matches!(self, Self::Tfsa | Self::Rrsp | Self::Fhsa | Self::Resp)
    }

    pub fn group(self) -> Group {
        match self {
            Self::Chequing | Self::Savings => Group::Cash,
            Self::Tfsa | Self::Rrsp | Self::Fhsa | Self::Resp | Self::NonRegistered | Self::Crypto => {
                Group::Investments
            }
            Self::Property | Self::Vehicle | Self::OtherAsset => Group::Property,
            _ => Group::Debt,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub symbol: Symbol,
    pub units: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub book_cost: Option<Decimal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PositionChange {
    pub date: NaiveDate,
    pub account: AccountId,
    pub symbol: Symbol,
    /// Signed: + buy, - sell.
    pub units: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price: Option<Decimal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub date: NaiveDate,
    pub note: String,
}

impl Ledger {
    pub fn starter(base_currency: &str) -> Self {
        Self {
            version: SCHEMA_VERSION,
            settings: Settings {
                base_currency: base_currency.to_string(),
                fx: BTreeMap::new(),
                providers: Providers::default(),
                holdings_urls: BTreeMap::new(),
                assistant: Assistant::default(),
            },
            accounts: Vec::new(),
            prices: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            position_changes: Vec::new(),
            events: Vec::new(),
            securities: BTreeMap::new(),
        }
    }

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }

    /// Every symbol held in any position account, deduplicated and sorted.
    pub fn symbols(&self) -> Vec<Symbol> {
        let mut v: Vec<Symbol> = self
            .accounts
            .iter()
            .flat_map(|a| a.positions.as_deref().unwrap_or(&[]).iter().map(|p| p.symbol.clone()))
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Every non-base currency an account is denominated in, sorted.
    pub fn foreign_currencies(&self) -> Vec<Currency> {
        let mut v: Vec<Currency> = self
            .accounts
            .iter()
            .filter(|a| a.currency != self.settings.base_currency)
            .map(|a| a.currency.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    }
}
