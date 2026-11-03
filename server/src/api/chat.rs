//! The chat boundary: build the assistant's context from the live snapshot, pick
//! the provider the ledger asks for, and turn a message into a [`assistant::Plan`].
//! Everything here proposes; `/api/apply` and the pending-proposal approve route are
//! the only places a plan's mutations go anywhere.

use assistant::{Anthropic, AssistantProvider, Context, Llm, LlmError, Ollama, Plan, Prompt, Proposal};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use ledger::Group;
use serde::{Deserialize, Serialize};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use chrono::Local;

use super::view::{view_of, LedgerView};
use super::{bad, ledger_error, ApiResult};
use crate::state::{AppState, Snapshot};

/// [`assistant::resolve`], with a category on every purchase from the budget's rules,
/// so the card shows where it will land.
pub fn resolve(l: &ledger::Ledger, ctx: &Context, p: &Proposal) -> Plan {
    let mut plan = assistant::resolve(l, ctx, p);
    for sp in plan.spends.iter_mut().filter(|sp| sp.category.is_none()) {
        let liability =
            sp.account.as_deref().and_then(|a| l.account(a)).is_some_and(|a| a.kind.is_liability());
        sp.category = Some(budget::categorize(
            &sp.merchant,
            sp.account.as_deref(),
            sp.amount,
            liability,
            &l.budget.rules,
        ));
    }
    plan
}

pub fn context(snap: &Snapshot, today: NaiveDate) -> Context {
    let l = &snap.ledger;
    let m = snap.market();
    let base = l.settings.base_currency.clone();
    let accounts = l
        .accounts
        .iter()
        .map(|a| {
            let (value, as_of) = match &a.positions {
                Some(_) => {
                    let v = derive::value_account(l, &m, a, today);
                    let (native, _, missing) = derive::positions_native(l, &m, a, today);
                    (if missing && native.is_zero() { None } else { Some(native.round_dp(2)) }, v.as_of)
                }
                None => match derive::balance_at(l, &a.id, today) {
                    Some((d, b)) => (Some(b), Some(d)),
                    None => (None, None),
                },
            };
            assistant::AccountCtx {
                id: a.id.clone(),
                name: a.name.clone(),
                institution: a.institution.clone(),
                kind: a.kind,
                currency: a.currency.clone(),
                liability: a.kind.is_liability(),
                value,
                as_of,
                positions: a.positions.as_ref().map(|ps| {
                    ps.iter()
                        .map(|p| assistant::PositionCtx {
                            symbol: p.symbol.clone(),
                            units: p.units,
                            price: m.price_at(&p.symbol, today).map(|x| x.price),
                        })
                        .collect()
                }),
            }
        })
        .collect();
    let rates = l
        .foreign_currencies()
        .into_iter()
        .filter_map(|c| m.rate_at(&base, &c, today).map(|r| (c, r.rate)))
        .collect();
    let totals = derive::totals(l, &m, today);
    let series = derive::net_worth_series(l, &m);
    let jan1 = NaiveDate::from_ymd_opt(today.year(), 1, 1).unwrap_or(today);
    let ytd = derive::delta_since(&series, jan1);
    let previous = series
        .len()
        .checked_sub(2)
        .and_then(|i| series.get(i))
        .and_then(|p| derive::delta_since(&series, p.date));
    Context {
        today,
        base_currency: base,
        accounts,
        rates,
        summary: assistant::Summary {
            net_worth: totals.net_worth.round_dp(2),
            assets: totals.assets.round_dp(2),
            liabilities: totals.liabilities.round_dp(2),
            by_group: [Group::Cash, Group::Investments, Group::Property, Group::Debt]
                .into_iter()
                .map(|g| {
                    (g.label().to_string(), totals.by_group.get(&g).copied().unwrap_or_default().round_dp(2))
                })
                .collect(),
            ytd_change: ytd.as_ref().map(|d| d.amount.round_dp(2)),
            ytd_pct: ytd.as_ref().and_then(|d| d.pct),
            previous_change: previous.as_ref().map(|d| d.amount.round_dp(2)),
            previous_from: previous.as_ref().map(|d| d.from),
        },
    }
}

/// How the chat panel is set up, for the status line.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct AssistantView {
    /// ollama | cloud | shorthand
    pub provider: String,
    pub model: Option<String>,
    pub url: Option<String>,
    /// Cloud only: whether `ANTHROPIC_API_KEY` is set.
    pub key_present: bool,
    pub offline: bool,
}

pub fn assistant_view(l: &ledger::Ledger, offline: bool) -> AssistantView {
    let s = &l.settings.assistant;
    match s.provider {
        AssistantProvider::Ollama => AssistantView {
            provider: "ollama".into(),
            model: Some(s.model.clone().unwrap_or_else(|| assistant::ollama::DEFAULT_MODEL.into())),
            url: Some(
                s.ollama_url
                    .clone()
                    .or_else(|| std::env::var("WORTHWATCH_OLLAMA_URL").ok().filter(|u| !u.trim().is_empty()))
                    .unwrap_or_else(|| assistant::ollama::DEFAULT_URL.into()),
            ),
            key_present: false,
            offline,
        },
        AssistantProvider::Cloud => AssistantView {
            provider: "cloud".into(),
            model: Some(s.model.clone().unwrap_or_else(|| assistant::anthropic::DEFAULT_MODEL.into())),
            url: Some(assistant::anthropic::URL.into()),
            key_present: std::env::var(assistant::anthropic::KEY_VAR).is_ok_and(|k| !k.trim().is_empty()),
            offline,
        },
        AssistantProvider::Shorthand => AssistantView {
            provider: "shorthand".into(),
            model: None,
            url: None,
            key_present: false,
            offline,
        },
    }
}

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub text: String,
    #[serde(default)]
    pub date: Option<NaiveDate>,
    /// Cloud only: the user saw the preview and said send.
    #[serde(default)]
    pub send: bool,
}

/// What would leave the machine, shown before it does.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SendPreview {
    pub url: String,
    pub model: String,
    pub prompt: Prompt,
}

#[derive(Debug, Serialize)]
pub struct ChatReply {
    pub text: String,
    /// Configured provider.
    pub provider: String,
    /// What actually produced the plan: ollama | cloud | shorthand.
    pub used: String,
    /// Why `used` differs from `provider`, or anything else worth a line.
    pub note: Option<String>,
    pub plan: Option<Plan>,
    /// Cloud, before consent: nothing was sent; here is what would be.
    pub preview: Option<SendPreview>,
}

fn run_model(llm: &dyn Llm, ctx: &Context, text: &str) -> Result<Proposal, String> {
    let prompt = assistant::build_prompt(ctx, text);
    let reply = llm.complete(&prompt).map_err(|e| e.to_string())?;
    assistant::parse_reply(&reply)
}

/// Blocking when a model is involved; call from `spawn_blocking`.
pub fn chat(snap: &Snapshot, offline: bool, req: &ChatRequest, today: NaiveDate) -> ChatReply {
    let l = &snap.ledger;
    let ctx = context(snap, today);
    let view = assistant_view(l, offline);
    let text = req.text.trim();
    let date = req.date;
    let finish = |p: Proposal, used: &str, note: Option<String>| {
        let mut p = p;
        if p.date.is_none() {
            p.date = date;
        }
        ChatReply {
            text: text.to_string(),
            provider: view.provider.clone(),
            used: used.into(),
            note,
            plan: Some(resolve(l, &ctx, &p)),
            preview: None,
        }
    };
    let shorthand = |note: Option<String>| finish(assistant::shorthand::parse(text, &ctx), "shorthand", note);
    if text.is_empty() {
        return shorthand(None);
    }
    match l.settings.assistant.provider {
        AssistantProvider::Shorthand => shorthand(None),
        _ if offline => {
            shorthand(Some("WORTHWATCH_OFFLINE is set, so no model is asked; parsed locally".into()))
        }
        AssistantProvider::Ollama => {
            let llm = Ollama {
                url: view.url.clone().unwrap_or_default(),
                model: view.model.clone().unwrap_or_default(),
            };
            match run_model(&llm, &ctx, text) {
                Ok(p) => finish(p, "ollama", None),
                Err(e) => shorthand(Some(format!("{e}; parsed locally instead"))),
            }
        }
        AssistantProvider::Cloud => {
            let model = view.model.clone().unwrap_or_default();
            if !req.send {
                return ChatReply {
                    text: text.to_string(),
                    provider: view.provider.clone(),
                    used: "none".into(),
                    note: None,
                    plan: None,
                    preview: Some(SendPreview {
                        url: assistant::anthropic::URL.into(),
                        model,
                        prompt: assistant::build_prompt(&ctx, text),
                    }),
                };
            }
            let api_key = std::env::var(assistant::anthropic::KEY_VAR).unwrap_or_default();
            let llm = Anthropic { model, api_key };
            match run_model(&llm, &ctx, text) {
                Ok(p) => finish(p, "cloud", None),
                Err(e) if e == LlmError::NoKey.to_string() => {
                    shorthand(Some(format!("{e}; parsed locally instead")))
                }
                Err(e) => shorthand(Some(format!("{e}; parsed locally instead"))),
            }
        }
    }
}

/// A proposal waiting for the user, from MCP or anywhere that is not the chat box.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Pending {
    pub id: u64,
    pub created: DateTime<Utc>,
    /// Who proposed: "mcp", a client name, ...
    pub origin: String,
    pub text: String,
    pub plan: Plan,
}

// ------------------------------------------------------------------ routes

/// Turn a message into a proposal. Writes nothing: the UI sends `plan.mutations` to
/// `/api/apply` when the user approves. With a cloud model and `send: false`, nothing
/// leaves the machine either; the reply carries the exact prompt as a preview.
pub async fn post_chat(
    State(s): State<AppState>,
    Json(req): Json<crate::api::chat::ChatRequest>,
) -> ApiResult<crate::api::chat::ChatReply> {
    let snap = s.snapshot();
    let offline = s.offline;
    let today = Local::now().date_naive();
    match tokio::task::spawn_blocking(move || crate::api::chat::chat(&snap, offline, &req, today)).await {
        Ok(r) => Ok(Json(r)),
        Err(e) => Err(bad(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())),
    }
}

/// What MCP clients read: the assistant's context (accounts with values, headline
/// figures). Same numbers as the dashboard, assembled server-side.
pub async fn get_context(State(s): State<AppState>) -> Json<assistant::Context> {
    Json(crate::api::chat::context(&s.snapshot(), Local::now().date_naive()))
}

/// The caller can't name itself: the card always says where a proposal came from
/// (`/api/propose` is the MCP bridge's route), so nothing can dress one up as the chat box.
#[derive(Deserialize)]
pub struct ProposeRequest {
    /// What the user said, for the card.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub proposal: Option<assistant::Proposal>,
}

#[derive(Serialize)]
pub struct ProposeReply {
    /// Set when the plan has something to approve and was queued.
    pub pending: Option<crate::api::chat::Pending>,
    pub plan: assistant::Plan,
}

/// Queue a proposal for the user (the MCP `propose_update` path). A structured
/// proposal is resolved as given; text alone goes through the shorthand parser. The
/// plan is returned either way so the caller sees rejections at once.
pub async fn propose(State(s): State<AppState>, Json(req): Json<ProposeRequest>) -> Json<ProposeReply> {
    let snap = s.snapshot();
    let today = Local::now().date_naive();
    let ctx = crate::api::chat::context(&snap, today);
    let p = req.proposal.unwrap_or_else(|| assistant::shorthand::parse(&req.text, &ctx));
    let plan = crate::api::chat::resolve(&snap.ledger, &ctx, &p);
    let pending = (!plan.mutations.is_empty() || !plan.spends.is_empty())
        .then(|| s.propose("mcp", &req.text, plan.clone()));
    Json(ProposeReply { pending, plan })
}

#[derive(Serialize)]
pub struct PendingStatus {
    pub id: u64,
    /// pending | approved | discarded | expired | unknown
    pub status: String,
}

pub async fn pending_status(State(s): State<AppState>, Path(id): Path<u64>) -> Json<PendingStatus> {
    let snap = s.snapshot();
    let status = if snap.pending.iter().any(|p| p.id == id) {
        "pending".to_string()
    } else {
        snap.decided.get(&id).cloned().unwrap_or_else(|| "unknown".into())
    };
    Json(PendingStatus { id, status })
}

pub async fn decide(
    State(s): State<AppState>,
    Path((id, action)): Path<(u64, String)>,
) -> ApiResult<LedgerView> {
    let approve = match action.as_str() {
        "approve" => true,
        "discard" => false,
        _ => return Err(bad(StatusCode::NOT_FOUND, format!("unknown action {action}"))),
    };
    match s.decide(id, approve) {
        Ok(Some(_)) => Ok(Json(view_of(&s))),
        Ok(None) => Err(bad(StatusCode::NOT_FOUND, format!("no pending proposal {id}"))),
        Err(e) => Err(ledger_error(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Snapshot;
    use rust_decimal_macros::dec;

    fn snap() -> Snapshot {
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let l = crate::seed::seed_ledger(today);
        let quotes = crate::seed::seed_quotes(&l, today);
        Snapshot {
            ledger: l,
            problem: None,
            quotes,
            holdings: Default::default(),
            version: 1,
            pending: vec![],
            decided: Default::default(),
            next_id: 1,
        }
    }

    #[test]
    fn context_carries_values_rates_and_a_summary() {
        let s = snap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let c = context(&s, today);
        assert_eq!(c.accounts.len(), 9);
        let chq = c.account("td-chq").unwrap();
        assert!(chq.value.is_some());
        assert!(chq.positions.is_none());
        let tfsa = c.account("ws-tfsa").unwrap();
        assert_eq!(tfsa.positions.as_ref().unwrap().len(), 2);
        assert!(tfsa.positions.as_ref().unwrap()[0].price.is_some());
        assert!(c.rates.contains_key("USD"));
        assert!(c.summary.net_worth > rust_decimal::Decimal::ZERO);
        assert!(c.summary.ytd_change.is_some());
        assert_eq!(c.summary.by_group.len(), 4);
    }

    #[test]
    fn offline_or_shorthand_never_asks_a_model_and_the_canonical_message_makes_a_card() {
        let s = snap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let r = chat(
            &s,
            true,
            &ChatRequest { text: "td chq 5102, visa 640".into(), date: None, send: false },
            today,
        );
        assert_eq!(r.provider, "ollama");
        assert_eq!(r.used, "shorthand");
        assert!(r.note.unwrap().contains("OFFLINE"));
        let plan = r.plan.unwrap();
        assert_eq!(plan.rows.len(), 2);
        assert_eq!(plan.rows[0].to, Some(dec!(5102)));
        assert_eq!(plan.rows[1].account.as_deref(), Some("td-visa"));
        assert_eq!(plan.mutations.len(), 2);
        assert_eq!(plan.date, today);
        assert!(plan.rejected.is_empty());
    }

    #[test]
    fn cloud_shows_a_preview_and_sends_nothing_until_asked() {
        let mut s = snap();
        s.ledger.settings.assistant.provider = AssistantProvider::Cloud;
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let r = chat(&s, false, &ChatRequest { text: "visa 612".into(), date: None, send: false }, today);
        assert_eq!(r.used, "none");
        assert!(r.plan.is_none());
        let p = r.preview.unwrap();
        assert_eq!(p.url, assistant::anthropic::URL);
        assert_eq!(p.prompt.user, "visa 612");
        assert!(p.prompt.system.contains("td-visa"));
        assert!(!p.prompt.system.contains("1240"), "no balance in the preview");
        // Offline, even with consent, nothing is sent.
        let r = chat(&s, true, &ChatRequest { text: "visa 612".into(), date: None, send: true }, today);
        assert_eq!(r.used, "shorthand");
        assert!(r.plan.is_some());
    }

    #[test]
    fn a_question_is_answered_without_a_model() {
        let s = snap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 2).unwrap();
        let r = chat(
            &s,
            true,
            &ChatRequest { text: "what is my net worth?".into(), date: None, send: false },
            today,
        );
        let plan = r.plan.unwrap();
        assert!(plan.answer.unwrap().starts_with("Net worth $"));
        assert!(plan.rows.is_empty());
    }
}
