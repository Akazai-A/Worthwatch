//! Issuer holdings CSVs. iShares (both the Canadian and US sites), Vanguard's export,
//! BMO and most others share a shape: a few metadata lines (fund name, "Holdings as
//! of ..."), a header row naming a ticker, a name, a weight and usually sector,
//! asset class, location and currency columns, the rows, then a disclaimer. The
//! parser sniffs the header row by its column names rather than trusting a position,
//! so a reordered column or an extra line of preamble does not break it.

use chrono::NaiveDate;

use crate::{parse_date, parse_number, AssetClass, Constituent, FundHoldings, ParseError};

const HEADER_SCAN: usize = 60;

/// Split one CSV line honouring double quotes (`"a, b",c` -> `a, b` and `c`).
pub fn split_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
            _ => cur.push(c),
        }
    }
    out.push(cur.trim().to_string());
    out
}

#[derive(Default)]
struct Columns {
    ticker: Option<usize>,
    name: Option<usize>,
    weight: Option<usize>,
    sector: Option<usize>,
    asset_class: Option<usize>,
    country: Option<usize>,
    currency: Option<usize>,
    exchange: Option<usize>,
}

fn detect(cells: &[String]) -> Option<Columns> {
    let mut c = Columns::default();
    for (i, raw) in cells.iter().enumerate() {
        let h = raw.to_ascii_lowercase();
        let h = h.trim();
        if h.is_empty() {
            continue;
        }
        if c.weight.is_none()
            && (h.starts_with("weight")
                || h.contains("% of")
                || h.contains("percent")
                || h == "% net assets"
                || h.contains("% weight"))
            && !h.contains("notional")
        {
            c.weight = Some(i);
        } else if c.ticker.is_none()
            && (h == "ticker" || h == "symbol" || h.ends_with(" ticker") || h == "security ticker")
        {
            c.ticker = Some(i);
        } else if c.name.is_none()
            && (h == "name"
                || h.contains("holding name")
                || h == "security name"
                || h == "holding"
                || h == "security"
                || h == "description"
                || h == "issuer name")
        {
            c.name = Some(i);
        } else if c.sector.is_none() && h.contains("sector") && !h.contains("sub") {
            c.sector = Some(i);
        } else if c.asset_class.is_none()
            && (h.contains("asset class") || h == "security type" || h == "asset type")
        {
            c.asset_class = Some(i);
        } else if c.country.is_none()
            && (h == "location" || h == "country" || h.contains("country of") || h == "market")
        {
            c.country = Some(i);
        } else if h == "market currency" || h == "trading currency" || h == "local currency" {
            c.currency = Some(i); // the security's own currency beats the fund's
        } else if c.currency.is_none() && h == "currency" {
            c.currency = Some(i);
        } else if c.exchange.is_none() && h.contains("exchange") {
            c.exchange = Some(i);
        }
    }
    (c.weight.is_some() && c.name.is_some()).then_some(c)
}

fn clean(s: Option<&String>) -> Option<String> {
    let v = s?.trim();
    (!v.is_empty() && v != "-" && v != "—").then(|| v.to_string())
}

/// Parse a holdings CSV. `fallback_as_of` is used when the file carries no date
/// (the server passes the file's modification date).
pub fn parse_csv(
    symbol: &str,
    text: &str,
    fallback_as_of: Option<NaiveDate>,
) -> Result<FundHoldings, ParseError> {
    let lines: Vec<&str> = text.lines().collect();
    let mut header_at = None;
    let mut cols = Columns::default();
    for (i, line) in lines.iter().enumerate().take(HEADER_SCAN) {
        let cells = split_line(line);
        if cells.len() >= 3 {
            if let Some(c) = detect(&cells) {
                header_at = Some(i);
                cols = c;
                break;
            }
        }
    }
    let Some(h) = header_at else { return Err(ParseError::NoHeader(HEADER_SCAN.min(lines.len()))) };

    // Metadata above the header: the fund name is the first plain line; the as-of date
    // is on a line that says so, else the first date anywhere above the header.
    let mut name = None;
    let mut as_of = None;
    let mut any_date = None;
    for line in &lines[..h] {
        let t = line.trim().trim_start_matches('\u{feff}');
        if t.is_empty() {
            continue;
        }
        let lower = t.to_ascii_lowercase();
        if name.is_none() && !t.contains(',') && !lower.contains("as of") && !lower.contains("as at") {
            name = Some(t.trim_matches('"').to_string());
        } else if name.is_none() {
            let first = split_line(t).into_iter().next().unwrap_or_default();
            if !first.is_empty()
                && !lower.contains("as of")
                && !lower.contains("as at")
                && parse_date(&first).is_none()
            {
                name = Some(first);
            }
        }
        if (lower.contains("as of") || lower.contains("as at")) && as_of.is_none() {
            as_of = parse_date(t);
        }
        if any_date.is_none() {
            any_date = split_line(t).iter().find_map(|c| parse_date(c));
        }
    }
    let as_of = as_of
        .or(any_date)
        .or(fallback_as_of)
        .ok_or_else(|| ParseError::Format("no 'as of' date in the file and no fallback date given".into()))?;

    let mut rows = Vec::new();
    let mut seen_rows = false;
    for line in &lines[h + 1..] {
        let cells = split_line(line);
        if cells.iter().all(|c| c.is_empty()) {
            if seen_rows {
                break; // blank line after the table: disclaimer follows
            }
            continue;
        }
        if cells.len() < 3 {
            if seen_rows {
                break;
            }
            continue;
        }
        let Some(weight) = cols.weight.and_then(|i| cells.get(i)).and_then(|w| parse_number(w)) else {
            if seen_rows {
                break;
            }
            continue;
        };
        seen_rows = true;
        if weight <= rust_decimal::Decimal::ZERO {
            continue; // short futures legs, negative cash: not a holding you can look through
        }
        let Some(name) = clean(cols.name.and_then(|i| cells.get(i))) else { continue };
        // No asset-class column at all means an equity-only export.
        let asset_class = cols
            .asset_class
            .and_then(|i| cells.get(i))
            .map(|s| AssetClass::parse(s))
            .unwrap_or(if cols.asset_class.is_none() { AssetClass::Equity } else { AssetClass::Other });
        let sector = clean(cols.sector.and_then(|i| cells.get(i)));
        let asset_class = match (&sector, asset_class) {
            (Some(s), AssetClass::Other) if s.to_ascii_lowercase().contains("cash") => AssetClass::Cash,
            (Some(s), AssetClass::Other)
                if s.to_ascii_lowercase().contains("fixed") || s.to_ascii_lowercase().contains("bond") =>
            {
                AssetClass::FixedIncome
            }
            (_, ac) => ac,
        };
        rows.push(Constituent {
            ticker: clean(cols.ticker.and_then(|i| cells.get(i))),
            name,
            weight,
            sector,
            country: clean(cols.country.and_then(|i| cells.get(i))),
            asset_class,
            currency: clean(cols.currency.and_then(|i| cells.get(i))),
            exchange: clean(cols.exchange.and_then(|i| cells.get(i))),
        });
    }
    if rows.is_empty() {
        return Err(ParseError::Empty);
    }
    Ok(FundHoldings { symbol: symbol.to_string(), name, as_of, source: "file".into(), rows })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn splits_quoted_cells() {
        assert_eq!(
            split_line(r#""APPLE INC","Information Technology","1,234.56",7.01"#),
            vec!["APPLE INC", "Information Technology", "1,234.56", "7.01"]
        );
        assert_eq!(split_line(r#"a,"he said ""hi""",c"#), vec!["a", "he said \"hi\"", "c"]);
    }

    #[test]
    fn ishares_fund_of_funds_file() {
        let h = parse_csv("XEQT.TO", include_str!("../fixtures/ishares_xeqt.csv"), None).unwrap();
        assert_eq!(h.name.as_deref(), Some("iShares Core Equity ETF Portfolio"));
        assert_eq!(h.as_of, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        assert_eq!(h.rows.len(), 7); // 5 funds + 2 cash lines; the negative FX line is dropped
        let itot = h.rows.iter().find(|r| r.ticker.as_deref() == Some("ITOT")).unwrap();
        assert_eq!(itot.weight, dec!(25.01));
        assert_eq!(itot.country.as_deref(), Some("United States"));
        assert_eq!(itot.currency.as_deref(), Some("USD"));
        assert_eq!(itot.asset_class, AssetClass::Equity);
        let cash = h.rows.iter().find(|r| r.name.contains("CASH")).unwrap();
        assert_eq!(cash.asset_class, AssetClass::Cash);
        assert!((h.coverage() - dec!(100)).abs() < dec!(0.5), "{}", h.coverage());
    }

    #[test]
    fn ishares_us_latest_holdings_file() {
        // ishares.com/{product page}/latest-holdings.csv: no BOM, metadata lines with
        // Stock/Bond/Cash/Other percentages, Quantity instead of Shares, trailing
        // futures and money-market lines.
        let h = parse_csv("ITOT", include_str!("../fixtures/ishares_itot_us.csv"), None).unwrap();
        assert_eq!(h.name.as_deref(), Some("iShares Core S&P Total U.S. Stock Market ETF"));
        assert_eq!(h.as_of, NaiveDate::from_ymd_opt(2026, 10, 31).unwrap());
        let aapl = h.rows.iter().find(|r| r.ticker.as_deref() == Some("AAPL")).unwrap();
        assert_eq!(aapl.weight, dec!(6.85));
        assert_eq!(aapl.sector.as_deref(), Some("Information Technology"));
        assert_eq!(aapl.exchange.as_deref(), Some("NASDAQ"));
        assert!(h.rows.iter().all(|r| r.weight > dec!(0)), "zero-weight futures line is dropped");
        assert!(
            h.rows.iter().all(|r| !crate::is_fundish(r)),
            "no row of a plain equity fund is a child fund"
        );
        let mm = h.rows.iter().find(|r| r.ticker.as_deref() == Some("XTSLA")).unwrap();
        assert_eq!(mm.asset_class, AssetClass::Cash);
    }

    #[test]
    fn ishares_equity_file_has_sectors_and_locations() {
        let h = parse_csv("XIC.TO", include_str!("../fixtures/ishares_xic.csv"), None).unwrap();
        assert_eq!(h.as_of, NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        let rbc = h.rows.iter().find(|r| r.ticker.as_deref() == Some("RY")).unwrap();
        assert_eq!(rbc.sector.as_deref(), Some("Financials"));
        assert_eq!(rbc.country.as_deref(), Some("Canada"));
        assert_eq!(rbc.exchange.as_deref(), Some("Toronto Stock Exchange"));
        assert!(h.rows.len() >= 20);
    }

    #[test]
    fn vanguard_csv_export() {
        let h = parse_csv("VFV.TO", include_str!("../fixtures/vanguard_vfv.csv"), None).unwrap();
        assert_eq!(h.as_of, NaiveDate::from_ymd_opt(2026, 9, 30).unwrap());
        let aapl = h.rows.iter().find(|r| r.ticker.as_deref() == Some("AAPL")).unwrap();
        assert_eq!(aapl.sector.as_deref(), Some("Information Technology"));
        assert_eq!(aapl.weight, dec!(6.84));
        assert!(h.rows.iter().all(|r| r.asset_class == AssetClass::Equity));
    }

    #[test]
    fn needs_a_header_and_a_date() {
        assert_eq!(parse_csv("X", "just,some,text\n1,2,3\n", None), Err(ParseError::NoHeader(2)));
        let undated = "Name,Ticker,Weight (%)\nAPPLE INC,AAPL,7.0\n";
        assert!(matches!(parse_csv("X", undated, None), Err(ParseError::Format(_))));
        let d = NaiveDate::from_ymd_opt(2026, 1, 2).unwrap();
        assert_eq!(parse_csv("X", undated, Some(d)).unwrap().as_of, d);
    }
}
