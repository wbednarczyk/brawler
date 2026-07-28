//! BiznesRadar-PRIMARY fundamentals pull (ADR 0086 decision 2, plan TOR C slice
//! C2).
//!
//! For every tracked company, resolve the three robots-allowed report pages
//! (income / balance / cash flow) under the per-page daily cadence
//! ([`crate::source_adapters::biznesradar_fundamentals::resolve_aggregator_page`]),
//! parse EVERY period column each page carries
//! ([`crate::fundamentals::extraction::html::parse_all_financials`]), and write a
//! fact per (period × metric) under `source_tier = html_aggregator`.
//!
//! ## Tier precedence (ADR 0086 decision 3)
//! Each slot is written through
//! [`crate::storage::KpiExtractionStore::record_aggregator_fact`], which applies
//! `manual > esef > espi_cover_note > positional > html_aggregator`: an empty slot
//! is filled, the aggregator's OWN slot is overwritten with the fresh value, and a
//! manual or higher-tier slot is left untouched.
//!
//! ## Reversed witnessing (ADR 0086 decision 4)
//! Where an ISSUER tier (ESEF / structured xHTML / WDF cover-note) holds a slot and
//! the aggregator value diverges beyond the shared tolerance, an informational
//! `witness_disagreement` extraction outcome is recorded — never blocking, never
//! overwriting the issuer value.
//!
//! ## Zero rule (ADR 0085 amendment)
//! An empty / dash / zero aggregator cell is NEVER written and never counts as
//! evidence — the cell BiznesRadar renders `0` for an unreported line is a scrape
//! artifact, not a filed value.
//!
//! ## Queue lane (ADR 0059)
//! [`AggregatorFundamentalsPullHandler`] is a durable-queue job in the **`sources`**
//! lane and serializes on the `biznesradar-fundamenty` adapter id (the same
//! per-adapter host-politeness posture the two other live BiznesRadar adapters
//! use), so at most one pull runs at a time and it shares the BiznesRadar politeness
//! with them. It is deliberately NOT part of the manual "Odśwież źródła" sweep — it
//! is driven by its own daily cadence and invoked on demand by the rebuild flow.

use serde::Serialize;

use crate::app_state::AppState;
use crate::fundamentals::extraction::html::{parse_all_financials, AggregatorPeriod};
use crate::fundamentals::extraction::{ExtractedFact, SourceTier};
use crate::fundamentals::validation::{CrossCheck, FactSet, Outcome, Tolerance};
use crate::jobs::queue::JobHandler;
use crate::source_adapters::biznesradar_fundamentals::{
    resolve_aggregator_page, witness_cross_check, PageResolution, ADAPTER_ID,
};
use crate::storage::{
    AggregatorFactCommit, AggregatorPageKind, NewExtractionOutcome, StructuredFactInput,
};

/// Durable-queue job kind for the BiznesRadar-primary fundamentals pull.
pub const AGGREGATOR_FUNDAMENTALS_PULL_KIND: &str = "aggregator_fundamentals_pull";

/// Stable queue job id for the daily auto-trigger — rescheduling reuses this one
/// row, so the queue holds at most one pending pull regardless of tick churn.
const DAILY_JOB_ID: &str = "aggregator_fundamentals_pull_daily";

/// Arm (or re-arm) the daily pull on the durable queue. Called by the scheduler's
/// app-open daily tick (`jobs::scheduler::spawn`); idempotent via the stable job
/// id, and cheap even when redundant — inside the 24h window the pull reuses its
/// per-(company, page_kind) cache and fetches nothing.
pub fn enqueue_daily_pull(state: &AppState) {
    if let Err(error) =
        state
            .jobs()
            .reschedule(DAILY_JOB_ID, AGGREGATOR_FUNDAMENTALS_PULL_KIND, "{}", 2)
    {
        log::warn!("failed to enqueue daily aggregator fundamentals pull: {error}");
    }
}

/// Whether a stored `source_tier` names an ISSUER tier — one read from the
/// issuer's own filing, whose held slot records a reversed-witnessing
/// `witness_disagreement` when the aggregator diverges (ADR 0086 decision 4,
/// amended 2026-07-22). Delegates to the canonical [`SourceTier`] taxonomy rather
/// than string-matching, so the positional `pdf` tier (the issuer's filing read
/// deterministically) is covered and a new tier cannot be silently omitted. A
/// `manual` slot (parses to `None`) is handled separately by the caller.
fn is_issuer_tier(tier: &str) -> bool {
    SourceTier::parse(tier).is_some_and(SourceTier::is_issuer)
}

/// What one pull run did, for the on-demand command + the rebuild verdict.
#[derive(Debug, Default, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AggregatorPullSummary {
    /// Companies iterated.
    pub companies: i64,
    /// Report pages resolved to a table (cache hit or fresh fetch).
    pub pages_resolved: i64,
    /// Report pages that could not be resolved (no coverage / fetch failed / no slug).
    pub pages_unavailable: i64,
    /// New aggregator facts written into an empty slot.
    pub facts_written: i64,
    /// Aggregator facts overwriting the aggregator's own prior slot value.
    pub facts_updated: i64,
    /// Slots whose value the aggregator already held identically (no-op).
    pub facts_reobserved: i64,
    /// Slots left untouched because a manual or higher tier holds them.
    pub slots_skipped_higher_tier: i64,
    /// Reversed-witnessing disagreements recorded against an issuer-held slot.
    pub witness_disagreements: i64,
    /// Empty / zero aggregator cells skipped (the zero rule) — never written.
    pub zero_cells_skipped: i64,
    /// Metric cells with no catalog KPI definition (defensive skip).
    pub no_definition: i64,
    /// Guardrail G3 (review 2026-07-22): metrics whose aggregator value disagreed
    /// with issuer/manual-held slots at `MAPPING_SUSPECT_MIN_COMPANIES`+ distinct
    /// companies in THIS run — a systematic disagreement is a dictionary-mapping
    /// suspect (the finding-1 signature), not per-company noise.
    pub mapping_suspects: Vec<String>,
}

/// G3 threshold: the same metric disagreeing at this many distinct companies in
/// one pull run flags a `mapping_suspect` (logged + diagnostic + summary), while
/// scattered per-company disagreements stay informational.
const MAPPING_SUSPECT_MIN_COMPANIES: usize = 5;

/// The on-demand entry point (the `run_aggregator_fundamentals_pull` command and
/// the rebuild's pass 1): same per-adapter serialization as the queue path
/// (issue #132) — the politeness posture is **at most one BiznesRadar pull at a
/// time**, regardless of who triggered it. The queue's dispatch acquires this
/// lock itself (via `serialization_key`), so a racing on-demand run is rejected
/// with a typed busy error instead of double-fetching the same pages and
/// caching a late failure over an earlier success (C4 live-rebuild finding,
/// 2026-07-22).
pub fn run_aggregator_fundamentals_pull_serialized(
    state: &AppState,
) -> Result<AggregatorPullSummary, String> {
    let _guard = state.try_acquire_source(ADAPTER_ID).ok_or_else(|| {
        "aggregator_pull_already_running: a BiznesRadar fundamentals pull is already in flight \
         (daily job or another on-demand run) — retry after it finishes"
            .to_owned()
    })?;
    run_aggregator_fundamentals_pull(state)
}

/// Run the full BiznesRadar-primary pull over every tracked company. Synchronous
/// and offloaded by the caller (the command via `run_blocking_task`, the queue via
/// a worker thread). Callers other than the queue dispatch (which holds the
/// per-adapter lock already) go through
/// [`run_aggregator_fundamentals_pull_serialized`].
pub fn run_aggregator_fundamentals_pull(state: &AppState) -> Result<AggregatorPullSummary, String> {
    let companies = state.list_companies().map_err(|error| error.to_string())?;
    let tolerance = Tolerance::default();
    let mut summary = AggregatorPullSummary::default();
    // metric_key -> distinct companies whose held slot the aggregator contradicted.
    let mut disagreement_ledger: std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    > = std::collections::BTreeMap::new();

    for company in &companies {
        summary.companies += 1;
        for kind in AggregatorPageKind::ALL {
            match resolve_aggregator_page(state, &company.id, kind) {
                PageResolution::Page { html, page_url } => {
                    summary.pages_resolved += 1;
                    pull_one_page(
                        state,
                        &company.id,
                        &html,
                        &page_url,
                        &tolerance,
                        &mut summary,
                        &mut disagreement_ledger,
                    )?;
                }
                PageResolution::Unavailable(_) => {
                    summary.pages_unavailable += 1;
                }
            }
        }
    }

    // G3: a metric contradicted at many companies at once is a mapping suspect —
    // the exact signature of a dictionary row filed under the wrong metric
    // (finding 1: BR parent equity as group total_equity would have lit up here
    // at 30+ companies). Logged + best-effort diagnostic + surfaced on the summary.
    for (metric_key, companies_hit) in &disagreement_ledger {
        if companies_hit.len() >= MAPPING_SUSPECT_MIN_COMPANIES {
            summary.mapping_suspects.push(metric_key.clone());
            log::warn!(
                "module=aggregator_fundamentals_pull stage=mapping_suspect metric={metric_key} \
                 companies={} — systematic issuer-vs-aggregator disagreement; check the \
                 dictionary mapping for this row",
                companies_hit.len()
            );
            let _ = state.record_diagnostic_event(crate::storage::NewDiagnosticEvent {
                occurred_at: None,
                module: "aggregator_fundamentals_pull".to_owned(),
                scope: None,
                stage: "mapping_suspect".to_owned(),
                severity: "warning".to_owned(),
                message:
                    "systematic issuer-vs-aggregator disagreement — dictionary mapping suspect"
                        .to_owned(),
                metadata: Some(serde_json::json!({
                    "metricKey": metric_key,
                    "companies": companies_hit.len(),
                })),
            });
        }
    }

    log::info!(
        "module=aggregator_fundamentals_pull stage=done companies={} written={} updated={} \
         reobserved={} skipped_higher={} disagreements={} zero_skipped={}",
        summary.companies,
        summary.facts_written,
        summary.facts_updated,
        summary.facts_reobserved,
        summary.slots_skipped_higher_tier,
        summary.witness_disagreements,
        summary.zero_cells_skipped,
    );
    Ok(summary)
}

/// One non-zero aggregator cell buffered for the page-level batched write: the
/// parsed period/fact it came from (borrowed from the page's parse) plus the owned
/// `value`/`citation` strings the [`StructuredFactInput`] borrows.
struct PendingAggregatorFact<'a> {
    period: &'a AggregatorPeriod,
    fact: &'a ExtractedFact,
    value: String,
    citation: String,
}

/// Write one aggregator page's facts under ONE transaction (ADR 0086 dec. 2 /
/// perf): the zero rule is applied while collecting, the whole page is written via
/// the batched store call ([`crate::storage::KpiExtractionStore::record_aggregator_facts`]),
/// and the tier-precedence bookkeeping + reversed witnessing run OUTSIDE that
/// transaction on the returned per-fact commits — the same per-fact outcomes and
/// summary counters as the prior per-fact path, just one checkout+commit per page.
#[allow(clippy::too_many_arguments)]
fn pull_one_page(
    state: &AppState,
    company_id: &str,
    html: &str,
    page_url: &str,
    tolerance: &Tolerance,
    summary: &mut AggregatorPullSummary,
    disagreement_ledger: &mut std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
) -> Result<(), String> {
    let parsed = parse_all_financials(html);

    // Collect every non-zero cell, applying the zero rule up front (ADR 0085
    // amendment): an empty/zero aggregator cell is never a filed value — never
    // written, never evidence.
    let mut pending: Vec<PendingAggregatorFact<'_>> = Vec::new();
    for (period, facts) in &parsed {
        for fact in facts {
            if fact.value.is_zero() {
                summary.zero_cells_skipped += 1;
                continue;
            }
            pending.push(PendingAggregatorFact {
                period,
                fact,
                value: fact.value.to_string(),
                // Attribution travels WITH the value (ADR 0086): the citation names
                // the row label and the aggregator page it was read from.
                citation: format!("{} | {page_url}", fact.citation),
            });
        }
    }

    let inputs: Vec<StructuredFactInput<'_>> = pending
        .iter()
        .map(|p| StructuredFactInput {
            company_id,
            fiscal_year: p.period.fiscal_year,
            period_type: p.period.period_type,
            period_end: Some(&p.period.period_end),
            // The BR page is the evidence, not a stored report document — the
            // citation already names it; this feeds `source_document_ref`.
            report_document_id: page_url,
            metric_key: &p.fact.metric_key,
            value_numeric: &p.value,
            currency: Some("PLN"),
            // Review-free (ADR 0086 decision 5): every writer stamps `confirmed`;
            // trust lives in `source_tier` + `extraction_method` + citation.
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some(&p.citation),
        })
        .collect();

    // ONE transaction for the whole page (the ~9k-fsync-a-run fix).
    let commits = state
        .kpi_extraction()
        .record_aggregator_facts(&inputs)
        .map_err(|error| error.to_string())?;

    for (p, commit) in pending.iter().zip(commits) {
        apply_commit(
            state,
            company_id,
            disagreement_ledger,
            p.period,
            p.fact,
            page_url,
            tolerance,
            summary,
            commit,
        )?;
    }
    Ok(())
}

/// Tally one committed aggregator write into the summary and, for a slot held by
/// an issuer or manual tier, record the reversed-witnessing disagreement. Runs
/// OUTSIDE the page write transaction (as the per-fact path did).
#[allow(clippy::too_many_arguments)]
fn apply_commit(
    state: &AppState,
    company_id: &str,
    disagreement_ledger: &mut std::collections::BTreeMap<
        String,
        std::collections::BTreeSet<String>,
    >,
    period: &AggregatorPeriod,
    fact: &ExtractedFact,
    page_url: &str,
    tolerance: &Tolerance,
    summary: &mut AggregatorPullSummary,
    commit: AggregatorFactCommit,
) -> Result<(), String> {
    match commit {
        AggregatorFactCommit::Created(_) => summary.facts_written += 1,
        AggregatorFactCommit::Updated(_) => summary.facts_updated += 1,
        AggregatorFactCommit::Reobserved(_) => summary.facts_reobserved += 1,
        AggregatorFactCommit::NoDefinition => summary.no_definition += 1,
        AggregatorFactCommit::SkippedHigherTier {
            existing_tier,
            existing_method,
            existing_value,
            ..
        } => {
            summary.slots_skipped_higher_tier += 1;
            // Reversed witnessing (ADR 0086 decision 4, amended 2026-07-22). A
            // genuine divergence (beyond tolerance, aggregator side non-zero)
            // records an informational `witness_disagreement` when the held slot is
            // EITHER an issuer tier (ESEF / structured xHTML / WDF cover-note / the
            // positional `pdf` tier — all the issuer's own filing) OR a MANUAL slot
            // (the user's own entry — ADR 0086 decision 3 "divergence is logged,
            // never applied", made concrete so the user learns of the conflict). It
            // never blocks and never overwrites. An unknown/legacy tier stays silent.
            let is_manual = existing_method == "manual";
            if is_issuer_tier(&existing_tier) || is_manual {
                if let Ok(issuer_value) = existing_value.trim().parse::<rust_decimal::Decimal>() {
                    let issuer_set = single(&fact.metric_key, issuer_value);
                    let aggregator_set = single(&fact.metric_key, fact.value);
                    // `witness_cross_check(primary=issuer, aggregator=BR)` applies the
                    // zero-guard on the aggregator side and flags a real disagreement.
                    let checks = witness_cross_check(&issuer_set, &aggregator_set, tolerance);
                    if checks.iter().any(|check| check.outcome.is_fail()) {
                        disagreement_ledger
                            .entry(fact.metric_key.clone())
                            .or_default()
                            .insert(company_id.to_owned());
                        record_disagreement(
                            state,
                            &HeldSlot {
                                company_id,
                                period,
                                fact,
                                page_url,
                                existing_tier: &existing_tier,
                                existing_value: &existing_value,
                            },
                            &checks,
                        )?;
                        summary.witness_disagreements += 1;
                    }
                }
            }
        }
    }
    Ok(())
}

fn single(metric_key: &str, value: rust_decimal::Decimal) -> FactSet {
    let mut set = FactSet::new();
    set.insert(metric_key.to_owned(), value);
    set
}

/// Record an informational `witness_disagreement` outcome — the held (issuer or
/// manual) value is kept, the aggregator value is logged, emission is never
/// blocked.
///
/// The detail is the CANONICAL gate shape the WDF witness seam writes
/// ([`crate::storage::espi_cover_note_facts`] `corroborate_with_witness`):
/// `{failedIdentities, failedCrossChecks, witnessDisagreements:[{metricKey,
/// detail:{expected, actual, residual, …}}]}`. The Coverage "Flagged periods"
/// panel renders exactly this shape as investor language
/// (`CoverageFlaggedPeriods.tsx` `GATE_DETAIL_KEYS` / `gateCheckValue`), so no raw
/// JSON key ever reaches the user. Convention (ADR 0085 decision 2, shared by the
/// FE): `expected` = aggregator, `actual` = the filing/manual value. The extra
/// context (`pageUrl`, `sourceAdapterId`, `issuerTier`) is nested INSIDE `detail`,
/// where `gateCheckValue` reads only `expected`/`actual`/`residual` and silently
/// ignores the rest — it is preserved for programmatic inspection without leaking.
/// The held slot a reversed-witnessing disagreement is recorded against — the
/// issuer/manual value that OUTRANKS the aggregator and stays in place.
struct HeldSlot<'a> {
    company_id: &'a str,
    period: &'a AggregatorPeriod,
    fact: &'a ExtractedFact,
    page_url: &'a str,
    existing_tier: &'a str,
    existing_value: &'a str,
}

fn record_disagreement(
    state: &AppState,
    slot: &HeldSlot<'_>,
    checks: &[CrossCheck],
) -> Result<(), String> {
    let HeldSlot {
        company_id,
        period,
        fact,
        page_url,
        existing_tier,
        existing_value,
    } = *slot;
    let disagreements: Vec<serde_json::Value> = checks
        .iter()
        .filter_map(|check| match &check.outcome {
            Outcome::Fail {
                expected,
                actual,
                residual,
            } => Some(serde_json::json!({
                "metricKey": check.metric_key,
                "detail": {
                    "expected": expected.to_string(),
                    "actual": actual.to_string(),
                    "residual": residual.to_string(),
                    "pageUrl": page_url,
                    "sourceAdapterId": ADAPTER_ID,
                    "issuerTier": existing_tier,
                },
            })),
            _ => None,
        })
        .collect();
    let detail = serde_json::json!({
        "failedIdentities": [],
        "failedCrossChecks": [],
        "witnessDisagreements": disagreements,
    })
    .to_string();

    // ADR 0086 decision 3 "divergence is logged" made concrete — a structured line
    // for the issuer AND manual paths alike.
    log::info!(
        "module=aggregator_fundamentals_pull stage=witness_disagreement company={company_id} \
         metric={} period={} tier={existing_tier} aggregator={} held={existing_value}",
        fact.metric_key,
        period.period_end,
        fact.value,
    );

    // One outcome slot per (company, page+metric, period): the metric is folded
    // into the document-ref discriminator so two diverging metrics in one period
    // do not overwrite each other's record.
    let outcome_ref = format!("{page_url}#{}", fact.metric_key);
    state
        .fundamentals_provenance()
        .record_extraction_outcome(NewExtractionOutcome {
            company_id,
            report_document_id: &outcome_ref,
            fiscal_year: period.fiscal_year,
            period_type: period.period_type,
            period_end: &period.period_end,
            tier: Some(existing_tier),
            acceptance: "flagged",
            reason_code: "witness_disagreement",
            detail_json: Some(&detail),
            drift_json: None,
            structure_changed: false,
            fact_count: 0,
        })
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// Durable-queue handler for the daily/queued pull (ADR 0059 `sources` lane).
pub struct AggregatorFundamentalsPullHandler;

impl JobHandler for AggregatorFundamentalsPullHandler {
    fn kind(&self) -> &'static str {
        AGGREGATOR_FUNDAMENTALS_PULL_KIND
    }

    /// Serialize on the aggregator adapter id so at most one pull runs at a time
    /// and it shares the per-adapter BiznesRadar politeness posture (ADR 0059).
    fn serialization_key(&self, _payload: &str) -> Option<String> {
        Some(ADAPTER_ID.to_owned())
    }

    fn run(&self, _payload: &str, state: &AppState) -> Result<(), String> {
        run_aggregator_fundamentals_pull(state).map(|_| ())
    }
}

#[cfg(test)]
mod tests;
