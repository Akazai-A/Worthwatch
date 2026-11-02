//! iShares (BlackRock) Canada and US. Each site publishes a product screener JSON
//! listing every fund with its exchange ticker and product page, and every product
//! page serves the full holdings CSV at a fixed `.ajax` address. So locating a fund is
//! one screener download per site per refresh plus a map lookup; nothing is hard-coded
//! per fund. The screener and CSV addresses are the ones in public use as of 2026;
//! if BlackRock moves them the UI shows the HTTP error and the drop-in file still works.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::ProviderError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Site {
    Canada,
    UnitedStates,
}

/// Ledger symbol -> which site to ask and the bare ticker it lists. `XEQT.TO` -> Canada,
/// `XEQT`; `ITOT` -> US, `ITOT`.
pub fn site_for(symbol: &str) -> (Site, &str) {
    match symbol.rsplit_once('.') {
        Some((t, suffix)) if suffix.eq_ignore_ascii_case("TO") || suffix.eq_ignore_ascii_case("NE") => {
            (Site::Canada, t)
        }
        Some((t, _)) => (Site::Canada, t),
        None => (Site::UnitedStates, symbol),
    }
}

pub fn screener_url(site: Site) -> String {
    match site {
        Site::Canada => "https://www.blackrock.com/ca/investors/en/product-screener/product-screener-v3.jsn?dcrPath=/templatedata/config/product-screener-v3/data/en/ca-one/product-screener&siteEntryPassthrough=true".into(),
        Site::UnitedStates => "https://www.blackrock.com/us/individual/product-screener/product-screener-v3.jsn?dcrPath=/templatedata/config/product-screener-v3/data/en/us-one/product-screener&siteEntryPassthrough=true".into(),
    }
}

/// Product page path -> holdings CSV address.
pub fn holdings_url(site: Site, product_page: &str, ticker: &str) -> String {
    let ajax = match site {
        Site::Canada => "1464253357814",
        Site::UnitedStates => "1467271812596",
    };
    let page = if product_page.starts_with("http") {
        product_page.to_string()
    } else {
        format!("https://www.blackrock.com{product_page}")
    };
    format!("{page}/{ajax}.ajax?fileType=csv&fileName={ticker}_holdings&dataType=fund")
}

fn cell_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Object(o) => o.get("r").or_else(|| o.get("d")).and_then(cell_text),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

/// Pure: screener JSON -> exchange ticker -> product page path.
pub fn parse_screener(json: &str) -> Result<BTreeMap<String, String>, ProviderError> {
    let root: Value = serde_json::from_str(json)?;
    let table = root.pointer("/data/tableData").ok_or_else(|| ProviderError::Parse {
        provider: "ishares",
        what: "product screener".into(),
        reason: "no data.tableData".into(),
    })?;
    let cols: Vec<String> = table
        .pointer("/columns")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|c| c.get("name").and_then(Value::as_str).map(str::to_string)).collect())
        .unwrap_or_default();
    let ticker_i = cols.iter().position(|c| c == "localExchangeTicker");
    let url_i = cols.iter().position(|c| c == "productPageUrl");
    let (Some(ti), Some(ui)) = (ticker_i, url_i) else {
        return Err(ProviderError::Parse {
            provider: "ishares",
            what: "product screener".into(),
            reason: "columns localExchangeTicker/productPageUrl not found".into(),
        });
    };
    let mut out = BTreeMap::new();
    for row in table.pointer("/data").and_then(Value::as_array).into_iter().flatten() {
        let Some(cells) = row.as_array() else { continue };
        if let (Some(t), Some(u)) = (cells.get(ti).and_then(cell_text), cells.get(ui).and_then(cell_text)) {
            if !t.trim().is_empty() && !u.trim().is_empty() {
                out.insert(t.trim().to_uppercase(), u);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screener_maps_tickers_to_product_pages() {
        let m = parse_screener(include_str!("../../fixtures/ishares_screener.json")).unwrap();
        assert_eq!(m.len(), 5);
        assert_eq!(m["XEQT"], "/ca/investors/en/products/309480/ishares-core-equity-etf-portfolio");
        assert_eq!(
            holdings_url(Site::Canada, &m["XIC"], "XIC"),
            "https://www.blackrock.com/ca/investors/en/products/239837/ishares-sptsx-capped-composite-index-etf/1464253357814.ajax?fileType=csv&fileName=XIC_holdings&dataType=fund"
        );
        assert_eq!(site_for("XEQT.TO"), (Site::Canada, "XEQT"));
        assert_eq!(site_for("ITOT"), (Site::UnitedStates, "ITOT"));
    }
}
