//! One-off `rebuild fundamentals` driver (ADR 0086 decision 6, plan TOR C
//! slice C4).
//!
//! After the owner-approved manual wipe of all `financial_facts` (a deliberate
//! one-time SQL cleanup on the live DB — NOT a migration; migrations stay
//! schema-only and never delete user data), this command repopulates the store
//! from the three issuer-independent, spec-driven surfaces of the new extraction
//! ladder (ADR 0086 dec. 3), in sequence:
//!
//! 1. **BiznesRadar-primary pull** — core KPIs for every tracked company × every
//!    period column of the three cached report pages
//!    ([`crate::jobs::aggregator_fundamentals_pull::run_aggregator_fundamentals_pull`]).
//!    Respects the per-page daily cadence: after a wipe the FACTS are gone but a
//!    cached page may still be fresh, and parsing the cache without refetching is
//!    exactly right.
//! 2. **ESEF re-extraction** — force-re-run the deterministic structured
//!    extraction over every stored ESEF-route document of every company
//!    ([`crate::jobs::health_facts_backfill::backfill_company_health_facts`],
//!    generalized to a full per-company pass). PDF documents spawn no attempt
//!    (post-C3 semantics — the route early-returns), so they are not special-cased.
//! 3. **WDF re-scan** — re-run the cover-note tier over every stored feed item
//!    whose "WYBRANE DANE FINANSOWE" carrier body survives
//!    ([`crate::storage::AppState::rescan_stored_cover_note_facts`]). Pruned
//!    bodies are lost by design and counted, never guessed.
//!
//! Every pass is tolerant of per-item failure: an error on one company/document
//! is collected and the rebuild proceeds — one bad document never aborts the
//! whole repopulation. The write paths are all review-free (every fact lands
//! `confirmed`, ADR 0086 dec. 5) and slot-aware, so running the rebuild twice
//! duplicates nothing (a second run re-observes existing slots).
//!
//! Facts land per current precedence (`manual > esef > espi_cover_note >
//! positional > html_aggregator`); the returned summary carries the per-pass
//! counts AND the post-rebuild fact totals per `source_tier` (plus the manual /
//! no-provenance bucket), so the operator gets the before/after verdict without
//! extra queries.
//!
//! Headless-only (ADR 0086 dec. 6): there is no UI entry point — the command is
//! invoked once via live-drive after the manual wipe.

use serde::Serialize;

use crate::app_state::AppState;
use crate::jobs::aggregator_fundamentals_pull::{
    run_aggregator_fundamentals_pull_serialized, AggregatorPullSummary,
};
use crate::jobs::health_facts_backfill::backfill_company_health_facts;
use crate::storage::FactTierBreakdown;

/// What one full rebuild did, for the on-demand verdict. Serialized to the IPC
/// contract (`ts-rs`), mirroring [`AggregatorPullSummary`].
#[derive(Debug, Default, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RebuildFundamentalsSummary {
    /// Pass 1 — the embedded BiznesRadar-primary pull summary.
    pub aggregator: AggregatorPullSummary,
    /// Pass 2 — ESEF-route documents actually re-extracted.
    pub esef_documents_processed: i64,
    /// Pass 2 — ESEF facts written OR tier-upgraded this pass. An ESEF fact may
    /// take over a slot pass 1's BiznesRadar pull wrote (ADR 0086 dec. 3 — the
    /// issuer tier outranks `html_aggregator`), so this is not "genuinely new":
    /// `factsByTier` is the deduplicated post-rebuild verdict.
    pub esef_facts_emitted: i64,
    /// Pass 2 — ESEF slots re-observed with the same value (idempotent no-ops).
    pub esef_facts_reobserved: i64,
    /// Pass 2 — ESEF re-observations whose value disagreed with the stored fact
    /// (surfaced, never overwritten).
    pub esef_divergences: i64,
    /// Pass 2 — companies whose ESEF re-extraction raised an error (collected,
    /// never aborting the rebuild).
    pub esef_errors: i64,
    /// Pass 3 — WDF cover-note carriers (stored bodies) scanned.
    pub wdf_carriers_scanned: i64,
    /// Pass 3 — WDF facts written or tier-upgraded.
    pub wdf_facts_written: i64,
    /// Pass 3 — periodic-report komunikaty whose body was pruned (lost by design).
    pub wdf_skipped_no_body: i64,
    /// The post-rebuild fact totals per `source_tier` + the manual / no-provenance
    /// bucket — the before/after verdict.
    pub facts_by_tier: FactTierBreakdown,
    /// Human-readable per-pass errors collected without aborting the rebuild.
    pub errors: Vec<String>,
}

/// Run the full three-pass rebuild over every tracked company. Synchronous and
/// offloaded by the caller (the command via `run_blocking_task`). Sequential,
/// and tolerant of per-item failure: an error on one company/document is
/// collected into `summary.errors` and the rebuild proceeds.
pub fn run_rebuild_fundamentals(state: &AppState) -> Result<RebuildFundamentalsSummary, String> {
    let mut summary = RebuildFundamentalsSummary::default();

    // --- Pass 1: BiznesRadar-primary pull (core KPIs, all companies × periods) ---
    // Internally per-company tolerant; a top-level failure (e.g. the company list
    // itself, or the per-adapter lock held by an in-flight pull — issue #132) is
    // collected without aborting the ESEF/WDF passes and surfaces in `errors`.
    match run_aggregator_fundamentals_pull_serialized(state) {
        Ok(aggregator) => summary.aggregator = aggregator,
        Err(error) => summary
            .errors
            .push(format!("aggregator pull failed: {error}")),
    }

    // --- Pass 2: ESEF re-extraction (force re-run over every stored ESEF doc) ---
    // Per company so one company's failure never denies the others their facts.
    match state.list_companies() {
        Ok(companies) => {
            for company in &companies {
                match backfill_company_health_facts(state, &company.id) {
                    Ok(result) => {
                        summary.esef_documents_processed += result.documents_processed as i64;
                        summary.esef_facts_emitted += result.facts_created as i64;
                        summary.esef_facts_reobserved += result.facts_reobserved as i64;
                        summary.esef_divergences += result.divergences as i64;
                    }
                    Err(error) => {
                        summary.esef_errors += 1;
                        summary
                            .errors
                            .push(format!("esef re-extraction ({}): {error}", company.id));
                    }
                }
            }
        }
        Err(error) => summary
            .errors
            .push(format!("esef pass: list_companies failed: {error}")),
    }

    // --- Pass 3: WDF re-scan (stored cover-note carriers whose body survives) ---
    match state.rescan_stored_cover_note_facts() {
        Ok(rescan) => {
            summary.wdf_carriers_scanned = rescan.carriers_scanned as i64;
            summary.wdf_facts_written = rescan.facts_written as i64;
            summary.wdf_skipped_no_body = rescan.skipped_no_body as i64;
            if rescan.errors > 0 {
                summary
                    .errors
                    .push(format!("wdf re-scan: {} per-item error(s)", rescan.errors));
            }
        }
        Err(error) => summary.errors.push(format!("wdf re-scan failed: {error}")),
    }

    // --- Verdict: post-rebuild fact totals per tier (best-effort) ---
    match state.count_facts_by_tier() {
        Ok(breakdown) => summary.facts_by_tier = breakdown,
        Err(error) => summary
            .errors
            .push(format!("count_facts_by_tier failed: {error}")),
    }

    log::info!(
        "module=rebuild_fundamentals stage=done aggregator_written={} esef_docs={} \
         esef_emitted={} wdf_scanned={} wdf_written={} wdf_pruned={} errors={}",
        summary.aggregator.facts_written,
        summary.esef_documents_processed,
        summary.esef_facts_emitted,
        summary.wdf_carriers_scanned,
        summary.wdf_facts_written,
        summary.wdf_skipped_no_body,
        summary.errors.len(),
    );

    Ok(summary)
}

#[cfg(test)]
mod tests;
