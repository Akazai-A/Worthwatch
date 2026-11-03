//! Backups: the whole data folder (ledger, fetched quotes, fund holdings snapshots) in
//! one `.tar.gz` under `<data>/backups/`. One is made automatically each day the app
//! runs and before every restore; the newest `KEEP` are kept. `worthwatch restore`
//! unpacks one into a folder and refuses to clobber an existing ledger without `--force`.

use std::fs;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context};
use chrono::{DateTime, Local, NaiveDate};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use serde::Serialize;

pub const DIR: &str = "backups";
pub const KEEP: usize = 30;
const PREFIX: &str = "worthwatch-";
const SUFFIX: &str = ".tar.gz";
/// What goes in, relative to the data folder. Anything else there is left out.
const ITEMS: [&str; 4] = ["ledger.yaml", "quotes.json", "holdings", "transactions"];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BackupInfo {
    pub name: String,
    pub bytes: u64,
    /// Parsed from the name (`worthwatch-2026-11-03-142210.tar.gz`), local time.
    pub created: String,
    pub date: NaiveDate,
}

/// A name this module made, and nothing that could escape the folder.
pub fn valid_name(name: &str) -> bool {
    name.starts_with(PREFIX)
        && name.ends_with(SUFFIX)
        && name.len() < 80
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
}

fn info(path: &Path) -> Option<BackupInfo> {
    let name = path.file_name()?.to_str()?.to_string();
    if !valid_name(&name) {
        return None;
    }
    let stamp = name.strip_prefix(PREFIX)?.strip_suffix(SUFFIX)?;
    // 2026-11-03-142210[-suffix]
    let date = NaiveDate::parse_from_str(stamp.get(..10)?, "%Y-%m-%d").ok()?;
    let time = stamp.get(11..17).unwrap_or("000000");
    let created = format!("{} {}:{}", &stamp[..10], time.get(..2)?, time.get(2..4)?);
    Some(BackupInfo { name, bytes: fs::metadata(path).ok()?.len(), created, date })
}

/// Newest first.
pub fn list(data_dir: &Path) -> Vec<BackupInfo> {
    let mut v: Vec<BackupInfo> = fs::read_dir(data_dir.join(DIR))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| info(&e.path()))
        .collect();
    v.sort_by(|a, b| b.name.cmp(&a.name));
    v
}

pub fn path_of(data_dir: &Path, name: &str) -> Option<PathBuf> {
    valid_name(name).then(|| data_dir.join(DIR).join(name)).filter(|p| p.is_file())
}

/// Write a backup of `data_dir` into `data_dir/backups/`, then prune to `KEEP`.
pub fn create(data_dir: &Path, now: DateTime<Local>) -> anyhow::Result<BackupInfo> {
    let dir = data_dir.join(DIR);
    fs::create_dir_all(&dir)?;
    let mut name = format!("{PREFIX}{}{SUFFIX}", now.format("%Y-%m-%d-%H%M%S"));
    let mut n = 1;
    while dir.join(&name).exists() {
        n += 1;
        name = format!("{PREFIX}{}-{n}{SUFFIX}", now.format("%Y-%m-%d-%H%M%S"));
    }
    let tmp = dir.join(format!(".{name}.tmp"));
    write_archive(data_dir, &tmp).context("writing backup")?;
    fs::rename(&tmp, dir.join(&name))?;
    prune(data_dir, KEEP);
    info(&dir.join(&name)).context("reading back the backup")
}

/// Make today's automatic backup unless one already exists for `today`.
pub fn daily(data_dir: &Path, now: DateTime<Local>) -> anyhow::Result<Option<BackupInfo>> {
    let today = now.date_naive();
    if !data_dir.join("ledger.yaml").exists() || list(data_dir).iter().any(|b| b.date == today) {
        return Ok(None);
    }
    create(data_dir, now).map(Some)
}

/// Keep the newest `keep` backups, and also the newest one of each of the last `keep`
/// days that have any. A burst of backups in one afternoon (or a script making them) can
/// then never push out last week's.
pub fn prune(data_dir: &Path, keep: usize) {
    let mut days: Vec<NaiveDate> = Vec::new();
    for (i, b) in list(data_dir).into_iter().enumerate() {
        let first_of_day = !days.contains(&b.date);
        if first_of_day {
            days.push(b.date);
        }
        if i < keep || (first_of_day && days.len() <= keep) {
            continue;
        }
        let _ = fs::remove_file(data_dir.join(DIR).join(b.name));
    }
}

fn write_archive(data_dir: &Path, out: &Path) -> anyhow::Result<()> {
    let file = fs::File::create(out)?;
    let mut tar = tar::Builder::new(GzEncoder::new(file, Compression::default()));
    tar.follow_symlinks(false);
    for item in ITEMS {
        let p = data_dir.join(item);
        if p.is_file() {
            tar.append_path_with_name(&p, item)?;
        } else if p.is_dir() {
            tar.append_dir_all(item, &p)?;
        }
    }
    tar.into_inner()?.finish()?.sync_all()?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RestoreReport {
    pub files: usize,
    /// The backup taken of what was there before (`--force` over an existing ledger).
    pub previous: Option<String>,
}

/// Only plain relative paths under one of `ITEMS`.
fn safe_entry(p: &Path) -> bool {
    let mut comps = p.components();
    let first = match comps.next() {
        Some(Component::Normal(c)) => c.to_str().unwrap_or(""),
        _ => return false,
    };
    ITEMS.contains(&first) && comps.all(|c| matches!(c, Component::Normal(_)))
}

/// Unpack `archive` into `data_dir`. The archive must hold a ledger that parses and
/// validates. An existing ledger is only replaced with `force`, and is backed up first.
pub fn restore(
    archive: &Path,
    data_dir: &Path,
    force: bool,
    now: DateTime<Local>,
) -> anyhow::Result<RestoreReport> {
    fs::create_dir_all(data_dir)?;
    let has_ledger = data_dir.join("ledger.yaml").exists();
    if has_ledger && !force {
        bail!(
            "{} already has a ledger.yaml; restore into an empty folder, or pass --force (the current data is backed up first)",
            data_dir.display()
        );
    }
    let staging = data_dir.join(".restore");
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(&staging)?;
    let result = unpack(archive, &staging);
    let files = match result {
        Ok(n) => n,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let ledger_text =
        fs::read_to_string(staging.join("ledger.yaml")).context("the backup has no ledger.yaml")?;
    if let Err(e) = ledger::io::parse(&ledger_text) {
        let _ = fs::remove_dir_all(&staging);
        bail!("the backup's ledger.yaml does not load: {e}");
    }
    let previous = if has_ledger { Some(create(data_dir, now)?.name) } else { None };
    for item in ITEMS {
        let target = data_dir.join(item);
        let src = staging.join(item);
        if target.is_dir() {
            fs::remove_dir_all(&target)?;
        } else if target.exists() {
            fs::remove_file(&target)?;
        }
        if src.exists() {
            fs::rename(&src, &target)?;
        }
    }
    fs::remove_dir_all(&staging)?;
    Ok(RestoreReport { files, previous })
}

fn unpack(archive: &Path, into: &Path) -> anyhow::Result<usize> {
    let mut bytes = Vec::new();
    fs::File::open(archive)
        .with_context(|| format!("opening {}", archive.display()))?
        .read_to_end(&mut bytes)?;
    let mut tar = tar::Archive::new(GzDecoder::new(io::Cursor::new(bytes)));
    let mut n = 0;
    for entry in tar.entries().context("not a Worthwatch backup (.tar.gz)")? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let kind = entry.header().entry_type();
        if !safe_entry(&path) || !(kind.is_file() || kind.is_dir()) {
            bail!("the backup contains an unexpected entry: {}", path.display());
        }
        if kind.is_file() {
            n += 1;
        }
        entry.unpack_in(into)?;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at(h: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 11, 3, h, 0, 0).unwrap()
    }

    fn seeded() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        crate::seed::write_seed(dir.path(), NaiveDate::from_ymd_opt(2026, 11, 3).unwrap(), false).unwrap();
        dir
    }

    #[test]
    fn backup_then_restore_onto_a_clean_folder_gives_the_same_dashboard() {
        let src = seeded();
        let b = create(src.path(), at(9)).unwrap();
        assert!(b.name.starts_with("worthwatch-2026-11-03-0900"));
        let dst = tempfile::tempdir().unwrap();
        let r = restore(&src.path().join(DIR).join(&b.name), dst.path(), false, at(10)).unwrap();
        assert!(r.files >= 2);
        assert_eq!(r.previous, None);

        let today = NaiveDate::from_ymd_opt(2026, 11, 3).unwrap();
        let view = |dir: &Path| {
            let s = crate::state::AppState::open(dir).unwrap().snapshot();
            let mut v = serde_json::to_value(crate::routes::build_view(&s, "x", today, true)).unwrap();
            let lt = serde_json::to_value(crate::routes::build_lookthrough(&s, "x", today, true)).unwrap();
            v["lookthrough"] = lt;
            v
        };
        assert_eq!(view(src.path()), view(dst.path()));
        // Fund holdings came along, so look-through has data.
        assert!(dst.path().join("holdings").is_dir());
    }

    #[test]
    fn restore_refuses_to_overwrite_without_force_and_backs_up_with_it() {
        let src = seeded();
        let b = create(src.path(), at(9)).unwrap();
        let archive = src.path().join(DIR).join(&b.name);
        let dst = tempfile::tempdir().unwrap();
        crate::state::AppState::open(dst.path()).unwrap(); // starter ledger
        let starter = fs::read_to_string(dst.path().join("ledger.yaml")).unwrap();
        assert!(restore(&archive, dst.path(), false, at(10)).is_err());
        assert_eq!(fs::read_to_string(dst.path().join("ledger.yaml")).unwrap(), starter);
        let r = restore(&archive, dst.path(), true, at(10)).unwrap();
        let prev = r.previous.expect("previous data backed up");
        assert!(path_of(dst.path(), &prev).is_some());
        assert_eq!(
            fs::read_to_string(dst.path().join("ledger.yaml")).unwrap(),
            fs::read_to_string(src.path().join("ledger.yaml")).unwrap()
        );
    }

    #[test]
    fn a_bad_archive_changes_nothing() {
        let dst = tempfile::tempdir().unwrap();
        let junk = dst.path().join("junk.tar.gz");
        fs::write(&junk, b"not a tarball").unwrap();
        assert!(restore(&junk, dst.path(), false, at(1)).is_err());
        assert!(!dst.path().join("ledger.yaml").exists());
        assert!(!dst.path().join(".restore").exists());
    }

    #[test]
    fn daily_makes_one_per_day_and_prune_keeps_the_newest() {
        let src = seeded();
        assert!(daily(src.path(), at(8)).unwrap().is_some());
        assert!(daily(src.path(), at(9)).unwrap().is_none());
        for h in 10..14 {
            create(src.path(), at(h)).unwrap();
        }
        prune(src.path(), 2);
        let names: Vec<String> = list(src.path()).into_iter().map(|b| b.name).collect();
        assert_eq!(names, vec!["worthwatch-2026-11-03-130000.tar.gz", "worthwatch-2026-11-03-120000.tar.gz"]);
    }

    #[test]
    fn a_burst_of_backups_today_never_prunes_earlier_days() {
        let src = seeded();
        for day in 1..=3 {
            create(src.path(), Local.with_ymd_and_hms(2026, 11, day, 9, 0, 0).unwrap()).unwrap();
        }
        for m in 0..40 {
            create(src.path(), Local.with_ymd_and_hms(2026, 11, 4, 10, m, 0).unwrap()).unwrap();
        }
        let dates: Vec<NaiveDate> = list(src.path()).into_iter().map(|b| b.date).collect();
        for day in 1..=3 {
            assert!(dates.contains(&NaiveDate::from_ymd_opt(2026, 11, day).unwrap()), "lost Nov {day}");
        }
        assert_eq!(dates.len(), KEEP + 3);
    }

    #[test]
    fn names_cannot_escape_the_folder() {
        assert!(valid_name("worthwatch-2026-11-03-130000.tar.gz"));
        assert!(!valid_name("../ledger.yaml"));
        assert!(!valid_name("worthwatch-../../x.tar.gz"));
        assert!(!safe_entry(Path::new("/etc/passwd")));
        assert!(!safe_entry(Path::new("holdings/../../x")));
        assert!(!safe_entry(Path::new("other.txt")));
        assert!(safe_entry(Path::new("holdings/XEQT.TO/2026-10-31.json")));
    }
}
