use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use ledger::{io, Ledger, LedgerError, Mutation};

#[derive(Clone)]
pub struct AppState {
    pub ledger_path: PathBuf,
    pub ledger: Arc<RwLock<Ledger>>,
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
        Ok(Self { ledger_path, ledger: Arc::new(RwLock::new(ledger)) })
    }

    pub fn read(&self) -> Ledger {
        self.ledger.read().expect("ledger lock").clone()
    }

    /// The single write path: apply, persist atomically, then swap in memory.
    pub fn apply_many(&self, ms: Vec<Mutation>) -> Result<Ledger, LedgerError> {
        let mut guard = self.ledger.write().expect("ledger lock");
        let next = guard.apply_many(ms)?;
        io::save(&self.ledger_path, &next)?;
        *guard = next.clone();
        Ok(next)
    }
}
