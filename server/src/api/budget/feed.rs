//! The Apple Pay feed: the ingest token, `POST /api/ingest/transaction` (Tailscale mode)
//! and the iCloud Drive reader (iCloud mode). Taps land in `transactions/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Local, NaiveDate, Utc};
use ledger::{FeedMode, Ledger};
use serde::Serialize;

use crate::state::AppState;

pub const TOKEN_FILE: &str = "ingest-token";
const SCAN_EVERY: Duration = Duration::from_secs(10);

// ------------------------------------------------------------------ token

pub(super) fn token_path(s: &AppState) -> PathBuf {
    s.data_dir().join(TOKEN_FILE)
}

/// 24 random bytes from the operating system, as 48 hex characters. Never falls back to
/// anything guessable: no randomness, no token.
pub(super) fn new_token() -> std::io::Result<String> {
    let mut buf = [0u8; 24];
    getrandom::fill(&mut buf).map_err(|e| std::io::Error::other(format!("no system randomness: {e}")))?;
    Ok(buf.iter().map(|b| format!("{b:02x}")).collect())
}

/// The token in `data/ingest-token`, made on first use.
pub fn token(s: &AppState) -> std::io::Result<String> {
    let p = token_path(s);
    match std::fs::read_to_string(&p) {
        Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
        _ => write_token(&p, &new_token()?),
    }
}

pub(super) fn write_token(p: &Path, t: &str) -> std::io::Result<String> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // Created private, and made private if an older version left it readable.
        opts.mode(0o600);
        if p.exists() {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    std::io::Write::write_all(&mut opts.open(p)?, format!("{t}\n").as_bytes())?;
    Ok(t.to_string())
}

pub(super) fn same(a: &str, b: &str) -> bool {
    // Length leaks nothing useful (tokens are fixed length); compare every byte.
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

// ------------------------------------------------------------------ ingest

#[derive(Serialize)]
pub struct IngestReply {
    /// The Shortcut deletes its queue only when it sees this.
    pub worthwatch_saved: bool,
    pub added: usize,
    pub duplicates: usize,
    pub errors: Vec<String>,
    /// What the Shortcut shows as a notification: what's left today, the jar. Empty
    /// when the game has no budget to work from.
    pub notify: String,
}

fn unauthorized(message: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "ok": false, "message": message }))).into_response()
}

/// The Bearer token check shared by the two routes the phone calls: `Some` is the
/// error to answer with.
pub fn token_error(s: &AppState, headers: &HeaderMap) -> Option<Response> {
    let Some(given) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
    else {
        return Some(unauthorized("missing Authorization: Bearer <token>"));
    };
    let Ok(expected) = token(s) else {
        return Some((StatusCode::INTERNAL_SERVER_ERROR, "could not read the ingest token").into_response());
    };
    if !same(given.trim(), &expected) {
        return Some(unauthorized("wrong token; copy it again from Settings"));
    }
    None
}

/// `POST /api/ingest/transaction`: one tap, an array, or a queue file of them.
pub async fn ingest(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    if let Some(r) = token_error(&s, &headers) {
        return r;
    }
    if s.snapshot().ledger.budget.feed.mode == FeedMode::Off {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "ok": false, "message": "the Apple Pay feed is off in Settings" })),
        )
            .into_response();
    }
    let text = String::from_utf8_lossy(&body);
    let parsed = budget::parse_taps(&text, Utc::now());
    let total = parsed.taps.len();
    let taps = parsed.taps.clone();
    let added = {
        let mut store = s.txns.write().expect("txn lock");
        match store.append(parsed.taps.into_iter().map(budget::store::Record::Tap).collect()) {
            Ok(n) => n,
            Err(e) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, format!("could not save: {e}")).into_response()
            }
        }
    };
    if added > 0 {
        tracing::info!("apple pay: {added} new tap(s) over the network");
        s.bump();
    }
    let notify = super::game::notify_for(&s, &taps, Local::now().date_naive()).unwrap_or_default();
    Json(IngestReply {
        worthwatch_saved: true,
        added,
        duplicates: total - added,
        errors: parsed.errors,
        notify,
    })
    .into_response()
}

// ------------------------------------------------------------------ iCloud reader

#[derive(Debug, Clone, Default, Serialize)]
pub struct FeedStatus {
    pub dir: Option<String>,
    pub found: bool,
    pub files: usize,
    /// Files iCloud has listed but not downloaded to this Mac yet.
    pub not_downloaded: Vec<String>,
    pub last_scan: Option<DateTime<Utc>>,
    pub last_added: Option<DateTime<Utc>>,
    pub errors: Vec<String>,
}

/// Shortcuts' own iCloud Drive folder on this Mac (where "iCloud Drive/Shortcuts" lives).
pub fn shortcuts_icloud_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join("Library/Mobile Documents/iCloud~is~workflow~my~workflows/Documents"))
}

pub fn icloud_dir(l: &Ledger) -> Option<PathBuf> {
    // In a container: the one taps folder, mounted read-only (see docs/run/security.md).
    if let Some(d) = std::env::var_os("WORTHWATCH_TAPS_DIR").filter(|d| !d.is_empty()) {
        return Some(PathBuf::from(d));
    }
    match &l.budget.feed.icloud_dir {
        Some(d) if !d.trim().is_empty() => {
            let d = d.trim();
            Some(match (d.strip_prefix("~/"), std::env::var_os("HOME")) {
                (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
                _ => PathBuf::from(d),
            })
        }
        _ => shortcuts_icloud_root().map(|r| r.join(budget::shortcut::FOLDER)),
    }
}

/// Read every `*.jsonl` in `dir` that changed since `seen`, store new taps. Returns how
/// many were new. Pure but for the file reads and the store append.
pub fn scan_icloud(
    dir: &Path,
    store: &std::sync::RwLock<budget::TxnStore>,
    seen: &mut BTreeMap<PathBuf, (u64, SystemTime)>,
    status: &mut FeedStatus,
) -> usize {
    status.dir = Some(dir.display().to_string());
    status.last_scan = Some(Utc::now());
    status.errors.clear();
    status.not_downloaded.clear();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            status.found = false;
            status.files = 0;
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                status.errors.push(
                    "macOS blocked reading this folder. Use the Tailscale feed instead: Full Disk Access would also open your Mail, Messages and the rest of iCloud Drive to Worthwatch"
                        .into(),
                );
            }
            return 0;
        }
    };
    status.found = true;
    let mut files = 0;
    let mut added = 0;
    for e in entries.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        // iCloud keeps a stub named `.taps-2026-11.jsonl.icloud` until it downloads the file.
        if name.starts_with('.') && name.ends_with(".icloud") {
            status.not_downloaded.push(name.trim_start_matches('.').trim_end_matches(".icloud").to_string());
            continue;
        }
        if name.starts_with('.')
            || !(name.ends_with(".jsonl") || name.ends_with(".json") || name.ends_with(".txt"))
        {
            continue;
        }
        files += 1;
        let Ok(meta) = e.metadata() else { continue };
        let stamp = (meta.len(), meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        if seen.get(&p) == Some(&stamp) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            status.errors.push(format!("{name}: could not read"));
            continue;
        };
        let parsed = budget::parse_taps(&text, Utc::now());
        status.errors.extend(parsed.errors.iter().map(|e| format!("{name}: {e}")));
        let mut st = store.write().expect("txn lock");
        match st.append(parsed.taps.into_iter().map(budget::store::Record::Tap).collect()) {
            Ok(n) => {
                added += n;
                seen.insert(p, stamp);
            }
            Err(e) => status.errors.push(format!("{name}: could not save ({e})")),
        }
    }
    status.files = files;
    if added > 0 {
        status.last_added = Some(Utc::now());
    }
    added
}

/// Watch the iCloud folder while Settings says iCloud. Local files only: no network.
pub fn watch_icloud(s: &AppState) {
    let state = s.clone();
    std::thread::Builder::new()
        .name("icloud-taps".into())
        .spawn(move || {
            let mut seen = BTreeMap::new();
            let mut last_dir: Option<PathBuf> = None;
            let mut status_key: Option<(u64, NaiveDate, PathBuf)> = None;
            loop {
                let l = state.snapshot().ledger;
                if l.budget.feed.mode == FeedMode::Icloud {
                    if let Some(dir) = icloud_dir(&l) {
                        if last_dir.as_ref() != Some(&dir) {
                            seen.clear();
                            last_dir = Some(dir.clone());
                        }
                        let mut status = state.feed.read().expect("feed lock").clone();
                        let added = scan_icloud(&dir, &state.txns, &mut seen, &mut status);
                        *state.feed.write().expect("feed lock") = status;
                        if added > 0 {
                            tracing::info!("apple pay: {added} new tap(s) from iCloud Drive");
                            state.bump();
                        }
                        // Keep game/today.json current for the phone: on any change, and
                        // when the day turns over.
                        let key = (state.snapshot().version, Local::now().date_naive(), dir.clone());
                        if status_key.as_ref() != Some(&key) && dir.is_dir() {
                            match super::game::write_status(&state, &dir, key.1) {
                                Ok(_) => status_key = Some(key),
                                Err(e) => tracing::warn!("game status file: {e}"),
                            }
                        }
                    }
                } else {
                    last_dir = None;
                }
                std::thread::sleep(SCAN_EVERY);
            }
        })
        .expect("spawn icloud watcher");
}
