//! The Settings screen: feed mode and status, the ingest token (rotate, reveal), and the
//! two iPhone Shortcuts (build, sign, save; or the unsigned plist).

use std::path::PathBuf;

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{DateTime, Utc};
use ledger::{FeedMode, Ledger};
use serde::{Deserialize, Serialize};

use super::feed::{icloud_dir, new_token, shortcuts_icloud_root, token, token_path, write_token, FeedStatus};
use crate::api::{bad, ApiError, ApiResult};
use crate::state::AppState;

const SHORTCUT_NAME: &str = "Worthwatch Tap.shortcut";
const DAY_SHORTCUT_NAME: &str = "Worthwatch Day.shortcut";

#[derive(Serialize)]
pub struct SettingsView {
    pub mode: FeedMode,
    /// The token's last 4 characters. The whole token comes only from
    /// `POST /api/settings/token/reveal`, so a page that can merely read can't take it.
    pub token_hint: String,
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
    /// The same for the morning recap Shortcut.
    pub day_shortcut_file: Option<String>,
    pub last_tap: Option<DateTime<Utc>>,
    pub taps: usize,
}

fn shortcuts_tool() -> Option<PathBuf> {
    let p = PathBuf::from("/usr/bin/shortcuts");
    (cfg!(target_os = "macos") && p.exists()).then_some(p)
}

pub(super) fn detect_tailscale() -> Option<String> {
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

fn shortcut_target(l: &Ledger, which: Which) -> Option<PathBuf> {
    // Beside the taps in iCloud mode; in Shortcuts' own iCloud folder otherwise.
    let dir = match l.budget.feed.mode {
        FeedMode::Icloud => icloud_dir(l),
        _ => shortcuts_icloud_root().map(|r| r.join(budget::shortcut::FOLDER)),
    }?;
    Some(dir.join(match which {
        Which::Tap => SHORTCUT_NAME,
        Which::Day => DAY_SHORTCUT_NAME,
    }))
}

/// The tap Shortcut, or the morning recap one.
#[derive(Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Which {
    #[default]
    Tap,
    Day,
}

#[derive(Deserialize, Default)]
pub struct WhichQuery {
    #[serde(default)]
    pub which: Which,
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
        token_hint: token.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect(),
        ingest_path: "/api/ingest/transaction",
        icloud_dir: icloud_dir(&l).map(|d| d.display().to_string()),
        icloud: s.feed.read().expect("feed lock").clone(),
        tailscale_detected: detected.is_some(),
        tailscale_url: configured.or(detected),
        can_sign: shortcuts_tool().is_some(),
        shortcut_file: shortcut_target(&l, Which::Tap)
            .filter(|p| p.exists())
            .map(|p| p.display().to_string()),
        day_shortcut_file: shortcut_target(&l, Which::Day)
            .filter(|p| p.exists())
            .map(|p| p.display().to_string()),
        last_tap: store.last_tap(),
        taps: store.taps.len(),
    })
}

pub async fn get_settings(State(s): State<AppState>) -> ApiResult<SettingsView> {
    let st = s.clone();
    tokio::task::spawn_blocking(move || settings_view(&st))
        .await
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .map(Json)
}

/// A new token. The Shortcut on the phone stops working until it is made again.
pub async fn rotate_token(State(s): State<AppState>) -> ApiResult<SettingsView> {
    new_token()
        .and_then(|t| write_token(&token_path(&s), &t))
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    get_settings(State(s)).await
}

#[derive(Serialize)]
pub struct TokenReveal {
    pub token: String,
}

/// The whole token, for setting the Shortcut up by hand. A POST, so only the app itself
/// (same origin) can ask for it.
pub async fn reveal_token(State(s): State<AppState>) -> ApiResult<TokenReveal> {
    token(&s)
        .map(|token| Json(TokenReveal { token }))
        .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, format!("ingest token: {e}")))
}

pub(super) fn plist_for(s: &AppState, which: Which) -> Result<String, (StatusCode, Json<ApiError>)> {
    let l = s.snapshot().ledger;
    let make = |t| match which {
        Which::Tap => budget::shortcut::plist(t),
        Which::Day => budget::shortcut::day_plist(t),
    };
    match l.budget.feed.mode {
        FeedMode::Off => Err(bad(StatusCode::CONFLICT, "pick iCloud Drive or Tailscale first")),
        FeedMode::Icloud => Ok(make(budget::shortcut::Transport::Icloud)),
        FeedMode::Tailscale => {
            let view = settings_view(s)?;
            let token =
                token(s).map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, format!("ingest token: {e}")))?;
            let Some(url) = view.tailscale_url else {
                return Err(bad(
                    StatusCode::CONFLICT,
                    "enter this Mac's Tailscale address first (https://<name>.<tailnet>.ts.net)",
                ));
            };
            Ok(make(budget::shortcut::Transport::Tailscale { url: &url, token: &token }))
        }
    }
}

/// The unsigned Shortcut, for signing by hand (`shortcuts sign`).
pub async fn shortcut_plist(State(s): State<AppState>, Query(q): Query<WhichQuery>) -> Response {
    let st = s.clone();
    let name = if q.which == Which::Day { "Worthwatch Day" } else { "Worthwatch Tap" };
    match tokio::task::spawn_blocking(move || plist_for(&st, q.which)).await {
        Ok(Ok(xml)) => (
            [
                (header::CONTENT_TYPE, "application/x-plist".to_string()),
                (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}.unsigned.plist\"")),
            ],
            xml,
        )
            .into_response(),
        Ok(Err(e)) => e.into_response(),
        Err(e) => bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

/// Build, sign (macOS) and save both Shortcuts into iCloud Drive, where the phone can
/// open them: the tap one and the morning recap.
pub async fn make_shortcut(State(s): State<AppState>) -> ApiResult<SettingsView> {
    let st = s.clone();
    tokio::task::spawn_blocking(move || -> Result<(), (StatusCode, Json<ApiError>)> {
        for which in [Which::Tap, Which::Day] {
            sign_and_save(&st, which)?;
        }
        Ok(())
    })
    .await
    .map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))??;
    get_settings(State(s)).await
}

fn sign_and_save(st: &AppState, which: Which) -> Result<(), (StatusCode, Json<ApiError>)> {
    let xml = plist_for(st, which)?;
    let tool = shortcuts_tool().ok_or_else(|| {
        bad(
            StatusCode::CONFLICT,
            "signing a Shortcut needs macOS. Download the unsigned file and sign it on a Mac.",
        )
    })?;
    let target = shortcut_target(&st.snapshot().ledger, which)
        .ok_or_else(|| bad(StatusCode::CONFLICT, "can't find iCloud Drive's Shortcuts folder on this Mac"))?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir).map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    let tmp = std::env::temp_dir().join(format!("worthwatch-{}.plist", std::process::id()));
    std::fs::write(&tmp, xml).map_err(|e| bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let out = std::process::Command::new(tool)
        // Only people who know you (your own iPhone) can import it: it carries the token.
        .args(["sign", "--mode", "people-who-know-me", "--input"])
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
}
