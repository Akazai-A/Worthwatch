use std::collections::HashSet;

use serde::Serialize;
use thiserror::Error;

use crate::schema::*;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("unsupported ledger version {0} (this build understands {SCHEMA_VERSION})")]
    Version(u8),
    #[error("base_currency must be set (e.g. CAD)")]
    MissingBaseCurrency,
    #[error("duplicate account id `{0}`")]
    DuplicateAccount(String),
    #[error("account id `{0}` must be kebab-case (a-z, 0-9, -)")]
    BadAccountId(String),
    #[error("account `{0}` needs a name")]
    EmptyName(String),
    #[error("account `{0}` needs a currency (e.g. CAD)")]
    EmptyCurrency(String),
    #[error("unknown account `{0}`")]
    UnknownAccount(String),
    #[error("account `{0}` has no position in `{1}`")]
    UnknownPosition(String, String),
    #[error("account `{0}` has both a balance snapshot and positions on {1}")]
    BalanceAndPositions(String, String),
    #[error("account `{0}` lists `{1}` more than once in positions")]
    DuplicatePosition(String, String),
    #[error("account `{0}` holds a negative number of `{1}` units")]
    NegativeUnits(String, String),
    #[error("price for `{0}` on {1} must be positive")]
    NonPositivePrice(String, String),
    #[error("fx rate for `{0}` must be positive")]
    NonPositiveFx(String),
    #[error("recurring amount for `{0}` must be positive")]
    NonPositiveRecurring(String),
    #[error("`{0}` is not a TFSA, RRSP or FHSA, so it has no contribution room")]
    NotRegistered(String),
    #[error("contribution room for {0} cannot be negative")]
    NegativeRoom(String),
    #[error("projection rate {0} is outside -50..100 percent")]
    BadRate(String),
    #[error("ledger.yaml has a problem; fix the file first: {0}")]
    FileHasProblem(String),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// A user-facing problem with a hand-edited file. Line is 1-based when known.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub line: Option<usize>,
    pub message: String,
}

impl From<&LedgerError> for Diagnostic {
    fn from(e: &LedgerError) -> Self {
        let line = match e {
            LedgerError::Yaml(y) => y.location().map(|l| l.line()),
            _ => None,
        };
        // serde_yaml's messages already carry "at line X column Y"; keep the human part.
        let message = match e {
            LedgerError::Yaml(y) => {
                let s = y.to_string();
                let s = s.split(" at line ").next().unwrap_or(&s).to_string();
                format!("yaml: {s}")
            }
            _ => e.to_string(),
        };
        Diagnostic { line, message }
    }
}

fn is_kebab(id: &str) -> bool {
    !id.is_empty()
        && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-')
}

pub fn validate(l: &Ledger) -> Result<(), LedgerError> {
    if l.version != SCHEMA_VERSION {
        return Err(LedgerError::Version(l.version));
    }
    if l.settings.base_currency.trim().is_empty() {
        return Err(LedgerError::MissingBaseCurrency);
    }
    for (cur, rate) in &l.settings.fx {
        if rate.is_sign_negative() || rate.is_zero() {
            return Err(LedgerError::NonPositiveFx(cur.clone()));
        }
    }
    let mut ids = HashSet::new();
    for a in &l.accounts {
        if !is_kebab(&a.id) {
            return Err(LedgerError::BadAccountId(a.id.clone()));
        }
        if !ids.insert(a.id.as_str()) {
            return Err(LedgerError::DuplicateAccount(a.id.clone()));
        }
        if a.name.trim().is_empty() {
            return Err(LedgerError::EmptyName(a.id.clone()));
        }
        if a.currency.trim().is_empty() {
            return Err(LedgerError::EmptyCurrency(a.id.clone()));
        }
        if let Some(positions) = &a.positions {
            let mut symbols = HashSet::new();
            for p in positions {
                if !symbols.insert(p.symbol.as_str()) {
                    return Err(LedgerError::DuplicatePosition(a.id.clone(), p.symbol.clone()));
                }
                if p.units.is_sign_negative() {
                    return Err(LedgerError::NegativeUnits(a.id.clone(), p.symbol.clone()));
                }
            }
        }
    }
    for (symbol, days) in &l.prices {
        for (date, price) in days {
            if price.is_sign_negative() || price.is_zero() {
                return Err(LedgerError::NonPositivePrice(symbol.clone(), date.to_string()));
            }
        }
    }
    for (date, day) in &l.snapshots {
        for id in day.keys() {
            let acct = l.account(id).ok_or_else(|| LedgerError::UnknownAccount(id.clone()))?;
            if acct.positions.is_some() {
                return Err(LedgerError::BalanceAndPositions(id.clone(), date.to_string()));
            }
        }
    }
    for c in &l.position_changes {
        if l.account(&c.account).is_none() {
            return Err(LedgerError::UnknownAccount(c.account.clone()));
        }
    }
    for r in &l.recurring {
        if l.account(&r.account).is_none() {
            return Err(LedgerError::UnknownAccount(r.account.clone()));
        }
        if r.amount <= rust_decimal::Decimal::ZERO {
            return Err(LedgerError::NonPositiveRecurring(r.account.clone()));
        }
    }
    for c in &l.contributions {
        let a = l.account(&c.account).ok_or_else(|| LedgerError::UnknownAccount(c.account.clone()))?;
        if !matches!(a.kind, AccountKind::Tfsa | AccountKind::Rrsp | AccountKind::Fhsa) {
            return Err(LedgerError::NotRegistered(c.account.clone()));
        }
    }
    for (kind, r) in &l.room {
        if r.amount.is_sign_negative() {
            return Err(LedgerError::NegativeRoom(kind.label().into()));
        }
    }
    let p = &l.settings.projection;
    let lo = rust_decimal::Decimal::from(-50);
    let hi = rust_decimal::Decimal::from(100);
    for (what, v) in
        [("return_pct", &p.return_pct), ("cash_pct", &p.cash_pct), ("property_pct", &p.property_pct)]
            .into_iter()
            .chain(p.rates.iter().map(|(k, v)| (k.as_str(), v)))
    {
        if *v < lo || *v > hi {
            return Err(LedgerError::BadRate(what.into()));
        }
    }
    for id in p.rates.keys() {
        if l.account(id).is_none() {
            return Err(LedgerError::UnknownAccount(id.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::parse;
    use rust_decimal_macros::dec;
    use std::collections::BTreeMap;

    fn account(id: &str) -> Account {
        Account {
            id: id.into(),
            name: "Some account".into(),
            institution: None,
            kind: AccountKind::Chequing,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        }
    }

    fn with_accounts(accounts: Vec<Account>) -> Ledger {
        Ledger { accounts, ..Ledger::starter("CAD") }
    }

    #[test]
    fn starter_is_valid() {
        validate(&Ledger::starter("CAD")).unwrap();
    }

    #[test]
    fn wrong_version_is_rejected() {
        let l = Ledger { version: 99, ..Ledger::starter("CAD") };
        assert!(matches!(validate(&l), Err(LedgerError::Version(99))));
    }

    #[test]
    fn base_currency_is_required() {
        assert!(matches!(validate(&Ledger::starter("  ")), Err(LedgerError::MissingBaseCurrency)));
    }

    #[test]
    fn account_ids_must_be_kebab_case() {
        for bad in ["TD Chq", "td_chq", "", "-td", "td-"] {
            let l = with_accounts(vec![account(bad)]);
            assert!(matches!(validate(&l), Err(LedgerError::BadAccountId(_))), "{bad:?} should fail");
        }
        validate(&with_accounts(vec![account("td-chq-2")])).unwrap();
    }

    #[test]
    fn duplicate_account_ids_are_rejected() {
        let l = with_accounts(vec![account("a"), account("a")]);
        assert!(matches!(validate(&l), Err(LedgerError::DuplicateAccount(_))));
    }

    #[test]
    fn name_and_currency_are_required() {
        let mut a = account("a");
        a.name = " ".into();
        assert!(matches!(validate(&with_accounts(vec![a])), Err(LedgerError::EmptyName(_))));
        let mut a = account("a");
        a.currency = "".into();
        assert!(matches!(validate(&with_accounts(vec![a])), Err(LedgerError::EmptyCurrency(_))));
    }

    #[test]
    fn snapshot_for_unknown_account_is_rejected() {
        let mut l = Ledger::starter("CAD");
        l.snapshots.insert(
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            BTreeMap::from([("ghost".to_string(), dec!(1))]),
        );
        assert!(matches!(validate(&l), Err(LedgerError::UnknownAccount(id)) if id == "ghost"));
    }

    #[test]
    fn balance_and_positions_cannot_coexist() {
        let mut a = account("tfsa");
        a.positions = Some(vec![Position { symbol: "XEQT.TO".into(), units: dec!(1), book_cost: None }]);
        let mut l = with_accounts(vec![a]);
        l.snapshots.insert(
            chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            BTreeMap::from([("tfsa".to_string(), dec!(1))]),
        );
        assert!(matches!(validate(&l), Err(LedgerError::BalanceAndPositions(..))));
    }

    #[test]
    fn positions_must_be_unique_and_non_negative() {
        let mut a = account("tfsa");
        a.positions = Some(vec![
            Position { symbol: "X".into(), units: dec!(1), book_cost: None },
            Position { symbol: "X".into(), units: dec!(1), book_cost: None },
        ]);
        assert!(matches!(validate(&with_accounts(vec![a])), Err(LedgerError::DuplicatePosition(..))));
        let mut a = account("tfsa");
        a.positions = Some(vec![Position { symbol: "X".into(), units: dec!(-1), book_cost: None }]);
        assert!(matches!(validate(&with_accounts(vec![a])), Err(LedgerError::NegativeUnits(..))));
    }

    #[test]
    fn prices_and_fx_must_be_positive() {
        let mut l = Ledger::starter("CAD");
        l.prices.insert(
            "X".into(),
            BTreeMap::from([(chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(), dec!(0))]),
        );
        assert!(matches!(validate(&l), Err(LedgerError::NonPositivePrice(..))));
        let mut l = Ledger::starter("CAD");
        l.settings.fx.insert("USD".into(), dec!(-1.38));
        assert!(matches!(validate(&l), Err(LedgerError::NonPositiveFx(..))));
    }

    #[test]
    fn broken_yaml_reports_a_line_number() {
        let src = "version: 1\nsettings:\n  base_currency: CAD\naccounts:\n  - id: td-chq\n    name: [oops\n";
        let err = parse(src).unwrap_err();
        let d = Diagnostic::from(&err);
        assert!(d.line.is_some(), "expected a line number, got {d:?}");
        assert!(d.message.starts_with("yaml:"));
    }

    #[test]
    fn semantic_errors_have_no_line_but_a_clear_message() {
        let src = "version: 1\nsettings:\n  base_currency: CAD\naccounts:\n  - id: Bad Id\n    name: x\n    kind: chequing\n    currency: CAD\n";
        let d = Diagnostic::from(&parse(src).unwrap_err());
        assert_eq!(d.line, None);
        assert!(d.message.contains("kebab-case"));
    }
}
