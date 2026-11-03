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
    Ok(format!("{HEADER}{}", unquote_numbers(&serde_yaml_ng::to_string(l)?)))
}

/// Decimals serialize as quoted strings ('5102.10'); write them bare so the file
/// reads like numbers. Parsing accepts both forms, so this is purely cosmetic.
fn unquote_numbers(yaml: &str) -> String {
    // Fields whose values are text and must keep their quotes even when numeric-looking.
    const TEXT_KEYS: [&str; 12] = [
        "id",
        "name",
        "institution",
        "kind",
        "currency",
        "base_currency",
        "symbol",
        "note",
        "account",
        "contains",
        "category",
        "matches",
    ];
    let mut out = String::with_capacity(yaml.len());
    for line in yaml.lines() {
        let key = line.split(':').next().map(|k| k.trim().trim_start_matches("- ")).unwrap_or("");
        if TEXT_KEYS.contains(&key) || line.trim_start().starts_with("- '") {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('\'') {
            let after = &rest[start + 1..];
            match after.find('\'') {
                Some(end) if is_plain_number(&after[..end]) => {
                    out.push_str(&rest[..start]);
                    out.push_str(&after[..end]);
                    rest = &after[end + 1..];
                }
                Some(end) => {
                    out.push_str(&rest[..start + 1 + end + 1]);
                    rest = &after[end + 1..];
                }
                None => break,
            }
        }
        out.push_str(rest);
        out.push('\n');
    }
    out
}

fn is_plain_number(s: &str) -> bool {
    let digits = s.strip_prefix('-').unwrap_or(s);
    let mut parts = digits.splitn(2, '.');
    let int = parts.next().unwrap_or("");
    let frac = parts.next();
    !int.is_empty()
        && int.bytes().all(|b| b.is_ascii_digit())
        && frac.is_none_or(|f| !f.is_empty() && f.bytes().all(|b| b.is_ascii_digit()))
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

    #[test]
    fn numbers_are_written_bare_and_strings_stay_quoted() {
        let src = include_str!("../../../data/ledger.example.yaml");
        let yaml = to_yaml(&parse(src).unwrap()).unwrap();
        assert!(yaml.contains("td-chq: 4210.55"), "{yaml}");
        assert!(yaml.contains("units: 412.5"), "{yaml}");
        assert!(!yaml.contains("'4210.55'"));
        // A date key stays a date, a note that looks numeric would stay quoted.
        let mut l = parse(src).unwrap();
        l.events.push(crate::Event { date: l.events[0].date, note: "2024".into() });
        assert!(to_yaml(&l).unwrap().contains("note: '2024'"));
    }
}
