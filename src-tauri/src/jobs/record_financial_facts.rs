//! MCP batch act `record_financial_facts` orchestration (ADR 0093 decision 6,
//! epic #285 T7): the port's first batch write — a preliminary release is a
//! table, not a fact. Wraps the battle-tested [`storage::record_structured_fact`]
//! primitive (T3's `data_quality` axis) with the reusable
//! [`super::supplied_set::validate_supplied_set`] judge, typed per-fact
//! outcomes, and the ADR 0093 decision-1 `agent` source tier.
//!
//! No Tauri command twin — this is an MCP-only tool (like the four MVP read
//! composites, `mcp::registry`): the strict wire input lives in `mcp::acts`
//! (the schemars/serde boundary); this module is the pure `AppState`
//! orchestration seam the handler binds to, independent of any protocol
//! concern.
//!
//! Per-fact policy (ADR 0093 dec. 6, verified against the deterministic
//! pipeline's own precedent — [`super::supplied_set`]'s module doc):
//! `no_definition` and `implausible` are never written. `identity_violation`
//! is ALSO never written: the pipeline's own `Acceptance::Flagged` ("a
//! contradiction... was found — do not emit; notify",
//! `fundamentals::extraction::pipeline`) withholds the ENTIRE set when a
//! same-period identity fails — `Acceptance::emits()` is `false` for
//! `Flagged`, so nothing in a flagged run ever reaches persistence. This
//! module mirrors that: a fact that is one of a broken identity's inputs is
//! reported, never persisted as a "flagged" row a reader could mistake for
//! usable data. `abstained_thin_history` IS written (validation_status
//! `unreviewed`, the same status the pipeline's own `AcceptedUnreviewed`
//! nothing-proven-but-uncontradicted case uses) — an honest abstention is
//! never a silent pass, but it is not a contradiction either.

use std::str::FromStr;

use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::json;

use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::fundamentals::extraction::SourceTier;
use crate::storage::{StructuredFactCommit, StructuredFactInput};

use super::supplied_set::{validate_supplied_set, CandidateFact, FactVerdict};

/// Batch cap (ADR 0093 dec. 6 "typed refusal" on an oversized/empty batch).
pub const MAX_BATCH_FACTS: usize = 100;

/// The vocabulary [`BatchFactInput::attribution`] accepts — a slot dimension,
/// never free text (ADR 0093 context: the defect this epic closes is prose
/// landing in a slot-identity field instead of a citation).
const ATTRIBUTION_VALUES: &[&str] = &["total", "owners_of_parent", "nci"];

/// One candidate fact in the batch — already decoupled from the MCP wire
/// shape (`mcp::acts::RecordFinancialFactsInput`); this module never sees
/// `serde_json::Value` or schemars, only plain Rust borrowed from the caller.
#[derive(Debug, Clone, Copy)]
pub struct BatchFactInput<'a> {
    pub metric_key: &'a str,
    pub value_numeric: &'a str,
    pub currency: Option<&'a str>,
    /// `total` | `owners_of_parent` | `nci` — validated up front (typed
    /// refusal on garbage), never defaulted silently for a present-but-wrong
    /// token (the storage boundary's own `total` default still applies when
    /// this is `None`).
    pub attribution: Option<&'a str>,
    pub measure_window: Option<&'a str>,
    /// REQUIRED, non-blank — the mandatory per-fact citation
    /// (`ProvenanceRequirement::DocumentAndPerFactCitations`, checked by
    /// `mcp::registry::act_gate` before this function ever runs).
    pub citation: &'a str,
}

/// The batch's shared period + document context.
#[derive(Debug, Clone, Copy)]
pub struct RecordFinancialFactsInput<'a> {
    pub company_id: &'a str,
    pub report_document_id: &'a str,
    pub fiscal_year: i64,
    pub period_type: &'a str,
    pub period_end: Option<&'a str>,
    /// `None` normalizes to `final` at the storage write boundary (T3).
    pub data_quality: Option<&'a str>,
    pub facts: &'a [BatchFactInput<'a>],
}

/// One fact's outcome in the batch response (ADR 0093 dec. 6 typed-outcome
/// vocabulary).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FactOutcome {
    pub metric_key: String,
    /// `created` | `reobserved` | `upgraded` | `divergent` | `no_definition`
    /// | `implausible` | `identity_violation`.
    pub outcome: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<serde_json::Value>,
    /// `abstained_thin_history` when the plausibility gate could not run for
    /// this fact but it was written anyway — honest abstention, never a
    /// silent pass (ADR 0093 dec. 6). Absent otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plausibility: Option<&'static str>,
}

/// Informational completeness against the company's expected primary-KPI set
/// (ADR 0061 dec. 4d) — `None` when the company has no expected set
/// configured (mirrors [`crate::jobs::supplied_set::SuppliedSetVerdict`]).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCompleteness {
    pub expected: usize,
    pub matched: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordFinancialFactsResult {
    pub period_id: String,
    pub outcomes: Vec<FactOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completeness: Option<BatchCompleteness>,
}

/// Validates and commits one batch of caller-supplied financial facts (ADR
/// 0093 dec. 6). Refuses the WHOLE batch, before any write, when: the batch
/// cap is violated, an `attribution` token is not in the slot vocabulary, the
/// company doesn't exist, or the `reportDocumentId` doesn't exist. Otherwise
/// ensures the period once (so `periodId` is always returned, even when every
/// fact is skipped), judges the set via
/// [`crate::jobs::supplied_set::validate_supplied_set`], and commits each fact
/// under `source_tier=agent` / `extraction_method=mcp_agent` /
/// `confirmation_state=confirmed` (ADR 0093 dec. 1 honesty rule).
pub fn record_financial_facts(
    state: &AppState,
    input: RecordFinancialFactsInput<'_>,
) -> Result<RecordFinancialFactsResult, CommandError> {
    if input.facts.is_empty() || input.facts.len() > MAX_BATCH_FACTS {
        return Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            format!(
                "facts must contain between 1 and {MAX_BATCH_FACTS} entries, got {}",
                input.facts.len()
            ),
        ));
    }
    for fact in input.facts {
        if let Some(attribution) = fact.attribution {
            if !ATTRIBUTION_VALUES.contains(&attribution) {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    format!(
                        "fact {:?}: attribution must be one of {}, got {:?}",
                        fact.metric_key,
                        ATTRIBUTION_VALUES.join("|"),
                        attribution
                    ),
                ));
            }
        }
    }

    state
        .companies()
        .ensure_company_exists(input.company_id)
        .map_err(CommandError::from)?;
    state
        .report_documents()
        .ensure_report_document_exists(input.report_document_id)
        .map_err(CommandError::from)?;

    // Ensured up front (not just as a side effect of the first successful
    // write) so `periodId` is always present in the response, even when
    // every submitted fact is skipped (no_definition/implausible/
    // identity_violation never reach `record_structured_fact`).
    let period_id = state
        .kpi_extraction()
        .ensure_financial_period(
            input.company_id,
            input.fiscal_year,
            input.period_type,
            input.period_end,
            input.report_document_id,
        )
        .map_err(CommandError::from)?;

    let candidates = parse_candidates(input.facts)?;
    let verdict = validate_supplied_set(
        state,
        input.company_id,
        input.fiscal_year,
        input.period_type,
        &candidates,
    )
    .map_err(|message| CommandError::new(CommandErrorCode::Internal, message))?;

    let outcomes = input
        .facts
        .iter()
        .zip(verdict.facts.iter())
        .map(|(fact, fact_verdict)| commit_one_fact(state, &input, fact, fact_verdict))
        .collect::<Result<Vec<_>, CommandError>>()?;

    let completeness = verdict.completeness.map(|c| BatchCompleteness {
        expected: c.expected,
        matched: c.present,
    });

    Ok(RecordFinancialFactsResult {
        period_id,
        outcomes,
        completeness,
    })
}

fn parse_candidates(facts: &[BatchFactInput<'_>]) -> Result<Vec<CandidateFact>, CommandError> {
    facts
        .iter()
        .map(|fact| {
            Decimal::from_str(fact.value_numeric.trim())
                .map(|value| CandidateFact {
                    metric_key: fact.metric_key.to_owned(),
                    value,
                })
                .map_err(|_| {
                    CommandError::new(
                        CommandErrorCode::InvalidInput,
                        format!(
                            "fact {:?}: valueNumeric {:?} is not a decimal",
                            fact.metric_key, fact.value_numeric
                        ),
                    )
                })
        })
        .collect()
}

fn commit_one_fact(
    state: &AppState,
    input: &RecordFinancialFactsInput<'_>,
    fact: &BatchFactInput<'_>,
    verdict: &FactVerdict,
) -> Result<FactOutcome, CommandError> {
    match verdict {
        FactVerdict::Implausible { history_median } => Ok(FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "implausible",
            detail: Some(json!({ "historyMedian": history_median.to_string() })),
            plausibility: None,
        }),
        // Never written — see the module doc's pipeline-precedent rationale.
        FactVerdict::IdentityViolation { check } => Ok(FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "identity_violation",
            detail: Some(json!({ "check": check })),
            plausibility: None,
        }),
        FactVerdict::Pass | FactVerdict::AbstainedThinHistory => {
            write_fact(state, input, fact, verdict)
        }
    }
}

fn write_fact(
    state: &AppState,
    input: &RecordFinancialFactsInput<'_>,
    fact: &BatchFactInput<'_>,
    verdict: &FactVerdict,
) -> Result<FactOutcome, CommandError> {
    let abstained = matches!(verdict, FactVerdict::AbstainedThinHistory);
    let validation_status = if abstained { "unreviewed" } else { "passed" };
    let plausibility = abstained.then_some("abstained_thin_history");

    let commit = state
        .kpi_extraction()
        .record_structured_fact(StructuredFactInput {
            company_id: input.company_id,
            fiscal_year: input.fiscal_year,
            period_type: input.period_type,
            period_end: input.period_end,
            report_document_id: input.report_document_id,
            metric_key: fact.metric_key,
            value_numeric: fact.value_numeric,
            currency: fact.currency,
            confirmation_state: "confirmed",
            source_tier: SourceTier::Agent.as_str(),
            extraction_method: "mcp_agent",
            validation_status,
            drift_json: None,
            citation: Some(fact.citation),
            attribution: fact.attribution,
            measure_window: fact.measure_window,
            data_quality: input.data_quality,
        })
        .map_err(CommandError::from)?;

    Ok(match commit {
        StructuredFactCommit::Created(_) => FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "created",
            detail: None,
            plausibility,
        },
        StructuredFactCommit::Reobserved(_) => FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "reobserved",
            detail: None,
            plausibility,
        },
        StructuredFactCommit::Upgraded {
            previous_tier,
            previous_value,
            ..
        } => FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "upgraded",
            detail: Some(json!({ "previousTier": previous_tier, "previousValue": previous_value })),
            plausibility,
        },
        StructuredFactCommit::Divergent {
            existing, incoming, ..
        } => FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "divergent",
            detail: Some(json!({ "existing": existing, "incoming": incoming })),
            plausibility,
        },
        // Defensive: `metric_key` names no catalog definition. Never a silent
        // drop — the agent is pointed at the fix.
        StructuredFactCommit::NoDefinition => FactOutcome {
            metric_key: fact.metric_key.to_owned(),
            outcome: "no_definition",
            detail: Some(json!({
                "hint": "no KPI definition for this metricKey — create one with create_kpi_definition, then retry"
            })),
            plausibility: None,
        },
    })
}

#[cfg(test)]
mod tests;
