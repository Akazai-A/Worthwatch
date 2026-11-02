//! What a fund owns, in one shape, whoever published it; and the pure maths that
//! multiplies your positions through those compositions.
//!
//! Nothing here does IO or touches the clock. An issuer's holdings file comes in as
//! text (from a download or a file the user dropped in), goes through one of the
//! `parse_*` functions into a [`FundHoldings`], and the server keeps those as
//! snapshots. [`lookthrough`] turns positions plus snapshots into companies,
//! exposure buckets and overlap.

pub mod csv;
pub mod diff;
pub mod lookthrough;
pub mod region;
pub mod sector;
pub mod vanguard;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use diff::{diff_fund, drift, fund_dollars, BucketDelta, ChangeKind, ConstituentChange, Drift, FundDiff};
pub use lookthrough::{
    lookthrough, overlap, Bucket, Company, FundStatus, Holding, LookThrough, Overlap, OverlapCell,
    SecurityMeta, Via,
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("no header row with a name and a weight column in the first {0} lines")]
    NoHeader(usize),
    #[error("no rows with a positive weight")]
    Empty,
    #[error("could not read the file as issuer holdings: {0}")]
    Format(String),
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Equity,
    FixedIncome,
    Cash,
    #[default]
    Other,
}

impl AssetClass {
    pub fn label(self) -> &'static str {
        match self {
            AssetClass::Equity => "Equity",
            AssetClass::FixedIncome => "Fixed income",
            AssetClass::Cash => "Cash",
            AssetClass::Other => "Other",
        }
    }

    /// Issuer wording ("Equity", "Fixed Income", "Cash and/or Derivatives", "Money Market",
    /// "Futures", "FX", ...) into the four buckets we report.
    pub fn parse(raw: &str) -> Self {
        let s = raw.trim().to_ascii_lowercase();
        if s.is_empty() || s == "-" {
            return AssetClass::Other;
        }
        if s.contains("equity") || s.contains("stock") || s.contains("reit") {
            AssetClass::Equity
        } else if s.contains("fixed") || s.contains("bond") || s.contains("debt") || s.contains("treasur") {
            AssetClass::FixedIncome
        } else if s.contains("cash") || s.contains("money market") {
            AssetClass::Cash
        } else {
            AssetClass::Other
        }
    }
}

/// One line of an issuer's holdings file, normalised.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Constituent {
    /// Issuer's ticker for the line, when it has one (cash and bonds often do not).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticker: Option<String>,
    pub name: String,
    /// Percent of the fund (0-100).
    pub weight: Decimal,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sector: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default)]
    pub asset_class: AssetClass,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exchange: Option<String>,
}

/// A fund's composition on one date. Stored as-is; never edited after the fact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FundHoldings {
    /// Ledger spelling of the fund (`XEQT.TO`, `VTI`).
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub as_of: NaiveDate,
    /// Where the file came from: `ishares`, `vanguard`, `file`, `seed`.
    pub source: String,
    pub rows: Vec<Constituent>,
}

impl FundHoldings {
    /// Sum of the weights. Funds that only disclose their top names sum well under 100.
    pub fn coverage(&self) -> Decimal {
        self.rows.iter().map(|r| r.weight).sum()
    }

    /// The asset class most of the disclosed weight sits in.
    pub fn dominant_asset_class(&self) -> AssetClass {
        let mut by: std::collections::BTreeMap<AssetClass, Decimal> = Default::default();
        for r in &self.rows {
            *by.entry(r.asset_class).or_default() += r.weight;
        }
        by.into_iter().max_by_key(|(_, w)| *w).map(|(c, _)| c).unwrap_or_default()
    }
}

/// Try every known format. CSV files from iShares, Vanguard, BMO and most other issuers
/// go through the header-sniffing CSV parser; Vanguard's site hands out JSON.
pub fn parse_any(
    symbol: &str,
    text: &str,
    fallback_as_of: Option<NaiveDate>,
) -> Result<FundHoldings, ParseError> {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    let head = t.get(..t.len().min(512)).unwrap_or(t).to_ascii_lowercase();
    if head.starts_with("<!doctype html") || head.starts_with("<html") || head.contains("<head>") {
        return Err(ParseError::Format(
            "the address answered with a web page, not a holdings file; download the CSV from the fund's page and drop it in, or set settings.holdings_urls".into(),
        ));
    }
    if t.starts_with('{') || t.starts_with('[') {
        return vanguard::parse_json(symbol, t, fallback_as_of);
    }
    csv::parse_csv(symbol, t, fallback_as_of)
}

/// Does this constituent read as a fund rather than a stock? The store's firm test is
/// the issuer's product list (`HoldingsProvider::knows`); this is the secondary one,
/// used to follow funds no list covers and, in the look-through, to report a child
/// fund with no snapshot. A row qualifies on its name ("ISHARES ...", "Vanguard ...",
/// "... ETF", "... Index Fund") or a sector of "Funds"; a numeric ticker (Asian stocks
/// such as 5347 VANGUARD INTERNATIONAL SEMICONDUCTOR) or a cash line never does.
pub fn is_fundish(c: &Constituent) -> bool {
    let n = c.name.to_ascii_lowercase();
    let named_like_a_fund = n.contains("ishares")
        || n.contains("vanguard")
        || n.contains(" etf")
        || n.starts_with("etf ")
        || n.contains("index fund");
    let sector_says_fund = c
        .sector
        .as_deref()
        .is_some_and(|s| s.eq_ignore_ascii_case("funds") || s.eq_ignore_ascii_case("fund"));
    let numeric_ticker =
        c.ticker.as_deref().is_some_and(|t| !t.is_empty() && t.chars().all(|ch| ch.is_ascii_digit()));
    (named_like_a_fund || sector_says_fund) && !numeric_ticker && c.asset_class != AssetClass::Cash
}

/// Is this row listed where iShares and Vanguard list their North American funds
/// (NYSE Arca, Cboe BZX, Nasdaq, Toronto, Cboe Canada)? A product-list hit on a bare
/// ticker only counts when the row trades there: Corbion (CRBN, Euronext Amsterdam)
/// shares its ticker with an iShares US ETF and must not be followed as one. With no
/// exchange column, the country stands in.
pub fn trades_on_north_american_fund_venue(c: &Constituent) -> bool {
    match c.exchange.as_deref().map(|e| e.trim().to_ascii_lowercase()) {
        Some(e) if !e.is_empty() && e != "-" => {
            e.contains("arca")
                || e.contains("cboe")
                || e.contains("bzx")
                || e.contains("bats")
                || e.contains("nasdaq")
                || e.contains("toronto")
                || e.contains("tsx")
                || e.contains("neo")
                || e.contains("new york stock exchange")
        }
        _ => c
            .country
            .as_deref()
            .is_some_and(|l| l.eq_ignore_ascii_case("united states") || l.eq_ignore_ascii_case("canada")),
    }
}

/// Ledger-style symbol for a constituent that is itself a listed fund, from the
/// issuer's bare ticker plus where it trades. `XIC` in Canada -> `XIC.TO`; `ITOT` in
/// the United States -> `ITOT`. Returns `None` for lines without a ticker.
pub fn child_symbol(c: &Constituent) -> Option<String> {
    let t = c.ticker.as_deref()?.trim();
    if t.is_empty() || t == "-" || t.contains(' ') {
        return None;
    }
    if t.contains('.') {
        return Some(t.to_uppercase());
    }
    let canada = c.exchange.as_deref().is_some_and(|e| e.to_ascii_lowercase().contains("toronto"))
        || c.country.as_deref().is_some_and(|l| l.eq_ignore_ascii_case("canada"))
        || (c.exchange.is_none() && c.currency.as_deref() == Some("CAD"));
    Some(if canada { format!("{}.TO", t.to_uppercase()) } else { t.to_uppercase() })
}

/// Parse an issuer date in the spellings we have seen: `Oct 31, 2025`, `31-Oct-2025`,
/// `2025-10-31`, `10/31/2025`, `October 31, 2025`, `as at September 30, 2025`.
pub fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim().trim_matches('"').trim();
    for f in
        ["%b %d, %Y", "%B %d, %Y", "%d-%b-%Y", "%Y-%m-%d", "%m/%d/%Y", "%d %b %Y", "%d %B %Y", "%b %d %Y"]
    {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return Some(d);
        }
    }
    // Find a date-looking run anywhere in the text.
    let words: Vec<&str> =
        s.split(|c: char| c == ',' || c == '"' || c.is_whitespace()).filter(|w| !w.is_empty()).collect();
    for i in 0..words.len() {
        for n in [3, 2, 1] {
            if i + n <= words.len() {
                let cand = words[i..i + n].join(" ");
                for f in ["%b %d %Y", "%B %d %Y", "%d %b %Y", "%d %B %Y", "%d-%b-%Y", "%Y-%m-%d", "%m/%d/%Y"]
                {
                    if let Ok(d) = NaiveDate::parse_from_str(&cand, f) {
                        return Some(d);
                    }
                }
            }
        }
    }
    None
}

/// `"1,234.56"`, `"25.01%"`, `"-"`, `""` -> a decimal or `None`.
pub fn parse_number(s: &str) -> Option<Decimal> {
    let t: String =
        s.trim().trim_matches('"').chars().filter(|c| *c != ',' && *c != '%' && *c != '$').collect();
    if t.is_empty() || t == "-" {
        return None;
    }
    t.parse::<Decimal>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_html_page_is_named_as_such_not_as_a_parse_error() {
        let html =
            "\n<!DOCTYPE html><html lang=\"en\"><head><title>iShares</title></head><body></body></html>";
        let err = parse_any("ITOT", html, None).unwrap_err();
        assert!(matches!(err, ParseError::Format(_)));
        assert!(err.to_string().contains("web page"), "{err}");
    }

    #[test]
    fn fundish_recognises_issuer_naming_and_funds_sector() {
        let row = |name: &str, sector: Option<&str>| Constituent {
            ticker: Some("X".into()),
            name: name.into(),
            weight: rust_decimal::Decimal::ONE,
            sector: sector.map(Into::into),
            country: None,
            asset_class: AssetClass::Equity,
            currency: None,
            exchange: None,
        };
        // Whatever the sector column says on a fund-of-fund line (iShares files vary).
        assert!(is_fundish(&row("ISHARES CORE S&P TOTAL U.S. STOCK", None)));
        assert!(is_fundish(&row("ISHARES CORE S&P TOTAL U.S. STOCK", Some("-"))));
        assert!(is_fundish(&row("ISHARES CORE S&P TOTAL US STOCK MARKET ETF", Some("Equity"))));
        assert!(is_fundish(&row("Vanguard FTSE Canada All Cap", None)));
        assert!(is_fundish(&row("Some Bond ETF", None)));
        assert!(is_fundish(&row("Whatever", Some("Funds"))));
        assert!(!is_fundish(&row("APPLE INC", Some("Information Technology"))));
        // A Taiwanese stock in XEF's file, named after Vanguard: its ticker is numeric.
        let mut tw = row("VANGUARD INTERNATIONAL SEMICONDUCT", Some("Information Technology"));
        tw.ticker = Some("5347".into());
        assert!(!is_fundish(&tw), "numeric tickers are never looked up as funds");
        let mut cash = row("ISHARES CASH", None);
        cash.asset_class = AssetClass::Cash;
        assert!(!is_fundish(&cash));
    }

    #[test]
    fn venue_gate_keeps_north_american_listings_only() {
        let mk = |exchange: Option<&str>, country: Option<&str>| Constituent {
            ticker: Some("CRBN".into()),
            name: "X".into(),
            weight: rust_decimal::Decimal::ONE,
            sector: None,
            country: country.map(Into::into),
            asset_class: AssetClass::Equity,
            currency: None,
            exchange: exchange.map(Into::into),
        };
        assert!(trades_on_north_american_fund_venue(&mk(Some("NYSE Arca"), Some("United States"))));
        assert!(trades_on_north_american_fund_venue(&mk(Some("Cboe BZX"), Some("United States"))));
        assert!(trades_on_north_american_fund_venue(&mk(Some("Toronto Stock Exchange"), Some("Canada"))));
        assert!(!trades_on_north_american_fund_venue(&mk(Some("Euronext Amsterdam"), Some("Netherlands"))));
        assert!(!trades_on_north_american_fund_venue(&mk(
            Some("London Stock Exchange"),
            Some("United Kingdom")
        )));
        assert!(
            trades_on_north_american_fund_venue(&mk(None, Some("United States"))),
            "no exchange column: country decides"
        );
        assert!(!trades_on_north_american_fund_venue(&mk(Some("-"), Some("Malaysia"))));
    }

    #[test]
    fn dates_in_issuer_spellings() {
        let d = NaiveDate::from_ymd_opt(2025, 10, 31).unwrap();
        for s in [
            "Oct 31, 2025",
            "\"Oct 31, 2025\"",
            "31-Oct-2025",
            "2025-10-31",
            "10/31/2025",
            "October 31, 2025",
            "Holdings as at October 31, 2025",
            "Fund Holdings as of,\"Oct 31, 2025\"",
        ] {
            assert_eq!(parse_date(s), Some(d), "{s}");
        }
        assert_eq!(parse_date("Shares Outstanding"), None);
    }

    #[test]
    fn numbers_with_grouping_and_percent() {
        assert_eq!(parse_number("\"1,234.56\""), Some(Decimal::new(123456, 2)));
        assert_eq!(parse_number("25.01%"), Some(Decimal::new(2501, 2)));
        assert_eq!(parse_number("-"), None);
    }

    #[test]
    fn child_symbols_follow_the_exchange() {
        let mk = |t: &str, ex: Option<&str>, loc: Option<&str>| Constituent {
            ticker: Some(t.into()),
            name: t.into(),
            weight: Decimal::ONE,
            sector: None,
            country: loc.map(Into::into),
            asset_class: AssetClass::Equity,
            currency: None,
            exchange: ex.map(Into::into),
        };
        assert_eq!(
            child_symbol(&mk("XIC", Some("Toronto Stock Exchange"), Some("Canada"))).as_deref(),
            Some("XIC.TO")
        );
        assert_eq!(
            child_symbol(&mk("ITOT", Some("NYSE Arca"), Some("United States"))).as_deref(),
            Some("ITOT")
        );
        assert_eq!(child_symbol(&mk("VUN", None, Some("Canada"))).as_deref(), Some("VUN.TO"));
        assert_eq!(child_symbol(&mk("-", None, None)), None);
    }
}
