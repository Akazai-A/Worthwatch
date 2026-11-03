//! Budget routes end to end through the router: taps, statements, spends, settings.

use super::feed::*;
use super::settings::*;
use super::*;
use crate::api::router;
use axum::body::Body;
use axum::http::Request;
use ledger::{Account, Feed, Mutation};
use std::collections::BTreeMap;
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

pub async fn post(s: &AppState, auth: Option<&str>, body: &str) -> (StatusCode, serde_json::Value) {
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
pub async fn the_reply_carries_a_notification_once_there_is_a_budget() {
    let (_dir, s) = app();
    let tok = format!("Bearer {}", token(&s).unwrap());
    let (_, body) = post(&s, Some(&tok), TAP).await;
    assert_eq!(body["notify"], "", "no budget, no game line");
    s.apply_many(vec![Mutation::SetBudgetLimit {
        category: "coffee".into(),
        amount: Some(Decimal::from(90)),
    }])
    .unwrap();
    let tap2 = TAP.replace("08:12:45", "09:00:00").replace("7.25", "4.10");
    let (st, body) = post(&s, Some(&tok), &tap2).await;
    assert_eq!(st, StatusCode::OK);
    let line = body["notify"].as_str().unwrap();
    assert!(line.starts_with('$') && line.contains("today") && line.contains("\nJar $"), "{line}");
    // The morning Shortcut's route takes the same token.
    let req = Request::get("/api/ingest/day").header("authorization", &tok).body(Body::empty()).unwrap();
    let res = router(s.clone()).oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let req = Request::get("/api/ingest/day").body(Body::empty()).unwrap();
    assert_eq!(router(s.clone()).oneshot(req).await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
pub async fn no_token_or_a_wrong_one_is_401_and_writes_nothing() {
    let (dir, s) = app();
    let (st, _) = post(&s, None, TAP).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    let (st, _) = post(&s, Some("Bearer nope"), TAP).await;
    assert_eq!(st, StatusCode::UNAUTHORIZED);
    assert!(s.txns.read().unwrap().taps.is_empty());
    assert!(!dir.path().join("transactions").exists());
}

#[tokio::test]
pub async fn a_tap_shows_in_the_budget_at_once_and_a_retry_is_a_no_op() {
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
pub async fn statement_import_confirms_the_tap_and_reimport_changes_nothing() {
    let (_dir, s) = app();
    let t = token(&s).unwrap();
    post(&s, Some(&format!("Bearer {t}")), TAP).await;
    let csv = "Date,Date Processed,Description,Cardmember,Amount\n04 Nov. 2026,05 Nov. 2026,SQ *BLUE BOTTLE TORONTO ON,ZAI A,7.25\n04 Nov. 2026,05 Nov. 2026,SHOPPERS DRUG MART,ZAI A,20.00\n";
    let req =
        ImportRequest { account: "amex".into(), text: csv.into(), preset: budget::Preset::Auto, flip: false };
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

pub async fn send(s: &AppState, path: &str, body: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let req =
        Request::post(path).header("content-type", "application/json").body(Body::from(body.to_string()));
    let res = router(s.clone()).oneshot(req.unwrap()).await.unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

#[tokio::test]
pub async fn a_cash_spend_counts_at_once_and_removing_it_takes_it_back_out() {
    let (_dir, s) = app();
    let today = Local::now().date_naive();
    let before = s.snapshot().version;
    let (st, body) = send(
        &s,
        "/api/budget/spend",
        serde_json::json!([{ "date": today, "merchant": "Farmers market", "amount": "18.50", "category": "Groceries" }]),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{body}");
    let id = body["added"][0].as_str().unwrap().to_string();
    assert!(id.starts_with("man-"));
    assert!(s.snapshot().version > before, "open pages hear about it");

    let v = build_budget(&s, &MonthQuery::default(), today);
    let t = &v.month.txns[0].txn;
    assert_eq!(
        (t.status, t.manual_id.as_deref(), t.category.as_str()),
        (budget::Status::Manual, Some(id.as_str()), "groceries")
    );
    assert_eq!(v.month.spent, Decimal::new(1850, 2));
    assert_eq!(v.month.provisional, Decimal::ZERO, "cash never waits for a statement");

    let (st, _) = send(&s, "/api/budget/spend/remove", serde_json::json!({ "id": id })).await;
    assert_eq!(st, StatusCode::OK);
    assert!(build_budget(&s, &MonthQuery::default(), today).month.txns.is_empty());
    let (st, _) = send(&s, "/api/budget/spend/remove", serde_json::json!({ "id": id })).await;
    assert_eq!(st, StatusCode::NOT_FOUND, "already gone");
    // Reopening reads the void from disk, so the entry stays removed.
    let again = AppState::open(_dir.path()).unwrap();
    assert!(again.txns.read().unwrap().manual.is_empty());
}

#[tokio::test]
pub async fn bad_entries_are_422_and_one_bad_entry_stores_none() {
    let (_dir, s) = app();
    let today = Local::now().date_naive();
    let ok = serde_json::json!({ "date": today, "merchant": "Lunch", "amount": "14" });
    for (bad, says) in [
        (serde_json::json!({ "date": today, "merchant": "  ", "amount": "5" }), "say what it was"),
        (serde_json::json!({ "date": today, "merchant": "Lunch", "amount": "0" }), "more than 0"),
        (
            serde_json::json!({ "date": today + chrono::Days::new(5), "merchant": "Lunch", "amount": "5" }),
            "future",
        ),
        (
            serde_json::json!({ "date": today, "merchant": "Lunch", "amount": "5", "account": "nope" }),
            "no account",
        ),
    ] {
        let (st, body) = send(&s, "/api/budget/spend", serde_json::json!([ok, bad])).await;
        assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["message"].as_str().unwrap_or_default().contains(says), "{body}");
    }
    assert!(s.txns.read().unwrap().manual.is_empty());
    // On a card you import, the entry is stored against the card.
    let (st, _) = send(
        &s,
        "/api/budget/spend",
        serde_json::json!([{ "date": today, "merchant": "Lunch", "amount": "14", "account": "amex" }]),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
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
    assert!(plist_for(&s, Which::Tap).is_err() || detect_tailscale().is_some());
    s.apply_many(vec![Mutation::SetFeed {
        feed: Feed {
            mode: FeedMode::Tailscale,
            tailscale_url: Some("https://mini.tail1.ts.net".into()),
            icloud_dir: None,
        },
    }])
    .unwrap();
    let xml = plist_for(&s, Which::Tap).unwrap();
    assert!(xml.contains("https://mini.tail1.ts.net/api/ingest/transaction"));
    assert!(xml.contains(&token(&s).unwrap()));
    s.apply_many(vec![Mutation::SetFeed { feed: Feed::default() }]).unwrap();
    assert!(plist_for(&s, Which::Tap).is_err());
}

#[tokio::test]
pub async fn settings_never_hands_out_the_whole_token_but_reveal_does() {
    let (_dir, s) = app();
    let t = token(&s).unwrap();
    let get =
        router(s.clone()).oneshot(Request::get("/api/settings").body(Body::empty()).unwrap()).await.unwrap();
    let body = axum::body::to_bytes(get.into_body(), usize::MAX).await.unwrap();
    let text = String::from_utf8_lossy(&body);
    assert!(!text.contains(&t), "GET /api/settings leaked the token");
    assert!(text.contains(&t[t.len() - 4..]));
    let post = router(s.clone())
        .oneshot(Request::post("/api/settings/token/reveal").body(Body::empty()).unwrap())
        .await
        .unwrap();
    let body = axum::body::to_bytes(post.into_body(), usize::MAX).await.unwrap();
    assert!(String::from_utf8_lossy(&body).contains(&t));
}

#[cfg(unix)]
#[test]
fn the_token_file_is_private_even_if_it_was_not() {
    use std::os::unix::fs::PermissionsExt;
    let (dir, s) = app();
    let p = dir.path().join(TOKEN_FILE);
    std::fs::write(&p, "").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
    let t = token(&s).unwrap();
    assert_eq!(t.len(), 48);
    assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    assert_ne!(new_token().unwrap(), new_token().unwrap());
}
