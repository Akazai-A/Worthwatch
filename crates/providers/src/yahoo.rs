//! Yahoo Finance chart endpoint: `/v8/finance/chart/<SYMBOL>?range=..&interval=1d`.
//! Symbols are Yahoo-style (`XEQT.TO`, `VTI`), the same spelling the ledger uses.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate};
use serde::Deserialize;

use crate::{decimal_from_f64, get, PriceHistory, PriceProvider, ProviderError, Span};

#[derive(Debug, Clone, Copy, Default)]
pub struct Yahoo;

#[derive(Deserialize)]
struct Root {
    chart: Chart,
}
#[derive(Deserialize)]
struct Chart {
    result: Option<Vec<Res>>,
    error: Option<Err>,
}
#[derive(Deserialize)]
struct Err {
    description: Option<String>,
}
#[derive(Deserialize)]
struct Res {
    meta: Meta,
    #[serde(default)]
    timestamp: Vec<i64>,
    indicators: Indicators,
}
#[derive(Deserialize)]
struct Meta {
    currency: Option<String>,
    #[serde(default)]
    gmtoffset: i64,
    #[serde(rename = "regularMarketPrice")]
    regular_market_price: Option<f64>,
    #[serde(rename = "regularMarketTime")]
    regular_market_time: Option<i64>,
}
#[derive(Deserialize)]
struct Indicators {
    quote: Vec<Quote>,
}
#[derive(Deserialize)]
struct Quote {
    #[serde(default)]
    close: Vec<Option<f64>>,
}

fn local_date(unix: i64, gmtoffset: i64) -> Option<NaiveDate> {
    DateTime::from_timestamp(unix + gmtoffset, 0).map(|t| t.date_naive())
}

/// Pure: the chart JSON for one symbol into dated closes (exchange-local dates).
/// The latest bar is the live `regularMarketPrice` when Yahoo gives one.
pub fn parse_chart(symbol: &str, json: &str) -> Result<PriceHistory, ProviderError> {
    let root: Root = serde_json::from_str(json).map_err(|e| ProviderError::Parse {
        provider: "yahoo",
        what: symbol.to_string(),
        reason: e.to_string(),
    })?;
    let Some(res) = root.chart.result.and_then(|mut v| v.pop()) else {
        let why = root.chart.error.and_then(|e| e.description).unwrap_or_else(|| "no result".into());
        return Err(ProviderError::Parse { provider: "yahoo", what: symbol.to_string(), reason: why });
    };
    let off = res.meta.gmtoffset;
    let mut closes = BTreeMap::new();
    if let Some(q) = res.indicators.quote.first() {
        for (ts, close) in res.timestamp.iter().zip(q.close.iter()) {
            if let (Some(d), Some(p)) = (local_date(*ts, off), close.and_then(|c| decimal_from_f64(c, 4))) {
                closes.insert(d, p);
            }
        }
    }
    if let (Some(ts), Some(p)) = (res.meta.regular_market_time, res.meta.regular_market_price) {
        if let (Some(d), Some(p)) = (local_date(ts, off), decimal_from_f64(p, 4)) {
            closes.insert(d, p);
        }
    }
    if closes.is_empty() {
        return Err(ProviderError::Unknown(symbol.to_string(), "yahoo"));
    }
    Ok(PriceHistory { symbol: symbol.to_string(), currency: res.meta.currency, closes })
}

pub fn url(symbol: &str, span: Span) -> String {
    let range = match span {
        Span::Year => "1y",
        Span::Recent => "5d",
    };
    format!("https://query1.finance.yahoo.com/v8/finance/chart/{symbol}?range={range}&interval=1d")
}

impl PriceProvider for Yahoo {
    fn name(&self) -> &'static str {
        "yahoo"
    }
    fn prices(&self, symbols: &[String], span: Span) -> Vec<Result<PriceHistory, ProviderError>> {
        symbols.iter().map(|s| get(&url(s, span)).and_then(|body| parse_chart(s, &body))).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn parses_closes_with_exchange_local_dates_and_live_price() {
        let h = parse_chart("XEQT.TO", include_str!("../fixtures/yahoo_chart_xeqt.json")).unwrap();
        assert_eq!(h.currency.as_deref(), Some("CAD"));
        assert_eq!(h.closes.len(), 5);
        let d = NaiveDate::from_ymd_opt(2025, 10, 31).unwrap();
        assert_eq!(h.closes[&d], dec!(34.12));
        assert_eq!(h.closes.values().next(), Some(&dec!(33.71)));
        assert_eq!(h.closes.keys().next().unwrap().to_string(), "2025-10-27");
    }

    #[test]
    fn unknown_symbol_is_an_error_with_yahoos_reason() {
        let e = parse_chart("NOPE.TO", include_str!("../fixtures/yahoo_chart_unknown.json")).unwrap_err();
        assert!(e.to_string().contains("delisted"), "{e}");
    }

    #[test]
    fn urls_use_the_ledger_spelling() {
        assert_eq!(
            url("XEQT.TO", Span::Recent),
            "https://query1.finance.yahoo.com/v8/finance/chart/XEQT.TO?range=5d&interval=1d"
        );
        assert!(url("VTI", Span::Year).contains("range=1y"));
    }
}
