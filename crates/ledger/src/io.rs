use std::fs;
use std::path::Path;

use crate::schema::Ledger;
use crate::validate::{validate, LedgerError};

pub const HEADER: &str = "# worthwatch ledger. Edit freely; the app reloads on save and refuses to\n# overwrite this file while it has an error. See ledger.example.yaml for the shape.\n# Liabilities are entered as positive amounts owed.\n";

pub fn parse(yaml: &str) -> Result<Ledger, LedgerError> {
    let l: Ledger = serde_yaml_ng::from_str(yaml)?;
    validate(&l)?;
    Ok(l)
}

pub fn to_yaml(l: &Ledger) -> Result<String, LedgerError> {
    Ok(format!("{HEADER}{}", serde_yaml_ng::to_string(l)?))
}

pub fn load(path: &Path) -> Result<Ledger, LedgerError> {
    parse(&fs::read_to_string(path)?)
}

/// Write to a temp file in the same directory, then rename over the target,
/// so a crash can never leave a half-written ledger.
pub fn save(path: &Path, l: &Ledger) -> Result<(), LedgerError> {
    let yaml = to_yaml(l)?;
    let tmp = path.with_extension("yaml.tmp");
    fs::write(&tmp, yaml)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_ledger_round_trips() {
        let src = include_str!("../../../data/ledger.example.yaml");
        let l = parse(src).expect("example ledger must parse");
        let again = parse(&to_yaml(&l).unwrap()).unwrap();
        assert_eq!(l, again);
    }

    #[test]
    fn save_is_atomic_and_loads_back() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ledger.yaml");
        let l = Ledger::starter("CAD");
        save(&p, &l).unwrap();
        assert!(!p.with_extension("yaml.tmp").exists());
        assert_eq!(load(&p).unwrap(), l);
        assert!(fs::read_to_string(&p).unwrap().starts_with("# worthwatch ledger"));
    }
}
