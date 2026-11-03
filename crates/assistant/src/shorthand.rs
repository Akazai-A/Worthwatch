//! The no-model path: "td chq 5102, visa down to 640, sold 20 XEQT in the TFSA".
//! Clauses split on commas, semicolons, newlines and "and"; each clause is a trade,
//! a relative move, an absolute amount, a price, or a question. Anything else goes
//! to `unparsed` so the user sees it was not understood rather than silently dropped.

use regex::Regex;
use rust_decimal::Decimal;
use std::sync::OnceLock;

use crate::{names, Context, Item, Proposal};

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid regex"))
}

static THOUSANDS: OnceLock<Regex> = OnceLock::new();
static TRADE: OnceLock<Regex> = OnceLock::new();
static AT_PRICE: OnceLock<Regex> = OnceLock::new();
static IN_ACCOUNT: OnceLock<Regex> = OnceLock::new();
static TO_VALUE: OnceLock<Regex> = OnceLock::new();
static RELATIVE: OnceLock<Regex> = OnceLock::new();
static ABSOLUTE: OnceLock<Regex> = OnceLock::new();
static QUESTION: OnceLock<Regex> = OnceLock::new();
static NOTE: OnceLock<Regex> = OnceLock::new();

const AMOUNT: &str = r"(-?)\$?(\d+(?:\.\d+)?)\s*([km])?";

/// "5,102.10" -> 5102.10, "18.5k" -> 18500, "-640" -> -640.
pub fn amount(neg: &str, num: &str, suffix: &str) -> Option<Decimal> {
    let mut d: Decimal = num.parse().ok()?;
    match suffix {
        "k" => d *= Decimal::ONE_THOUSAND,
        "m" => d *= Decimal::from(1_000_000),
        _ => {}
    }
    Some(if neg.is_empty() { d } else { -d })
}

fn clauses(text: &str) -> Vec<String> {
    let t = re(&THOUSANDS, r"(\d),(\d{3})\b").replace_all(text, "$1$2").to_string();
    let t = re(&THOUSANDS, r"(\d),(\d{3})\b").replace_all(&t, "$1$2").to_string();
    t.split([',', ';', '\n'])
        .flat_map(|c| c.split(" and ").map(str::to_string).collect::<Vec<_>>())
        .map(|c| c.trim().trim_end_matches('.').trim().to_string())
        .filter(|c| !c.is_empty())
        .collect()
}

fn strip_article(s: &str) -> &str {
    let s = s.trim();
    ["the ", "my ", "our "].iter().find_map(|a| s.strip_prefix(a)).unwrap_or(s).trim()
}

/// Pure: the message into a proposal, against the accounts in the context.
pub fn parse(text: &str, ctx: &Context) -> Proposal {
    let mut p = Proposal::default();
    let lower = text.to_ascii_lowercase();
    if re(&QUESTION, r"^\s*(how|what|which|when|where|why|is|are|do|does|did|can|show|tell)\b")
        .is_match(&lower)
        || lower.trim().ends_with('?')
    {
        p.query = Some(lower.trim().trim_end_matches('?').to_string());
        return p;
    }
    for clause in clauses(text) {
        let c = clause.to_ascii_lowercase();
        let source = Some(clause.clone());
        if let Some(m) = re(&NOTE, r"^(?:note|remember|event):?\s+(.+)$").captures(&c) {
            p.items.push(Item::Note { note: clause[m.get(1).unwrap().range()].to_string(), source });
            continue;
        }
        if let Some(m) = re(
            &TRADE,
            r"^(sold|sell|bought|buy|added|add|dropped)\s+(\d+(?:\.\d+)?)\s*(?:units?|shares?)?\s*(?:of\s+)?([a-z][a-z0-9.\-]*)\s*(.*)$",
        )
        .captures(&c)
        {
            let verb = &m[1];
            let n: Decimal = m[2].parse().unwrap_or_default();
            let symbol = m[3].to_ascii_uppercase();
            let mut rest = m[4].to_string();
            let mut price = None;
            if let Some(pm) = re(&AT_PRICE, r"(?:@|\bat)\s*\$?(\d+(?:\.\d+)?)").captures(&rest) {
                price = pm[1].parse().ok();
                let r = pm.get(0).unwrap().range();
                rest.replace_range(r, " ");
            }
            let account = re(&IN_ACCOUNT, r"(?:^|\s)(?:in|into|to|from|out of)\s+(.+)$")
                .captures(rest.trim())
                .map(|am| strip_article(&am[1]).to_string())
                .filter(|a| !a.is_empty());
            let units = if matches!(verb, "sold" | "sell" | "dropped") { -n } else { n };
            p.items.push(Item::Trade { account, symbol, units, price, source });
            continue;
        }
        let abs = |p: &mut Proposal, who: &str, value: Decimal, source: Option<String>| {
            let who = strip_article(who);
            if let (Some(sym), holders) = names::accounts_holding(&ctx.accounts, who) {
                if !holders.is_empty() && names::match_account(&ctx.accounts, who).is_err() {
                    p.items.push(Item::Price { symbol: sym, price: value, source });
                    return;
                }
            }
            p.items.push(Item::Balance { account: who.to_string(), value, source });
        };
        if let Some(m) = re(
            &TO_VALUE,
            &format!(r"^(.+?)\s+(?:is\s+|went\s+|now\s+)?(?:up|down|back|now)\s+to\s+{AMOUNT}$"),
        )
        .captures(&c)
        {
            if let Some(v) = amount(&m[2], &m[3], m.get(4).map_or("", |x| x.as_str())) {
                abs(&mut p, &m[1], v, source);
                continue;
            }
        }
        if let Some(m) = re(
            &RELATIVE,
            &format!(r"^(.+?)\s+(?:is\s+|went\s+)?(up|down|plus|minus|\+|-)\s*(?:by\s+)?{AMOUNT}$"),
        )
        .captures(&c)
        {
            if let Some(v) = amount(&m[3], &m[4], m.get(5).map_or("", |x| x.as_str())) {
                let delta = if matches!(&m[2], "down" | "minus" | "-") { -v } else { v };
                p.items.push(Item::Adjust { account: strip_article(&m[1]).to_string(), delta, source });
                continue;
            }
        }
        if let Some(m) = re(
            &ABSOLUTE,
            &format!(r"^(.+?)\s*(?:\s(?:is|at|=|:|now|->|→|to)\s*)?(?:now\s+)?(?::|=)?\s*{AMOUNT}$"),
        )
        .captures(&c)
        {
            if let Some(v) = amount(&m[2], &m[3], m.get(4).map_or("", |x| x.as_str())) {
                let who = m[1].trim().trim_end_matches([':', '=']).trim();
                let who = ["is", "at", "now", "to"]
                    .iter()
                    .fold(who, |w, kw| w.strip_suffix(&format!(" {kw}")).unwrap_or(w));
                if !who.is_empty() {
                    abs(&mut p, who, v, source);
                    continue;
                }
            }
        }
        p.unparsed.push(clause);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testctx::ctx;
    use rust_decimal_macros::dec;

    fn items(s: &str) -> Vec<Item> {
        parse(s, &ctx()).items
    }

    #[test]
    fn balances_in_every_spelling() {
        let p = parse(
            "td chq 5102.10, visa down to 640; mortgage is 410,950\neq savings: 18.5k and condo = $615,000",
            &ctx(),
        );
        assert!(p.unparsed.is_empty(), "{:?}", p.unparsed);
        let got: Vec<(String, Decimal)> = p
            .items
            .iter()
            .map(|i| match i {
                Item::Balance { account, value, .. } => (account.clone(), *value),
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(
            got,
            vec![
                ("td chq".into(), dec!(5102.10)),
                ("visa".into(), dec!(640)),
                ("mortgage".into(), dec!(410950)),
                ("eq savings".into(), dec!(18500)),
                ("condo".into(), dec!(615000)),
            ]
        );
        assert_eq!(p.items[1].source().unwrap(), "visa down to 640");
    }

    #[test]
    fn relative_moves_are_adjusts() {
        assert_eq!(
            items("visa down 200, eq sav up 500, td chq +1.2k, mortgage minus 1050"),
            vec![
                Item::Adjust {
                    account: "visa".into(),
                    delta: dec!(-200),
                    source: Some("visa down 200".into())
                },
                Item::Adjust {
                    account: "eq sav".into(),
                    delta: dec!(500),
                    source: Some("eq sav up 500".into())
                },
                Item::Adjust {
                    account: "td chq".into(),
                    delta: dec!(1200),
                    source: Some("td chq +1.2k".into())
                },
                Item::Adjust {
                    account: "mortgage".into(),
                    delta: dec!(-1050),
                    source: Some("mortgage minus 1050".into())
                },
            ]
        );
    }

    #[test]
    fn trades_with_account_and_price_in_any_order() {
        let got = items("sold 20 XEQT in the TFSA, bought 10 vfv @ 118.2 into my tfsa, buy 5 VTI in rrsp at 290, added 3 shares of ZAG");
        assert_eq!(
            got[0],
            Item::Trade {
                account: Some("tfsa".into()),
                symbol: "XEQT".into(),
                units: dec!(-20),
                price: None,
                source: Some("sold 20 XEQT in the TFSA".into())
            }
        );
        assert!(
            matches!(&got[1], Item::Trade { account: Some(a), symbol, units, price: Some(px), .. } if a == "tfsa" && symbol == "VFV" && *units == dec!(10) && *px == dec!(118.2))
        );
        assert!(
            matches!(&got[2], Item::Trade { account: Some(a), units, price: Some(px), .. } if a == "rrsp" && *units == dec!(5) && *px == dec!(290))
        );
        assert!(
            matches!(&got[3], Item::Trade { account: None, symbol, units, .. } if symbol == "ZAG" && *units == dec!(3))
        );
    }

    #[test]
    fn a_held_symbol_with_a_number_is_a_price_and_notes_are_notes() {
        assert_eq!(
            items("xeqt 35.10"),
            vec![Item::Price {
                symbol: "XEQT.TO".into(),
                price: dec!(35.10),
                source: Some("xeqt 35.10".into())
            }]
        );
        assert_eq!(
            items("note: paid the November mortgage"),
            vec![Item::Note {
                note: "paid the November mortgage".into(),
                source: Some("note: paid the November mortgage".into())
            }]
        );
    }

    #[test]
    fn questions_become_queries_and_nonsense_is_kept_as_unparsed() {
        let p = parse("how much did my net worth grow this year?", &ctx());
        assert_eq!(p.query.as_deref(), Some("how much did my net worth grow this year"));
        assert!(p.items.is_empty());
        let p = parse("td chq 5102, lol idk, visa 640", &ctx());
        assert_eq!(p.items.len(), 2);
        assert_eq!(p.unparsed, vec!["lol idk".to_string()]);
    }

    #[test]
    fn end_to_end_the_canonical_message_resolves_cleanly() {
        let c = ctx();
        let p = parse("td chq 5102, visa 640", &c);
        let plan = crate::resolve(&crate::testctx::ledger(), &c, &p);
        assert!(plan.rejected.is_empty(), "{:?}", plan.rejected);
        assert_eq!(plan.mutations.len(), 2);
        assert_eq!(plan.rows[0].account.as_deref(), Some("td-chq"));
        assert_eq!(plan.rows[0].to, Some(dec!(5102)));
        assert_eq!(plan.rows[1].account.as_deref(), Some("td-visa"));
        assert_eq!(plan.rows[1].effect, Some(dec!(600)));
        assert_eq!(plan.net_change, Some(dec!(1428.80)));
    }
}
