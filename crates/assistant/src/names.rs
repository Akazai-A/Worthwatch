//! Matching the user's words ("td chq", "visa", "the tfsa") to accounts and symbols.
//! Every query token must match an alias of the account: a word of its name or
//! institution, a piece of its id, or a synonym of its kind. One account must win
//! outright; two equally good matches are an ambiguity the user has to settle.

use ledger::AccountKind;

use crate::AccountCtx;

const STOP: &[&str] =
    &["the", "my", "our", "a", "an", "account", "acct", "in", "at", "on", "is", "to", "of", "for"];

pub fn kind_synonyms(kind: AccountKind) -> &'static [&'static str] {
    match kind {
        AccountKind::Chequing => &["chequing", "checking", "chq", "cheq", "cheque", "chequing", "current"],
        AccountKind::Savings => &["savings", "saving", "sav", "hisa"],
        AccountKind::Tfsa => &["tfsa"],
        AccountKind::Rrsp => &["rrsp", "rsp"],
        AccountKind::Fhsa => &["fhsa"],
        AccountKind::Resp => &["resp"],
        AccountKind::NonRegistered => &["nonreg", "non", "registered", "margin", "taxable", "brokerage"],
        AccountKind::Crypto => &["crypto"],
        AccountKind::Property => &["property", "house", "home", "condo", "apartment"],
        AccountKind::Vehicle => &["vehicle", "car", "truck"],
        AccountKind::OtherAsset => &["asset"],
        AccountKind::CreditCard => &["credit", "card", "visa", "mastercard", "mc", "amex", "cc"],
        AccountKind::LineOfCredit => &["loc", "heloc", "line", "credit"],
        AccountKind::Mortgage => &["mortgage", "mtg"],
        AccountKind::Loan => &["loan"],
        AccountKind::OtherLiability => &["liability", "debt"],
    }
}

pub fn tokens(s: &str) -> Vec<String> {
    s.to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty() && !STOP.contains(t))
        .map(str::to_string)
        .collect()
}

fn aliases(a: &AccountCtx) -> Vec<String> {
    let mut v = tokens(&a.name);
    v.extend(a.id.split('-').map(str::to_string));
    if let Some(i) = &a.institution {
        v.extend(tokens(i));
    }
    v.extend(kind_synonyms(a.kind).iter().map(|s| s.to_string()));
    v
}

/// 2 = exact alias, 1 = prefix (3+ chars), 0 = no match.
fn token_score(token: &str, aliases: &[String]) -> u8 {
    if aliases.iter().any(|a| a == token) {
        2
    } else if token.len() >= 3 && aliases.iter().any(|a| a.starts_with(token)) {
        1
    } else {
        0
    }
}

/// The account the words mean, or why not (reason, hint).
pub fn match_account<'a>(
    accounts: &'a [AccountCtx],
    query: &str,
) -> Result<&'a AccountCtx, (String, Option<String>)> {
    if let Some(a) = accounts.iter().find(|a| a.id == query.trim()) {
        return Ok(a);
    }
    let q = tokens(query);
    if q.is_empty() {
        return Err((format!("\"{}\" does not name an account", query.trim()), None));
    }
    let mut scored: Vec<(u32, &AccountCtx)> = accounts
        .iter()
        .filter_map(|a| {
            let al = aliases(a);
            let mut total = 0u32;
            for t in &q {
                let s = token_score(t, &al);
                if s == 0 {
                    return None;
                }
                total += u32::from(s);
            }
            Some((total, a))
        })
        .collect();
    scored.sort_by_key(|x| std::cmp::Reverse(x.0));
    match scored.as_slice() {
        [] => Err((
            format!("no account matches \"{}\"", query.trim()),
            Some(format!(
                "accounts: {}",
                accounts.iter().map(|a| a.name.as_str()).collect::<Vec<_>>().join(", ")
            )),
        )),
        [(_, a)] => Ok(a),
        [(s1, a), (s2, _), ..] if s1 > s2 => Ok(a),
        [(s, _), ..] => {
            let names: Vec<&str> =
                scored.iter().filter(|(x, _)| x == s).map(|(_, a)| a.name.as_str()).collect();
            Err((
                format!("\"{}\" could be {}", query.trim(), names.join(" or ")),
                Some("say which one".into()),
            ))
        }
    }
}

const VENUES: &[&str] = &[".TO", ".V", ".NE", ".CN", ".L", ".AX"];

fn bare(sym: &str) -> &str {
    VENUES.iter().find_map(|v| sym.strip_suffix(v)).unwrap_or(sym)
}

/// The held symbol the word means: exact, or the same ticker without its venue suffix.
pub fn match_symbol<'a>(held: impl IntoIterator<Item = &'a str>, query: &str) -> Option<&'a str> {
    let q = query.trim().to_ascii_uppercase();
    let held: Vec<&str> = held.into_iter().collect();
    held.iter()
        .copied()
        .find(|s| s.eq_ignore_ascii_case(&q))
        .or_else(|| held.iter().copied().find(|s| bare(s) == bare(&q)))
}

/// A bare ticker in a CAD account is on the TSX; elsewhere it is used as typed.
pub fn canonical_symbol(query: &str, currency: &str) -> String {
    let q = query.trim().to_ascii_uppercase();
    if currency == "CAD" && !q.contains('.') {
        format!("{q}.TO")
    } else {
        q
    }
}

/// Every account holding the symbol, with the symbol as the ledger spells it.
pub fn accounts_holding<'a>(
    accounts: &'a [AccountCtx],
    query: &str,
) -> (Option<String>, Vec<&'a AccountCtx>) {
    let mut sym = None;
    let mut holders = Vec::new();
    for a in accounts {
        if let Some(ps) = &a.positions {
            if let Some(s) = match_symbol(ps.iter().map(|p| p.symbol.as_str()), query) {
                sym.get_or_insert_with(|| s.to_string());
                holders.push(a);
            }
        }
    }
    (sym, holders)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testctx::ctx;

    fn id(q: &str) -> Result<String, String> {
        match_account(&ctx().accounts, q).map(|a| a.id.clone()).map_err(|(r, _)| r)
    }

    #[test]
    fn words_find_accounts() {
        assert_eq!(id("td chq").unwrap(), "td-chq");
        assert_eq!(id("TD Chequing").unwrap(), "td-chq");
        assert_eq!(id("visa").unwrap(), "td-visa");
        assert_eq!(id("the tfsa").unwrap(), "ws-tfsa");
        assert_eq!(id("my rrsp").unwrap(), "qt-rrsp");
        assert_eq!(id("eq sav").unwrap(), "eq-sav");
        assert_eq!(id("savings").unwrap(), "eq-sav");
        assert_eq!(id("mortgage").unwrap(), "td-mortgage");
        assert_eq!(id("usd chequing").unwrap(), "rbc-usd");
        assert_eq!(id("rbc").unwrap(), "rbc-usd");
        assert_eq!(id("condo").unwrap(), "condo");
        assert_eq!(id("house").unwrap(), "condo");
        assert_eq!(id("ws-tfsa").unwrap(), "ws-tfsa");
    }

    #[test]
    fn ambiguity_and_misses_say_why() {
        let e = id("td").unwrap_err();
        assert!(e.starts_with("\"td\" could be TD Chequing or"), "{e}");
        assert!(id("chequing").unwrap_err().contains("TD Chequing or USD Chequing"));
        assert!(id("bitcoin").unwrap_err().starts_with("no account matches"));
        assert!(id("the").unwrap_err().contains("does not name"));
        // A prefix only counts from three letters, so "tf" is not the TFSA.
        assert!(id("tf").is_err());
        assert_eq!(id("wealth").unwrap(), "ws-tfsa");
    }

    #[test]
    fn symbols_match_with_or_without_venue() {
        let held = ["XEQT.TO", "VFV.TO"];
        assert_eq!(match_symbol(held, "xeqt"), Some("XEQT.TO"));
        assert_eq!(match_symbol(held, "XEQT.TO"), Some("XEQT.TO"));
        assert_eq!(match_symbol(held, "vti"), None);
        assert_eq!(canonical_symbol("zag", "CAD"), "ZAG.TO");
        assert_eq!(canonical_symbol("vti", "USD"), "VTI");
        let c = ctx();
        let (s, h) = accounts_holding(&c.accounts, "xeqt");
        assert_eq!(s.as_deref(), Some("XEQT.TO"));
        assert_eq!(h.len(), 1);
    }
}
