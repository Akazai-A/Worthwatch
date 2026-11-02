use std::collections::HashSet;

use serde::Serialize;
use thiserror::Error;

use crate::schema::*;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("unsupported ledger version {0} (this build understands {SCHEMA_VERSION})")]
    Version(u8),
    #[error("duplicate account id `{0}`")]
    DuplicateAccount(String),
    #[error("account id `{0}` must be kebab-case (a-z, 0-9, -)")]
    BadAccountId(String),
    #[error("unknown account `{0}`")]
    UnknownAccount(String),
    #[error("account `{0}` has both a balance snapshot and positions on {1}")]
    BalanceAndPositions(String, String),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// A user-facing problem with a hand-edited file. Line is 1-based when known.
#[derive(Debug, Clone, Serialize)]
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
        Diagnostic { line, message: e.to_string() }
    }
}

pub fn validate(l: &Ledger) -> Result<(), LedgerError> {
    if l.version != SCHEMA_VERSION {
        return Err(LedgerError::Version(l.version));
    }
    let mut ids = HashSet::new();
    for a in &l.accounts {
        if !a.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') || a.id.is_empty()
        {
            return Err(LedgerError::BadAccountId(a.id.clone()));
        }
        if !ids.insert(a.id.as_str()) {
            return Err(LedgerError::DuplicateAccount(a.id.clone()));
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
    Ok(())
}
