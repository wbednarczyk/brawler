//! Structured-first extraction pipeline orchestrator (ADR 0061; ladder amended
//! by ADR 0086 dec. 1/3).
//!
//! Walks the tiers highest-trust first and applies the deterministic "good"
//! gate, so a fact is auto-accepted only when it is provably consistent:
//!
//! 1. **ESEF/iXBRL** — if a tagged instance parses and does not *fail*
//!    validation, it is the source of truth. Done.
//! 2. **HTML aggregator** — as a last structured resort, act as the source
//!    (an inconclusive-but-uncontradicted set is accepted unreviewed).
//!
//! The PDF fact-extraction arm (parse-with-profile + drift) is RETIRED (ADR
//! 0086 dec. 1): no tier reads financial facts out of PDF statements. Core
//! KPIs arrive from the BiznesRadar-primary daily pull.
//!
//! Pure and IO-free: callers pass the already-fetched bytes and (lazily) the
//! witness facts, so the whole chain is deterministic and unit-testable.

use std::collections::BTreeSet;

use super::esef::parse_esef;
use super::{fact_set_for_period, ExtractedFact, SourceTier};
use crate::fundamentals::validation::{
    completeness, validate, Completeness, FactSet, Status, Tolerance, ValidationReport,
};

/// The inputs available for one report's extraction. Any tier whose input is
/// absent is skipped.
#[derive(Default)]
pub struct PipelineInput<'a> {
    /// ISO `YYYY-MM-DD` period end the facts belong to.
    pub period_end: &'a str,
    /// Raw inline-XBRL instance bytes (tier 1), if a structured filing exists.
    pub esef_bytes: Option<&'a [u8]>,
    /// Previously-stored facts for the immediately prior period. Doubles as
    /// **both** inputs `validate` takes for cross-period checks: the
    /// cash-flow tie's opening balance, and the comparative cross-check's
    /// `stored_prior` — the same "already known" prior period backs both.
    pub prior: Option<&'a FactSet>,
    /// ISO `YYYY-MM-DD` end date of the immediately prior period, if known.
    /// Drives the comparative cross-check (ADR 0061 dec. 4b): each tier reads
    /// its own freshly-extracted facts for this period end (the prior-period
    /// column read out of the *current* report) and cross-checks it against
    /// `prior`. `None` skips the cross-check (nothing to compare against, or
    /// this is the earliest tracked period).
    pub prior_period_end: Option<&'a str>,
    /// The company's expected primary-KPI `metric_key`s, for the completeness
    /// check (ADR 0061 dec. 4d). `None`/empty skips it.
    pub expected_keys: Option<&'a BTreeSet<String>>,
}

/// How the pipeline resolved the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acceptance {
    /// A structured tier produced a validation-clean set — auto-accept.
    Accepted,
    /// Validation could not fully prove the set, but the witness corroborated
    /// it — accept, marked witness-confirmed.
    AcceptedViaWitness,
    /// Nothing could be proven and no witness corroborates, but no contradiction
    /// was found either — accept, marked unreviewed (never claimed as verified).
    AcceptedUnreviewed,
    /// A contradiction or a layout drift was found — do not emit; notify.
    Flagged,
    /// No tier produced any tracked fact.
    Empty,
}

impl Acceptance {
    /// Whether the accepted facts should be persisted.
    pub fn emits(self) -> bool {
        matches!(
            self,
            Acceptance::Accepted | Acceptance::AcceptedViaWitness | Acceptance::AcceptedUnreviewed
        )
    }
    /// Stable machine string for the acceptance decision (UI/telemetry).
    pub fn as_str(self) -> &'static str {
        match self {
            Acceptance::Accepted => "accepted",
            Acceptance::AcceptedViaWitness => "accepted_via_witness",
            Acceptance::AcceptedUnreviewed => "accepted_unreviewed",
            Acceptance::Flagged => "flagged",
            Acceptance::Empty => "empty",
        }
    }

    /// The `validation_status` string stored on each emitted fact.
    pub fn validation_status(self) -> &'static str {
        match self {
            Acceptance::Accepted => "passed",
            Acceptance::AcceptedViaWitness => "witness_confirmed",
            Acceptance::AcceptedUnreviewed => "unreviewed",
            Acceptance::Flagged => "flagged",
            Acceptance::Empty => "none",
        }
    }
}

/// The pipeline's decision for one report.
#[derive(Debug, Clone)]
pub struct PipelineOutcome {
    pub acceptance: Acceptance,
    /// The tier whose facts were accepted (or attempted, when flagged).
    pub tier: Option<SourceTier>,
    /// The accepted facts (empty when flagged/empty).
    pub facts: Vec<ExtractedFact>,
    /// The validation report for the chosen set.
    pub status: Status,
    /// The **full** gate report for the chosen set — which identities and
    /// comparative cross-checks were evaluated and which of them objected.
    /// `status` alone says a contradiction exists; this says *what* contradicted,
    /// which is the detail a flagged outcome has to persist to be reviewable
    /// (ADR 0061 decision 2). `None` when no tier produced a set to validate.
    pub validation: Option<ValidationReport>,
}

impl PipelineOutcome {
    fn empty() -> Self {
        Self {
            acceptance: Acceptance::Empty,
            tier: None,
            facts: Vec::new(),
            status: Status::Inconclusive,
            validation: None,
        }
    }
}

/// Chooses between `Accepted` and `AcceptedUnreviewed` for a validation-clean
/// (or self-consistent, witnessless) set, applying the completeness downgrade
/// (ADR 0061 dec. 4d): when an expected primary-KPI set was checked and
/// **none** of it showed up, that is the strongest deterministic signal the
/// parse read the wrong table entirely — not proof of a contradiction, so it
/// only downgrades the trust label, never blocks emission.
fn accepted_unless_hollow(completeness: Option<&Completeness>) -> Acceptance {
    match completeness {
        Some(c) if c.expected > 0 && c.present == 0 => Acceptance::AcceptedUnreviewed,
        _ => Acceptance::Accepted,
    }
}

/// The single acceptance decision table for a validated candidate set — the ONE
/// home of "a contradiction is `Flagged`, a clean set is `Accepted` (downgraded to
/// `AcceptedUnreviewed` when it covers none of the expected primary KPIs), and an
/// uncontradicted-but-unproven set is `AcceptedUnreviewed`". Both the tier-1 ESEF
/// arm ([`run_pipeline`]) and the job-layer [`validate_parsed_set_report`] route
/// through here so the policy can never drift between them.
fn acceptance_for(status: Status, completeness: Option<&Completeness>) -> Acceptance {
    match status {
        Status::Failed => Acceptance::Flagged,
        Status::Passed => accepted_unless_hollow(completeness),
        Status::Inconclusive => Acceptance::AcceptedUnreviewed,
    }
}

/// Runs `validate` for one tier's candidate set, wiring in the comparative
/// cross-check (`prior_period_end`, read from the tier's own freshly-extracted
/// `facts`) and the completeness gate. `prior` doubles as both `validate`
/// inputs that need a "previously known" period: the cash-flow tie's opening
/// balance and the cross-check's `stored_prior`.
fn validate_tier(
    set: &FactSet,
    facts: &[ExtractedFact],
    input: &PipelineInput<'_>,
    tol: &Tolerance,
) -> crate::fundamentals::validation::ValidationReport {
    let comparatives = input
        .prior_period_end
        .map(|pe| fact_set_for_period(facts, pe))
        .filter(|s| !s.is_empty());
    let mut report = validate(set, input.prior, comparatives.as_ref(), input.prior, tol);
    if let Some(expected) = input.expected_keys.filter(|e| !e.is_empty()) {
        report.completeness = Some(completeness(set, expected));
    }
    report
}

/// Runs the tiered pipeline over the available inputs. See the module docs for
/// the tier order and gate semantics.
pub fn run_pipeline(input: &PipelineInput<'_>) -> PipelineOutcome {
    let tol = Tolerance::default();

    // ---- Tier 1: ESEF/iXBRL (source of truth) --------------------------
    if let Some(bytes) = input.esef_bytes {
        if let Ok(facts) = parse_esef(bytes) {
            let set = fact_set_for_period(&facts, input.period_end);
            if !set.is_empty() {
                let report = validate_tier(&set, &facts, input, &tol);
                // Tagged data is authoritative unless it self-contradicts — the
                // shared acceptance table decides (a `Failed` set is `Flagged` and
                // emits no facts; ADR 0061 dec. 2: what objected stays reviewable,
                // never a silent empty).
                let acceptance = acceptance_for(report.status, report.completeness.as_ref());
                let facts = if acceptance.emits() {
                    facts_for_period(facts, input.period_end)
                } else {
                    Vec::new()
                };
                return PipelineOutcome {
                    acceptance,
                    tier: Some(SourceTier::Esef),
                    facts,
                    status: report.status,
                    validation: Some(report.clone()),
                };
            }
        }
    }

    PipelineOutcome::empty()
}

/// Validates an already-parsed candidate set through the **same** gate the tiered
/// pipeline applies to a structured tier, returning the acceptance verdict.
///
/// This exists for the job-layer tiers whose parse lives outside this pure module
/// — the ESPI cover-note (WDF) reader and the positional xHTML reader — whose
/// facts must pass the identical `validate`/`validate_tier` regime as ESEF: the
/// balance-sheet identity and the prior-period magnitude cross-check are exactly
/// what catch a 1000× mis-scale or a repainted total. The
/// verdict mirrors the tier-1 arm: a self-contradiction (`Failed`) is `Flagged`
/// (the caller routes those values to proposals), a clean set is `Accepted`
/// (downgraded to `AcceptedUnreviewed` when it covers none of the expected
/// primary KPIs), and an uncontradicted-but-unproven set is `AcceptedUnreviewed`.
/// An empty set is `Empty`.
pub fn validate_parsed_set(
    facts: &[ExtractedFact],
    period_end: &str,
    prior: Option<&FactSet>,
    prior_period_end: Option<&str>,
    expected_keys: Option<&BTreeSet<String>>,
) -> (Acceptance, Status) {
    let (acceptance, report) =
        validate_parsed_set_report(facts, period_end, prior, prior_period_end, expected_keys);
    let status = report.map(|r| r.status).unwrap_or(Status::Inconclusive);
    (acceptance, status)
}

/// [`validate_parsed_set`] keeping the **full** gate report, so a caller that
/// has to persist *what objected* (the flagged-outcome record, ADR 0061 dec. 2)
/// gets the failing identities and cross-checks rather than a bare `Status`.
/// `None` report = an empty set, where there was nothing to validate.
pub fn validate_parsed_set_report(
    facts: &[ExtractedFact],
    period_end: &str,
    prior: Option<&FactSet>,
    prior_period_end: Option<&str>,
    expected_keys: Option<&BTreeSet<String>>,
) -> (Acceptance, Option<ValidationReport>) {
    let set = fact_set_for_period(facts, period_end);
    if set.is_empty() {
        return (Acceptance::Empty, None);
    }
    let input = PipelineInput {
        period_end,
        prior,
        prior_period_end,
        expected_keys,
        ..PipelineInput::default()
    };
    let report = validate_tier(&set, facts, &input, &Tolerance::default());
    let acceptance = acceptance_for(report.status, report.completeness.as_ref());
    (acceptance, Some(report))
}

/// Keeps only the facts whose period end matches (the accepted set drops
/// comparative-period rows an ESEF/witness parse also carries).
fn facts_for_period(facts: Vec<ExtractedFact>, period_end: &str) -> Vec<ExtractedFact> {
    facts
        .into_iter()
        .filter(|f| f.period.end_date() == period_end)
        .collect()
}

#[cfg(test)]
mod tests;
