//! Reusable validation/quarantine service for a caller-supplied candidate fact
//! set (ADR 0093 decision 6).
//!
//! [`crate::jobs::structured_extraction`] already decides Accepted/Flagged for
//! a whole extraction run BEFORE its per-fact history-plausibility quarantine
//! runs (the tiered arm ~975-1030, the positional arm ~1265-1300, both
//! duplicating the same `metric_histories` + `implausible_against_history`
//! wiring) — a Flagged set never reaches persistence there, so "which fact
//! caused it" is a diagnostic, not a required per-fact outcome. A caller with
//! no such prior set-level decision (the MCP batch fact tool, epic #285 T7:
//! an agent submits a table of candidate facts with nothing upstream having
//! judged them) needs the inverse shape: a typed verdict **per submitted
//! fact**, so it can commit the good ones and report the rest — never a
//! single Flagged bit for the whole batch. This module is that shared gate,
//! factored out of the extraction job's duplicated wiring.
//!
//! Pure judge: reads the company's stored history and expected-KPI set via
//! [`AppState`], but performs NO writes — the caller decides what to do with
//! each verdict.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use crate::app_state::AppState;
use crate::fundamentals::validation::{
    completeness, identity_check_metric_keys, plausibility_verdict, validate_period, Completeness,
    FactSet, PlausibilityVerdict, Tolerance,
};

/// One candidate fact in a caller-supplied set — the minimal shape a
/// consumer (the MCP batch tool) maps its own input onto, independent of the
/// extraction pipeline's `ExtractedFact`.
#[derive(Debug, Clone)]
pub struct CandidateFact {
    pub metric_key: String,
    pub value: Decimal,
}

/// Per-fact verdict (ADR 0093 dec. 6 typed-outcome vocabulary + the honest-
/// abstention doctrine — an abstention is never a silent pass).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FactVerdict {
    /// Nothing objected.
    Pass,
    /// ≥100× off the metric's own >=2-point stored history.
    Implausible { history_median: Decimal },
    /// This fact is one of the inputs of a same-period identity the SET
    /// fails.
    IdentityViolation { check: &'static str },
    /// The plausibility gate could not run for this fact (thin history <2
    /// points, a split-sensitive metric, or a zero value/median) — a DISTINCT
    /// verdict from `Pass`, never a silent pass (ADR 0093 dec. 6).
    AbstainedThinHistory,
}

/// The full judgment for a caller-supplied fact set.
#[derive(Debug, Clone)]
pub struct SuppliedSetVerdict {
    /// One verdict per supplied fact, same order/length as the input.
    pub facts: Vec<FactVerdict>,
    /// Completeness against the company's expected primary-KPI set
    /// (ADR 0061 dec. 4d). Informational only — never blocks. `None` when the
    /// company has no expected set configured.
    pub completeness: Option<Completeness>,
}

/// Judges `facts` — a caller-supplied candidate set for one
/// `(company_id, fiscal_year, period_type)` slot — against the company's
/// stored history and the same-period accounting identities
/// [`validate_period`] checks. Read-only: never writes.
///
/// Scope: same-period identities only (balance sheet, free-cash-flow
/// definition) — a caller-supplied batch carries no comparative prior-period
/// column to cross-check and no notion of "the stored prior period's full
/// fact set" the extraction pipeline has, so the cross-period cash-flow tie
/// and the comparative cross-check never fire here (`NotApplicable`, not a
/// gap).
pub fn validate_supplied_set(
    state: &AppState,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    facts: &[CandidateFact],
) -> Result<SuppliedSetVerdict, String> {
    if facts.is_empty() {
        return Ok(SuppliedSetVerdict {
            facts: Vec::new(),
            completeness: None,
        });
    }

    let set: FactSet = facts
        .iter()
        .map(|f| (f.metric_key.clone(), f.value))
        .collect();
    let report = validate_period(&set, &Tolerance::default());

    // Map every FAILED identity check onto the metric_keys it consumes, so a
    // fact that is one of a violated identity's inputs is attributed to it
    // (ADR 0093 dec. 6: "attributed to the facts involved"). `Outcome::Fail`
    // is only reachable once every one of the check's `require()` inputs was
    // present (see `fundamentals::validation`), so every key this loop names
    // is guaranteed to be one of the supplied facts — no unattributed case is
    // reachable with same-period-only identities.
    let mut key_to_check: BTreeMap<&str, &'static str> = BTreeMap::new();
    for identity in report.identities.iter().filter(|c| c.outcome.is_fail()) {
        for key in identity_check_metric_keys(identity.id) {
            key_to_check.entry(key).or_insert(identity.id);
        }
    }

    // Company-wide history read, once, for every distinct supplied metric —
    // the same batched shape `structured_extraction`'s quarantine step uses.
    let history_keys: BTreeSet<String> = facts.iter().map(|f| f.metric_key.clone()).collect();
    let histories = state
        .financials()
        .metric_histories(company_id, &history_keys, fiscal_year, period_type)
        .map_err(|e| e.to_string())?;
    let empty_history: Vec<Decimal> = Vec::new();

    let verdicts = facts
        .iter()
        .map(|fact| {
            if let Some(&check) = key_to_check.get(fact.metric_key.as_str()) {
                return FactVerdict::IdentityViolation { check };
            }
            let history = histories.get(&fact.metric_key).unwrap_or(&empty_history);
            match plausibility_verdict(&fact.metric_key, fact.value, history) {
                PlausibilityVerdict::Plausible => FactVerdict::Pass,
                PlausibilityVerdict::Implausible { history_median } => {
                    FactVerdict::Implausible { history_median }
                }
                PlausibilityVerdict::Abstained { .. } => FactVerdict::AbstainedThinHistory,
            }
        })
        .collect();

    let expected_keys = state
        .financials()
        .expected_primary_metric_keys(company_id)
        .map_err(|e| e.to_string())?;
    let set_completeness = expected_keys
        .filter(|e| !e.is_empty())
        .map(|expected| completeness(&set, &expected));

    Ok(SuppliedSetVerdict {
        facts: verdicts,
        completeness: set_completeness,
    })
}

#[cfg(test)]
mod tests;
