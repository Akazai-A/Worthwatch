//! The Plan screen: `GET /api/plan`.

use axum::extract::{Query, State};
use axum::Json;
use chrono::{Local, NaiveDate};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::state::{AppState, Snapshot};

/// The Plan screen: projection under the ledger's assumptions (or the slider's), contribution
/// room and the recurring rules behind both.
#[derive(Serialize)]
pub struct PlanView {
    pub projection: derive::projection::Projection,
    pub room: Vec<derive::RoomView>,
    pub recurring: Vec<RecurringView>,
    /// The ledger's own assumptions, so the screen can show where the slider started.
    pub settings: ledger::Projection,
}

#[derive(Serialize)]
pub struct RecurringView {
    #[serde(flatten)]
    pub rule: ledger::Recurring,
    pub name: String,
    pub liability: bool,
    /// Monthly equivalent in base currency.
    pub monthly: Option<Decimal>,
}

#[derive(Debug, Default, Deserialize)]
pub struct PlanQuery {
    pub years: Option<u32>,
    pub return_pct: Option<Decimal>,
    pub extra: Option<Decimal>,
    pub target: Option<Decimal>,
}

pub fn build_plan(snap: &Snapshot, q: &PlanQuery, today: NaiveDate) -> PlanView {
    let l = &snap.ledger;
    let m = snap.market();
    let mut a = derive::Assumptions::from_ledger(l);
    a.years = q.years.unwrap_or(30).clamp(1, 50);
    if let Some(r) = q.return_pct {
        a.return_pct = r.clamp(Decimal::from(-20), Decimal::from(30));
    }
    if let Some(x) = q.extra {
        a.extra_monthly = x.max(Decimal::ZERO);
    }
    if q.target.is_some() {
        a.target = q.target.filter(|t| *t > Decimal::ZERO);
    }
    let recurring = l
        .recurring
        .iter()
        .map(|r| {
            let acct = l.account(&r.account);
            let per_month = r.amount * Decimal::from(r.every.per_year()) / Decimal::from(12);
            RecurringView {
                rule: r.clone(),
                name: acct.map_or_else(|| r.account.clone(), |a| a.name.clone()),
                liability: acct.is_some_and(|a| a.kind.is_liability()),
                monthly: acct
                    .and_then(|a| derive::to_base(l, &m, per_month, &a.currency, today))
                    .map(|v| v.round_dp(2)),
            }
        })
        .collect();
    PlanView {
        projection: derive::project(l, &m, today, &a),
        room: derive::room(l, &m, today),
        recurring,
        settings: l.settings.projection.clone(),
    }
}

pub async fn get_plan(State(s): State<AppState>, Query(q): Query<PlanQuery>) -> Json<PlanView> {
    Json(build_plan(&s.snapshot(), &q, Local::now().date_naive()))
}
