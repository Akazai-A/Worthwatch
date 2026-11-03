//! The Budget screen: the month view, statement import and spends added by hand.
//! The Apple Pay feed (token, tap endpoint, iCloud reader) is in [`feed`], the Settings
//! routes in [`settings`] and the game in [`game`].
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

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::{DateTime, Local, NaiveDate, Utc};
use ledger::{AccountKind, FeedMode, Ledger, TxnSource};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::api::{bad, ApiError, ApiResult};
use crate::state::AppState;

pub mod feed;
pub mod game;
pub mod settings;
#[cfg(test)]
mod tests;

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

pub fn parse_month(s: &str) -> Option<NaiveDate> {
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
    let txns = store.reconcile(l, today);
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

pub async fn get_budget(State(s): State<AppState>, Query(q): Query<MonthQuery>) -> Json<BudgetView> {
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
pub async fn import_preview(
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

pub async fn import(State(s): State<AppState>, Json(req): Json<ImportRequest>) -> ApiResult<ImportReply> {
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

pub async fn set_category(
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

// ------------------------------------------------------------------ added by hand

/// Check and store purchases added by hand (the + Add form, Play, or an approved chat
/// card). All or nothing: one bad entry stores none. Returns the new ids.
pub fn add_spends(
    s: &AppState,
    spends: &[assistant::Spend],
    now: chrono::DateTime<chrono::Utc>,
) -> Result<Vec<String>, String> {
    if spends.is_empty() {
        return Err("nothing to add".into());
    }
    let snap = s.snapshot();
    let l = &snap.ledger;
    let tomorrow = Local::now().date_naive() + chrono::Days::new(1);
    let mut records = Vec::with_capacity(spends.len());
    for (i, sp) in spends.iter().enumerate() {
        let merchant = sp.merchant.trim();
        if merchant.is_empty() || merchant.chars().count() > 120 {
            return Err("say what it was, in up to 120 characters".into());
        }
        if sp.amount.is_zero() || sp.amount.abs() > Decimal::from(1_000_000) {
            return Err(format!("{merchant}: the amount must be more than 0"));
        }
        if sp.date > tomorrow {
            return Err(format!("{merchant}: {} is in the future", sp.date));
        }
        if let Some(a) = sp.account.as_deref() {
            match l.account(a) {
                Some(acct) if acct.kind.pays() => {}
                Some(acct) => return Err(format!("{} isn't something you pay with", acct.name)),
                None => return Err(format!("no account \"{a}\"")),
            }
        }
        let category = sp.category.as_deref().map(str::trim).filter(|c| !c.is_empty()).map(str::to_string);
        records.push(budget::store::Record::Manual(budget::Manual::new(
            sp.date,
            merchant,
            sp.amount.round_dp(2),
            sp.account.clone(),
            category,
            now,
            i,
        )));
    }
    let ids: Vec<String> = records.iter().map(|r| r.id().to_string()).collect();
    s.txns.write().expect("txn lock").append(records).map_err(|e| e.to_string())?;
    s.bump();
    Ok(ids)
}

#[derive(Serialize)]
pub struct SpendReply {
    pub added: Vec<String>,
}

pub async fn add_spend(
    State(s): State<AppState>,
    Json(spends): Json<Vec<assistant::Spend>>,
) -> ApiResult<SpendReply> {
    add_spends(&s, &spends, chrono::Utc::now())
        .map(|added| Json(SpendReply { added }))
        .map_err(|e| bad(StatusCode::UNPROCESSABLE_ENTITY, e))
}

#[derive(Deserialize)]
pub struct RemoveRequest {
    pub id: String,
}

pub async fn remove_spend(
    State(s): State<AppState>,
    Json(req): Json<RemoveRequest>,
) -> ApiResult<serde_json::Value> {
    let removed = s
        .txns
        .write()
        .expect("txn lock")
        .remove_manual(&req.id, chrono::Utc::now())
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    if !removed {
        return Err(bad(StatusCode::NOT_FOUND, format!("no entry added by hand with id {}", req.id)));
    }
    s.bump();
    Ok(Json(serde_json::json!({ "ok": true })))
}
