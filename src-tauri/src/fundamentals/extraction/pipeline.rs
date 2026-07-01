//! Structured-first extraction pipeline orchestrator (ADR 0061, S5).
//!
//! Walks the tiers highest-trust first and applies the deterministic "good"
//! gate, so a fact is auto-accepted only when it is provably consistent — and a
//! failure escalates to the HTML witness and then to a "structure changed"
//! notification, never a silent emit:
//!
//! 1. **ESEF/iXBRL** — if a tagged instance parses and does not *fail*
//!    validation, it is the source of truth. Done.
//! 2. **PDF** — parse with the company profile; if validation passes and the
//!    layout has not drifted, auto-accept. Otherwise consult the witness.
//! 3. **HTML witness** — corroborate the PDF (agreement ⇒ accept via witness)
//!    or, as a last structured resort, act as the source. Disagreement or drift
//!    ⇒ *flagged*, with a clean diff for the notification.
//!
//! Pure and IO-free: callers pass the already-fetched bytes/text and (lazily)
//! the witness facts, so the whole chain is deterministic and unit-testable —
//! this is the e2e pipeline test the ADR 0049 harness was missing.

use super::esef::parse_esef;
use super::pdf::parse_pdf_text;
use super::profile::{detect_drift, Drift, DriftReport, ExtractionProfile};
use super::{fact_set_for_period, ExtractedFact, SourceTier};
use crate::fundamentals::validation::{cross_check_prior, validate, FactSet, Status, Tolerance};

/// The inputs available for one report's extraction. Any tier whose input is
/// absent is skipped.
#[derive(Default)]
pub struct PipelineInput<'a> {
    /// ISO `YYYY-MM-DD` period end the facts belong to.
    pub period_end: &'a str,
    /// Raw inline-XBRL instance bytes (tier 1), if a structured filing exists.
    pub esef_bytes: Option<&'a [u8]>,
    /// Extracted PDF text (tier 2), if only a PDF exists.
    pub pdf_text: Option<&'a str>,
    /// The confirmed per-company extraction profile (drives PDF parsing + drift).
    pub profile: Option<&'a ExtractionProfile>,
    /// Previously-stored facts for the immediately prior period (cash-flow tie).
    pub prior: Option<&'a FactSet>,
    /// Aggregator (BiznesRadar/Bankier) facts for this period end (tier 3),
    /// already parsed by [`super::html`]. Used as witness and last resort.
    pub witness: Option<&'a [ExtractedFact]>,
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
    /// The clean layout diff, when a drift was detected (PDF tier).
    pub drift: Option<DriftReport>,
}

impl PipelineOutcome {
    fn empty() -> Self {
        Self {
            acceptance: Acceptance::Empty,
            tier: None,
            facts: Vec::new(),
            status: Status::Inconclusive,
            drift: None,
        }
    }
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
                let report = validate(&set, input.prior, None, None, &tol);
                if report.status != Status::Failed {
                    // Tagged data is authoritative unless it self-contradicts.
                    let acceptance = match report.status {
                        Status::Passed => Acceptance::Accepted,
                        _ => Acceptance::AcceptedUnreviewed,
                    };
                    return PipelineOutcome {
                        acceptance,
                        tier: Some(SourceTier::Esef),
                        facts: facts_for_period(facts, input.period_end),
                        status: report.status,
                        drift: None,
                    };
                }
                // ESEF failed validation (corrupt tagging) → fall to PDF.
            }
        }
    }

    // ---- Tier 2: PDF + profile + drift ---------------------------------
    if let Some(text) = input.pdf_text {
        let parse = parse_pdf_text(text, input.period_end, input.profile);
        let drift = input
            .profile
            .map(|p| detect_drift(p, &parse))
            .unwrap_or(Drift::None);
        let set = fact_set_for_period(&parse.facts, input.period_end);
        if !set.is_empty() {
            let report = validate(&set, input.prior, None, None, &tol);
            let drift_report = match &drift {
                Drift::Detected(r) => Some(r.clone()),
                Drift::None => None,
            };
            let clean = report.status == Status::Passed && !drift.is_drift();
            if clean {
                return PipelineOutcome {
                    acceptance: Acceptance::Accepted,
                    tier: Some(SourceTier::Pdf),
                    facts: parse.facts,
                    status: report.status,
                    drift: None,
                };
            }
            // Not clean → consult the witness before deciding.
            if let Some(witness) = input.witness {
                let wset = fact_set_for_period(witness, input.period_end);
                let checks = cross_check_prior(&set, &wset, &tol);
                let corroborated = !checks.is_empty() && checks.iter().all(|c| c.outcome.is_pass());
                if corroborated && report.status != Status::Failed {
                    return PipelineOutcome {
                        acceptance: Acceptance::AcceptedViaWitness,
                        tier: Some(SourceTier::Pdf),
                        facts: parse.facts,
                        status: report.status,
                        drift: drift_report,
                    };
                }
                // Witness disagrees (or PDF self-contradicts) → flag with diff.
                return PipelineOutcome {
                    acceptance: Acceptance::Flagged,
                    tier: Some(SourceTier::Pdf),
                    facts: Vec::new(),
                    status: report.status,
                    drift: drift_report,
                };
            }
            // No witness: a hard failure or a drift is flagged; a merely
            // inconclusive-but-uncontradicted set is accepted unreviewed.
            if report.status == Status::Failed || drift.is_drift() {
                return PipelineOutcome {
                    acceptance: Acceptance::Flagged,
                    tier: Some(SourceTier::Pdf),
                    facts: Vec::new(),
                    status: report.status,
                    drift: drift_report,
                };
            }
            return PipelineOutcome {
                acceptance: Acceptance::AcceptedUnreviewed,
                tier: Some(SourceTier::Pdf),
                facts: parse.facts,
                status: report.status,
                drift: None,
            };
        }
    }

    // ---- Tier 3: HTML aggregator as last structured resort -------------
    if let Some(witness) = input.witness {
        let set = fact_set_for_period(witness, input.period_end);
        if !set.is_empty() {
            let report = validate(&set, input.prior, None, None, &tol);
            if report.status != Status::Failed {
                let acceptance = match report.status {
                    Status::Passed => Acceptance::Accepted,
                    _ => Acceptance::AcceptedUnreviewed,
                };
                return PipelineOutcome {
                    acceptance,
                    tier: Some(SourceTier::HtmlAggregator),
                    facts: facts_for_period(witness.to_vec(), input.period_end),
                    status: report.status,
                    drift: None,
                };
            }
        }
    }

    PipelineOutcome::empty()
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
