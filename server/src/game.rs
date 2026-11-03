//! The budget game over HTTP, and the lines the iPhone shows.
//!
//! ```text
//!  GET /api/game?month=2026-11 ─────────────▶ the board (UI)
//!  POST /api/ingest/transaction ─▶ reply.notify ─▶ Shortcut "Show Notification"   (Tailscale)
//!  GET  /api/ingest/day ─────────▶ {text}      ─▶ Worthwatch Day Shortcut          (Tailscale)
//!  watcher ─▶ iCloud Drive/Shortcuts/Worthwatch/game/today.json ─▶ both Shortcuts  (iCloud)
//! ```
//! `game/` is a subfolder so the tap reader, which reads `Worthwatch/*.json*`, never
//! mistakes it for taps.

use std::path::{Path, PathBuf};

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Local, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;

use crate::budget::MonthQuery;
use crate::state::AppState;

pub const STATUS_DIR: &str = "game";
pub const STATUS_FILE: &str = "today.json";

#[derive(Serialize)]
pub struct GameReply {
    pub version: u64,
    pub base_currency: String,
    pub as_of: NaiveDate,
    #[serde(flatten)]
    pub game: budget::GameView,
    /// First month with any transaction, for the month arrows.
    pub first_month: Option<NaiveDate>,
}

/// The game as of `today`, plus the category of a tap when `tap_id` names one.
pub fn build(
    s: &AppState,
    month: Option<&str>,
    today: NaiveDate,
    tap_id: Option<&str>,
) -> (GameReply, Option<String>) {
    let snap = s.snapshot();
    let l = &snap.ledger;
    let market = snap.market();
    let base = l.settings.base_currency.clone();
    let store = s.txns.read().expect("txn lock");
    let txns = store.reconcile(l, today);
    let month = month.and_then(crate::budget::parse_month).unwrap_or(today);
    let rate = |c: &str, d: NaiveDate| market.rate_at(&base, c, d).map(|r| r.rate);
    let game = budget::game::view(l, &txns, month, today, &rate);
    let category = tap_id.and_then(|id| {
        txns.iter().find(|t| t.tap_id.as_deref() == Some(id) || t.id == id).map(|t| t.category.clone())
    });
    let first_month = txns.iter().map(|t| t.date).min().map(budget::month::first_of);
    (GameReply { version: snap.version, base_currency: base, as_of: today, game, first_month }, category)
}

pub async fn get_game(State(s): State<AppState>, Query(q): Query<MonthQuery>) -> Json<GameReply> {
    let today = Local::now().date_naive();
    Json(build(&s, q.month.as_deref(), today, None).0)
}

/// The notification text for the newest of `taps` (what the phone just sent).
pub fn notify_for(s: &AppState, taps: &[budget::Tap], today: NaiveDate) -> Option<String> {
    let tap = taps.iter().max_by_key(|t| t.at)?;
    let (g, category) = build(s, None, today, Some(&tap.id));
    if g.game.allowance <= Decimal::ZERO {
        return None;
    }
    Some(budget::game::tap_line(&g.game, category.as_deref()))
}

/// `GET /api/ingest/day`: the morning recap, for the Worthwatch Day Shortcut.
pub async fn get_day(State(s): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(r) = crate::budget::token_error(&s, &headers) {
        return r;
    }
    let (g, _) = build(&s, None, Local::now().date_naive(), None);
    let text = budget::game::recap_line(&g.game).unwrap_or_else(|| budget::game::left_line(&g.game));
    Json(serde_json::json!({ "text": text })).into_response()
}

#[derive(Serialize, PartialEq)]
struct StatusFile {
    date: NaiveDate,
    /// Shown by the tap Shortcut in iCloud mode, which can't ask the Mac.
    tap: String,
    /// Shown by the morning Shortcut.
    day: String,
    left: Option<Decimal>,
    jar: Decimal,
    goal: Option<Decimal>,
    written: DateTime<Utc>,
}

pub fn status_path(dir: &Path) -> PathBuf {
    dir.join(STATUS_DIR).join(STATUS_FILE)
}

/// Write `game/today.json` beside the taps, for the iCloud Shortcuts. Returns whether
/// it wrote. Skips the write when the game is not set up.
pub fn write_status(s: &AppState, dir: &Path, today: NaiveDate) -> std::io::Result<bool> {
    let (g, _) = build(s, None, today, None);
    if g.game.allowance <= Decimal::ZERO {
        return Ok(false);
    }
    let left = budget::game::left_line(&g.game);
    let file = StatusFile {
        date: today,
        day: budget::game::recap_line(&g.game).unwrap_or_else(|| left.clone()),
        tap: left,
        left: g.game.today.as_ref().map(|t| t.left),
        jar: g.game.jar,
        goal: g.game.goal,
        written: Utc::now(),
    };
    let path = status_path(dir);
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(&file).unwrap_or_default())?;
    std::fs::rename(&tmp, &path)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_file_lands_in_a_subfolder_the_tap_reader_skips() {
        let dir = tempfile::tempdir().unwrap();
        let s = AppState::open(dir.path()).unwrap();
        s.apply_many(vec![ledger::Mutation::SetBudgetLimit {
            category: "dining".into(),
            amount: Some(Decimal::from(300)),
        }])
        .unwrap();
        let feed = dir.path().join("icloud");
        std::fs::create_dir_all(&feed).unwrap();
        assert!(write_status(&s, &feed, chrono::NaiveDate::from_ymd_opt(2026, 11, 18).unwrap()).unwrap());
        let text = std::fs::read_to_string(status_path(&feed)).unwrap();
        assert!(text.contains("\"tap\": \"$10 left today before this tap"), "{text}");
        // The reader only looks at files directly in the folder.
        let mut seen = Default::default();
        let mut st = crate::budget::FeedStatus::default();
        assert_eq!(crate::budget::scan_icloud(&feed, &s.txns, &mut seen, &mut st), 0);
        assert!(st.errors.is_empty(), "{:?}", st.errors);
    }
}
