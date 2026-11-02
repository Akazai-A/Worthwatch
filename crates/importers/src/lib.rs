//! Pasted broker holdings exports -> ledger mutations. Two pure steps:
//! [`parse_holdings`] reads whatever text the user pasted into rows, and
//! [`reconcile`] compares those rows with an account's positions and proposes
//! the trades, prices and book costs that make the ledger match.

use chrono::NaiveDate;
use ledger::{Account, Mutation, PositionChange};
use rust_decimal::Decimal;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ParsedRow {
    /// 1-based line in the pasted text.
    pub line: usize,
    pub symbol: String,
    pub units: Decimal,
    pub price: Option<Decimal>,
    /// Total cost of the position (avg cost x units when the export gives an average).
    pub book_cost: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Parsed {
    pub rows: Vec<ParsedRow>,
    /// Lines that did not look like a holding, with why, for the preview.
    pub skipped: Vec<(usize, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Col {
    Symbol,
    Units,
    Price,
    AvgCost,
    BookCost,
    Other,
}

fn classify_header(cell: &str) -> Col {
    let c = cell.trim().to_ascii_lowercase();
    let c = c.as_str();
    if c == "symbol" || c == "ticker" {
        Col::Symbol
    } else if matches!(c, "qty" | "quantity" | "units" | "shares" | "position") || c.starts_with("quantity") {
        Col::Units
    } else if matches!(
        c,
        "avg cost" | "average cost" | "avg price" | "average price" | "acb/unit" | "avg cost/share"
    ) {
        Col::AvgCost
    } else if matches!(c, "book cost" | "book value" | "cost basis" | "total cost" | "acb" | "cost") {
        Col::BookCost
    } else if matches!(c, "price" | "market price" | "last price" | "last" | "current price" | "quote") {
        Col::Price
    } else {
        Col::Other
    }
}

/// "$14,552.18" / "426.5 shares" / "−$190.00" / "(12.50)" -> a number, if the cell is one.
/// Returns the value and whether it was marked as money ($) or percent (%).
fn number(cell: &str) -> Option<(Decimal, bool, bool)> {
    let t = cell.trim();
    let money =
        t.contains('$') || t.contains("CAD") || t.contains("USD") || t.contains("C$") || t.contains("US$");
    let percent = t.contains('%');
    let mut s: String = t
        .to_ascii_lowercase()
        .replace("shares", "")
        .replace("share", "")
        .replace("units", "")
        .replace("unit", "")
        .replace("cad", "")
        .replace("usd", "")
        .replace(['$', ',', '%', ' ', '+'], "")
        .replace(['−', '–'], "-");
    if s.starts_with('(') && s.ends_with(')') {
        s = format!("-{}", &s[1..s.len() - 1]);
    }
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b'-') {
        return None;
    }
    s.parse::<Decimal>().ok().map(|d| (d, money, percent))
}

fn looks_like_symbol(cell: &str) -> bool {
    let t = cell.trim();
    let ok_len = (1..=12).contains(&t.len());
    ok_len
        && t.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
        && t.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'.' || b == b'-')
        && t.bytes().any(|b| b.is_ascii_uppercase())
        && !matches!(t, "CAD" | "USD" | "ETF" | "N/A" | "TOTAL" | "CASH")
}

fn split(line: &str) -> Vec<String> {
    if line.contains('\t') {
        return line.split('\t').map(|c| c.trim().to_string()).collect();
    }
    if line.contains(',') && !line.contains("  ") {
        // CSV with quoted cells ("75,361.00").
        let mut out = Vec::new();
        let mut cur = String::new();
        let mut quoted = false;
        for ch in line.chars() {
            match ch {
                '"' => quoted = !quoted,
                ',' if !quoted => out.push(std::mem::take(&mut cur).trim().to_string()),
                _ => cur.push(ch),
            }
        }
        out.push(cur.trim().to_string());
        return out;
    }
    line.split("  ").map(str::trim).filter(|c| !c.is_empty()).map(str::to_string).collect()
}

/// Pure: the pasted text into holdings rows. Handles a header row (Questrade-style CSV
/// or any export with "Symbol"/"Qty" columns) and header-less tables (Wealthsimple-style
/// copy from the web page) where the symbol is the first ticker-looking cell, units the
/// first bare number, and prices the money cells.
pub fn parse_holdings(text: &str) -> Parsed {
    let mut out = Parsed::default();
    let lines: Vec<(usize, &str)> = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim_end()))
        .filter(|(_, l)| !l.trim().is_empty())
        .collect();
    let Some((_, first)) = lines.first() else { return out };

    // Header-driven path.
    let header: Vec<Col> = split(first).iter().map(|c| classify_header(c)).collect();
    if header.contains(&Col::Symbol) && header.contains(&Col::Units) {
        let idx = |c: Col| header.iter().position(|h| *h == c);
        let (si, ui) = (idx(Col::Symbol).unwrap(), idx(Col::Units).unwrap());
        let (pi, ai, bi) = (idx(Col::Price), idx(Col::AvgCost), idx(Col::BookCost));
        for (n, line) in lines.iter().skip(1) {
            let cells = split(line);
            let sym = cells.get(si).map(|s| s.trim()).unwrap_or("");
            let units = cells.get(ui).and_then(|c| number(c)).map(|(d, _, _)| d);
            match (looks_like_symbol(sym), units) {
                (true, Some(units)) if !units.is_zero() => {
                    let price = pi
                        .and_then(|i| cells.get(i))
                        .and_then(|c| number(c))
                        .map(|(d, _, _)| d)
                        .filter(|d| *d > Decimal::ZERO);
                    let book = bi
                        .and_then(|i| cells.get(i))
                        .and_then(|c| number(c))
                        .map(|(d, _, _)| d)
                        .or_else(|| {
                            ai.and_then(|i| cells.get(i))
                                .and_then(|c| number(c))
                                .map(|(d, _, _)| (d * units).round_dp(2))
                        })
                        .filter(|d| *d > Decimal::ZERO);
                    out.rows.push(ParsedRow {
                        line: *n,
                        symbol: sym.to_string(),
                        units: units.abs(),
                        price,
                        book_cost: book,
                    });
                }
                (false, _) => out.skipped.push((*n, "no symbol".into())),
                _ => out.skipped.push((*n, "no quantity".into())),
            }
        }
        return out;
    }

    // Header-less path.
    for (n, line) in &lines {
        let cells = split(line);
        let Some(sym) = cells.iter().find(|c| looks_like_symbol(c)) else {
            out.skipped.push((*n, "no symbol".into()));
            continue;
        };
        let nums: Vec<(Decimal, bool, bool)> =
            cells.iter().filter(|c| c.as_str() != sym).filter_map(|c| number(c)).collect();
        let bare: Vec<Decimal> = nums.iter().filter(|(_, m, p)| !m && !p).map(|(d, _, _)| *d).collect();
        let money: Vec<Decimal> = nums.iter().filter(|(_, m, _)| *m).map(|(d, _, _)| *d).collect();
        let Some(units) = bare.first().copied().filter(|u| *u > Decimal::ZERO) else {
            out.skipped.push((*n, "no quantity".into()));
            continue;
        };
        // Money cells: price then value, unless the pair only makes sense the other way round.
        let price = match money.as_slice() {
            [] => None,
            [one] => Some(*one),
            [a, b, ..] => {
                let close = |x: Decimal, y: Decimal| {
                    (x - y).abs() <= (y.abs() * Decimal::new(1, 2)).max(Decimal::new(5, 2))
                };
                if close(units * *a, *b) {
                    Some(*a)
                } else if close(units * *b, *a) {
                    Some(*b)
                } else {
                    Some(*a)
                }
            }
        }
        .filter(|p| *p > Decimal::ZERO);
        out.rows.push(ParsedRow { line: *n, symbol: sym.clone(), units, price, book_cost: None });
    }
    out
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct PlanRow {
    pub line: usize,
    /// What was pasted, before resolution.
    pub pasted_symbol: String,
    /// The ledger symbol it resolved to.
    pub symbol: String,
    pub new: bool,
    pub current_units: Decimal,
    pub units: Decimal,
    pub delta: Decimal,
    pub price: Option<Decimal>,
    pub book_cost: Option<Decimal>,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct Plan {
    pub rows: Vec<PlanRow>,
    /// Positions in the account the paste did not mention; left as they are.
    pub untouched: Vec<String>,
    pub skipped: Vec<(usize, String)>,
    pub mutations: Vec<Mutation>,
}

/// Yahoo-style exchange suffix for bare symbols, by the account's currency.
pub fn default_suffix(currency: &str) -> &'static str {
    match currency {
        "CAD" => ".TO",
        _ => "",
    }
}

/// Resolve a pasted symbol to the account's spelling: exact match, then same root
/// (`XEQT` = `XEQT.TO`), then the currency's default suffix for a bare ticker.
pub fn resolve_symbol(pasted: &str, existing: &[String], currency: &str) -> String {
    let p = pasted.trim().to_ascii_uppercase();
    if existing.contains(&p) {
        return p;
    }
    let root = |s: &str| s.split('.').next().unwrap_or(s).to_string();
    if let Some(e) = existing.iter().find(|e| root(e) == root(&p)) {
        return e.clone();
    }
    if p.contains('.') {
        p
    } else {
        format!("{p}{}", default_suffix(currency))
    }
}

/// Pure: compare parsed rows with the account's positions and build the mutations.
/// Differences become trades dated `date`; every price becomes a manual price for
/// that date; a book cost that changed is recorded.
pub fn reconcile(account: &Account, parsed: &Parsed, date: NaiveDate) -> Plan {
    let positions = account.positions.as_deref().unwrap_or(&[]);
    let existing: Vec<String> = positions.iter().map(|p| p.symbol.clone()).collect();
    let mut plan = Plan { skipped: parsed.skipped.clone(), ..Plan::default() };
    let mut seen: Vec<String> = Vec::new();
    for r in &parsed.rows {
        let symbol = resolve_symbol(&r.symbol, &existing, &account.currency);
        if seen.contains(&symbol) {
            plan.skipped.push((r.line, format!("{symbol} listed twice")));
            continue;
        }
        seen.push(symbol.clone());
        let current = positions.iter().find(|p| p.symbol == symbol);
        let current_units = current.map_or(Decimal::ZERO, |p| p.units);
        let delta = r.units - current_units;
        if !delta.is_zero() {
            plan.mutations.push(Mutation::RecordPositionChange {
                change: PositionChange {
                    date,
                    account: account.id.clone(),
                    symbol: symbol.clone(),
                    units: delta,
                    price: r.price,
                    note: Some("from pasted holdings".into()),
                },
            });
        }
        if let Some(px) = r.price {
            plan.mutations.push(Mutation::SetPrice { symbol: symbol.clone(), date, price: px });
        }
        if let Some(b) = r.book_cost {
            if current.and_then(|p| p.book_cost) != Some(b) {
                plan.mutations.push(Mutation::SetBookCost {
                    account: account.id.clone(),
                    symbol: symbol.clone(),
                    book_cost: Some(b),
                });
            }
        }
        plan.rows.push(PlanRow {
            line: r.line,
            pasted_symbol: r.symbol.clone(),
            symbol,
            new: current.is_none(),
            current_units,
            units: r.units,
            delta,
            price: r.price,
            book_cost: r.book_cost,
        });
    }
    plan.untouched = existing.into_iter().filter(|e| !seen.contains(e)).collect();
    plan
}

#[cfg(test)]
mod tests {
    use super::*;
    use ledger::{AccountKind, Ledger, Position};
    use rust_decimal_macros::dec;

    fn tfsa() -> Account {
        Account {
            id: "ws-tfsa".into(),
            name: "Wealthsimple TFSA".into(),
            institution: None,
            kind: AccountKind::Tfsa,
            currency: "CAD".into(),
            tags: vec![],
            positions: Some(vec![
                Position { symbol: "XEQT.TO".into(), units: dec!(426.5), book_cost: Some(dec!(11240)) },
                Position { symbol: "VFV.TO".into(), units: dec!(600), book_cost: None },
                Position { symbol: "ZSP.TO".into(), units: dec!(10), book_cost: None },
            ]),
        }
    }

    #[test]
    fn wealthsimple_style_paste_without_header() {
        let p = parse_holdings(include_str!("../fixtures/wealthsimple_paste.txt"));
        assert_eq!(p.rows.len(), 3, "{p:?}");
        assert_eq!(
            p.rows[0],
            ParsedRow {
                line: 2,
                symbol: "XEQT".into(),
                units: dec!(426.5),
                price: Some(dec!(34.12)),
                book_cost: None
            }
        );
        assert_eq!(p.rows[1].units, dec!(620));
        assert_eq!(p.rows[1].price, Some(dec!(128.40)));
        assert_eq!(p.rows[2].symbol, "ZAG");
        // "Holdings" title and the Cash row are skipped, with reasons.
        assert_eq!(p.skipped.len(), 2);
    }

    #[test]
    fn questrade_style_csv_with_header_and_avg_cost() {
        let p = parse_holdings(include_str!("../fixtures/questrade_positions.csv"));
        assert_eq!(p.rows.len(), 2, "{p:?}");
        assert_eq!(p.rows[0].symbol, "VTI");
        assert_eq!(p.rows[0].units, dec!(310));
        assert_eq!(p.rows[0].price, Some(dec!(243.10)));
        assert_eq!(p.rows[0].book_cost, Some(dec!(62000.00)));
        assert_eq!(p.rows[1].symbol, "XEQT.TO");
        assert_eq!(p.rows[1].book_cost, Some(dec!(2980.00)));
    }

    #[test]
    fn money_pair_in_the_other_order_is_still_read_as_value_then_price() {
        let p = parse_holdings("XEQT  $14,552.18  426.5  $34.12\n");
        assert_eq!(p.rows[0].price, Some(dec!(34.12)));
        assert_eq!(p.rows[0].units, dec!(426.5));
    }

    #[test]
    fn garbage_is_skipped_not_guessed() {
        let p = parse_holdings("Total value $94,160.00\n\nhello world\nVFV 0 shares\n");
        assert!(p.rows.is_empty());
        assert_eq!(p.skipped.len(), 3);
    }

    #[test]
    fn symbols_resolve_to_the_accounts_spelling() {
        let ex = vec!["XEQT.TO".to_string(), "VTI".to_string()];
        assert_eq!(resolve_symbol("xeqt", &ex, "CAD"), "XEQT.TO");
        assert_eq!(resolve_symbol("XEQT.TO", &ex, "CAD"), "XEQT.TO");
        assert_eq!(resolve_symbol("VTI", &ex, "USD"), "VTI");
        assert_eq!(resolve_symbol("ZAG", &ex, "CAD"), "ZAG.TO");
        assert_eq!(resolve_symbol("VOO", &ex, "USD"), "VOO");
        assert_eq!(resolve_symbol("VOD.L", &ex, "CAD"), "VOD.L");
    }

    #[test]
    fn reconcile_builds_trades_prices_and_book_costs_and_applies_cleanly() {
        let a = tfsa();
        let parsed = parse_holdings(include_str!("../fixtures/wealthsimple_paste.txt"));
        let date = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let plan = reconcile(&a, &parsed, date);
        assert_eq!(plan.rows.len(), 3);
        let x = &plan.rows[0];
        assert_eq!((x.symbol.as_str(), x.delta, x.new), ("XEQT.TO", dec!(0), false));
        let v = &plan.rows[1];
        assert_eq!((v.symbol.as_str(), v.current_units, v.delta), ("VFV.TO", dec!(600), dec!(20)));
        let z = &plan.rows[2];
        assert_eq!((z.symbol.as_str(), z.delta, z.new), ("ZAG.TO", dec!(200), true));
        assert_eq!(plan.untouched, vec!["ZSP.TO".to_string()]);
        // 2 trades + 3 prices, no book costs (the paste has none).
        assert_eq!(
            plan.mutations.iter().filter(|m| matches!(m, Mutation::RecordPositionChange { .. })).count(),
            2
        );
        assert_eq!(plan.mutations.iter().filter(|m| matches!(m, Mutation::SetPrice { .. })).count(), 3);
        assert!(!plan.mutations.iter().any(|m| matches!(m, Mutation::SetBookCost { .. })));

        let l = Ledger::starter("CAD").apply(Mutation::UpsertAccount { account: a }).unwrap();
        let l = l.apply_many(plan.mutations).unwrap();
        let ps = l.account("ws-tfsa").unwrap().positions.as_ref().unwrap();
        assert_eq!(ps.iter().find(|p| p.symbol == "VFV.TO").unwrap().units, dec!(620));
        assert_eq!(ps.iter().find(|p| p.symbol == "ZAG.TO").unwrap().units, dec!(200));
        assert_eq!(ps.iter().find(|p| p.symbol == "ZSP.TO").unwrap().units, dec!(10));
        assert_eq!(l.prices["ZAG.TO"][&date], dec!(14.05));
    }

    #[test]
    fn reconcile_records_a_changed_book_cost() {
        let a = tfsa();
        let parsed = parse_holdings("Symbol,Qty,Avg Cost\nXEQT.TO,426.5,30.00\n");
        let plan = reconcile(&a, &parsed, NaiveDate::from_ymd_opt(2026, 11, 2).unwrap());
        assert_eq!(plan.rows[0].book_cost, Some(dec!(12795.00)));
        assert!(plan
            .mutations
            .iter()
            .any(|m| matches!(m, Mutation::SetBookCost { book_cost: Some(b), .. } if *b == dec!(12795.00))));
        assert_eq!(plan.mutations.len(), 1);
    }
}
