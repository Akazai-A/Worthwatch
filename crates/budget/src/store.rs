//! `data/transactions/`: every tap and statement row, append-only.
//!
//! ```text
//! transactions/
//!   2026-11.jsonl     one JSON record per line: {"source":"tap",...} or {"source":"csv",...}
//!   categories.json   transaction id -> category you picked by hand
//! ```
//! Records are never rewritten. A record whose id is already stored is skipped, so a
//! retried tap or a re-imported statement adds nothing.

use std::collections::{BTreeMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use crate::statement::CsvRow;
use crate::tap::Tap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Record {
    Tap(Tap),
    Csv(CsvRow),
}

impl Record {
    pub fn id(&self) -> &str {
        match self {
            Record::Tap(t) => &t.id,
            Record::Csv(r) => &r.id,
        }
    }
    fn date(&self) -> NaiveDate {
        match self {
            Record::Tap(t) => t.at.date_naive(),
            Record::Csv(r) => r.date,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TxnStore {
    pub dir: PathBuf,
    pub taps: Vec<Tap>,
    pub rows: Vec<CsvRow>,
    pub overrides: BTreeMap<String, String>,
    /// Lines that could not be read, as "file:line: why".
    pub errors: Vec<String>,
    ids: HashSet<String>,
}

const OVERRIDES: &str = "categories.json";

impl TxnStore {
    /// Read everything under `dir`. A missing dir is an empty store.
    pub fn load(dir: &Path) -> Self {
        let mut s = TxnStore { dir: dir.to_path_buf(), ..Default::default() };
        if let Ok(text) = std::fs::read_to_string(dir.join(OVERRIDES)) {
            match serde_json::from_str(&text) {
                Ok(o) => s.overrides = o,
                Err(e) => s.errors.push(format!("{OVERRIDES}: {e}")),
            }
        }
        let Ok(entries) = std::fs::read_dir(dir) else { return s };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "jsonl"))
            .collect();
        files.sort();
        for f in files {
            let name = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            for (i, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Record>(line) {
                    Ok(r) => s.insert(r),
                    Err(e) => s.errors.push(format!("{name}:{}: {e}", i + 1)),
                }
            }
        }
        s
    }

    fn insert(&mut self, r: Record) {
        if !self.ids.insert(r.id().to_string()) {
            return;
        }
        match r {
            Record::Tap(t) => self.taps.push(t),
            Record::Csv(c) => self.rows.push(c),
        }
    }

    pub fn contains(&self, id: &str) -> bool {
        self.ids.contains(id)
    }

    /// The newest tap this server received.
    pub fn last_tap(&self) -> Option<DateTime<Utc>> {
        self.taps.iter().map(|t| t.received).max()
    }

    /// Append the records not stored yet, one file per month. Returns how many were new.
    pub fn append(&mut self, records: Vec<Record>) -> std::io::Result<usize> {
        let mut by_month: BTreeMap<String, Vec<Record>> = BTreeMap::new();
        let mut batch = HashSet::new();
        for r in records {
            if self.ids.contains(r.id()) || !batch.insert(r.id().to_string()) {
                continue;
            }
            by_month.entry(r.date().format("%Y-%m").to_string()).or_default().push(r);
        }
        if by_month.is_empty() {
            return Ok(0);
        }
        std::fs::create_dir_all(&self.dir)?;
        let mut added = 0;
        for (month, rs) in by_month {
            let mut text = String::new();
            for r in &rs {
                text.push_str(&serde_json::to_string(r).map_err(std::io::Error::other)?);
                text.push('\n');
            }
            let path = self.dir.join(format!("{month}.jsonl"));
            // A line torn by a crash must not swallow the first new record.
            let torn = std::fs::read(&path).ok().is_some_and(|b| b.last().is_some_and(|c| *c != b'\n'));
            let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
            if torn {
                f.write_all(b"\n")?;
            }
            f.write_all(text.as_bytes())?;
            f.sync_all()?;
            added += rs.len();
            for r in rs {
                self.insert(r);
            }
        }
        Ok(added)
    }

    /// Pick a category for one transaction by hand (`None` goes back to the rules).
    pub fn set_category(&mut self, id: &str, category: Option<&str>) -> std::io::Result<()> {
        match category {
            Some(c) => self.overrides.insert(id.to_string(), c.trim().to_lowercase()),
            None => self.overrides.remove(id),
        };
        std::fs::create_dir_all(&self.dir)?;
        let tmp = self.dir.join(format!("{OVERRIDES}.tmp"));
        std::fs::write(&tmp, serde_json::to_string_pretty(&self.overrides).map_err(std::io::Error::other)?)?;
        std::fs::rename(tmp, self.dir.join(OVERRIDES))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::tests::{row, tap, ymd};
    use rust_decimal_macros::dec;

    #[test]
    fn appends_by_month_skips_known_ids_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = TxnStore::load(dir.path());
        assert!(s.taps.is_empty());
        let t = tap("Amex", "Loblaws", dec!(10), "2026-10-31T23:00:00-05:00");
        let r = row("amex", ymd(2026, 11, 2), dec!(10), "LOBLAWS");
        assert_eq!(
            s.append(vec![Record::Tap(t.clone()), Record::Csv(r.clone()), Record::Tap(t.clone())]).unwrap(),
            2
        );
        assert_eq!(
            s.append(vec![Record::Tap(t.clone()), Record::Csv(r.clone())]).unwrap(),
            0,
            "retries add nothing"
        );
        assert!(dir.path().join("2026-10.jsonl").exists() && dir.path().join("2026-11.jsonl").exists());

        s.set_category(&r.id, Some("Shopping")).unwrap();
        // A torn last line (power cut mid-append) is reported, not fatal.
        let mut f = std::fs::OpenOptions::new().append(true).open(dir.path().join("2026-11.jsonl")).unwrap();
        f.write_all(b"{\"source\":\"csv\",\"id\":").unwrap();
        let back = TxnStore::load(dir.path());
        assert_eq!((back.taps.len(), back.rows.len()), (1, 1));
        assert_eq!(back.taps[0], t);
        assert_eq!(back.overrides[&r.id], "shopping");
        assert_eq!(back.errors.len(), 1);
        assert!(back.contains(&t.id));
        // The next append starts on a fresh line.
        let mut back = back;
        let r2 = row("amex", ymd(2026, 11, 3), dec!(5), "PRESTO");
        assert_eq!(back.append(vec![Record::Csv(r2.clone())]).unwrap(), 1);
        assert!(TxnStore::load(dir.path()).contains(&r2.id));
    }
}
