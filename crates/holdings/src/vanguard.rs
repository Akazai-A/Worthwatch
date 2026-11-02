//! Vanguard's web site is fed by JSON. The exact wrapper has changed over the years, so
//! this parser looks for the first array of objects that carry a weight-like field and
//! reads each record by the names Vanguard has used (`longName`/`holdingName`/`name`,
//! `ticker`/`symbol`, `percentWeight`/`weight`/`marketValPercent`, `sector`,
//! `countryName`/`country`, `assetClass`/`securityType`, `currency`), with the as-of
//! date from `asOfDate`/`effectiveDate` on the record or any enclosing object.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde_json::Value;

use crate::{parse_date, parse_number, AssetClass, Constituent, FundHoldings, ParseError};

const WEIGHT_KEYS: &[&str] =
    &["percentWeight", "percentOfFund", "weight", "marketValPercent", "pctOfFund", "weighting"];
const NAME_KEYS: &[&str] = &["longName", "holdingName", "securityName", "name", "shortName"];
const TICKER_KEYS: &[&str] = &["ticker", "symbol", "tickerSymbol"];
const SECTOR_KEYS: &[&str] = &["sector", "sectorName", "gicsSector", "industrySector"];
const COUNTRY_KEYS: &[&str] = &["countryName", "country", "countryOfRisk", "location"];
const CLASS_KEYS: &[&str] = &["assetClass", "securityType", "assetType", "holdingType"];
const CURRENCY_KEYS: &[&str] = &["currency", "currencyCode", "localCurrency"];
const DATE_KEYS: &[&str] = &["asOfDate", "effectiveDate", "holdingsAsOf", "asOf", "date"];

fn first<'a>(o: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter().find_map(|k| o.get(*k))
}

fn text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.trim().is_empty() && s.trim() != "-" => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn number(v: Option<&Value>) -> Option<Decimal> {
    match v? {
        Value::String(s) => parse_number(s),
        Value::Number(n) => n.to_string().parse().ok(),
        _ => None,
    }
}

/// Depth-first: the first array whose objects have a weight field.
fn find_rows<'a>(v: &'a Value, date: &mut Option<NaiveDate>) -> Option<&'a Vec<Value>> {
    match v {
        Value::Array(a) => {
            if a.iter().any(|x| x.as_object().is_some_and(|o| first(o, WEIGHT_KEYS).is_some())) {
                return Some(a);
            }
            a.iter().find_map(|x| find_rows(x, date))
        }
        Value::Object(o) => {
            if date.is_none() {
                *date = text(first(o, DATE_KEYS)).and_then(|s| parse_date(&s));
            }
            o.values().find_map(|x| find_rows(x, date))
        }
        _ => None,
    }
}

pub fn parse_json(
    symbol: &str,
    text_in: &str,
    fallback_as_of: Option<NaiveDate>,
) -> Result<FundHoldings, ParseError> {
    let root: Value = serde_json::from_str(text_in).map_err(|e| ParseError::Format(e.to_string()))?;
    let mut as_of = None;
    let rows = find_rows(&root, &mut as_of)
        .ok_or_else(|| ParseError::Format("no array of holdings with a weight field".into()))?;
    let mut out = Vec::new();
    for r in rows {
        let Some(o) = r.as_object() else { continue };
        let Some(weight) = number(first(o, WEIGHT_KEYS)) else { continue };
        if weight <= Decimal::ZERO {
            continue;
        }
        let Some(name) = text(first(o, NAME_KEYS)) else { continue };
        if as_of.is_none() {
            as_of = text(first(o, DATE_KEYS)).and_then(|s| parse_date(&s));
        }
        out.push(Constituent {
            ticker: text(first(o, TICKER_KEYS)),
            name,
            weight,
            sector: text(first(o, SECTOR_KEYS)),
            country: text(first(o, COUNTRY_KEYS)),
            asset_class: text(first(o, CLASS_KEYS))
                .map(|s| AssetClass::parse(&s))
                .unwrap_or(AssetClass::Equity),
            currency: text(first(o, CURRENCY_KEYS)),
            exchange: None,
        });
    }
    if out.is_empty() {
        return Err(ParseError::Empty);
    }
    let name = root.as_object().and_then(|o| text(first(o, &["fundName", "longName", "name"])));
    let as_of = as_of
        .or(fallback_as_of)
        .ok_or_else(|| ParseError::Format("no as-of date in the JSON and no fallback date given".into()))?;
    Ok(FundHoldings { symbol: symbol.to_string(), name, as_of, source: "vanguard".into(), rows: out })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn fund_of_funds_json() {
        let h = parse_json("VEQT.TO", include_str!("../fixtures/vanguard_veqt.json"), None).unwrap();
        assert_eq!(h.as_of, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
        assert_eq!(h.rows.len(), 4);
        let vun = h.rows.iter().find(|r| r.ticker.as_deref() == Some("VUN")).unwrap();
        assert_eq!(vun.weight, dec!(44.9));
        assert_eq!(vun.country.as_deref(), Some("Canada"));
        assert!((h.coverage() - dec!(100)).abs() < dec!(0.5));
    }

    #[test]
    fn rejects_json_without_weights() {
        assert!(matches!(parse_json("X", r#"{"a":[{"name":"x"}]}"#, None), Err(ParseError::Format(_))));
    }
}
