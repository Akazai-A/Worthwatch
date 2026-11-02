//! The provider boundary. Each source is a struct with a pure `parse_*` function
//! over the bytes it returns, and a thin blocking HTTP shim. Tests use the
//! fixtures and [`Manual`]; nothing here is reached by `cargo test` over the network.

pub mod boc;
pub mod cache;
pub mod stooq;
pub mod yahoo;

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use thiserror::Error;

pub use cache::QuoteCache;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("{0}")]
    Http(String),
    #[error("{provider}: could not read the response for {what}: {reason}")]
    Parse { provider: &'static str, what: String, reason: String },
    #[error("{0} is not quoted by {1}")]
    Unknown(String, &'static str),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// Dated closes for one symbol, in the currency the provider quotes it in.
#[derive(Debug, Clone, PartialEq)]
pub struct PriceHistory {
    pub symbol: String,
    pub currency: Option<String>,
    pub closes: BTreeMap<NaiveDate, Decimal>,
}

/// Dated rates for one currency: units of `base` per 1 unit of `currency`.
#[derive(Debug, Clone, PartialEq)]
pub struct RateHistory {
    pub currency: String,
    pub rates: BTreeMap<NaiveDate, Decimal>,
}

/// Depth of history to ask for: a backfill on first sight of a symbol, a top-up after.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Year,
    Recent,
}

pub trait PriceProvider {
    fn name(&self) -> &'static str;
    /// One result per symbol, in order; a symbol the source does not know is an `Err`.
    fn prices(&self, symbols: &[String], span: Span) -> Vec<Result<PriceHistory, ProviderError>>;
}

pub trait FxProvider {
    fn name(&self) -> &'static str;
    fn rates(&self, base: &str, currencies: &[String], span: Span)
        -> Vec<Result<RateHistory, ProviderError>>;
}

/// Fixed answers; the provider used in tests and for a future "paste a price" path.
#[derive(Debug, Clone, Default)]
pub struct Manual {
    pub prices: BTreeMap<String, PriceHistory>,
    pub rates: BTreeMap<String, RateHistory>,
}

impl PriceProvider for Manual {
    fn name(&self) -> &'static str {
        "manual"
    }
    fn prices(&self, symbols: &[String], _span: Span) -> Vec<Result<PriceHistory, ProviderError>> {
        symbols
            .iter()
            .map(|s| self.prices.get(s).cloned().ok_or_else(|| ProviderError::Unknown(s.clone(), "manual")))
            .collect()
    }
}

impl FxProvider for Manual {
    fn name(&self) -> &'static str {
        "manual"
    }
    fn rates(
        &self,
        _base: &str,
        currencies: &[String],
        _span: Span,
    ) -> Vec<Result<RateHistory, ProviderError>> {
        currencies
            .iter()
            .map(|c| self.rates.get(c).cloned().ok_or_else(|| ProviderError::Unknown(c.clone(), "manual")))
            .collect()
    }
}

/// Nothing is fetched; every symbol is "unknown".
#[derive(Debug, Clone, Copy, Default)]
pub struct Off;

impl PriceProvider for Off {
    fn name(&self) -> &'static str {
        "off"
    }
    fn prices(&self, symbols: &[String], _span: Span) -> Vec<Result<PriceHistory, ProviderError>> {
        symbols.iter().map(|s| Err(ProviderError::Unknown(s.clone(), "off"))).collect()
    }
}

impl FxProvider for Off {
    fn name(&self) -> &'static str {
        "off"
    }
    fn rates(
        &self,
        _base: &str,
        currencies: &[String],
        _span: Span,
    ) -> Vec<Result<RateHistory, ProviderError>> {
        currencies.iter().map(|c| Err(ProviderError::Unknown(c.clone(), "off"))).collect()
    }
}

pub(crate) const USER_AGENT: &str = concat!("worthwatch/", env!("CARGO_PKG_VERSION"));

/// Blocking GET with a short timeout. Returns the body as text.
pub(crate) fn get(url: &str) -> Result<String, ProviderError> {
    let agent =
        ureq::AgentBuilder::new().timeout(std::time::Duration::from_secs(15)).user_agent(USER_AGENT).build();
    match agent.get(url).call() {
        Ok(resp) => resp.into_string().map_err(|e| ProviderError::Http(e.to_string())),
        Err(ureq::Error::Status(code, resp)) => {
            // Yahoo answers 404 with a JSON error body we can still read.
            let body = resp.into_string().unwrap_or_default();
            if code == 404 && !body.is_empty() {
                Ok(body)
            } else {
                Err(ProviderError::Http(format!("HTTP {code} from {url}")))
            }
        }
        Err(e) => Err(ProviderError::Http(e.to_string())),
    }
}

pub(crate) fn decimal_from_f64(x: f64, dp: u32) -> Option<Decimal> {
    Decimal::try_from(x).ok().map(|d| d.round_dp(dp)).filter(|d| !d.is_zero() && d.is_sign_positive())
}
