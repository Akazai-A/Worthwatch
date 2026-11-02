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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    pub base_currency: Currency,
    /// currency -> units of base currency per 1 unit of that currency
    #[serde(default)]
    pub fx: BTreeMap<Currency, Decimal>,
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
            settings: Settings { base_currency: base_currency.to_string(), fx: BTreeMap::new() },
            accounts: Vec::new(),
            prices: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            position_changes: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn account(&self, id: &str) -> Option<&Account> {
        self.accounts.iter().find(|a| a.id == id)
    }
}
