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

use super::esef::projection::project_period;
use super::{fact_set_for_period, ExtractedFact, SourceTier};
use crate::fundamentals::validation::{
    completeness, validate, Completeness, FactSet, Status, Tolerance, ValidationReport,
};
use crate::storage::NewTaggedFact;

/// The inputs available for one report's extraction. Any tier whose input is
/// absent is skipped.
#[derive(Default)]
pub struct PipelineInput<'a> {
    /// ISO `YYYY-MM-DD` period end the facts belong to.
    pub period_end: &'a str,
    /// Layer 1 raw tagged-fact rows (tier 1), if a structured filing exists —
    /// every period the document carries, not just `period_end` (ADR 0100
    /// decisions 1/4/7, epic #398). `run_pipeline` projects only `period_end`
    /// for the candidate set it validates/emits, and separately (read-only)
    /// projects `prior_period_end` to build the comparative cross-check —
    /// comparative periods are never written. Replaces the pre-epic
    /// `esef_bytes: Option<&[u8]>` field: the ESEF tier's fact production is
    /// now a projection over these rows, not a fresh `parse_esef` per call.
    pub layer1_facts: Option<&'a [NewTaggedFact]>,
    /// Whether `layer1_facts`' document carried any presentation-linkbase
    /// evidence (ADR 0100 decision 3 regression fix, epic #398): a bare
    /// iXBRL instance has none, so its facts would fail the strict role
    /// filter unconditionally and the document would silently project zero
    /// facts. `false` routes the ESEF projection through its no-linkbase
    /// fallback (dimensionless + crosswalk-resolved, no role filter)
    /// instead. Irrelevant when `layer1_facts` is `None`.
    pub has_presentation_linkbase: bool,
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

/// Runs `validate` plus the completeness gate against an already-resolved
/// comparatives set. The shared core both [`validate_tier`] (metric-key-only
/// comparatives, the other tiers) and `run_pipeline`'s ESEF branch
/// (projection-resolved comparatives, ADR 0100 decision 4) route through, so
/// the acceptance policy itself can never drift between the two.
fn validate_tier_with_comparatives(
    set: &FactSet,
    comparatives: Option<&FactSet>,
    input: &PipelineInput<'_>,
    tol: &Tolerance,
) -> crate::fundamentals::validation::ValidationReport {
    let mut report = validate(set, input.prior, comparatives, input.prior, tol);
    if let Some(expected) = input.expected_keys.filter(|e| !e.is_empty()) {
        report.completeness = Some(completeness(set, expected));
    }
    report
}

/// Runs `validate` for one tier's candidate set, wiring in the comparative
/// cross-check (`prior_period_end`, read from the tier's own freshly-extracted
/// `facts`) and the completeness gate. `prior` doubles as both `validate`
/// inputs that need a "previously known" period: the cash-flow tie's opening
/// balance and the cross-check's `stored_prior`.
///
/// Used by [`validate_parsed_set_report`] — the OTHER tiers (ESPI cover note,
/// legacy positional) whose facts arrive as a flat `ExtractedFact` list with
/// no Layer 1 backing. Comparatives there still derive via the metric-key-only
/// collapse (`fact_set_for_period`), UNCHANGED: ADR 0100's projection replaces
/// the ESEF tier's fact production only (epic #398).
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
    validate_tier_with_comparatives(set, comparatives.as_ref(), input, tol)
}

/// A projected period's candidates as a `FactSet`, discarding the per-fact
/// Layer-1 provenance (`ProjectedFact::contributing_fact_identities`) — the
/// shape `validate`/`record_structured_fact` need.
fn fact_set_from_projection(facts: &[super::esef::projection::ProjectedFact]) -> FactSet {
    facts
        .iter()
        .map(|pf| (pf.fact.metric_key.clone(), pf.fact.value))
        .collect()
}

/// Runs the tiered pipeline over the available inputs. See the module docs for
/// the tier order and gate semantics.
pub fn run_pipeline(input: &PipelineInput<'_>) -> PipelineOutcome {
    let tol = Tolerance::default();

    // ---- Tier 1: ESEF/iXBRL (source of truth) --------------------------
    // ADR 0100 decisions 1/4/7 (epic #398): the candidate set is a Layer 1
    // projection, not a fresh `parse_esef` pass — see `esef::projection`.
    if let Some(layer1_facts) = input.layer1_facts {
        let projected = project_period(
            layer1_facts,
            input.period_end,
            input.has_presentation_linkbase,
        );
        if !projected.facts.is_empty() {
            let set = fact_set_from_projection(&projected.facts);
            // The comparative column is ALSO projected (never written — decision
            // 7 restricts WRITES to the declared period only) so the cross-check
            // is resolved by the same order-independent rule, not document order.
            let comparatives = input
                .prior_period_end
                .map(|pe| {
                    fact_set_from_projection(
                        &project_period(layer1_facts, pe, input.has_presentation_linkbase).facts,
                    )
                })
                .filter(|s| !s.is_empty());
            let report = validate_tier_with_comparatives(&set, comparatives.as_ref(), input, &tol);
            // Tagged data is authoritative unless it self-contradicts — the
            // shared acceptance table decides (a `Failed` set is `Flagged` and
            // emits no facts; ADR 0061 dec. 2: what objected stays reviewable,
            // never a silent empty). ADR 0100 decision 5: the projected
            // candidates pass this SAME gate as the pre-epic ESEF facts did —
            // widening capture must not widen trust.
            let acceptance = acceptance_for(report.status, report.completeness.as_ref());
            let facts = if acceptance.emits() {
                projected.facts.into_iter().map(|pf| pf.fact).collect()
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

#[cfg(test)]
mod tests;
