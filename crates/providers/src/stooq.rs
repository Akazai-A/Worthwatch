//! Stooq's CSV quote endpoint. Symbols are Stooq-style (`xeqt.ca`, `vti.us`); this
//! module maps the ledger's Yahoo-style spelling both ways. Latest close only.

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{get, PriceHistory, PriceProvider, ProviderError, Span};

#[derive(Debug, Clone, Copy, Default)]
pub struct Stooq;

/// `XEQT.TO` -> `xeqt.ca`, `VTI` -> `vti.us`, `VOD.L` -> `vod.uk`.
pub fn to_stooq(symbol: &str) -> String {
    let (base, suffix) = symbol.rsplit_once('.').unwrap_or((symbol, ""));
    let market = match suffix.to_ascii_uppercase().as_str() {
        "" => "us",
        "TO" | "V" | "CN" | "NE" => "ca",
        "L" => "uk",
        "DE" | "F" => "de",
        other => return format!("{}.{}", base.to_ascii_lowercase(), other.to_ascii_lowercase()),
    };
    format!("{}.{market}", base.to_ascii_lowercase())
}

fn currency_for(stooq_symbol: &str) -> Option<String> {
    let suffix = stooq_symbol.rsplit_once('.')?.1;
    Some(
        match suffix.to_ascii_lowercase().as_str() {
            "us" => "USD",
            "ca" => "CAD",
            "uk" => "GBP",
            "de" => "EUR",
            _ => return None,
        }
        .to_string(),
    )
}

/// Pure: the CSV for several symbols into one history per requested symbol (ledger spelling).
pub fn parse_csv(symbols: &[String], csv: &str) -> Vec<Result<PriceHistory, ProviderError>> {
    let mut by_stooq: BTreeMap<String, (NaiveDate, Decimal)> = BTreeMap::new();
    let mut lines = csv.lines();
    let header: Vec<String> =
        lines.next().unwrap_or("").split(',').map(|h| h.trim().to_ascii_lowercase()).collect();
    let col = |name: &str| header.iter().position(|h| h == name);
    let (Some(si), Some(di), Some(ci)) = (col("symbol"), col("date"), col("close")) else {
        let e = || ProviderError::Parse {
            provider: "stooq",
            what: "quotes".into(),
            reason: "unexpected header".into(),
        };
        return symbols.iter().map(|_| Err(e())).collect();
    };
    for line in lines {
        let cells: Vec<&str> = line.split(',').map(str::trim).collect();
        let (Some(sym), Some(date), Some(close)) = (cells.get(si), cells.get(di), cells.get(ci)) else {
            continue;
        };
        let (Ok(date), Ok(close)) = (NaiveDate::parse_from_str(date, "%Y-%m-%d"), close.parse::<Decimal>())
        else {
            continue;
        };
        if close.is_sign_positive() && !close.is_zero() {
            by_stooq.insert(sym.to_ascii_lowercase(), (date, close));
        }
    }
    symbols
        .iter()
        .map(|s| {
            let key = to_stooq(s);
            match by_stooq.get(&key) {
                Some((d, p)) => Ok(PriceHistory {
                    symbol: s.clone(),
                    currency: currency_for(&key),
                    closes: BTreeMap::from([(*d, *p)]),
                }),
                None => Err(ProviderError::Unknown(s.clone(), "stooq")),
            }
        })
        .collect()
}

pub fn url(symbols: &[String]) -> String {
    let s: Vec<String> = symbols.iter().map(|x| to_stooq(x)).collect();
    format!("https://stooq.com/q/l/?s={}&f=sd2t2ohlcv&h&e=csv", s.join("+"))
}

impl PriceProvider for Stooq {
    fn name(&self) -> &'static str {
        "stooq"
    }
    fn prices(&self, symbols: &[String], _span: Span) -> Vec<Result<PriceHistory, ProviderError>> {
        if symbols.is_empty() {
            return Vec::new();
        }
        match get(&url(symbols)) {
            Ok(body) => parse_csv(symbols, &body),
            Err(e) => symbols.iter().map(|_| Err(ProviderError::Http(e.to_string()))).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn maps_symbols_both_ways() {
        assert_eq!(to_stooq("XEQT.TO"), "xeqt.ca");
        assert_eq!(to_stooq("VTI"), "vti.us");
        assert_eq!(to_stooq("BRK-B"), "brk-b.us");
        assert_eq!(
            url(&["XEQT.TO".into(), "VTI".into()]),
            "https://stooq.com/q/l/?s=xeqt.ca+vti.us&f=sd2t2ohlcv&h&e=csv"
        );
    }

    #[test]
    fn parses_known_rows_and_flags_nd() {
        let syms = vec!["XEQT.TO".to_string(), "VTI".to_string(), "NOPE".to_string()];
        let r = parse_csv(&syms, include_str!("../fixtures/stooq_quotes.csv"));
        let x = r[0].as_ref().unwrap();
        assert_eq!(x.currency.as_deref(), Some("CAD"));
        assert_eq!(x.closes.values().next(), Some(&dec!(34.12)));
        assert_eq!(r[1].as_ref().unwrap().currency.as_deref(), Some("USD"));
        assert!(matches!(r[2], Err(ProviderError::Unknown(..))));
    }
}
