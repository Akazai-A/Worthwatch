//! Bank of Canada Valet: daily noon rates, `FX<CUR>CAD` = CAD per 1 unit of CUR.
//! Only serves a CAD base; for another base the server treats FX as off.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{get, FxProvider, ProviderError, RateHistory, Span};

#[derive(Debug, Clone, Copy, Default)]
pub struct BankOfCanada;

#[derive(Deserialize)]
struct Root {
    #[serde(default)]
    observations: Vec<BTreeMap<String, serde_json::Value>>,
}

pub fn series(currency: &str) -> String {
    format!("FX{}CAD", currency.to_ascii_uppercase())
}

/// Pure: a Valet observations payload into one history per requested currency.
pub fn parse_valet(currencies: &[String], json: &str) -> Vec<Result<RateHistory, ProviderError>> {
    let root: Root = match serde_json::from_str(json) {
        Ok(r) => r,
        Err(e) => {
            return currencies
                .iter()
                .map(|_| {
                    Err(ProviderError::Parse {
                        provider: "bank_of_canada",
                        what: "rates".into(),
                        reason: e.to_string(),
                    })
                })
                .collect()
        }
    };
    currencies
        .iter()
        .map(|c| {
            let key = series(c);
            let mut rates = BTreeMap::new();
            for obs in &root.observations {
                let Some(d) = obs
                    .get("d")
                    .and_then(|v| v.as_str())
                    .and_then(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
                else {
                    continue;
                };
                let Some(v) = obs.get(&key).and_then(|v| v.get("v")).and_then(|v| v.as_str()) else {
                    continue;
                };
                if let Ok(r) = v.parse::<Decimal>() {
                    if r.is_sign_positive() && !r.is_zero() {
                        rates.insert(d, r);
                    }
                }
            }
            if rates.is_empty() {
                Err(ProviderError::Unknown(c.clone(), "bank_of_canada"))
            } else {
                Ok(RateHistory { currency: c.clone(), rates })
            }
        })
        .collect()
}

pub fn url(currencies: &[String], span: Span) -> String {
    let s: Vec<String> = currencies.iter().map(|c| series(c)).collect();
    let recent = match span {
        Span::Year => 260,
        Span::Recent => 10,
    };
    format!("https://www.bankofcanada.ca/valet/observations/{}/json?recent={recent}", s.join(","))
}

impl FxProvider for BankOfCanada {
    fn name(&self) -> &'static str {
        "bank_of_canada"
    }
    fn rates(
        &self,
        base: &str,
        currencies: &[String],
        span: Span,
    ) -> Vec<Result<RateHistory, ProviderError>> {
        if currencies.is_empty() {
            return Vec::new();
        }
        if base != "CAD" {
            return currencies
                .iter()
                .map(|_| {
                    Err(ProviderError::Http(format!(
                        "Bank of Canada rates are CAD-based; base currency is {base}"
                    )))
                })
                .collect();
        }
        match get(&url(currencies, span)) {
            Ok(body) => parse_valet(currencies, &body),
            Err(e) => currencies.iter().map(|_| Err(ProviderError::Http(e.to_string()))).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_series_per_currency_and_skips_missing_days() {
        let r = parse_valet(
            &["USD".into(), "EUR".into(), "JPY".into()],
            include_str!("../fixtures/boc_valet.json"),
        );
        let usd = r[0].as_ref().unwrap();
        assert_eq!(usd.rates.len(), 3);
        assert_eq!(usd.rates[&NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()], dec!(1.3912));
        let eur = r[1].as_ref().unwrap();
        assert_eq!(eur.rates.len(), 2); // no EUR observation on Oct 31
        assert!(matches!(r[2], Err(ProviderError::Unknown(..))));
    }

    #[test]
    fn url_names_the_series() {
        assert_eq!(
            url(&["USD".into(), "EUR".into()], Span::Recent),
            "https://www.bankofcanada.ca/valet/observations/FXUSDCAD,FXEURCAD/json?recent=10"
        );
    }
}
