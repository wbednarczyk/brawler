//! Morning-briefing job (ADR 0068 decision 4, plan v0.54-attention-routing-briefing §T5).
//!
//! The handler gathers domain reads and runs the DETERMINISTIC composer
//! ([`storage::compose_briefing`]). The composed structured item list is the
//! ONLY briefing: the AI narrative-phrasing half is retired with the in-app
//! analysis layer (ADR 0084, amending ADR 0068 — the briefing is
//! deterministic-list only). The composed briefing is persisted to
//! `morning_briefings` (+ `morning_briefing_items`).

use serde::Deserialize;

use crate::app_state::AppState;
use crate::storage::{
    self, AttentionEventListInput, CompanySignalListInput, ListAutopilotRunsInput,
    ReportSeasonInput,
};

/// Job kind: compose a morning briefing. Deterministic composition only, so it
/// drains on the **autopilot** lane in [`crate::jobs::handlers::pool_layout`].
pub const MORNING_BRIEFING_KIND: &str = "morning_briefing";

/// Stable queue id for the on-demand (force) compose.
const ON_DEMAND_JOB_ID: &str = "morning_briefing:on_demand";
/// Stable queue id for the daily auto-trigger compose.
const DAILY_JOB_ID: &str = "morning_briefing:daily";

/// Payload for a briefing job. `force = true` (on-demand) recomposes even if a
/// briefing already exists for today; `force = false` (daily auto-trigger) is a
/// no-op once today's briefing exists, so repeated scheduler ticks are safe.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MorningBriefingPayload {
    #[serde(default)]
    force: bool,
}

/// Enqueue an on-demand briefing compose (`force = true`) under a stable id.
pub fn enqueue_on_demand_briefing(state: &AppState) {
    if let Err(error) = state.jobs().reschedule(
        ON_DEMAND_JOB_ID,
        MORNING_BRIEFING_KIND,
        "{\"force\":true}",
        1,
    ) {
        log::warn!("failed to enqueue on-demand morning briefing: {error}");
    }
}

/// Enqueue the daily auto-trigger compose (`force = false`, idempotent per day)
/// under a stable id. Called once per day by the scheduler while the app is open.
pub fn enqueue_daily_briefing(state: &AppState) {
    if let Err(error) =
        state
            .jobs()
            .reschedule(DAILY_JOB_ID, MORNING_BRIEFING_KIND, "{\"force\":false}", 1)
    {
        log::warn!("failed to enqueue daily morning briefing: {error}");
    }
}

/// Today's date (`YYYY-MM-DD`, UTC).
fn today_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Iso8601::DATE)
        .unwrap_or_else(|_| "0000-01-01".to_owned())
}

/// Run one briefing compose. Storage failures return `Err` (queue may retry).
/// The briefing is always the deterministic structured list (ADR 0084).
pub fn run_morning_briefing_job(state: &AppState, payload: &str) -> Result<(), String> {
    let force = serde_json::from_str::<MorningBriefingPayload>(payload)
        .map(|payload| payload.force)
        .unwrap_or(false);
    let today = today_iso();

    let briefings = state.morning_briefings();
    if !force
        && briefings
            .briefing_exists_on(&today)
            .map_err(|error| error.to_string())?
    {
        // Daily auto-trigger already ran today — idempotent no-op.
        return Ok(());
    }

    let since = briefings
        .latest_since_boundary()
        .map_err(|error| error.to_string())?;
    let sources = gather_sources(state)?;
    let composed = storage::compose_briefing(&sources, &since, &today);
    briefings
        .insert_morning_briefing(&composed)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Gather the composer's reads across every watched company (ADR 0068: "what
/// changed in my companies").
fn gather_sources(state: &AppState) -> Result<storage::BriefingSources, String> {
    let memberships = state
        .watchlists()
        .list_watchlist_memberships()
        .map_err(|error| error.to_string())?;
    let mut company_ids: Vec<String> = memberships
        .into_iter()
        .map(|membership| membership.company_id)
        .collect();
    company_ids.sort();
    company_ids.dedup();

    let mut sources = storage::BriefingSources::default();
    for company_id in &company_ids {
        sources.signals.extend(
            state
                .signals()
                .list_company_signals(CompanySignalListInput {
                    company_id: Some(company_id.clone()),
                    watchlist_id: None,
                    category: None,
                    status: Some("confirmed".to_owned()),
                })
                .map_err(|error| error.to_string())?,
        );
        sources.autopilot_runs.extend(
            state
                .autopilot()
                .list_runs(&ListAutopilotRunsInput {
                    company_id: Some(company_id.clone()),
                    notification_state: None,
                    limit: Some(50),
                })
                .map_err(|error| error.to_string())?,
        );
        let claims = state
            .management_claims()
            .list_claims_to_verify(company_id)
            .map_err(|error| error.to_string())?;
        sources
            .claims_due
            .extend(claims.due.into_iter().map(|entry| entry.claim));
        sources
            .claims_due
            .extend(claims.overdue.into_iter().map(|entry| entry.claim));
    }

    sources.upcoming_reports = state
        .report_season()
        .list_report_season(ReportSeasonInput { watchlist_id: None })
        .map_err(|error| error.to_string())?
        .upcoming;

    sources.attention_events = state
        .attention()
        .list_attention_events(AttentionEventListInput {
            company_id: None,
            include_dismissed: false,
        })
        .map_err(|error| error.to_string())?;

    Ok(sources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, AppState};

    #[test]
    fn job_composes_a_deterministic_briefing_and_is_idempotent_per_day() {
        // ADR 0084: the composed structured list is the only briefing. The job
        // completes with no narrative and no provider anywhere in the path, and
        // a second non-forced run is a per-day no-op (no duplicate briefing).
        let state = AppState::new(open_in_memory_database().expect("db"));

        run_morning_briefing_job(&state, "{\"force\":false}").expect("first compose");
        let first = state
            .morning_briefings()
            .latest_morning_briefing()
            .expect("latest")
            .expect("a briefing exists after the first run");

        run_morning_briefing_job(&state, "{\"force\":false}").expect("second compose is a no-op");
        let second = state
            .morning_briefings()
            .latest_morning_briefing()
            .expect("latest")
            .expect("still a briefing");
        assert_eq!(
            first.id, second.id,
            "a second same-day non-forced run does not create a new briefing"
        );
    }

    #[test]
    fn dispatches_through_the_worker() {
        // The kind is registered + lane-assigned, so an enqueued on-demand briefing
        // is claimed and run to success by the worker.
        let state = AppState::new(open_in_memory_database().expect("db"));
        enqueue_on_demand_briefing(&state);
        let worker = crate::jobs::handlers::build_worker(state.clone());
        assert!(worker.process_one().expect("process one"), "job dispatched");
        assert_eq!(state.jobs().counts().expect("counts").succeeded, 1);
        assert!(state
            .morning_briefings()
            .latest_morning_briefing()
            .expect("latest")
            .is_some());
    }
}
