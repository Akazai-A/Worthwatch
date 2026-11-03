//! Bank statements: CSV exports from Canadian banks, or OFX/QFX. A statement is the
//! record; each row becomes a [`CsvRow`] with an id stable across re-imports, so
//! importing the same file twice (or two overlapping months) adds nothing new.
//!
//! Layouts differ by bank. Headed files are read by column name; the headerless ones
//! (TD, CIBC, older Scotiabank) by shape. Which sign means "money out" is guessed from
//! the rows (most rows on any statement are spending) and can be flipped in the preview.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::tap::normalize_number;

/// A bank's export layout. `Auto` looks at the file and picks one.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    #[default]
    Auto,
    Td,
    Rbc,
    Cibc,
    Scotiabank,
    Bmo,
    Tangerine,
    Amex,
    Simplii,
    Eq,
    Generic,
    Ofx,
}

impl Preset {
    pub const CHOICES: [Preset; 11] = [
        Preset::Auto,
        Preset::Td,
        Preset::Rbc,
        Preset::Cibc,
        Preset::Scotiabank,
        Preset::Bmo,
        Preset::Tangerine,
        Preset::Amex,
        Preset::Simplii,
        Preset::Eq,
        Preset::Ofx,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Preset::Auto => "Detect",
            Preset::Td => "TD",
            Preset::Rbc => "RBC",
            Preset::Cibc => "CIBC",
            Preset::Scotiabank => "Scotiabank",
            Preset::Bmo => "BMO",
            Preset::Tangerine => "Tangerine",
            Preset::Amex => "American Express",
            Preset::Simplii => "Simplii",
            Preset::Eq => "EQ Bank",
            Preset::Generic => "CSV with headers",
            Preset::Ofx => "OFX / QFX",
        }
    }
}

/// One line of a statement, money out positive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatementRow {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    /// OFX's own transaction id, when the file has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fitid: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ParsedStatement {
    /// The layout that was used (never `Auto`).
    pub preset: Preset,
    pub rows: Vec<StatementRow>,
    /// (1-based line, why) for lines that looked like data but could not be read.
    pub skipped: Vec<(usize, String)>,
    /// True when the file's amounts were negated to make money out positive.
    pub flipped: bool,
    /// The sign came from the data, not the layout; the preview offers to flip it.
    pub sign_guessed: bool,
}

/// A statement row as stored, tied to an account.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CsvRow {
    pub id: String,
    pub account: String,
    pub date: NaiveDate,
    /// Positive out, negative in, in the account's currency.
    pub amount: Decimal,
    pub description: String,
    pub imported: DateTime<Utc>,
}

/// Parse a statement. `flip` negates every amount after the sign is decided.
pub fn parse_statement(text: &str, preset: Preset, flip: bool) -> ParsedStatement {
    let text = text.trim_start_matches('\u{feff}');
    let is_ofx = preset == Preset::Ofx
        || (preset == Preset::Auto && {
            let head = text.get(..text.len().min(2000)).unwrap_or(text).to_uppercase();
            head.contains("OFXHEADER") || head.contains("<OFX>")
        });
    let mut p = if is_ofx { parse_ofx(text) } else { parse_csv(text, preset) };
    if flip {
        for r in &mut p.rows {
            r.amount = -r.amount;
        }
        p.flipped = !p.flipped;
    }
    p
}

/// Give rows ids and an account. Identical rows on one day (two $4.60 transit fares)
/// stay distinct by their order in the file, which is the same in every export.
pub fn rows_for_account(account: &str, rows: &[StatementRow], imported: DateTime<Utc>) -> Vec<CsvRow> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    rows.iter()
        .map(|r| {
            let id = match &r.fitid {
                Some(f) => format!("ofx-{}", crate::fnv(&[account, f])),
                None => {
                    let key = format!("{}|{}|{}", r.date, r.amount.normalize(), squash(&r.description));
                    let n = seen.entry(key.clone()).or_default();
                    *n += 1;
                    format!("csv-{}", crate::fnv(&[account, &key, &n.to_string()]))
                }
            };
            CsvRow {
                id,
                account: account.to_string(),
                date: r.date,
                amount: r.amount,
                description: r.description.clone(),
                imported,
            }
        })
        .collect()
}

fn squash(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_uppercase()
}

// ---------------------------------------------------------------- CSV

fn split_line(line: &str) -> Vec<String> {
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

#[derive(Default, Debug)]
struct Columns {
    date: Option<usize>,
    description: Vec<usize>,
    amount: Option<usize>,
    debit: Option<usize>,
    credit: Option<usize>,
}

fn find(header: &[String], names: &[&str]) -> Option<usize> {
    names.iter().find_map(|n| header.iter().position(|h| h == n))
}

fn columns_from_header(header: &[String]) -> Columns {
    let h: Vec<String> = header.iter().map(|c| c.trim().trim_matches('"').to_lowercase()).collect();
    let mut description = Vec::new();
    if let Some(i) = find(
        &h,
        &[
            "description",
            "description 1",
            "transaction details",
            "merchant",
            "payee",
            "name",
            "details",
            "merchant name",
        ],
    ) {
        description.push(i);
    }
    for extra in ["description 2", "sub-description", "memo"] {
        if let Some(i) = find(&h, &[extra]) {
            if !description.contains(&i) {
                description.push(i);
            }
        }
    }
    Columns {
        date: find(
            &h,
            &[
                "transaction date",
                "date",
                "trans. date",
                "trans date",
                "date posted",
                "posted date",
                "posting date",
                "date processed",
            ],
        ),
        description,
        amount: find(&h, &["amount", "transaction amount", "cad$", "amount (cad)", "amount ($)", "montant"]),
        debit: find(
            &h,
            &[
                "debit",
                "withdrawal",
                "withdrawals",
                "funds out",
                "money out",
                "debit amount",
                "withdrawals ($)",
            ],
        ),
        credit: find(
            &h,
            &["credit", "deposit", "deposits", "funds in", "money in", "credit amount", "deposits ($)"],
        ),
    }
}

fn preset_from_header(h: &[String]) -> Preset {
    let has = |n: &str| h.iter().any(|c| c.trim().eq_ignore_ascii_case(n));
    if has("account type") && has("description 1") {
        Preset::Rbc
    } else if has("transaction") && has("name") && has("memo") {
        Preset::Tangerine
    } else if has("date processed") || has("cardmember") {
        Preset::Amex
    } else if has("transaction amount") && (has("date posted") || has("posting date")) {
        Preset::Bmo
    } else if has("sub-description") || has("type of transaction") {
        Preset::Scotiabank
    } else if has("funds out") || has("funds in") {
        Preset::Simplii
    } else {
        Preset::Generic
    }
}

fn looks_like_header(cells: &[String]) -> bool {
    let c = columns_from_header(cells);
    c.date.is_some() && (c.amount.is_some() || c.debit.is_some() || c.credit.is_some())
}

fn parse_csv(text: &str, preset: Preset) -> ParsedStatement {
    let lines: Vec<(usize, Vec<String>)> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| (i + 1, split_line(l)))
        .collect();
    // A header may sit below a few lines of account details (BMO, Scotiabank).
    let header_at = lines.iter().take(20).position(|(_, cells)| looks_like_header(cells));
    let date_strings: Vec<&str> = match header_at {
        Some(h) => {
            let col = columns_from_header(&lines[h].1).date;
            lines[h + 1..]
                .iter()
                .filter_map(|(_, c)| col.and_then(|i| c.get(i)).map(String::as_str))
                .collect()
        }
        None => lines.iter().filter_map(|(_, c)| c.first().map(String::as_str)).collect(),
    };
    let order = slash_order(&date_strings);

    let (detected, cols, body) = match header_at {
        Some(h) => {
            let header = &lines[h].1;
            let detected = if preset == Preset::Auto { preset_from_header(header) } else { preset };
            (detected, columns_from_header(header), &lines[h + 1..])
        }
        None => {
            // Headerless: TD (MM/DD/YYYY, description, debit, credit, balance), CIBC
            // (YYYY-MM-DD, description, debit, credit[, card]), older Scotiabank
            // (date, signed amount, "-", type, description).
            let first = lines.iter().find(|(_, c)| c.first().and_then(|d| parse_date(d, order)).is_some());
            let scotia = first.is_some_and(|(_, c)| c.get(2).is_some_and(|x| x == "-"));
            let iso = first.is_some_and(|(_, c)| c[0].len() == 10 && c[0].as_bytes()[4] == b'-');
            let detected = match preset {
                Preset::Auto if scotia => Preset::Scotiabank,
                Preset::Auto if iso => Preset::Cibc,
                Preset::Auto => Preset::Td,
                p => p,
            };
            let cols = if detected == Preset::Scotiabank {
                Columns { date: Some(0), description: vec![4, 3], amount: Some(1), ..Default::default() }
            } else {
                Columns {
                    date: Some(0),
                    description: vec![1],
                    debit: Some(2),
                    credit: Some(3),
                    ..Default::default()
                }
            };
            (detected, cols, &lines[..])
        }
    };

    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    for (line_no, cells) in body {
        let Some(date_cell) = cols.date.and_then(|i| cells.get(i)) else { continue };
        let Some(date) = parse_date(date_cell, order) else {
            // A footer or a second header; only complain when it looks like data.
            if cells.len() > 2 && cells.iter().any(|c| c.chars().any(|ch| ch.is_ascii_digit())) {
                skipped.push((*line_no, format!("can't read the date `{date_cell}`")));
            }
            continue;
        };
        let description = cols
            .description
            .iter()
            .filter_map(|i| cells.get(*i))
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        // Money out positive: a debit column is out, a credit column is in. A single
        // signed column is read as written for now; the sign is settled below.
        let amount = match (cols.amount, cols.debit, cols.credit) {
            (Some(i), _, _) => cells.get(i).and_then(|s| money(s)),
            (None, d, c) => {
                let out = d.and_then(|i| cells.get(i)).and_then(|s| money(s));
                let inn = c.and_then(|i| cells.get(i)).and_then(|s| money(s));
                match (out, inn) {
                    (None, None) => None,
                    (o, i) => Some(-(o.unwrap_or_default().abs() - i.unwrap_or_default().abs())),
                }
            }
        };
        let Some(amount) = amount else {
            skipped.push((*line_no, "no amount".into()));
            continue;
        };
        rows.push(StatementRow { date, amount, description, fitid: None });
    }
    // Rows now hold the file's own sign. Debit/credit files were built "in positive",
    // so they always flip. A signed column flips when most rows are negative: most
    // lines on any statement, bank or card, are spending.
    let signed = cols.amount.is_some();
    let flip = if signed {
        let neg = rows.iter().filter(|r| r.amount.is_sign_negative()).count();
        neg * 2 > rows.len()
    } else {
        true
    };
    if flip {
        for r in &mut rows {
            r.amount = -r.amount;
        }
    }
    ParsedStatement { preset: detected, rows, skipped, flipped: flip && signed, sign_guessed: signed }
}

fn money(s: &str) -> Option<Decimal> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let neg = t.starts_with('-') || t.ends_with('-') || (t.starts_with('(') && t.ends_with(')'));
    let digits: String = t.chars().filter(|c| c.is_ascii_digit() || *c == '.' || *c == ',').collect();
    let d: Decimal = normalize_number(&digits)?.parse().ok()?;
    Some(if neg { -d } else { d })
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SlashOrder {
    MonthFirst,
    DayFirst,
}

/// North American exports write MM/DD/YYYY; a file with a first part over 12 is DD/MM.
fn slash_order(dates: &[&str]) -> SlashOrder {
    let day_first = dates.iter().any(|d| {
        let mut parts = d.trim().split('/');
        parts.next().and_then(|p| p.parse::<u32>().ok()).is_some_and(|n| n > 12)
    });
    if day_first {
        SlashOrder::DayFirst
    } else {
        SlashOrder::MonthFirst
    }
}

fn parse_date(s: &str, order: SlashOrder) -> Option<NaiveDate> {
    let s = s.trim().trim_matches('"').replace('.', "");
    if s.len() == 8 && s.chars().all(|c| c.is_ascii_digit()) {
        return NaiveDate::parse_from_str(&s, "%Y%m%d").ok();
    }
    if let Ok(d) = NaiveDate::parse_from_str(&s, "%Y-%m-%d") {
        return Some(d);
    }
    if s.contains('/') {
        let f = match order {
            SlashOrder::MonthFirst => ["%m/%d/%Y", "%m/%d/%y"],
            SlashOrder::DayFirst => ["%d/%m/%Y", "%d/%m/%y"],
        };
        return f
            .iter()
            .find_map(|f| NaiveDate::parse_from_str(&s, f).ok())
            .or_else(|| NaiveDate::parse_from_str(&s, "%Y/%m/%d").ok());
    }
    // "03 Nov 2026" (Amex), "Nov 3, 2026", "3-Nov-2026", "2026-Nov-03".
    for f in
        ["%d %b %Y", "%b %d, %Y", "%b %d %Y", "%d-%b-%Y", "%d-%b-%y", "%Y-%b-%d", "%d %B %Y", "%B %d, %Y"]
    {
        if let Ok(d) = NaiveDate::parse_from_str(&s, f) {
            return Some(d);
        }
    }
    None
}

// ---------------------------------------------------------------- OFX

fn ofx_field(block: &str, tag: &str) -> Option<String> {
    let up = block.to_ascii_uppercase();
    let open = format!("<{tag}>");
    let start = up.find(&open)? + open.len();
    let rest = &block[start..];
    let end = rest.find(['<', '\n', '\r']).unwrap_or(rest.len());
    let v = rest[..end].trim();
    (!v.is_empty()).then(|| v.to_string())
}

fn parse_ofx(text: &str) -> ParsedStatement {
    let up = text.to_ascii_uppercase();
    let mut rows = Vec::new();
    let mut skipped = Vec::new();
    let mut from = 0;
    while let Some(i) = up[from..].find("<STMTTRN>") {
        let start = from + i;
        let end = up[start..].find("</STMTTRN>").map_or(up.len(), |e| start + e);
        let block = &text[start..end];
        from = end.max(start + 1);
        let line_no = text[..start].lines().count() + 1;
        let date = ofx_field(block, "DTPOSTED")
            .and_then(|d| d.get(..8).and_then(|d| NaiveDate::parse_from_str(d, "%Y%m%d").ok()));
        let amount = ofx_field(block, "TRNAMT").and_then(|a| money(&a));
        let (Some(date), Some(amount)) = (date, amount) else {
            skipped.push((line_no, "transaction without a date or amount".into()));
            continue;
        };
        let name = ofx_field(block, "NAME").unwrap_or_default();
        let memo = ofx_field(block, "MEMO").unwrap_or_default();
        let description =
            if memo.is_empty() || name.contains(&memo) { name } else { format!("{name} {memo}") };
        // OFX: negative is money out, for bank and card accounts alike.
        rows.push(StatementRow {
            date,
            amount: -amount,
            description: description.trim().to_string(),
            fitid: ofx_field(block, "FITID"),
        });
    }
    ParsedStatement { preset: Preset::Ofx, rows, skipped, flipped: false, sign_guessed: false }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn ymd(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn ofx_with_letters_that_change_length_in_uppercase_does_not_panic() {
        // "ﬁ" uppercases to "FI" and "ı" to "I": offsets taken from a Unicode-uppercased
        // copy used to land inside a character of the original.
        let text = fixture("statement.ofx").replace("BLUE BOTTLE", "ﬁne café ı BOTTLE");
        let p = parse_statement(&text, Preset::Auto, false);
        assert!(!p.rows.is_empty());
        let _ = parse_statement("<STMTTRN>ﬁﬁﬁ<DTPOSTED>2026ı</STMTTRN>", Preset::Auto, false);
    }

    #[test]
    fn td_headerless_debit_credit() {
        let p = parse_statement(&fixture("td-chequing.csv"), Preset::Auto, false);
        assert_eq!(p.preset, Preset::Td);
        assert_eq!(p.rows.len(), 5);
        assert_eq!(p.rows[0].date, ymd(2026, 11, 2));
        assert_eq!(p.rows[0].amount, dec!(1204.55), "a debit is money out");
        assert!(p.rows.iter().any(|r| r.amount == dec!(-2850.00)), "payroll is money in");
        assert!(!p.sign_guessed);
    }

    #[test]
    fn cibc_headerless_iso_dates() {
        let p = parse_statement(&fixture("cibc.csv"), Preset::Auto, false);
        assert_eq!(p.preset, Preset::Cibc);
        assert_eq!(p.rows.len(), 3);
        assert_eq!(p.rows[0].amount, dec!(23.45));
        assert_eq!(p.rows[2].amount, dec!(-500.00));
    }

    #[test]
    fn rbc_signed_column_with_two_descriptions() {
        let p = parse_statement(&fixture("rbc-visa.csv"), Preset::Auto, false);
        assert_eq!(p.preset, Preset::Rbc);
        assert_eq!(p.rows.len(), 4);
        assert_eq!(p.rows[0].description, "UBER CANADA/UBEREATS TORONTO ON");
        // RBC writes purchases negative; most rows are purchases, so it flips.
        assert!(p.flipped && p.sign_guessed);
        assert_eq!(p.rows[0].amount, dec!(31.20));
        assert_eq!(p.rows[3].amount, dec!(-640.12), "the payment is money in");
        // The preview's "flip" undoes the guess.
        let q = parse_statement(&fixture("rbc-visa.csv"), Preset::Auto, true);
        assert_eq!(q.rows[0].amount, dec!(-31.20));
        assert!(!q.flipped);
    }

    #[test]
    fn amex_positive_charges_and_written_dates() {
        let p = parse_statement(&fixture("amex.csv"), Preset::Auto, false);
        assert_eq!(p.preset, Preset::Amex);
        assert_eq!(p.rows.len(), 4);
        assert_eq!(p.rows[0].date, ymd(2026, 11, 4));
        assert_eq!(p.rows[0].amount, dec!(56.40));
        assert!(!p.flipped);
    }

    #[test]
    fn tangerine_and_bmo_headers() {
        let t = parse_statement(&fixture("tangerine.csv"), Preset::Auto, false);
        assert_eq!(t.preset, Preset::Tangerine);
        assert_eq!(t.rows.len(), 3);
        assert_eq!(t.rows[0].description, "LOBLAWS #1012 Groceries");
        assert_eq!(t.rows[0].amount, dec!(84.10));
        let b = parse_statement(&fixture("bmo-mastercard.csv"), Preset::Auto, false);
        assert_eq!(b.preset, Preset::Bmo);
        assert_eq!(b.rows.len(), 3, "{:?}", b.skipped);
        assert_eq!(b.rows[0].date, ymd(2026, 11, 3));
    }

    #[test]
    fn ofx_uses_fitid_and_negative_is_out() {
        let p = parse_statement(&fixture("statement.ofx"), Preset::Auto, false);
        assert_eq!(p.preset, Preset::Ofx);
        assert_eq!(p.rows.len(), 3);
        assert_eq!(p.rows[0].amount, dec!(7.25));
        assert_eq!(p.rows[0].fitid.as_deref(), Some("202611040001"));
        assert_eq!(p.rows[2].amount, dec!(-1204.55));
    }

    #[test]
    fn ids_are_stable_and_identical_rows_stay_distinct() {
        let p = parse_statement(&fixture("td-chequing.csv"), Preset::Auto, false);
        let now = Utc::now();
        let a = rows_for_account("td-chq", &p.rows, now);
        let b = rows_for_account("td-chq", &p.rows, now + chrono::Duration::days(3));
        assert_eq!(a.iter().map(|r| &r.id).collect::<Vec<_>>(), b.iter().map(|r| &r.id).collect::<Vec<_>>());
        // Two $4.60 fares on Nov 3 are two rows.
        let fares: Vec<_> = a.iter().filter(|r| r.amount == dec!(4.60)).collect();
        assert_eq!(fares.len(), 2);
        assert_ne!(fares[0].id, fares[1].id);
        // Another account, other ids.
        assert_ne!(rows_for_account("visa", &p.rows, now)[0].id, a[0].id);
    }

    #[test]
    fn day_first_dates_are_detected_from_the_file() {
        let p = parse_statement(
            "Date,Description,Amount\n25/10/2026,COFFEE,-4.00\n02/11/2026,BOOKS,-20.00\n",
            Preset::Auto,
            false,
        );
        assert_eq!(p.rows[0].date, ymd(2026, 10, 25));
        assert_eq!(p.rows[1].date, ymd(2026, 11, 2));
    }
}
