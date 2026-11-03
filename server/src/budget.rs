//! Budget routes, the Apple Pay tap endpoint and the iCloud Drive reader.
//!
//! ```text
//! iPhone Shortcut ── iCloud mode ──▶ iCloud Drive/Shortcuts/Worthwatch/taps-YYYY-MM.jsonl
//!                                          │ (synced to this Mac; read every 10 s)
//!                 ── Tailscale mode ─▶ POST /api/ingest/transaction  (Bearer token)
//!                                          ▼
//!                             transactions/YYYY-MM.jsonl ◀── POST /api/budget/import (CSV / OFX)
//!                                          ▼
//!                       budget::reconcile ─▶ budget::month ─▶ GET /api/budget
//! ```
//! The ingest route is the only one that takes the token, and the token opens nothing
//! else. Taps never touch `ledger.yaml`: balances and net worth stay as you entered them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use axum::body::Bytes;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, Local, NaiveDate, Utc};
use ledger::{AccountKind, FeedMode, Ledger, TxnSource};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub const TOKEN_FILE: &str = "ingest-token";
const SCAN_EVERY: Duration = Duration::from_secs(10);
const SHORTCUT_NAME: &str = "Worthwatch Tap.shortcut";

// ------------------------------------------------------------------ token

fn token_path(s: &AppState) -> PathBuf {
    s.data_dir().join(TOKEN_FILE)
}

fn new_token() -> String {
    use std::io::Read;
    let mut buf = [0u8; 24];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| f.read_exact(&mut buf)).is_ok();
    if !ok {
        // No /dev/urandom (not macOS or Linux): mix the clock and the process id.
        let seed = format!("{:?}{}", SystemTime::now(), std::process::id());
        let h = budget_fnv(&seed);
        buf[..8].copy_from_slice(&h.to_le_bytes());
        buf[8..16].copy_from_slice(&budget_fnv(&format!("{seed}x")).to_le_bytes());
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

fn budget_fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3))
}

/// The token in `data/ingest-token`, made on first use.
pub fn token(s: &AppState) -> std::io::Result<String> {
    let p = token_path(s);
    match std::fs::read_to_string(&p) {
        Ok(t) if !t.trim().is_empty() => Ok(t.trim().to_string()),
        _ => write_token(&p, &new_token()),
    }
}

fn write_token(p: &Path, t: &str) -> std::io::Result<String> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(p, format!("{t}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(t.to_string())
}

fn same(a: &str, b: &str) -> bool {
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
}

fn unauthorized(message: &str) -> Response {
    (StatusCode::UNAUTHORIZED, Json(serde_json::json!({ "ok": false, "message": message }))).into_response()
}

/// `POST /api/ingest/transaction`: one tap, an array, or a queue file of them.
async fn ingest(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(given) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer ").or_else(|| v.strip_prefix("bearer ")))
    else {
        return unauthorized("missing Authorization: Bearer <token>");
    };
    let Ok(expected) = token(&s) else {
        return (StatusCode::INTERNAL_SERVER_ERROR, "could not read the ingest token").into_response();
    };
    if !same(given.trim(), &expected) {
        return unauthorized("wrong token; copy it again from Settings");
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
    Json(IngestReply { worthwatch_saved: true, added, duplicates: total - added, errors: parsed.errors })
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
                    "macOS blocked reading this folder: give worthwatch Full Disk Access in System Settings › Privacy & Security"
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
                    }
                } else {
                    last_dir = None;
                }
                std::thread::sleep(SCAN_EVERY);
            }
        })
        .expect("spawn icloud watcher");
}

// ------------------------------------------------------------------ budget view

#[derive(Serialize)]
pub struct BudgetView {
    pub version: u64,
    pub as_of: NaiveDate,
    pub base_currency: String,
    #[serde(flatten)]
    pub month: budget::MonthView,
    /// First month anything is recorded, for the month picker.
    pub first_month: Option<NaiveDate>,
    /// Every category you can pick (built-in, budgeted, from rules).
    pub options: Vec<CategoryOption>,
    pub accounts: Vec<AccountFeed>,
    pub settings: ledger::Budget,
    pub feed: FeedSummary,
    pub store_errors: Vec<String>,
}

#[derive(Serialize)]
pub struct CategoryOption {
    pub id: String,
    pub label: String,
}

#[derive(Serialize)]
pub struct AccountFeed {
    pub id: String,
    pub name: String,
    pub kind: AccountKind,
    pub group: ledger::Group,
    pub source: TxnSource,
    /// Wallet cards linked to it.
    pub cards: Vec<String>,
    /// Newest statement row imported for it.
    pub last_statement: Option<NaiveDate>,
    pub statement_rows: usize,
}

#[derive(Serialize)]
pub struct FeedSummary {
    pub mode: FeedMode,
    pub last_tap: Option<DateTime<Utc>>,
    pub taps: usize,
}

#[derive(Deserialize, Default)]
pub struct MonthQuery {
    /// `2026-11` or any date in the month.
    pub month: Option<String>,
}

fn parse_month(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(&format!("{s}-01"), "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
}

pub fn build_budget(s: &AppState, q: &MonthQuery, today: NaiveDate) -> BudgetView {
    let snap = s.snapshot();
    let l = &snap.ledger;
    let market = snap.market();
    let base = l.settings.base_currency.clone();
    let store = s.txns.read().expect("txn lock");
    let txns = budget::reconcile(l, &store.taps, &store.rows, &store.overrides, today);
    let month = q.month.as_deref().and_then(parse_month).unwrap_or(today);
    let rate = |c: &str, d: NaiveDate| market.rate_at(&base, c, d).map(|r| r.rate);
    let view = budget::month(l, &txns, month, today, &rate);

    let mut options: Vec<CategoryOption> = budget::CATEGORIES
        .iter()
        .map(|(id, label)| CategoryOption { id: id.to_string(), label: label.to_string() })
        .collect();
    let extra = l
        .budget
        .limits
        .keys()
        .chain(l.budget.rules.iter().map(|r| &r.category))
        .chain(store.overrides.values());
    for c in extra {
        if !options.iter().any(|o| o.id == *c) {
            options.push(CategoryOption { id: c.clone(), label: budget::label(c) });
        }
    }
    let accounts = l
        .accounts
        .iter()
        .filter(|a| {
            matches!(a.kind.group(), ledger::Group::Cash | ledger::Group::Debt)
                && a.kind != AccountKind::Mortgage
        })
        .map(|a| {
            let rows: Vec<&budget::CsvRow> = store.rows.iter().filter(|r| r.account == a.id).collect();
            AccountFeed {
                id: a.id.clone(),
                name: a.name.clone(),
                kind: a.kind,
                group: a.kind.group(),
                source: l.budget.source(&a.id),
                cards: l.budget.cards.iter().filter(|(_, id)| **id == a.id).map(|(c, _)| c.clone()).collect(),
                last_statement: rows.iter().map(|r| r.date).max(),
                statement_rows: rows.len(),
            }
        })
        .collect();
    let first_month = txns.iter().map(|t| t.date).min().map(budget::month::first_of);
    BudgetView {
        version: snap.version,
        as_of: today,
        base_currency: base,
        month: view,
        first_month,
        options,
        accounts,
        settings: l.budget.clone(),
        feed: FeedSummary { mode: l.budget.feed.mode, last_tap: store.last_tap(), taps: store.taps.len() },
        store_errors: store.errors.clone(),
    }
}

async fn get_budget(State(s): State<AppState>, Query(q): Query<MonthQuery>) -> Json<BudgetView> {
    Json(build_budget(&s, &q, Local::now().date_naive()))
}

// ------------------------------------------------------------------ statements

#[derive(Deserialize)]
pub struct ImportRequest {
    pub account: String,
    pub text: String,
    #[serde(default)]
    pub preset: budget::Preset,
    #[serde(default)]
    pub flip: bool,
}

#[derive(Serialize)]
pub struct ImportPreview {
    pub preset: budget::Preset,
    pub preset_label: String,
    pub presets: Vec<(budget::Preset, String)>,
    pub sign_guessed: bool,
    pub flipped: bool,
    pub rows: Vec<PreviewRow>,
    pub skipped: Vec<(usize, String)>,
    pub new_rows: usize,
    pub duplicates: usize,
    pub money_out: Decimal,
    pub money_in: Decimal,
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
}

#[derive(Serialize)]
pub struct PreviewRow {
    pub date: NaiveDate,
    pub amount: Decimal,
    pub description: String,
    pub category: String,
    pub duplicate: bool,
}

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub message: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ApiError>)>;

fn bad(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ApiError>) {
    (status, Json(ApiError { message: message.into() }))
}

type Prepared = (budget::ParsedStatement, Vec<budget::CsvRow>, Ledger);

fn prepare(s: &AppState, req: &ImportRequest) -> Result<Prepared, (StatusCode, Json<ApiError>)> {
    let l = s.snapshot().ledger;
    if l.account(&req.account).is_none() {
        return Err(bad(StatusCode::UNPROCESSABLE_ENTITY, format!("unknown account `{}`", req.account)));
    }
    let parsed = budget::parse_statement(&req.text, req.preset, req.flip);
    if parsed.rows.is_empty() {
        return Err(bad(
            StatusCode::UNPROCESSABLE_ENTITY,
            "no transactions found. Export CSV or OFX from your bank's website and paste or drop the whole file.",
        ));
    }
    let rows = budget::rows_for_account(&req.account, &parsed.rows, Utc::now());
    Ok((parsed, rows, l))
}

/// Parse a statement against an account. Writes nothing.
async fn import_preview(
    State(s): State<AppState>,
    Json(req): Json<ImportRequest>,
) -> ApiResult<ImportPreview> {
    let (parsed, rows, l) = prepare(&s, &req)?;
    let store = s.txns.read().expect("txn lock");
    let liability = l.account(&req.account).is_some_and(|a| a.kind.is_liability());
    let out: Vec<PreviewRow> = rows
        .iter()
        .map(|r| PreviewRow {
            date: r.date,
            amount: r.amount,
            description: r.description.clone(),
            category: budget::categorize(
                &r.description,
                Some(&req.account),
                r.amount,
                liability,
                &l.budget.rules,
            ),
            duplicate: store.contains(&r.id),
        })
        .collect();
    let duplicates = out.iter().filter(|r| r.duplicate).count();
    Ok(Json(ImportPreview {
        preset: parsed.preset,
        preset_label: parsed.preset.label().to_string(),
        presets: budget::Preset::CHOICES.iter().map(|p| (*p, p.label().to_string())).collect(),
        sign_guessed: parsed.sign_guessed,
        flipped: parsed.flipped,
        new_rows: out.len() - duplicates,
        duplicates,
        money_out: out.iter().filter(|r| r.amount > Decimal::ZERO).map(|r| r.amount).sum(),
        money_in: -out.iter().filter(|r| r.amount < Decimal::ZERO).map(|r| r.amount).sum::<Decimal>(),
        from: out.iter().map(|r| r.date).min(),
        to: out.iter().map(|r| r.date).max(),
        rows: out,
        skipped: parsed.skipped,
    }))
}

#[derive(Serialize)]
pub struct ImportReply {
    pub added: usize,
    pub duplicates: usize,
}

async fn import(State(s): State<AppState>, Json(req): Json<ImportRequest>) -> ApiResult<ImportReply> {
    let (_, rows, _) = prepare(&s, &req)?;
    let total = rows.len();
    let added = s
        .txns
        .write()
        .expect("txn lock")
        .append(rows.into_iter().map(budget::store::Record::Csv).collect())
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if added > 0 {
        s.bump();
    }
    Ok(Json(ImportReply { added, duplicates: total - added }))
}

#[derive(Deserialize)]
pub struct CategoryRequest {
    pub id: String,
    pub category: Option<String>,
}

async fn set_category(
    State(s): State<AppState>,
    Json(req): Json<CategoryRequest>,
) -> ApiResult<serde_json::Value> {
    s.txns
        .write()
        .expect("txn lock")
        .set_category(&req.id, req.category.as_deref().filter(|c| !c.trim().is_empty()))
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    s.bump();
    Ok(Json(serde_json::json!({ "ok": true })))
}

// ------------------------------------------------------------------ settings

#[derive(Serialize)]
pub struct SettingsView {
    pub mode: FeedMode,
    pub token: String,
    pub ingest_path: &'static str,
    /// The folder read in iCloud mode, and what was seen there.
    pub icloud_dir: Option<String>,
    pub icloud: FeedStatus,
    /// Configured, else what `tailscale status` reports for this machine.
    pub tailscale_url: Option<String>,
    pub tailscale_detected: bool,
    /// This machine can sign a Shortcut (macOS with the `shortcuts` tool).
    pub can_sign: bool,
    /// Where a signed Shortcut was saved, if one exists.
    pub shortcut_file: Option<String>,
    pub last_tap: Option<DateTime<Utc>>,
    pub taps: usize,
}

fn shortcuts_tool() -> Option<PathBuf> {
    let p = PathBuf::from("/usr/bin/shortcuts");
    (cfg!(target_os = "macos") && p.exists()).then_some(p)
}

fn detect_tailscale() -> Option<String> {
    for bin in [
        "tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "/opt/homebrew/bin/tailscale",
        "/usr/local/bin/tailscale",
    ] {
        let Ok(out) = std::process::Command::new(bin).args(["status", "--json"]).output() else { continue };
        if !out.status.success() {
            continue;
        }
        let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
        let name = v["Self"]["DNSName"].as_str()?.trim_end_matches('.');
        if !name.is_empty() {
            return Some(format!("https://{name}"));
        }
    }
    None
}

fn shortcut_target(l: &Ledger) -> Option<PathBuf> {
    // Beside the taps in iCloud mode; in Shortcuts' own iCloud folder otherwise.
    let dir = match l.budget.feed.mode {
        FeedMode::Icloud => icloud_dir(l),
        _ => shortcuts_icloud_root().map(|r| r.join(budget::shortcut::FOLDER)),
    }?;
    Some(dir.join(SHORTCUT_NAME))
}

fn settings_view(s: &AppState) -> Result<SettingsView, (StatusCode, Json<ApiError>)> {
    let l = s.snapshot().ledger;
    let token = token(s).map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, format!("ingest token: {e}")))?;
    let configured = l.budget.feed.tailscale_url.clone().filter(|u| !u.trim().is_empty());
    let detected = if configured.is_none() && l.budget.feed.mode == FeedMode::Tailscale {
        detect_tailscale()
    } else {
        None
    };
    let store = s.txns.read().expect("txn lock");
    Ok(SettingsView {
        mode: l.budget.feed.mode,
        token,
        ingest_path: "/api/ingest/transaction",
        icloud_dir: icloud_dir(&l).map(|d| d.display().to_string()),
        icloud: s.feed.read().expect("feed lock").clone(),
        tailscale_detected: detected.is_some(),
        tailscale_url: configured.or(detected),
        can_sign: shortcuts_tool().is_some(),
        shortcut_file: shortcut_target(&l).filter(|p| p.exists()).map(|p| p.display().to_string()),
        last_tap: store.last_tap(),
        taps: store.taps.len(),
    })
}

async fn get_settings(State(s): State<AppState>) -> ApiResult<SettingsView> {
    let st = s.clone();
    tokio::task::spawn_blocking(move || settings_view(&st))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
}

/// A new token. The Shortcut on the phone stops working until it is made again.
async fn rotate_token(State(s): State<AppState>) -> ApiResult<SettingsView> {
    write_token(&token_path(&s), &new_token())
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    get_settings(State(s)).await
}

fn plist_for(s: &AppState) -> Result<String, (StatusCode, Json<ApiError>)> {
    let l = s.snapshot().ledger;
    match l.budget.feed.mode {
        FeedMode::Off => Err(bad(StatusCode::CONFLICT, "pick iCloud Drive or Tailscale first")),
        FeedMode::Icloud => Ok(budget::shortcut::plist(budget::shortcut::Transport::Icloud)),
        FeedMode::Tailscale => {
            let view = settings_view(s)?;
            let Some(url) = view.tailscale_url else {
                return Err(bad(
                    StatusCode::CONFLICT,
                    "enter this Mac's Tailscale address first (https://<name>.<tailnet>.ts.net)",
                ));
            };
            Ok(budget::shortcut::plist(budget::shortcut::Transport::Tailscale {
                url: &url,
                token: &view.token,
            }))
        }
    }
}

/// The unsigned Shortcut, for signing by hand (`shortcuts sign`).
async fn shortcut_plist(State(s): State<AppState>) -> Response {
    let st = s.clone();
    match tokio::task::spawn_blocking(move || plist_for(&st)).await {
        Ok(Ok(xml)) => (
            [
                (header::CONTENT_TYPE, "application/x-plist".to_string()),
                (
                    header::CONTENT_DISPOSITION,
                    "attachment; filename=\"Worthwatch Tap.unsigned.plist\"".to_string(),
                ),
            ],
            xml,
        )
            .into_response(),
        Ok(Err(e)) => e.into_response(),
        Err(e) => bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Build, sign (macOS) and save the Shortcut into iCloud Drive, where the phone can open it.
async fn make_shortcut(State(s): State<AppState>) -> ApiResult<SettingsView> {
    let st = s.clone();
    tokio::task::spawn_blocking(move || -> Result<(), (StatusCode, Json<ApiError>)> {
        let xml = plist_for(&st)?;
        let tool = shortcuts_tool().ok_or_else(|| {
            bad(
                StatusCode::CONFLICT,
                "signing a Shortcut needs macOS. Download the unsigned file and sign it on a Mac.",
            )
        })?;
        let target = shortcut_target(&st.snapshot().ledger).ok_or_else(|| {
            bad(StatusCode::CONFLICT, "can't find iCloud Drive's Shortcuts folder on this Mac")
        })?;
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        }
        let tmp = std::env::temp_dir().join(format!("worthwatch-{}.plist", std::process::id()));
        std::fs::write(&tmp, xml).map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let out = std::process::Command::new(tool)
            .args(["sign", "--mode", "anyone", "--input"])
            .arg(&tmp)
            .arg("--output")
            .arg(&target)
            .output();
        let _ = std::fs::remove_file(&tmp);
        match out {
            Ok(o) if o.status.success() => Ok(()),
            Ok(o) => Err(bad(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("shortcuts sign failed: {}", String::from_utf8_lossy(&o.stderr).trim()),
            )),
            Err(e) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, format!("could not run shortcuts: {e}"))),
        }
    })
    .await
    .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    get_settings(State(s)).await
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/ingest/transaction", post(ingest))
        .route("/api/budget", get(get_budget))
        .route("/api/budget/import/preview", post(import_preview))
        .route("/api/budget/import", post(import))
        .route("/api/budget/category", post(set_category))
        .route("/api/settings", get(get_settings))
        .route("/api/settings/token", post(rotate_token))
        .route("/api/settings/shortcut", post(make_shortcut))
        .route("/api/settings/shortcut.plist", get(shortcut_plist))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use ledger::{Account, Feed, Mutation};
    use tower::ServiceExt;

    fn card(id: &str) -> Account {
        Account {
            id: id.into(),
            name: "Amex Cobalt".into(),
            institution: None,
            kind: AccountKind::CreditCard,
            currency: "CAD".into(),
            tags: vec![],
            positions: None,
        }
    }

    fn app() -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let s = AppState::open(dir.path()).unwrap();
        s.apply_many(vec![
            Mutation::UpsertAccount { account: card("amex") },
            Mutation::LinkCard { card: "Amex Cobalt".into(), account: Some("amex".into()) },
            Mutation::SetFeed { feed: Feed { mode: FeedMode::Tailscale, ..Default::default() } },
        ])
        .unwrap();
        (dir, s)
    }

    async fn post(s: &AppState, auth: Option<&str>, body: &str) -> (StatusCode, serde_json::Value) {
        let mut req = Request::post("/api/ingest/transaction");
        if let Some(a) = auth {
            req = req.header("authorization", a);
        }
        let res = router(s.clone()).oneshot(req.body(Body::from(body.to_string())).unwrap()).await.unwrap();
        let status = res.status();
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
    }

    const TAP: &str = r#"{"merchant":"Blue Bottle","amount":"$7.25","card":"Amex Cobalt","at":"2026-11-03T08:12:45.120-05:00"}"#;

    #[tokio::test]
    async fn no_token_or_a_wrong_one_is_401_and_writes_nothing() {
        let (dir, s) = app();
        let (st, _) = post(&s, None, TAP).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        let (st, _) = post(&s, Some("Bearer nope"), TAP).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        assert!(s.txns.read().unwrap().taps.is_empty());
        assert!(!dir.path().join("transactions").exists());
    }

    #[tokio::test]
    async fn a_tap_shows_in_the_budget_at_once_and_a_retry_is_a_no_op() {
        let (_dir, s) = app();
        let t = token(&s).unwrap();
        assert_eq!(t.len(), 48);
        let before = s.snapshot().version;
        let (st, body) = post(&s, Some(&format!("Bearer {t}")), TAP).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(body["worthwatch_saved"], true);
        assert_eq!(body["added"], 1);
        assert!(s.snapshot().version > before, "open pages hear about it");
        let today = NaiveDate::from_ymd_opt(2026, 11, 3).unwrap();
        let v = build_budget(&s, &MonthQuery::default(), today);
        assert_eq!(v.month.txns.len(), 1);
        assert_eq!(v.month.txns[0].txn.status, budget::Status::Provisional);
        assert_eq!(v.month.provisional, Decimal::new(725, 2));

        // The phone's queue: the same tap again plus one made while the Mac was offline.
        let later = r#"{"merchant":"Loblaws","amount":"$52.18","card":"Amex Cobalt","at":"2026-11-03T12:00:00.000-05:00"}"#;
        let (_, body) = post(&s, Some(&format!("Bearer {t}")), &format!("{TAP}\n{later}\n")).await;
        assert_eq!((body["added"].as_u64(), body["duplicates"].as_u64()), (Some(1), Some(1)));
        assert_eq!(s.txns.read().unwrap().taps.len(), 2, "each tap exactly once");
    }

    #[tokio::test]
    async fn statement_import_confirms_the_tap_and_reimport_changes_nothing() {
        let (_dir, s) = app();
        let t = token(&s).unwrap();
        post(&s, Some(&format!("Bearer {t}")), TAP).await;
        let csv = "Date,Date Processed,Description,Cardmember,Amount\n04 Nov. 2026,05 Nov. 2026,SQ *BLUE BOTTLE TORONTO ON,ZAI A,7.25\n04 Nov. 2026,05 Nov. 2026,SHOPPERS DRUG MART,ZAI A,20.00\n";
        let req = ImportRequest {
            account: "amex".into(),
            text: csv.into(),
            preset: budget::Preset::Auto,
            flip: false,
        };
        let Json(p) = import_preview(State(s.clone()), Json(clone_req(&req))).await.unwrap();
        assert_eq!((p.preset, p.new_rows, p.duplicates), (budget::Preset::Amex, 2, 0));
        let Json(r) = import(State(s.clone()), Json(clone_req(&req))).await.unwrap();
        assert_eq!(r.added, 2);
        let today = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let v = build_budget(&s, &MonthQuery { month: Some("2026-11".into()) }, today);
        assert_eq!(v.month.txns.len(), 2, "the tap merged into its row");
        assert!(v.month.txns.iter().any(|m| m.txn.status == budget::Status::Confirmed));
        assert_eq!(v.month.provisional, Decimal::ZERO);
        let Json(r) = import(State(s.clone()), Json(clone_req(&req))).await.unwrap();
        assert_eq!((r.added, r.duplicates), (0, 2));
        assert_eq!(v.accounts[0].last_statement, NaiveDate::from_ymd_opt(2026, 11, 4));
    }

    fn clone_req(r: &ImportRequest) -> ImportRequest {
        ImportRequest { account: r.account.clone(), text: r.text.clone(), preset: r.preset, flip: r.flip }
    }

    #[test]
    fn icloud_files_are_read_once_and_stubs_reported() {
        let (dir, s) = app();
        let icloud = dir.path().join("icloud");
        std::fs::create_dir_all(&icloud).unwrap();
        std::fs::write(icloud.join("taps-2026-11.jsonl"), format!("{TAP}\n")).unwrap();
        std::fs::write(icloud.join(".taps-2026-10.jsonl.icloud"), "").unwrap();
        let mut seen = BTreeMap::new();
        let mut status = FeedStatus::default();
        assert_eq!(scan_icloud(&icloud, &s.txns, &mut seen, &mut status), 1);
        assert!(status.found);
        assert_eq!(status.not_downloaded, vec!["taps-2026-10.jsonl".to_string()]);
        assert_eq!(scan_icloud(&icloud, &s.txns, &mut seen, &mut status), 0, "unchanged file skipped");
        // The phone appends a second tap; the whole file is re-read, only the new one is added.
        let later = TAP.replace("08:12:45.120", "09:00:00.000");
        std::fs::write(icloud.join("taps-2026-11.jsonl"), format!("{TAP}\n{later}\n")).unwrap();
        seen.clear();
        assert_eq!(scan_icloud(&icloud, &s.txns, &mut seen, &mut status), 1);
        assert_eq!(s.txns.read().unwrap().taps.len(), 2);
        let mut missing = FeedStatus::default();
        assert_eq!(scan_icloud(&dir.path().join("nope"), &s.txns, &mut seen, &mut missing), 0);
        assert!(!missing.found);
    }

    #[test]
    fn shortcut_needs_a_mode_and_a_tailscale_address() {
        let (_dir, s) = app();
        assert!(plist_for(&s).is_err() || detect_tailscale().is_some());
        s.apply_many(vec![Mutation::SetFeed {
            feed: Feed {
                mode: FeedMode::Tailscale,
                tailscale_url: Some("https://mini.tail1.ts.net".into()),
                icloud_dir: None,
            },
        }])
        .unwrap();
        let xml = plist_for(&s).unwrap();
        assert!(xml.contains("https://mini.tail1.ts.net/api/ingest/transaction"));
        assert!(xml.contains(&token(&s).unwrap()));
        s.apply_many(vec![Mutation::SetFeed { feed: Feed::default() }]).unwrap();
        assert!(plist_for(&s).is_err());
    }
}
