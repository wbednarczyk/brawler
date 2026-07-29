//! The class rule made executable (ADR 0091 dec. 3): the enumeration gate over
//! the REAL job registry + the per-surface visibility tests. A new job kind
//! reddens twice — once here as unclassified, once as classified-without-a-
//! visibility-test (the visibility tests below assert a kind list derived from
//! the map, so a new `TodayAttention` kind must be proven to reach the stream).

use std::collections::BTreeSet;

use super::{failure_surface, FailureSurface};
use crate::app_state::AppState;
use crate::jobs::handlers::build_worker;
use crate::storage::{
    open_in_memory_database, AttentionEventListInput, AttentionSeverity, NewCompany, EVIDENCE_JOB,
    TRIGGER_JOB_FAILED,
};

/// Every kind the worker registers, sorted.
fn registered_kinds(state: &AppState) -> Vec<&'static str> {
    let worker = build_worker(state.clone());
    let mut kinds = worker.registered_kinds();
    kinds.sort_unstable();
    kinds
}

#[test]
fn every_registered_kind_has_a_failure_surface() {
    // THE gate (ADR 0091 dec. 3). The inventory comes from the real registry
    // (`build_worker` → `registered_kinds`), never a hand copy, so a job kind
    // registered without classifying where its terminal failure becomes visible
    // reddens here — the "5 of 12 kinds die in silence" class, made impossible.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let unclassified: Vec<&str> = registered_kinds(&state)
        .into_iter()
        .filter(|kind| failure_surface(kind).is_none())
        .collect();

    assert!(
        unclassified.is_empty(),
        "these job kinds have no failure surface (ADR 0091 dec. 3 — classify them in \
         jobs::failure_surface, and ship the visibility test that proves the failure \
         reaches it): {unclassified:?}"
    );
}

#[test]
fn the_classification_covers_the_registry_and_nothing_else() {
    // The map is a classification OF the registry: a kind that no longer exists
    // must not linger in it (a stale arm silently keeps a retired kind "covered"),
    // and each surface must actually be used — the three-surface taxonomy is a
    // decision (ADR 0091 dec. 1), not a wish list.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let registered: BTreeSet<&str> = registered_kinds(&state).into_iter().collect();

    for kind in [
        crate::jobs::scheduler::SOURCE_REFRESH_KIND,
        crate::jobs::scheduler::REGISTRY_REFRESH_KIND,
        crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND,
        crate::jobs::fx_daily_pull::FX_DAILY_PULL_KIND,
        crate::jobs::autopilot::AUTOPILOT_STAGE_KIND,
        crate::jobs::morning_briefing::MORNING_BRIEFING_KIND,
        crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
        crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND,
        crate::jobs::management_holdings_extraction::MANAGEMENT_EXTRACTION_KIND,
        crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND,
        crate::jobs::backfill::COMPANY_BACKFILL_KIND,
        crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND,
    ] {
        assert!(
            registered.contains(kind),
            "{kind} is classified but no longer registered — drop the stale arm"
        );
    }

    let surfaces: BTreeSet<&str> = registered
        .iter()
        .map(|kind| match failure_surface(kind).expect("classified") {
            FailureSurface::TodayAttention => "today",
            FailureSurface::SourcesAdapterHealth => "sources",
            FailureSurface::AutopilotRunCard => "autopilot",
        })
        .collect();
    assert_eq!(
        surfaces,
        ["autopilot", "sources", "today"].into_iter().collect(),
        "all three surfaces are in use"
    );

    // An unknown kind is NOT silently defaulted — it is unclassified, and the gate
    // above turns that into a failure.
    assert_eq!(failure_surface("brand_new_unseen_kind"), None);
}

#[test]
fn a_terminally_failed_today_attention_job_reaches_the_stream() {
    // Visibility test for `FailureSurface::TodayAttention` (ADR 0091 dec. 1+3): a
    // real registered kind exhausts its attempts and the failure lands as a system
    // `job_failed` attention event, scoped to the company the payload names, with
    // the job as its evidence. Before this slice the job died in silence.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");

    // `ownership_extraction` is classified TodayAttention; a payload missing
    // `reportDocumentId` fails deserialization on every attempt (deterministic,
    // offline). max_attempts = 1 → the first failure is terminal.
    assert_eq!(
        failure_surface(crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND),
        Some(FailureSurface::TodayAttention)
    );
    let payload = format!("{{\"companyId\":\"{}\"}}", company.id);
    state
        .jobs()
        .enqueue(
            "job_own_1",
            crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND,
            &payload,
            1,
        )
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker.process_one().expect("process"), "a job was claimed");
    assert_eq!(
        state.jobs().counts().expect("counts").failed,
        1,
        "retries exhausted — the job is terminally failed"
    );

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events");
    assert_eq!(events.len(), 1, "the terminal failure raised ONE event");
    let event = &events[0];
    assert_eq!(event.trigger_type, TRIGGER_JOB_FAILED);
    assert_eq!(event.evidence_type, EVIDENCE_JOB);
    assert_eq!(
        event.evidence_ref, "job_own_1",
        "evidence_ref is the job id — the row that holds kind + last_error"
    );
    assert_eq!(
        event.company_id.as_deref(),
        Some(company.id.as_str()),
        "the payload's company scopes the event"
    );
    assert_eq!(event.rule_id, None, "a system event owns no alert rule");
    assert_eq!(
        event.severity,
        AttentionSeverity::Notable,
        "every job failure is notable (owner decision 1)"
    );
}

#[test]
fn a_retry_that_is_still_pending_raises_nothing() {
    // Owner decision 2: events fire on TERMINAL failure only. A first failure with
    // retries left is a transient hiccup — it must stay silent, or the stream fills
    // with noise the queue is about to resolve by itself.
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue(
            "job_sweep_retry",
            crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
            "{}",
            3,
        )
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker.process_one().expect("process"));
    let counts = state.jobs().counts().expect("counts");
    assert_eq!(counts.failed, 0, "not terminal yet");
    assert_eq!(counts.pending, 1, "requeued with backoff");
    assert!(
        state
            .attention()
            .list_attention_events(AttentionEventListInput::default())
            .expect("events")
            .is_empty(),
        "a transient failure raises no attention event"
    );
}

#[test]
fn a_workspace_wide_job_failure_is_stored_without_a_company() {
    // ADR 0091 dec. 2 end to end: a job whose payload names no company (a history
    // sweep with a malformed payload) still surfaces — as a system event with a NULL
    // company scope. Before migration 0118 this row could not exist at all, so the
    // failure had nowhere to go.
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .jobs()
        .enqueue(
            "job_sweep_1",
            crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
            "{}",
            1,
        )
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker.process_one().expect("process"));

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].trigger_type, TRIGGER_JOB_FAILED);
    assert_eq!(
        events[0].company_id, None,
        "a workspace-wide job failure belongs to no single company"
    );
}

#[test]
fn a_sources_adapter_health_job_does_not_double_fire_a_generic_event() {
    // Exclusivity (ADR 0091 dec. 1): a kind whose failure already reaches a richer
    // domain surface must NOT also raise the generic event. `scheduled_source_refresh`
    // for an unknown adapter fails terminally, offline and deterministically; the
    // Sources screen owns that failure (see `source_refresh::tests::
    // a_failing_source_refresh_records_the_adapter_error_on_its_row`).
    let state = AppState::new(open_in_memory_database().expect("db"));
    assert_eq!(
        failure_surface(crate::jobs::scheduler::SOURCE_REFRESH_KIND),
        Some(FailureSurface::SourcesAdapterHealth)
    );
    state
        .jobs()
        .enqueue(
            "job_refresh_1",
            crate::jobs::scheduler::SOURCE_REFRESH_KIND,
            "{\"adapterId\":\"no-such-adapter\"}",
            1,
        )
        .expect("enqueue");

    let worker = build_worker(state.clone());
    assert!(worker.process_one().expect("process"));
    assert_eq!(
        state.jobs().counts().expect("counts").failed,
        1,
        "the refresh failed terminally"
    );
    assert!(
        state
            .attention()
            .list_attention_events(AttentionEventListInput::default())
            .expect("events")
            .is_empty(),
        "the Sources surface owns this failure — no generic event on top"
    );
}

#[test]
fn the_same_failed_job_never_fires_twice() {
    // Dedup on the system partial index (trigger, evidence_type, evidence_ref):
    // re-dispatching the same job row (a reclaim after a crash, a manual requeue)
    // must not raise a second event for the same failure.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let worker = build_worker(state.clone());
    for _ in 0..2 {
        state
            .jobs()
            .enqueue(
                "job_sweep_dup",
                crate::jobs::history_sweep::HISTORY_SWEEP_KIND,
                "{}",
                1,
            )
            .expect("enqueue");
        state
            .jobs()
            .defer("job_sweep_dup", 0)
            .expect("make it claimable again");
        worker.process_one().expect("process");
    }

    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput::default())
        .expect("events");
    assert_eq!(events.len(), 1, "one job row, one event");
}
