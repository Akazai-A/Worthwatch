//! `worthwatch mcp`: a Model Context Protocol server on stdio, for Claude Desktop or
//! any MCP client. It is a thin client of the running app's HTTP API, so it reads the
//! same numbers the dashboard shows and can only *propose*: `propose_update` queues
//! a card in the Chat screen, and nothing is written until the user approves there.
//!
//! The protocol handling is pure ([`handle`] over a [`Backend`]); the stdio loop and
//! the HTTP backend are the thin shells around it.

use std::io::{BufRead, Write};

use serde_json::{json, Value};

pub const PROTOCOL_VERSION: &str = "2025-06-18";

/// What the tools need from the app. HTTP in production, a fake in tests.
pub trait Backend {
    fn get(&self, path: &str) -> Result<Value, String>;
    fn post(&self, path: &str, body: Value) -> Result<Value, String>;
}

pub struct Http {
    pub base: String,
}

impl Backend for Http {
    fn get(&self, path: &str) -> Result<Value, String> {
        ureq::get(&format!("{}{path}", self.base))
            .call()
            .map_err(|e| self.explain(e))?
            .into_json()
            .map_err(|e| e.to_string())
    }
    fn post(&self, path: &str, body: Value) -> Result<Value, String> {
        ureq::post(&format!("{}{path}", self.base))
            .send_json(body)
            .map_err(|e| self.explain(e))?
            .into_json()
            .map_err(|e| e.to_string())
    }
}

impl Http {
    fn explain(&self, e: ureq::Error) -> String {
        match e {
            ureq::Error::Transport(_) => {
                format!("worthwatch is not running at {} (start it, or set WORTHWATCH_URL)", self.base)
            }
            ureq::Error::Status(code, r) => {
                let body = r.into_string().unwrap_or_default();
                let msg = serde_json::from_str::<Value>(&body)
                    .ok()
                    .and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_string))
                    .unwrap_or(body);
                format!("HTTP {code}: {msg}")
            }
        }
    }
}

fn tools() -> Value {
    json!([
        {
            "name": "get_net_worth",
            "description": "Net worth today with assets, debt, totals per group (Cash, Investments, Property, Debt), the change since Jan 1 and since the previous check-in. Base currency.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "get_accounts",
            "description": "Every account: id, name, institution, kind, currency, current value in its own currency, as-of date, and positions (symbol, units, price) for investment accounts. Use the ids in propose_update.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "get_portfolio",
            "description": "Investments by security: units, price, value, weight, gain, day change; plus totals by currency and account.",
            "inputSchema": { "type": "object", "properties": {} },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "get_exposure",
            "description": "Look-through exposure through funds (ETF holdings files): by sector, region, country, asset class, currency, and the top companies with your dollars in each.",
            "inputSchema": {
                "type": "object",
                "properties": { "top": { "type": "integer", "description": "How many companies to list (default 25, max 100)." } }
            },
            "annotations": { "readOnlyHint": true }
        },
        {
            "name": "propose_update",
            "description": "Propose changes to the ledger. NOTHING IS WRITTEN: the proposal appears as a card in Worthwatch's Chat screen and the user approves or discards it there. Either pass `text` in the user's shorthand (\"td chq 5102, visa 640, sold 20 XEQT in the TFSA\") or a structured `items` list. Items: {kind:'balance',account,value} (liabilities: amount owed), {kind:'adjust',account,delta}, {kind:'trade',account?,symbol,units (+buy/-sell),price?}, {kind:'price',symbol,price}, {kind:'new_account',name,account_kind,currency?,value?}, {kind:'note',note}, {kind:'spend',what,amount,paid_with?,category?} (a purchase for the budget: paid_with is an account id or 'cash'; it never changes a balance). `account` is an id from get_accounts. Returns the checked plan with any rejections and a pending id for check_proposal.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "What the user said; shown on the card. Parsed when no items are given." },
                    "items": { "type": "array", "items": { "type": "object" } },
                    "date": { "type": "string", "description": "YYYY-MM-DD; today when omitted." }
                }
            }
        },
        {
            "name": "check_proposal",
            "description": "Whether the user approved or discarded a proposal: pending | approved | discarded | expired | unknown.",
            "inputSchema": {
                "type": "object",
                "properties": { "id": { "type": "integer" } },
                "required": ["id"]
            },
            "annotations": { "readOnlyHint": true }
        }
    ])
}

fn text_result(v: &Value, is_error: bool) -> Value {
    let text = match v {
        Value::String(s) => s.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    };
    json!({ "content": [{ "type": "text", "text": text }], "isError": is_error })
}

fn call_tool(b: &dyn Backend, name: &str, args: &Value) -> Result<Value, String> {
    match name {
        "get_net_worth" => {
            let c = b.get("/api/context")?;
            Ok(json!({
                "as_of": c["today"],
                "base_currency": c["base_currency"],
                "net_worth": c["summary"]["net_worth"],
                "assets": c["summary"]["assets"],
                "debt": c["summary"]["liabilities"],
                "by_group": c["summary"]["by_group"],
                "change_since_jan_1": c["summary"]["ytd_change"],
                "change_since_jan_1_pct": c["summary"]["ytd_pct"],
                "change_since_previous_check_in": c["summary"]["previous_change"],
                "previous_check_in": c["summary"]["previous_from"],
            }))
        }
        "get_accounts" => {
            let c = b.get("/api/context")?;
            Ok(json!({ "as_of": c["today"], "base_currency": c["base_currency"], "accounts": c["accounts"] }))
        }
        "get_portfolio" => {
            let v = b.get("/api/view")?;
            let mut p = v["portfolio"].clone();
            if let Some(rows) = p.get_mut("securities").and_then(Value::as_array_mut) {
                for r in rows {
                    if let Some(o) = r.as_object_mut() {
                        o.remove("accounts");
                    }
                }
            }
            Ok(json!({ "as_of": v["as_of"], "base_currency": v["base_currency"], "portfolio": p }))
        }
        "get_exposure" => {
            let top = args.get("top").and_then(Value::as_u64).unwrap_or(25).clamp(1, 100) as usize;
            let v = b.get("/api/lookthrough")?;
            let lt = &v["lookthrough"];
            let companies: Vec<Value> = lt["companies"]
                .as_array()
                .map(|cs| {
                    cs.iter()
                        .take(top)
                        .map(|c| {
                            json!({
                                "name": c["name"], "ticker": c["ticker"], "sector": c["sector"],
                                "country": c["country"], "value": c["value"], "pct": c["pct"],
                                "via": c["via"].as_array().map(|vs| vs.iter().map(|x| x["path"].clone()).collect::<Vec<_>>()),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(json!({
                "as_of": v["as_of"],
                "base_currency": v["base_currency"],
                "total": lt["total"],
                "funds_resolved": lt["funds"].as_array().map(|fs| fs.iter().filter(|f| f["resolved"] == true).count()),
                "by_sector": lt["by_sector"],
                "by_region": lt["by_region"],
                "by_country": lt["by_country"],
                "by_asset_class": lt["by_asset_class"],
                "by_currency": lt["by_currency"],
                "top10_pct": lt["top10_pct"],
                "companies": companies,
            }))
        }
        "propose_update" => {
            let text = args.get("text").and_then(Value::as_str).unwrap_or_default().to_string();
            let items = args.get("items").filter(|v| v.as_array().is_some_and(|a| !a.is_empty()));
            if text.trim().is_empty() && items.is_none() {
                return Err("give `text` or `items`".into());
            }
            let proposal = items.map(|items| json!({ "items": items, "date": args.get("date") }));
            let r = b.post("/api/propose", json!({ "origin": "mcp", "text": text, "proposal": proposal }))?;
            let queued = !r["pending"].is_null();
            Ok(json!({
                "queued": queued,
                "pending_id": r["pending"]["id"],
                "message": if queued {
                    "Shown to the user in Worthwatch › Chat. Nothing is written until they approve it there."
                } else {
                    "Nothing could be proposed; see rejected."
                },
                "rows": r["plan"]["rows"],
                "rejected": r["plan"]["rejected"],
                "net_change": r["plan"]["net_change"],
            }))
        }
        "check_proposal" => {
            let id = args.get("id").and_then(Value::as_u64).ok_or("`id` is required")?;
            b.get(&format!("/api/pending/{id}"))
        }
        other => Err(format!("unknown tool {other}")),
    }
}

/// One JSON-RPC message in, at most one out (notifications get no reply).
pub fn handle(b: &dyn Backend, msg: &Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or_default();
    let id = id?;
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": msg.pointer("/params/protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSION),
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "worthwatch", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Read-only tools report the user's net worth, accounts, portfolio and exposure. propose_update never writes: the user approves each proposal in Worthwatch's Chat screen.",
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => {
            let name = msg.pointer("/params/name").and_then(Value::as_str).unwrap_or_default();
            let args = msg.pointer("/params/arguments").cloned().unwrap_or(json!({}));
            Ok(match call_tool(b, name, &args) {
                Ok(v) => text_result(&v, false),
                Err(e) => text_result(&Value::String(e), true),
            })
        }
        _ => Err(json!({ "code": -32601, "message": format!("method not found: {method}") })),
    };
    Some(match result {
        Ok(r) => json!({ "jsonrpc": "2.0", "id": id, "result": r }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": e }),
    })
}

/// Newline-delimited JSON-RPC on stdin/stdout until stdin closes.
pub fn run(base: &str) -> anyhow::Result<()> {
    let b = Http { base: base.trim_end_matches('/').to_string() };
    let stdin = std::io::stdin();
    let mut out = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let reply = match serde_json::from_str::<Value>(&line) {
            Ok(msg) => handle(&b, &msg),
            Err(e) => Some(
                json!({ "jsonrpc": "2.0", "id": null, "error": { "code": -32700, "message": e.to_string() } }),
            ),
        };
        if let Some(r) = reply {
            writeln!(out, "{r}")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;
    use chrono::NaiveDate;

    /// The real routes' builders over an in-memory state: no sockets.
    struct InProcess {
        state: AppState,
        today: NaiveDate,
    }

    impl Backend for InProcess {
        fn get(&self, path: &str) -> Result<Value, String> {
            let snap = self.state.snapshot();
            match path {
                "/api/context" => {
                    Ok(serde_json::to_value(crate::api::chat::context(&snap, self.today)).unwrap())
                }
                "/api/view" => {
                    Ok(serde_json::to_value(crate::api::view::build_view(&snap, "x", self.today, true))
                        .unwrap())
                }
                "/api/lookthrough" => Ok(serde_json::to_value(crate::api::portfolio::build_lookthrough(
                    &snap, "x", self.today, true,
                ))
                .unwrap()),
                p if p.starts_with("/api/pending/") => {
                    let id: u64 = p.rsplit('/').next().unwrap().parse().unwrap();
                    let status = if snap.pending.iter().any(|x| x.id == id) {
                        "pending".to_string()
                    } else {
                        snap.decided.get(&id).cloned().unwrap_or_else(|| "unknown".into())
                    };
                    Ok(json!({ "id": id, "status": status }))
                }
                _ => Err(format!("no route {path}")),
            }
        }
        fn post(&self, path: &str, body: Value) -> Result<Value, String> {
            assert_eq!(path, "/api/propose");
            let snap = self.state.snapshot();
            let ctx = crate::api::chat::context(&snap, self.today);
            let text = body["text"].as_str().unwrap_or_default();
            let p: assistant::Proposal = match &body["proposal"] {
                Value::Null => assistant::shorthand::parse(text, &ctx),
                v => serde_json::from_value(v.clone()).map_err(|e| e.to_string())?,
            };
            let plan = crate::api::chat::resolve(&snap.ledger, &ctx, &p);
            let pending = (!plan.mutations.is_empty() || !plan.spends.is_empty())
                .then(|| self.state.propose("mcp", text, plan.clone()));
            Ok(json!({ "pending": pending, "plan": plan }))
        }
    }

    fn backend() -> (tempfile::TempDir, InProcess) {
        let dir = tempfile::tempdir().unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        crate::seed::write_seed(dir.path(), today, true).unwrap();
        let mut state = AppState::open(dir.path()).unwrap();
        state.offline = true;
        (dir, InProcess { state, today })
    }

    fn call(b: &dyn Backend, name: &str, args: Value) -> (Value, bool) {
        let r = handle(b, &json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": { "name": name, "arguments": args } }))
            .unwrap();
        let res = &r["result"];
        let text = res["content"][0]["text"].as_str().unwrap();
        (serde_json::from_str(text).unwrap_or(Value::String(text.into())), res["isError"] == true)
    }

    #[test]
    fn handshake_and_tool_list() {
        let (_d, b) = backend();
        let r = handle(&b, &json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-03-26" } })).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["serverInfo"]["name"], "worthwatch");
        assert!(handle(&b, &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })).is_none());
        let r = handle(&b, &json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).unwrap();
        let names: Vec<&str> =
            r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
        assert_eq!(
            names,
            [
                "get_net_worth",
                "get_accounts",
                "get_portfolio",
                "get_exposure",
                "propose_update",
                "check_proposal"
            ]
        );
        let r = handle(&b, &json!({ "jsonrpc": "2.0", "id": 3, "method": "resources/list" })).unwrap();
        assert_eq!(r["error"]["code"], -32601);
    }

    #[test]
    fn read_tools_report_the_dashboard_numbers() {
        let (_d, b) = backend();
        let (nw, err) = call(&b, "get_net_worth", json!({}));
        assert!(!err);
        let snap = b.state.snapshot();
        let totals = derive::totals(&snap.ledger, &snap.market(), b.today);
        assert_eq!(nw["net_worth"], json!(totals.net_worth.round_dp(2).to_string()));
        assert_eq!(nw["by_group"].as_object().unwrap().len(), 4);
        let (accts, _) = call(&b, "get_accounts", json!({}));
        assert_eq!(accts["accounts"].as_array().unwrap().len(), 9);
        let (p, _) = call(&b, "get_portfolio", json!({}));
        assert!(p["portfolio"]["securities"][0].get("accounts").is_none());
        let (x, _) = call(&b, "get_exposure", json!({ "top": 3 }));
        assert!(x["companies"].as_array().unwrap().len() <= 3);
    }

    #[test]
    fn propose_queues_a_card_and_writes_nothing_until_approved() {
        let (_d, b) = backend();
        let before = std::fs::read_to_string(&b.state.ledger_path).unwrap();
        let (r, err) = call(&b, "propose_update", json!({ "text": "mortgage 410950" }));
        assert!(!err);
        assert_eq!(r["queued"], true);
        let id = r["pending_id"].as_u64().unwrap();
        assert_eq!(std::fs::read_to_string(&b.state.ledger_path).unwrap(), before, "nothing written");
        assert_eq!(b.state.snapshot().pending.len(), 1);
        let (s, _) = call(&b, "check_proposal", json!({ "id": id }));
        assert_eq!(s["status"], "pending");

        b.state.decide(id, true).unwrap().unwrap();
        assert_ne!(std::fs::read_to_string(&b.state.ledger_path).unwrap(), before);
        let (s, _) = call(&b, "check_proposal", json!({ "id": id }));
        assert_eq!(s["status"], "approved");
        assert!(b.state.snapshot().pending.is_empty());
    }

    #[test]
    fn structured_items_are_checked_and_bad_ones_are_never_queued() {
        let (_d, b) = backend();
        let (r, _) = call(
            &b,
            "propose_update",
            json!({ "text": "sold XEQT in the RRSP", "items": [{ "kind": "trade", "account": "qt-rrsp", "symbol": "XEQT", "units": -20 }] }),
        );
        assert_eq!(r["queued"], false);
        assert!(r["rejected"][0]["reason"].as_str().unwrap().contains("holds no XEQT.TO"));
        assert!(b.state.snapshot().pending.is_empty());
        let (r, err) = call(&b, "propose_update", json!({ "items": [{ "kind": "drop_table" }] }));
        assert!(err);
        assert!(r.as_str().unwrap().contains("unknown variant"));
        let (_, err) = call(&b, "propose_update", json!({}));
        assert!(err);
        let (r, _) = call(
            &b,
            "propose_update",
            json!({ "items": [{ "kind": "balance", "account": "td-visa", "value": 612 }] }),
        );
        assert_eq!(r["queued"], true);
        // Discard: nothing written, status recorded.
        let id = r["pending_id"].as_u64().unwrap();
        let before = std::fs::read_to_string(&b.state.ledger_path).unwrap();
        b.state.decide(id, false).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&b.state.ledger_path).unwrap(), before);
        assert_eq!(b.state.snapshot().decided[&id], "discarded");
    }

    #[test]
    fn a_purchase_queues_a_card_and_approval_adds_it_to_the_budget_not_the_ledger() {
        let (_d, b) = backend();
        let before = std::fs::read_to_string(&b.state.ledger_path).unwrap();
        let (r, err) = call(&b, "propose_update", json!({ "text": "coffee 6 cash" }));
        assert!(!err);
        assert_eq!(r["queued"], true);
        assert_eq!(
            (r["rows"][0]["kind"].as_str(), r["rows"][0]["detail"].as_str()),
            (Some("spend"), Some("Coffee"))
        );
        let id = r["pending_id"].as_u64().unwrap();
        assert!(b.state.txns.read().unwrap().manual.is_empty(), "nothing written before approval");

        b.state.decide(id, true).unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&b.state.ledger_path).unwrap(), before, "net worth untouched");
        let txns = b.state.txns.read().unwrap();
        assert_eq!(txns.manual.len(), 1);
        assert_eq!((txns.manual[0].merchant.as_str(), txns.manual[0].account.as_deref()), ("Coffee", None));
        assert_eq!(txns.manual[0].amount, rust_decimal::Decimal::from(6));
    }
}
