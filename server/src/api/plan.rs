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
    /// TFSA, FHSA and RRSP accounts added up by type, with room and growth.
    pub registered: Vec<derive::RegisteredView>,
    /// CRA's limits, so the setup form can preview an answer before it is saved.
    pub limits: CraLimits,
    pub recurring: Vec<RecurringView>,
    /// The ledger's own assumptions, so the screen can show where the slider started.
    pub settings: ledger::Projection,
}

#[derive(Serialize)]
pub struct CraLimits {
    /// Every year from 2009 through next year (next year may be an estimate).
    pub tfsa: Vec<(i32, Decimal)>,
    pub tfsa_first_year: i32,
    pub fhsa_first_year: i32,
    pub fhsa_annual: Decimal,
    pub fhsa_carry: Decimal,
    pub fhsa_lifetime: Decimal,
}

impl CraLimits {
    fn through(year: i32) -> Self {
        use ledger::cra::*;
        Self {
            tfsa: (TFSA_FIRST_YEAR..=year + 1).map(|y| (y, tfsa_limit(y).0)).collect(),
            tfsa_first_year: TFSA_FIRST_YEAR,
            fhsa_first_year: FHSA_FIRST_YEAR,
            fhsa_annual: Decimal::from(FHSA_ANNUAL),
            fhsa_carry: Decimal::from(FHSA_CARRY),
            fhsa_lifetime: Decimal::from(FHSA_LIFETIME),
        }
    }
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
        registered: derive::registered(l, &m, today),
        limits: CraLimits::through(chrono::Datelike::year(&today)),
        recurring,
        settings: l.settings.projection.clone(),
    }
}

pub async fn get_plan(State(s): State<AppState>, Query(q): Query<PlanQuery>) -> Json<PlanView> {
    Json(build_plan(&s.snapshot(), &q, Local::now().date_naive()))
}
