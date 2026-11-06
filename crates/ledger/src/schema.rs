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
    /// Money that moves on a schedule ("$500 to the TFSA every month"). Used for
    /// projections and contribution-room plans, never to make up actual balances.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recurring: Vec<Recurring>,
    /// Contribution room per registered-account kind, as CRA reported it on a date.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub room: BTreeMap<RoomKind, Room>,
    /// When TFSA and FHSA room started, and the total put in when you said so. Lets
    /// Worthwatch work out room from the CRA limit table instead of asking for it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub registered: BTreeMap<RoomKind, Registered>,
    /// Money you put into (positive) or took out of (negative) a registered account.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributions: Vec<Contribution>,
    /// Budgeting: monthly limits, bills, where each account's transactions come from,
    /// which Wallet card is which account, categorization rules. The transactions
    /// themselves live in `transactions/`, not here.
    #[serde(default, skip_serializing_if = "Budget::is_default")]
    pub budget: Budget,
}

/// Budget settings. Amounts are monthly, in the base currency.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Budget {
    /// category -> monthly limit.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub limits: BTreeMap<String, Decimal>,
    /// Wallet card name, exactly as the Shortcut sends it -> the credit card account it is.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub cards: BTreeMap<String, AccountId>,
    /// account -> where its transactions come from. Missing means a bank statement.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sources: BTreeMap<AccountId, TxnSource>,
    /// Checked in order before the built-in rules; first match wins.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<CategoryRule>,
    /// Fixed amounts that leave every month or so (rent, phone). Shown as paid or due.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bills: Vec<Bill>,
    /// How Apple Pay taps reach this server.
    #[serde(default, skip_serializing_if = "Feed::is_default")]
    pub feed: Feed,
    /// The budget game: a monthly savings goal and the challenge cards you played.
    #[serde(default, skip_serializing_if = "Game::is_default")]
    pub game: Game,
}

/// The budget game. Each day's allowance is what the budget leaves for that day once
/// bills are out; what you don't spend goes into the month's jar. `goal` is how full
/// you want the jar by month end. Cards are weekly challenges you chose to play.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Game {
    /// Monthly jar goal, base currency. `None` means the game is not set up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<Decimal>,
    /// Cards played, oldest first. The rule is copied in when played, so a card keeps
    /// its terms even if your spending history changes later.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cards: Vec<PlayedCard>,
    /// Ids of cards you passed on, so they are not offered again that week.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub passed: Vec<String>,
}

impl Game {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayedCard {
    pub id: String,
    /// Monday of the week the card runs.
    pub week: NaiveDate,
    /// First day it counts, when played mid-week; the card then runs to Sunday.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<NaiveDate>,
    pub rule: CardRule,
}

impl PlayedCard {
    /// First day the card counts.
    pub fn start(&self) -> NaiveDate {
        self.from.filter(|f| *f > self.week).unwrap_or(self.week)
    }
}

/// What a card asks of you for its week.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CardRule {
    /// Spend at most `amount` on `category` this week.
    Cap { category: String, amount: Decimal },
    /// Spend nothing on these weekdays (0 is Monday), on `category` or on anything.
    NoSpend {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        category: Option<String>,
        weekdays: Vec<u8>,
    },
}

impl Budget {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
    /// Where an account's transactions come from (a statement unless set).
    pub fn source(&self, account: &str) -> TxnSource {
        self.sources.get(account).copied().unwrap_or_default()
    }
    /// The account a Wallet card is linked to.
    pub fn card_account(&self, card: &str) -> Option<&AccountId> {
        self.cards.get(card)
    }
}

/// `statement`: bank CSV/OFX only. `apple_pay_statement`: taps first, confirmed by the
/// statement. `apple_pay`: taps are the record (a card you never export).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TxnSource {
    #[default]
    Statement,
    ApplePayStatement,
    ApplePay,
}

impl TxnSource {
    pub fn uses_taps(self) -> bool {
        !matches!(self, TxnSource::Statement)
    }
}

/// "Anything whose description contains `contains` is `category`." Case-insensitive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CategoryRule {
    pub contains: String,
    pub category: String,
    /// Only for this account, when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<AccountId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Bill {
    pub name: String,
    /// Positive, base currency.
    pub amount: Decimal,
    #[serde(default = "Bill::default_every")]
    pub every: Every,
    #[serde(default = "Bill::default_category")]
    pub category: String,
    /// Text on the statement row that pays it ("ROGERS"); defaults to the name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matches: Option<String>,
    /// Day of the month it is usually due, for "due Nov 22".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub day: Option<u32>,
}

impl Bill {
    fn default_every() -> Every {
        Every::Month
    }
    fn default_category() -> String {
        "bills".into()
    }
}

/// `off`: no Apple Pay feed. `icloud`: the Shortcut appends taps to a file in iCloud
/// Drive that this server reads. `tailscale`: the Shortcut posts them over your tailnet.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Feed {
    #[serde(default)]
    pub mode: FeedMode,
    /// The iCloud folder to read; defaults to Shortcuts' own folder on this Mac.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icloud_dir: Option<String>,
    /// Address the phone posts to in Tailscale mode, e.g. https://mac-mini.tail1234.ts.net
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tailscale_url: Option<String>,
}

impl Feed {
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedMode {
    #[default]
    Off,
    Icloud,
    Tailscale,
}

/// One scheduled amount into an account. For an asset it is a deposit; for a debt it
/// is a payment that lowers what you owe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Recurring {
    pub account: AccountId,
    /// Positive, in the account's currency.
    pub amount: Decimal,
    pub every: Every,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Every {
    Week,
    TwoWeeks,
    Month,
    Year,
}

impl Every {
    /// Occurrences per year.
    pub fn per_year(self) -> u32 {
        match self {
            Every::Week => 52,
            Every::TwoWeeks => 26,
            Every::Month => 12,
            Every::Year => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RoomKind {
    Tfsa,
    Rrsp,
    Fhsa,
}

impl RoomKind {
    pub const ALL: [RoomKind; 3] = [RoomKind::Tfsa, RoomKind::Rrsp, RoomKind::Fhsa];

    pub fn account_kind(self) -> AccountKind {
        match self {
            RoomKind::Tfsa => AccountKind::Tfsa,
            RoomKind::Rrsp => AccountKind::Rrsp,
            RoomKind::Fhsa => AccountKind::Fhsa,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RoomKind::Tfsa => "TFSA",
            RoomKind::Rrsp => "RRSP",
            RoomKind::Fhsa => "FHSA",
        }
    }
}

/// Room as CRA reported it (My Account, or the notice of assessment for RRSP).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Room {
    pub as_of: NaiveDate,
    pub amount: Decimal,
}

/// TFSA: the year room started (turned 18, or became resident; 2009 at the earliest).
/// FHSA: the year you opened your first FHSA. `put_in` is deposits minus withdrawals
/// across every account of the kind, ever, as of `as_of`; logged contributions after
/// that add to it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Registered {
    pub since: i32,
    pub as_of: NaiveDate,
    pub put_in: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Contribution {
    pub date: NaiveDate,
    pub account: AccountId,
    /// Positive in, negative out (a withdrawal).
    pub amount: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Assumptions for projections. Percentages per year.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Projection {
    /// Expected return on investment accounts.
    #[serde(default = "Projection::default_return")]
    pub return_pct: Decimal,
    /// Interest on chequing and savings.
    #[serde(default)]
    pub cash_pct: Decimal,
    /// Growth of property and other assets.
    #[serde(default)]
    pub property_pct: Decimal,
    /// account id -> rate that replaces the group default (a debt's interest rate,
    /// a high-interest savings account).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub rates: BTreeMap<AccountId, Decimal>,
    /// Net worth you are aiming for ("years to target").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<Decimal>,
}

impl Projection {
    fn default_return() -> Decimal {
        Decimal::from(5)
    }
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

impl Default for Projection {
    fn default() -> Self {
        Self {
            return_pct: Self::default_return(),
            cash_pct: Decimal::ZERO,
            property_pct: Decimal::ZERO,
            rates: BTreeMap::new(),
            target: None,
        }
    }
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
    /// Projection assumptions. Omitted means 5% on investments, 0% elsewhere.
    #[serde(default, skip_serializing_if = "Projection::is_default")]
    pub projection: Projection,
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

    /// Something you pay for things from: a bank account, a card or a line of credit.
    pub fn pays(self) -> bool {
        matches!(self, Self::Chequing | Self::Savings | Self::CreditCard | Self::LineOfCredit)
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
                projection: Projection::default(),
            },
            accounts: Vec::new(),
            prices: BTreeMap::new(),
            snapshots: BTreeMap::new(),
            position_changes: Vec::new(),
            events: Vec::new(),
            securities: BTreeMap::new(),
            recurring: Vec::new(),
            room: BTreeMap::new(),
            registered: BTreeMap::new(),
            contributions: Vec::new(),
            budget: Budget::default(),
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
