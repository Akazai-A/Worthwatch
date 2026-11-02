use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use ledger::{io, Diagnostic, Ledger, LedgerError, Mutation};
use tokio::sync::broadcast;

/// What the server knows about the ledger file right now.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The last ledger that parsed and validated.
    pub ledger: Ledger,
    /// Set while the file on disk is unreadable; the UI keeps showing `ledger`.
    pub problem: Option<Diagnostic>,
    /// Bumps on every change; the UI reloads when it sees a new one.
    pub version: u64,
}

#[derive(Clone)]
pub struct AppState {
    pub ledger_path: PathBuf,
    inner: Arc<RwLock<Snapshot>>,
    changed: broadcast::Sender<u64>,
}

impl AppState {
    /// Open `<data_dir>/ledger.yaml`, creating a starter file on first run.
    pub fn open(data_dir: &Path) -> Result<Self, LedgerError> {
        std::fs::create_dir_all(data_dir)?;
        let ledger_path = data_dir.join("ledger.yaml");
        let ledger = if ledger_path.exists() {
            io::load(&ledger_path)?
        } else {
            let l = Ledger::starter("CAD");
            io::save(&ledger_path, &l)?;
            l
        };
        let (changed, _) = broadcast::channel(16);
        let inner = Arc::new(RwLock::new(Snapshot { ledger, problem: None, version: 1 }));
        Ok(Self { ledger_path, inner, changed })
    }

    pub fn snapshot(&self) -> Snapshot {
        self.inner.read().expect("ledger lock").clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<u64> {
        self.changed.subscribe()
    }

    /// The single write path: apply, persist atomically, then swap in memory.
    /// Refused while the file on disk is broken, so a hand edit is never overwritten.
    pub fn apply_many(&self, ms: Vec<Mutation>) -> Result<Snapshot, LedgerError> {
        let mut guard = self.inner.write().expect("ledger lock");
        if let Some(p) = &guard.problem {
            return Err(LedgerError::FileHasProblem(p.message.clone()));
        }
        let next = guard.ledger.apply_many(ms)?;
        io::save(&self.ledger_path, &next)?;
        guard.ledger = next;
        guard.version += 1;
        let _ = self.changed.send(guard.version);
        Ok(guard.clone())
    }

    /// Re-read the file. Called by the watcher; returns true when anything changed.
    pub fn reload(&self) -> bool {
        let parsed = io::load(&self.ledger_path);
        let mut guard = self.inner.write().expect("ledger lock");
        let changed = match parsed {
            Ok(l) => {
                let changed = guard.problem.is_some() || l != guard.ledger;
                guard.ledger = l;
                guard.problem = None;
                changed
            }
            Err(e) => {
                let d = Diagnostic::from(&e);
                let changed = guard.problem.as_ref() != Some(&d);
                guard.problem = Some(d);
                changed
            }
        };
        if changed {
            guard.version += 1;
            let _ = self.changed.send(guard.version);
        }
        changed
    }

    /// Poll the file a few times a second and reload when its bytes change. Polling
    /// (not inotify) so it also works on Docker bind mounts, where change events never
    /// arrive; hashing the bytes rather than trusting mtime, which editors and fast
    /// successive writes can leave unchanged.
    pub fn watch(&self) {
        let state = self.clone();
        std::thread::Builder::new()
            .name("ledger-watch".into())
            .spawn(move || {
                let mut last = stamp(&state.ledger_path);
                loop {
                    std::thread::sleep(Duration::from_millis(250));
                    let now = stamp(&state.ledger_path);
                    if now != last {
                        last = now;
                        if state.reload() {
                            tracing::info!("ledger.yaml changed on disk, reloaded");
                        }
                    }
                }
            })
            .expect("spawn watcher");
    }
}

fn stamp(p: &Path) -> Option<u64> {
    // FNV-1a over the file bytes; the ledger is small, so this is cheap at 4 Hz.
    let bytes = std::fs::read(p).ok()?;
    Some(bytes.iter().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use ledger::{Account, AccountKind};
    use rust_decimal::Decimal;

    fn chq() -> Account {
        Account {
            id: "chq".into(),
            name: "Chequing".into(),
            institution: None,
            kind: AccountKind::Chequing,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        }
    }

    #[test]
    fn first_run_creates_a_starter_ledger() {
        let dir = tempfile::tempdir().unwrap();
        let s = AppState::open(dir.path()).unwrap();
        assert!(s.ledger_path.exists());
        assert_eq!(s.snapshot().ledger, Ledger::starter("CAD"));
    }

    #[test]
    fn broken_file_keeps_last_good_ledger_and_blocks_writes() {
        let dir = tempfile::tempdir().unwrap();
        let s = AppState::open(dir.path()).unwrap();
        s.apply_many(vec![Mutation::UpsertAccount { account: chq() }]).unwrap();
        let good = s.snapshot();
        assert_eq!(good.ledger.accounts.len(), 1);

        let text = std::fs::read_to_string(&s.ledger_path).unwrap();
        std::fs::write(&s.ledger_path, text + "accounts: [oops\n").unwrap();
        assert!(s.reload());
        let snap = s.snapshot();
        assert!(snap.problem.is_some());
        assert_eq!(snap.ledger, good.ledger);
        assert!(snap.version > good.version);

        let err = s
            .apply_many(vec![Mutation::SetBalance {
                date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                account: "chq".into(),
                value: Decimal::ONE,
            }])
            .unwrap_err();
        assert!(matches!(err, LedgerError::FileHasProblem(_)));
        // The broken edit is still on disk, untouched.
        assert!(std::fs::read_to_string(&s.ledger_path).unwrap().ends_with("[oops\n"));

        io::save(&s.ledger_path, &good.ledger).unwrap();
        assert!(s.reload());
        assert!(s.snapshot().problem.is_none());
    }

    #[test]
    fn reload_ignores_identical_content() {
        let dir = tempfile::tempdir().unwrap();
        let s = AppState::open(dir.path()).unwrap();
        assert!(!s.reload());
    }
}
