//! Where a job kind's TERMINAL failure becomes visible to the user (ADR 0091
//! dec. 1 + 3, epic #40 S3).
//!
//! Recon of the post-v0.50 defect record found 5 of the 12 registered job kinds
//! with no user-visible failure path at all: a background job could exhaust its
//! retries and die in silence, with `job_queue` having no read model, no command
//! and no screen. This module is the enforceable half of the fix.
//!
//! Every registered kind MUST classify its failure surface here:
//!
//! - [`FailureSurface::TodayAttention`] — the generic surface: a terminal failure
//!   raises a system `job_failed` attention event (severity Notable for every
//!   kind) that the Today stream renders. The default for a kind with no richer
//!   domain surface.
//! - [`FailureSurface::SourcesAdapterHealth`] — the kind's own failure path already
//!   writes `record_source_adapter_error`, so the Sources screen states it per
//!   source row.
//! - [`FailureSurface::AutopilotRunCard`] — the run itself is finalized `failed`
//!   with its `last_error`, and the autopilot run card states it.
//!
//! The map is **exclusive**: a kind with a richer domain surface keeps that
//! surface ONLY, so one failure never fires twice (ADR 0091 dec. 1). Dev-gated
//! Diagnostics is explicitly NOT an acceptable surface.
//!
//! The gate below derives its inventory from `registered_kinds()` (the real
//! registry, never a hand copy), so a NEW job kind reddens here until it is
//! classified — and again in its per-surface visibility test until its failure is
//! proven to reach that surface.

/// The one user-visible place a terminally failed job of a given kind shows up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureSurface {
    /// A system `job_failed` attention event in the Today stream (the generic surface).
    TodayAttention,
    /// The source adapter's own health row on the Sources screen.
    SourcesAdapterHealth,
    /// The autopilot run card (the run is finalized `failed` with its error).
    AutopilotRunCard,
}

/// The failure surface of one job `kind`, or `None` when the kind is not
/// classified — which the enumeration gate treats as a defect, never a default.
pub fn failure_surface(kind: &str) -> Option<FailureSurface> {
    use crate::jobs::aggregator_fundamentals_pull::AGGREGATOR_FUNDAMENTALS_PULL_KIND;
    use crate::jobs::autopilot::AUTOPILOT_STAGE_KIND;
    use crate::jobs::backfill::COMPANY_BACKFILL_KIND;
    use crate::jobs::fx_daily_pull::FX_DAILY_PULL_KIND;
    use crate::jobs::history_sweep::HISTORY_SWEEP_KIND;
    use crate::jobs::kpi_ingest_queue::{KPI_INGEST_COMMIT_KIND, KPI_INGEST_VALIDATE_KIND};
    use crate::jobs::management_holdings_extraction::MANAGEMENT_EXTRACTION_KIND;
    use crate::jobs::morning_briefing::MORNING_BRIEFING_KIND;
    use crate::jobs::ownership_extraction::OWNERSHIP_EXTRACTION_KIND;
    use crate::jobs::quote_backfill::QUOTE_BACKFILL_KIND;
    use crate::jobs::scheduler::{REGISTRY_REFRESH_KIND, SOURCE_REFRESH_KIND};
    use crate::jobs::source_refresh::SOURCE_COMPANY_REFRESH_KIND;

    Some(match kind {
        // Source-refresh family: every failure path in these jobs calls
        // `record_source_adapter_error`, so the Sources screen already states WHICH
        // source failed and why. Exclusive — no generic event on top. The registry
        // refresh runs the GPW + NewConnect directories, which record their own
        // adapter errors; the NBP FX pull records its own before returning Err.
        SOURCE_REFRESH_KIND
        | SOURCE_COMPANY_REFRESH_KIND
        | REGISTRY_REFRESH_KIND
        | FX_DAILY_PULL_KIND => FailureSurface::SourcesAdapterHealth,
        // An autopilot stage finalizes its run as `failed` with the stage + error;
        // the run card (and its completion attention event) is the richer surface.
        AUTOPILOT_STAGE_KIND => FailureSurface::AutopilotRunCard,
        // Everything else has no domain surface of its own: the deterministic
        // extractions, the sweeps, the briefing, the quote/company backfills and the
        // aggregator pull all die silently today. They take the generic surface.
        // (The company backfill records an adapter fault only for a genuine
        // Bankier-interaction fault, and never returns Err after doing so — its
        // terminal failure is a malformed payload — so there is no double fire.)
        // The KPI ingest steps (#364) also take the generic surface: the run's
        // own `failed` + `last_error` has no UI in E1 (runs surface over MCP in
        // #353), so the Today event is the one user-visible path. The domain
        // transition itself happens in the handlers' `on_terminal_failure`
        // hook, AFTER this event fires — never a double fire, the event and
        // the run state describe the same terminal failure once each.
        MORNING_BRIEFING_KIND
        | HISTORY_SWEEP_KIND
        | OWNERSHIP_EXTRACTION_KIND
        | MANAGEMENT_EXTRACTION_KIND
        | QUOTE_BACKFILL_KIND
        | COMPANY_BACKFILL_KIND
        | AGGREGATOR_FUNDAMENTALS_PULL_KIND
        | KPI_INGEST_VALIDATE_KIND
        | KPI_INGEST_COMMIT_KIND => FailureSurface::TodayAttention,
        _ => return None,
    })
}

#[cfg(test)]
mod tests;
