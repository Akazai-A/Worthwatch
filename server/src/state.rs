use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use chrono::Utc;
use ledger::{io, Diagnostic, FxSource, Ledger, LedgerError, Mutation, PriceSource};
use providers::cache::RefreshReport;
use providers::{FxProvider, PriceProvider, QuoteCache};
use tokio::sync::broadcast;

/// What the server knows about the ledger file right now.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// The last ledger that parsed and validated.
    pub ledger: Ledger,
    /// Set while the file on disk is unreadable; the UI keeps showing `ledger`.
    pub problem: Option<Diagnostic>,
    /// Everything fetched so far (`quotes.json`).
    pub quotes: QuoteCache,
    /// Bumps on every change; the UI reloads when it sees a new one.
    pub version: u64,
}

impl Snapshot {
    /// Manual data from the ledger overlaid on the fetched cache.
    pub fn market(&self) -> derive::Market {
        derive::Market::merge(&self.ledger, &self.quotes.prices, &self.quotes.fx, &self.quotes.currencies)
    }
}

#[derive(Clone)]
pub struct AppState {
    pub ledger_path: PathBuf,
    pub quotes_path: PathBuf,
    /// `WORTHWATCH_OFFLINE=1`: never touch the network (tests, air-gapped use).
    pub offline: bool,
    inner: Arc<RwLock<Snapshot>>,
    changed: broadcast::Sender<u64>,
}

impl AppState {
    /// Open `<data_dir>/ledger.yaml`, creating a starter file on first run.
    pub fn open(data_dir: &Path) -> Result<Self, LedgerError> {
        std::fs::create_dir_all(data_dir)?;
        let ledger_path = data_dir.join("ledger.yaml");
        let quotes_path = data_dir.join("quotes.json");
        let ledger = if ledger_path.exists() {
            io::load(&ledger_path)?
        } else {
            let l = Ledger::starter("CAD");
            io::save(&ledger_path, &l)?;
            l
        };
        let quotes = match QuoteCache::load(&quotes_path) {
            Ok(q) => q,
            Err(e) => {
                tracing::warn!("quotes.json unreadable ({e}); starting with an empty cache");
                QuoteCache::default()
            }
        };
        let offline = std::env::var("WORTHWATCH_OFFLINE").is_ok_and(|v| v == "1" || v == "true");
        let (changed, _) = broadcast::channel(16);
        let inner = Arc::new(RwLock::new(Snapshot { ledger, problem: None, quotes, version: 1 }));
        Ok(Self { ledger_path, quotes_path, offline, inner, changed })
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

    /// Fetch prices and FX for everything the ledger holds, using the providers named
    /// in `settings.providers`, then persist `quotes.json` and bump the version.
    /// Blocking (network); call from `spawn_blocking`. Does nothing when offline.
    pub fn refresh(&self) -> RefreshReport {
        let snap = self.snapshot();
        let l = &snap.ledger;
        let (prices, fx) = providers_for(l, self.offline);
        let mut quotes = snap.quotes.clone();
        let report = quotes.refresh(
            prices.as_ref(),
            fx.as_ref(),
            &l.settings.base_currency,
            &l.symbols(),
            &l.foreign_currencies(),
            Utc::now(),
        );
        if let Err(e) = quotes.save(&self.quotes_path) {
            tracing::warn!("could not write quotes.json: {e}");
        }
        let mut guard = self.inner.write().expect("ledger lock");
        guard.quotes = quotes;
        guard.version += 1;
        let _ = self.changed.send(guard.version);
        report
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

    /// Refresh once at startup when today's quotes are missing, then check every
    /// half hour. One fetch per day per symbol is all the dashboard needs.
    pub fn schedule_refresh(&self) {
        if self.offline {
            tracing::info!("WORTHWATCH_OFFLINE set; prices and FX will not be fetched");
            return;
        }
        let state = self.clone();
        tokio::spawn(async move {
            loop {
                let due = {
                    let s = state.snapshot();
                    let l = &s.ledger;
                    let wants = !l.symbols().is_empty() || !l.foreign_currencies().is_empty();
                    let today = chrono::Local::now().date_naive();
                    let fetched_today = s
                        .quotes
                        .fetched_at
                        .is_some_and(|t| t.with_timezone(&chrono::Local).date_naive() == today);
                    wants && !fetched_today
                };
                if due {
                    let st = state.clone();
                    match tokio::task::spawn_blocking(move || st.refresh()).await {
                        Ok(r) => tracing::info!(
                            "refreshed quotes: {} prices, {} rates, {} errors",
                            r.prices_ok.len(),
                            r.fx_ok.len(),
                            r.errors.len()
                        ),
                        Err(e) => tracing::warn!("refresh task failed: {e}"),
                    }
                }
                tokio::time::sleep(Duration::from_secs(30 * 60)).await;
            }
        });
    }
}

/// The provider pair the ledger asks for. `Box<dyn>` only here, at the boundary.
pub fn providers_for(
    l: &Ledger,
    offline: bool,
) -> (Box<dyn PriceProvider + Send>, Box<dyn FxProvider + Send>) {
    if offline {
        return (Box::new(providers::Off), Box::new(providers::Off));
    }
    let p: Box<dyn PriceProvider + Send> = match l.settings.providers.prices {
        PriceSource::Yahoo => Box::new(providers::yahoo::Yahoo),
        PriceSource::Stooq => Box::new(providers::stooq::Stooq),
        PriceSource::Off => Box::new(providers::Off),
    };
    let f: Box<dyn FxProvider + Send> = match l.settings.providers.fx {
        FxSource::BankOfCanada => Box::new(providers::boc::BankOfCanada),
        FxSource::Off => Box::new(providers::Off),
    };
    (p, f)
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
        assert_eq!(s.snapshot().quotes, QuoteCache::default());
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

    #[test]
    fn offline_refresh_fetches_nothing_but_still_writes_the_cache_and_bumps_version() {
        let dir = tempfile::tempdir().unwrap();
        let mut s = AppState::open(dir.path()).unwrap();
        s.offline = true;
        let mut tfsa = chq();
        tfsa.id = "tfsa".into();
        tfsa.kind = AccountKind::Tfsa;
        tfsa.positions =
            Some(vec![ledger::Position { symbol: "XEQT.TO".into(), units: Decimal::ONE, book_cost: None }]);
        s.apply_many(vec![Mutation::UpsertAccount { account: tfsa }]).unwrap();
        let before = s.snapshot().version;
        let r = s.refresh();
        assert!(r.prices_ok.is_empty());
        assert!(r.errors.contains_key("XEQT.TO"));
        let snap = s.snapshot();
        assert!(snap.version > before);
        assert!(snap.quotes.fetched_at.is_some());
        assert!(s.quotes_path.exists());
    }

    #[test]
    fn fetched_quotes_in_the_cache_value_positions() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = QuoteCache::default();
        let d = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        cache.prices.insert("XEQT.TO".into(), std::collections::BTreeMap::from([(d, Decimal::new(3412, 2))]));
        cache.save(&dir.path().join("quotes.json")).unwrap();
        let s = AppState::open(dir.path()).unwrap();
        let mut tfsa = chq();
        tfsa.id = "tfsa".into();
        tfsa.kind = AccountKind::Tfsa;
        tfsa.positions =
            Some(vec![ledger::Position { symbol: "XEQT.TO".into(), units: Decimal::TEN, book_cost: None }]);
        let snap = s.apply_many(vec![Mutation::UpsertAccount { account: tfsa }]).unwrap();
        let m = snap.market();
        let v = derive::value_account(&snap.ledger, &m, snap.ledger.account("tfsa").unwrap(), d);
        assert_eq!(v.value, Decimal::new(34120, 2));
        assert_eq!(m.price_at("XEQT.TO", d).unwrap().source, derive::Source::Fetched);
    }
}
