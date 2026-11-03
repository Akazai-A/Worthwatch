//! Every parser that reads outside input (bank statements, Apple Pay taps, pasted broker
//! exports, fetched fund holdings, prices and FX) gets mutated real files and junk, and
//! must never panic: a panic in a route or a watcher thread takes a screen down.
use std::panic::{catch_unwind, AssertUnwindSafe};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

const BITS: &[&str] = &[
    "é",
    "ß",
    "ﬁ",
    "ı",
    "€",
    "日本",
    "\u{feff}",
    "\"",
    ",",
    ";",
    "\n",
    "\r\n",
    "\t",
    "<",
    ">",
    "</STMTTRN>",
    "<STMTTRN>",
    "<DTPOSTED>",
    "<TRNAMT>",
    "{",
    "}",
    "[",
    "]",
    ":",
    "-",
    "$",
    "(",
    ")",
    "1",
    "0",
    ".",
    "9999999999999999999999999999",
    "-1e400",
    "2026-11-03",
    "20261303",
    "SQ *",
    "null",
    "\"amount\"",
    "ﬀﬀﬀ",
    "İ",
    "ǅ",
    " ",
];

fn mutate(r: &mut Rng, base: &str) -> String {
    let mut s: Vec<char> = base.chars().collect();
    for _ in 0..(1 + r.below(12)) {
        let piece: Vec<char> = BITS[r.below(BITS.len())].chars().collect();
        let at = if s.is_empty() { 0 } else { r.below(s.len() + 1) };
        match r.below(3) {
            0 => {
                s.splice(at..at, piece);
            }
            1 if !s.is_empty() => {
                let end = (at + r.below(8)).min(s.len());
                s.drain(at.min(s.len())..end);
            }
            _ => {
                let end = (at + piece.len()).min(s.len());
                s.splice(at.min(s.len())..end, piece);
            }
        }
    }
    s.into_iter().collect()
}

fn fixture(p: &str) -> String {
    std::fs::read_to_string(format!("{}/../{p}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

#[test]
fn parsers_never_panic() {
    let bases = [
        "crates/budget/fixtures/amex.csv",
        "crates/budget/fixtures/bmo-mastercard.csv",
        "crates/budget/fixtures/cibc.csv",
        "crates/budget/fixtures/rbc-visa.csv",
        "crates/budget/fixtures/statement.ofx",
        "crates/budget/fixtures/tangerine.csv",
        "crates/budget/fixtures/td-chequing.csv",
        "crates/holdings/fixtures/ishares_xic.csv",
        "crates/holdings/fixtures/vanguard_veqt.json",
        "crates/holdings/fixtures/vanguard_vfv.csv",
        "crates/providers/fixtures/boc_valet.json",
        "crates/providers/fixtures/stooq_quotes.csv",
        "crates/providers/fixtures/yahoo_chart_xeqt.json",
    ]
    .map(fixture);
    let taps = r#"{"amount":"12.50","merchant":"Café","card":"Amex","date":"2026-11-03T10:00:00Z"}
{"amount":"4","merchant":"SQ *BLUE","card":"Visa","date":"2026-11-03"}"#
        .to_string();
    let now = chrono::Utc::now();
    let mut r = Rng(0x9e3779b97f4a7c15);
    let mut fails = vec![];
    for i in 0..4000 {
        let base = if i % 7 == 0 { &taps } else { &bases[i % bases.len()] };
        let t = if i % 11 == 0 { mutate(&mut r, "") } else { mutate(&mut r, base) };
        let t2 = t.clone();
        let res = catch_unwind(AssertUnwindSafe(|| {
            for p in budget::statement::Preset::CHOICES {
                let _ = budget::statement::parse_statement(&t, p, i % 2 == 0);
            }
            let _ = budget::tap::parse_taps(&t, now);
            let _ = budget::reconcile::tidy(&t);
            let _ = budget::categorize(&t, None, rust_decimal::Decimal::ONE, false, &[]);
            let _ = holdings::parse_any("XIC.TO", &t, None);
            let _ = importers::parse_holdings(&t);
            let _ = providers::stooq::parse_csv(&["XEQT.TO".into(), "VFV.TO".into()], &t);
            let _ = providers::yahoo::parse_chart("XEQT.TO", &t);
            let _ = providers::boc::parse_valet(&["USD".into()], &t);
            let _ = holdings::parse_date(&t);
            let _ = holdings::parse_number(&t);
        }));
        if res.is_err() {
            fails.push(t2);
            if fails.len() > 5 {
                break;
            }
        }
    }
    for f in &fails {
        eprintln!("PANIC INPUT ({} bytes): {:?}", f.len(), f.chars().take(300).collect::<String>());
    }
    assert!(fails.is_empty(), "{} panics", fails.len());
}
