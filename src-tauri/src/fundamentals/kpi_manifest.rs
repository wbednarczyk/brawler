//! Deterministic KPI ingest manifest (ADR 0098 dec. 4; epic #352, card #361).
//!
//! The validator's whole output is one versioned, content-hashed manifest:
//! every staged observation gets a stable diagnostic verdict, `outcome` is
//! DERIVED from those verdicts (never caller-supplied — a hand-built manifest
//! claiming `ready` over a flagged observation is rejected by [`SealedManifest::seal`]),
//! and [`SealedManifest`] is the sole way to freeze one into hashed, canonical
//! bytes. Pure: no IO, no dependency on the storage layer (enforced by a
//! guard test in this module's `tests`) — storage maps its own rows onto the
//! storage-free DTOs below in `jobs::kpi_ingest_validation`, mirroring how
//! [`super::validation`] stays a pure core for the structured-extraction
//! pipeline).

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::extraction::text_numbers::parse_amount;
use super::metrics::is_flow_key;
use super::validation::{
    identity_check_metric_keys, plausibility_verdict, validate_period, AbstentionReason, FactSet,
    Outcome as IdentityOutcome, PlausibilityVerdict, Tolerance,
};

/// Bumped when a diagnostic RULE's behavior changes (a code starts/stops
/// firing under the same inputs). Persisted per attempt row — every stored
/// manifest, old or `failed`, stays interpretable by the version it was built
/// under.
pub const VALIDATOR_VERSION: &str = "1";
/// Bumped when the manifest's JSON SHAPE changes (a field added/removed/
/// retyped). Independent of [`VALIDATOR_VERSION`] — a shape change need not
/// imply a behavior change and vice versa.
pub const MANIFEST_SCHEMA_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Diagnostic code registry (closed, wildcard-free)
// ---------------------------------------------------------------------------

/// Whether a diagnostic code, when present on an observation, forces
/// `flagged` or is merely `unreviewed` (an honest abstention — the check
/// could not run, not that it passed). Derives the observation's
/// `validationState`; never assigned by a caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Flagged,
    Unreviewed,
}

/// The closed diagnostic code registry (data-model.md § KPI ingest —
/// normative, mirrored here). Wildcard-free `as_str`/`parse`/`severity` force
/// a new variant to update all three; [`REGISTRY`] and the golden test in
/// this module's `tests` keep the wire vocabulary honest across refactors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum DiagnosticCode {
    #[serde(rename = "mapping.unresolved")]
    MappingUnresolved,
    #[serde(rename = "value.unparseable")]
    ValueUnparseable,
    #[serde(rename = "unit.currency_missing")]
    UnitCurrencyMissing,
    #[serde(rename = "unit.currency_unexpected")]
    UnitCurrencyUnexpected,
    #[serde(rename = "unit.scale_incoherent")]
    UnitScaleIncoherent,
    #[serde(rename = "unit.scale_unverifiable")]
    UnitScaleUnverifiable,
    #[serde(rename = "period.window_missing")]
    PeriodWindowMissing,
    #[serde(rename = "period.window_kind_mismatch")]
    PeriodWindowKindMismatch,
    #[serde(rename = "scope.run_mismatch")]
    ScopeRunMismatch,
    #[serde(rename = "citation.missing")]
    CitationMissing,
    #[serde(rename = "duplicate.conflict")]
    DuplicateConflict,
    #[serde(rename = "duplicate.repeat")]
    DuplicateRepeat,
    #[serde(rename = "identity.violation")]
    IdentityViolation,
    #[serde(rename = "plausibility.implausible")]
    PlausibilityImplausible,
    #[serde(rename = "plausibility.abstained")]
    PlausibilityAbstained,
    #[serde(rename = "run.unsupported_period_grammar")]
    RunUnsupportedPeriodGrammar,
    #[serde(rename = "run.invalid_missing_reasons")]
    RunInvalidMissingReasons,
    #[serde(rename = "completeness.missing_without_reason")]
    CompletenessMissingWithoutReason,
}

/// Every closed-enum variant, in declaration order — the golden list the
/// registry guard test pins (data-model.md § KPI ingest / Rejestr kodów).
pub const REGISTRY: &[DiagnosticCode] = &[
    DiagnosticCode::MappingUnresolved,
    DiagnosticCode::ValueUnparseable,
    DiagnosticCode::UnitCurrencyMissing,
    DiagnosticCode::UnitCurrencyUnexpected,
    DiagnosticCode::UnitScaleIncoherent,
    DiagnosticCode::UnitScaleUnverifiable,
    DiagnosticCode::PeriodWindowMissing,
    DiagnosticCode::PeriodWindowKindMismatch,
    DiagnosticCode::ScopeRunMismatch,
    DiagnosticCode::CitationMissing,
    DiagnosticCode::DuplicateConflict,
    DiagnosticCode::DuplicateRepeat,
    DiagnosticCode::IdentityViolation,
    DiagnosticCode::PlausibilityImplausible,
    DiagnosticCode::PlausibilityAbstained,
    DiagnosticCode::RunUnsupportedPeriodGrammar,
    DiagnosticCode::RunInvalidMissingReasons,
    DiagnosticCode::CompletenessMissingWithoutReason,
];

/// A stored/parsed token this closed registry does not recognize.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown kpi diagnostic code: {value}")]
pub struct UnknownDiagnosticCode {
    pub value: String,
}

impl DiagnosticCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::MappingUnresolved => "mapping.unresolved",
            Self::ValueUnparseable => "value.unparseable",
            Self::UnitCurrencyMissing => "unit.currency_missing",
            Self::UnitCurrencyUnexpected => "unit.currency_unexpected",
            Self::UnitScaleIncoherent => "unit.scale_incoherent",
            Self::UnitScaleUnverifiable => "unit.scale_unverifiable",
            Self::PeriodWindowMissing => "period.window_missing",
            Self::PeriodWindowKindMismatch => "period.window_kind_mismatch",
            Self::ScopeRunMismatch => "scope.run_mismatch",
            Self::CitationMissing => "citation.missing",
            Self::DuplicateConflict => "duplicate.conflict",
            Self::DuplicateRepeat => "duplicate.repeat",
            Self::IdentityViolation => "identity.violation",
            Self::PlausibilityImplausible => "plausibility.implausible",
            Self::PlausibilityAbstained => "plausibility.abstained",
            Self::RunUnsupportedPeriodGrammar => "run.unsupported_period_grammar",
            Self::RunInvalidMissingReasons => "run.invalid_missing_reasons",
            Self::CompletenessMissingWithoutReason => "completeness.missing_without_reason",
        }
    }

    pub fn parse(value: &str) -> Result<Self, UnknownDiagnosticCode> {
        REGISTRY
            .iter()
            .copied()
            .find(|code| code.as_str() == value)
            .ok_or_else(|| UnknownDiagnosticCode {
                value: value.to_owned(),
            })
    }

    /// Per-observation severity (data-model.md § Rejestr kodów). Defined for
    /// every variant so the match stays wildcard-free; the three run-level
    /// codes never attach to an observation, so their severity is never
    /// consulted by [`derive_validation_state`] — `Flagged` here is a
    /// harmless, honest default (a run-level code is never "an honest
    /// abstention").
    pub fn severity(self) -> Severity {
        use Severity::*;
        match self {
            Self::MappingUnresolved => Flagged,
            Self::ValueUnparseable => Flagged,
            Self::UnitCurrencyMissing => Flagged,
            Self::UnitCurrencyUnexpected => Flagged,
            Self::UnitScaleIncoherent => Flagged,
            Self::UnitScaleUnverifiable => Unreviewed,
            Self::PeriodWindowMissing => Flagged,
            Self::PeriodWindowKindMismatch => Flagged,
            Self::ScopeRunMismatch => Flagged,
            Self::CitationMissing => Flagged,
            Self::DuplicateConflict => Flagged,
            Self::DuplicateRepeat => Flagged,
            Self::IdentityViolation => Flagged,
            Self::PlausibilityImplausible => Flagged,
            Self::PlausibilityAbstained => Unreviewed,
            Self::RunUnsupportedPeriodGrammar => Flagged,
            Self::RunInvalidMissingReasons => Flagged,
            Self::CompletenessMissingWithoutReason => Flagged,
        }
    }
}

/// A code's payload, shaped per-variant (data-model.md § Rejestr kodów).
/// `#[serde(untagged)]`: each variant's own field names carry the shape, no
/// extra type tag — the field sets are disjoint across variants, so
/// deserialization is unambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DiagnosticDetail {
    #[serde(rename_all = "camelCase")]
    ObservationIds {
        observation_ids: BTreeSet<String>,
    },
    #[serde(rename_all = "camelCase")]
    CheckResidual {
        check_id: String,
        expected: String,
        actual: String,
        residual: String,
    },
    #[serde(rename_all = "camelCase")]
    HistoryMedian {
        history_median: String,
    },
    Reason {
        reason: String,
    },
    #[serde(rename_all = "camelCase")]
    MetricKeys {
        metric_keys: BTreeSet<String>,
    },
}

/// One `{code, detail}` entry — exactly the wire shape, `detail` an explicit
/// `null` when absent (no `skip_serializing_if`, sol r1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Code {
    pub code: DiagnosticCode,
    pub detail: Option<DiagnosticDetail>,
}

impl Code {
    fn bare(code: DiagnosticCode) -> Self {
        Self { code, detail: None }
    }
}

pub(crate) fn abstention_reason_str(reason: AbstentionReason) -> &'static str {
    match reason {
        AbstentionReason::ThinHistory => "thin_history",
        AbstentionReason::SplitSensitive => "split_sensitive",
        AbstentionReason::ZeroValue => "zero_value",
    }
}

pub(crate) fn decimal_str(value: Decimal) -> String {
    value.normalize().to_string()
}

/// `normalizedValue` emission (data-model.md § KPI Ingest Validation,
/// finding 2): Decimal-canonical (`.normalize().to_string()`) when the raw
/// stored string parses; carried verbatim otherwise — the same string
/// `value.unparseable` already fired against, never silently dropped.
fn canonical_normalized_value(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    match Decimal::from_str(raw.trim()) {
        Ok(value) => Some(decimal_str(value)),
        Err(_) => Some(raw.to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Storage-free input DTOs (evaluate() takes none of the storage layer's types)
// ---------------------------------------------------------------------------

/// Already-parsed/validated `kpi_ingest_runs.missing_reasons_json` — the
/// PARSING happens in `jobs::kpi_ingest_validation` (row-mapping, not this
/// pure core); `evaluate` only branches on which of these three it got.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum MissingReasons {
    /// The column was `NULL` — no ledger, no code.
    #[default]
    None,
    /// A JSON object of non-blank string reasons.
    Valid(BTreeMap<String, String>),
    /// Malformed, not an object, or a non-string/blank value anywhere.
    Invalid,
}

/// The stamped `expected_kpis_json` snapshot (data-model.md § Stempel).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExpectedSnapshot {
    pub schema_version: i64,
    pub source: String,
    pub pack_version: Option<String>,
    pub keys: BTreeSet<String>,
}

/// One resolved-definition's slot-aware history
/// (`storage::financials::slot_metric_histories`) — company-wide values for
/// the EXACT `(definition_id, attribution_eff, measure_window_eff,
/// statement_basis, variant='reported')` slot this observation occupies.
#[derive(Debug, Clone, Default)]
pub struct SlotHistoryInput {
    pub values: Vec<Decimal>,
}

/// The definition the sector-aware resolver matched for one observation
/// (`storage::kpi_extraction`'s resolver, #362 pins on `definitionId`).
#[derive(Debug, Clone)]
pub struct ResolvedDefinitionInput {
    pub definition_id: String,
    pub metric_key: String,
    /// `monetary | ratio | percentage | count | physical | duration`.
    pub value_kind: String,
    pub history: SlotHistoryInput,
}

/// One staged observation, storage-free (mirrors
/// `storage::kpi_ingest_staging::StagedObservation` minus its DB-only
/// columns).
#[derive(Debug, Clone)]
pub struct ManifestObservationInput {
    pub observation_id: String,
    pub ordinal: i64,
    pub raw_value: String,
    pub metric_key_candidate: Option<String>,
    /// `unmapped | mapped | no_definition`.
    pub mapping_status: String,
    pub normalized_value: Option<String>,
    pub currency: Option<String>,
    pub unit_scale: Option<String>,
    pub measure_window: Option<String>,
    pub attribution: Option<String>,
    pub scope: Option<String>,
    pub citation_page: Option<i64>,
    pub citation_table: Option<String>,
    pub citation_row: Option<String>,
    pub citation_quote: Option<String>,
    /// `None` when unmapped/`no_definition`, or the resolver found nothing.
    pub definition: Option<ResolvedDefinitionInput>,
}

/// The run context a manifest is built against, storage-free.
#[derive(Debug, Clone)]
pub struct ManifestRunInput {
    pub run_id: String,
    pub revision: i64,
    pub company_id: String,
    pub report_document_id: String,
    pub source_content_hash: Option<String>,
    /// `standalone | consolidated`.
    pub scope: String,
    /// `final | preliminary | estimated`.
    pub data_quality: String,
    pub period_id: Option<String>,
    pub fiscal_year: i64,
    /// `financial_periods.period_type` vocabulary.
    pub period_type: String,
    pub missing_reasons: MissingReasons,
    pub expected: Option<ExpectedSnapshot>,
}

// ---------------------------------------------------------------------------
// Evaluation (pure rule engine)
// ---------------------------------------------------------------------------

/// One missing expected KPI in the completeness report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingKpi {
    pub metric_key: String,
    pub reason: Option<String>,
}

/// Coverage of the observations against the stamped expected-KPI snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Completeness {
    pub expected: BTreeSet<String>,
    pub present: BTreeSet<String>,
    pub missing: Vec<MissingKpi>,
}

/// The rule engine's raw output — everything [`build_manifest`] needs to
/// assemble a [`KpiIngestManifest`], keyed loosely enough to stay independent
/// of the manifest's own wire shape.
pub struct Evaluation {
    pub observation_codes: HashMap<String, Vec<Code>>,
    pub run_diagnostics: Vec<Code>,
    pub completeness: Option<Completeness>,
}

const SUPPORTED_PERIOD_GRAMMAR: &[&str] = &["Q1", "H1", "9M", "FY"];

fn scale_factor(scale_eff: &str) -> Decimal {
    match scale_eff {
        "thousands" => Decimal::from(1_000),
        "millions" => Decimal::from(1_000_000),
        // "ones" and any other stored value (vocabulary is CHECK-guarded
        // upstream) default to no multiplier.
        _ => Decimal::ONE,
    }
}

fn citation_is_missing(obs: &ManifestObservationInput) -> bool {
    let quote_blank = obs
        .citation_quote
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty();
    let page_present = obs.citation_page.is_some();
    let table_row_present = obs
        .citation_table
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
        && obs
            .citation_row
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
    quote_blank && !page_present && !table_row_present
}

/// The pure rule core (mirrors [`super::validation`]'s shape): no IO, no
/// storage-layer dependency. Runs every diagnostic in data-model.md § Rejestr kodów
/// order, honoring the short-circuit rules (mapping.unresolved skips every
/// definition-dependent check; value.unparseable additionally skips
/// identity/plausibility/duplicate).
pub fn evaluate(run: &ManifestRunInput, observations: &[ManifestObservationInput]) -> Evaluation {
    let mut observation_codes: HashMap<String, Vec<Code>> = HashMap::new();
    for obs in observations {
        observation_codes.insert(obs.observation_id.clone(), Vec::new());
    }

    // Per-observation, definition-independent checks (data-model.md: "niezależne działają").
    for obs in observations {
        let codes = observation_codes
            .get_mut(&obs.observation_id)
            .expect("seeded above");
        let mapping_unresolved = obs.mapping_status != "mapped" || obs.definition.is_none();
        if mapping_unresolved {
            codes.push(Code::bare(DiagnosticCode::MappingUnresolved));
        }
        let normalized_decimal = obs
            .normalized_value
            .as_deref()
            .and_then(|s| Decimal::from_str(s.trim()).ok());
        let value_unparseable = normalized_decimal.is_none();
        if value_unparseable {
            codes.push(Code::bare(DiagnosticCode::ValueUnparseable));
        }
        if obs.measure_window.is_none() {
            codes.push(Code::bare(DiagnosticCode::PeriodWindowMissing));
        }
        if let Some(obs_scope) = &obs.scope {
            if obs_scope != &run.scope {
                codes.push(Code::bare(DiagnosticCode::ScopeRunMismatch));
            }
        }
        if citation_is_missing(obs) {
            codes.push(Code::bare(DiagnosticCode::CitationMissing));
        }

        if mapping_unresolved {
            continue;
        }
        let def = obs.definition.as_ref().expect("checked above");

        // unit.currency_*
        let is_monetary = def.value_kind == "monetary";
        let currency_set = obs.currency.is_some();
        if is_monetary && !currency_set {
            codes.push(Code::bare(DiagnosticCode::UnitCurrencyMissing));
        }
        if !is_monetary && currency_set {
            codes.push(Code::bare(DiagnosticCode::UnitCurrencyUnexpected));
        }

        // unit.scale_incoherent / unit.scale_unverifiable
        let scale_eff = obs.unit_scale.as_deref().unwrap_or("ones");
        match parse_amount(&obs.raw_value) {
            None => codes.push(Code::bare(DiagnosticCode::UnitScaleUnverifiable)),
            Some(parsed) => {
                let expected = parsed * scale_factor(scale_eff);
                let coherent = normalized_decimal.is_some_and(|actual| actual == expected);
                if !coherent {
                    codes.push(Code::bare(DiagnosticCode::UnitScaleIncoherent));
                }
            }
        }

        // period.window_kind_mismatch
        if let Some(window) = &obs.measure_window {
            let flow = is_flow_key(&def.metric_key, Some(def.value_kind.as_str()));
            let stock_conflict =
                !flow && matches!(window.as_str(), "flow" | "cumulative" | "trailing");
            let flow_conflict = flow && window == "point_in_time";
            if stock_conflict || flow_conflict {
                codes.push(Code::bare(DiagnosticCode::PeriodWindowKindMismatch));
            }
        }

        if value_unparseable {
            continue;
        }
        let value = normalized_decimal.expect("checked above");

        // plausibility.* against the slot-aware history.
        match plausibility_verdict(&def.metric_key, value, &def.history.values) {
            PlausibilityVerdict::Plausible => {}
            PlausibilityVerdict::Implausible { history_median } => {
                codes.push(Code {
                    code: DiagnosticCode::PlausibilityImplausible,
                    detail: Some(DiagnosticDetail::HistoryMedian {
                        history_median: decimal_str(history_median),
                    }),
                });
            }
            PlausibilityVerdict::Abstained { reason } => {
                codes.push(Code {
                    code: DiagnosticCode::PlausibilityAbstained,
                    detail: Some(DiagnosticDetail::Reason {
                        reason: abstention_reason_str(reason).to_owned(),
                    }),
                });
            }
        }
    }

    // duplicate.conflict / duplicate.repeat: grouped by
    // (definition_id, attribution_eff, measure_window_eff), eligible
    // observations only (definition resolved + value parseable).
    type SlotKey = (String, String, Option<String>);
    let mut duplicate_groups: BTreeMap<SlotKey, Vec<(&ManifestObservationInput, Decimal)>> =
        BTreeMap::new();
    for obs in observations {
        let Some(def) = obs.definition.as_ref() else {
            continue;
        };
        if obs.mapping_status != "mapped" {
            continue;
        }
        let Some(value) = obs
            .normalized_value
            .as_deref()
            .and_then(|s| Decimal::from_str(s.trim()).ok())
        else {
            continue;
        };
        let attribution_eff = obs
            .attribution
            .clone()
            .unwrap_or_else(|| "total".to_owned());
        let key = (
            def.definition_id.clone(),
            attribution_eff,
            obs.measure_window.clone(),
        );
        duplicate_groups.entry(key).or_default().push((obs, value));
    }
    for group in duplicate_groups.values() {
        if group.len() < 2 {
            continue;
        }
        let first_value = group[0].1;
        let all_equal = group.iter().all(|(_, v)| *v == first_value);
        let code = if all_equal {
            DiagnosticCode::DuplicateRepeat
        } else {
            DiagnosticCode::DuplicateConflict
        };
        let ids: BTreeSet<String> = group
            .iter()
            .map(|(o, _)| o.observation_id.clone())
            .collect();
        for (obs, _) in group {
            observation_codes
                .get_mut(&obs.observation_id)
                .expect("seeded above")
                .push(Code {
                    code,
                    detail: Some(DiagnosticDetail::ObservationIds {
                        observation_ids: ids.clone(),
                    }),
                });
        }
    }

    // identity.violation: same-period identities over the total-attribution
    // FactSet (definition resolved + value parseable + attribution_eff == 'total').
    let mut fact_set: FactSet = FactSet::new();
    // A metric_key colliding across measure_window at total attribution
    // feeds the FactSet from only its lowest-ordinal observation's value —
    // the plan names no tie-break for that (unspecified) case; ordinal is
    // the one stable, deterministic key every observation carries. The
    // ATTRIBUTION of a failed identity's code, below, is NOT limited to this
    // representative — every qualifying observation sharing the metric key
    // gets flagged (#361 finding 3).
    let mut representative: BTreeMap<String, (&ManifestObservationInput, Decimal)> =
        BTreeMap::new();
    let mut total_attribution_by_metric_key: BTreeMap<String, Vec<&ManifestObservationInput>> =
        BTreeMap::new();
    for obs in observations {
        let Some(def) = obs.definition.as_ref() else {
            continue;
        };
        if obs.mapping_status != "mapped" {
            continue;
        }
        let attribution_eff = obs
            .attribution
            .clone()
            .unwrap_or_else(|| "total".to_owned());
        if attribution_eff != "total" {
            continue;
        }
        let Some(value) = obs
            .normalized_value
            .as_deref()
            .and_then(|s| Decimal::from_str(s.trim()).ok())
        else {
            continue;
        };
        total_attribution_by_metric_key
            .entry(def.metric_key.clone())
            .or_default()
            .push(obs);
        match representative.get(&def.metric_key) {
            Some((existing, _)) if existing.ordinal <= obs.ordinal => {}
            _ => {
                representative.insert(def.metric_key.clone(), (obs, value));
            }
        }
    }
    for (metric_key, (_, value)) in &representative {
        fact_set.insert(metric_key.clone(), *value);
    }
    if !fact_set.is_empty() {
        let report = validate_period(&fact_set, &Tolerance::default());
        let mut key_to_violation: BTreeMap<&str, (&'static str, IdentityOutcome)> = BTreeMap::new();
        for identity in report.identities.iter().filter(|c| c.outcome.is_fail()) {
            for key in identity_check_metric_keys(identity.id) {
                key_to_violation
                    .entry(key)
                    .or_insert((identity.id, identity.outcome.clone()));
            }
        }
        for (metric_key, (check_id, outcome)) in &key_to_violation {
            let IdentityOutcome::Fail {
                expected,
                actual,
                residual,
            } = outcome
            else {
                continue;
            };
            let Some(members) = total_attribution_by_metric_key.get(*metric_key) else {
                continue;
            };
            for obs in members {
                observation_codes
                    .get_mut(&obs.observation_id)
                    .expect("seeded above")
                    .push(Code {
                        code: DiagnosticCode::IdentityViolation,
                        detail: Some(DiagnosticDetail::CheckResidual {
                            check_id: (*check_id).to_owned(),
                            expected: decimal_str(*expected),
                            actual: decimal_str(*actual),
                            residual: decimal_str(*residual),
                        }),
                    });
            }
        }
    }

    // Run-level diagnostics, in registry order.
    let mut run_diagnostics = Vec::new();
    if !SUPPORTED_PERIOD_GRAMMAR.contains(&run.period_type.as_str()) {
        run_diagnostics.push(Code::bare(DiagnosticCode::RunUnsupportedPeriodGrammar));
    }
    if matches!(run.missing_reasons, MissingReasons::Invalid) {
        run_diagnostics.push(Code::bare(DiagnosticCode::RunInvalidMissingReasons));
    }

    // completeness.* — only when a non-empty expected snapshot was stamped.
    let completeness = run
        .expected
        .as_ref()
        .filter(|e| !e.keys.is_empty())
        .map(|expected| {
            let present: BTreeSet<String> = observations
                .iter()
                .filter(|o| o.mapping_status == "mapped")
                .filter_map(|o| o.definition.as_ref().map(|d| d.metric_key.clone()))
                .filter(|key| expected.keys.contains(key))
                .collect();
            let reasons = match &run.missing_reasons {
                MissingReasons::Valid(map) => map.clone(),
                _ => BTreeMap::new(),
            };
            let mut missing = Vec::new();
            let mut missing_without_reason = BTreeSet::new();
            for key in &expected.keys {
                if present.contains(key) {
                    continue;
                }
                let reason = reasons.get(key).cloned();
                if reason.is_none() {
                    missing_without_reason.insert(key.clone());
                }
                missing.push(MissingKpi {
                    metric_key: key.clone(),
                    reason,
                });
            }
            if !missing_without_reason.is_empty() {
                run_diagnostics.push(Code {
                    code: DiagnosticCode::CompletenessMissingWithoutReason,
                    detail: Some(DiagnosticDetail::MetricKeys {
                        metric_keys: missing_without_reason,
                    }),
                });
            }
            Completeness {
                expected: expected.keys.clone(),
                present,
                missing,
            }
        });

    for codes in observation_codes.values_mut() {
        codes.sort_by_key(|c| c.code);
    }

    Evaluation {
        observation_codes,
        run_diagnostics,
        completeness,
    }
}

// ---------------------------------------------------------------------------
// Manifest (output shape) + builder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationState {
    Passed,
    Unreviewed,
    Flagged,
}

impl ValidationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Unreviewed => "unreviewed",
            Self::Flagged => "flagged",
        }
    }
}

fn derive_validation_state(codes: &[Code]) -> ValidationState {
    if codes.iter().any(|c| c.code.severity() == Severity::Flagged) {
        ValidationState::Flagged
    } else if !codes.is_empty() {
        ValidationState::Unreviewed
    } else {
        ValidationState::Passed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Ready,
    Failed,
}

impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

fn derive_outcome(observations: &[ManifestObservation], run_diagnostics: &[Code]) -> Outcome {
    let any_flagged = observations
        .iter()
        .any(|o| o.validation_state == ValidationState::Flagged);
    if any_flagged || !run_diagnostics.is_empty() {
        Outcome::Failed
    } else {
        Outcome::Ready
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub passed: u32,
    pub unreviewed: u32,
    pub flagged: u32,
}

fn derive_counts(observations: &[ManifestObservation]) -> Counts {
    let mut counts = Counts {
        passed: 0,
        unreviewed: 0,
        flagged: 0,
    };
    for obs in observations {
        match obs.validation_state {
            ValidationState::Passed => counts.passed += 1,
            ValidationState::Unreviewed => counts.unreviewed += 1,
            ValidationState::Flagged => counts.flagged += 1,
        }
    }
    counts
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Citation {
    pub page: Option<i64>,
    pub table: Option<String>,
    pub row: Option<String>,
    pub quote: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestObservation {
    pub observation_id: String,
    pub ordinal: i64,
    pub metric_key: String,
    pub definition_id: Option<String>,
    pub normalized_value: Option<String>,
    pub currency: Option<String>,
    pub unit_scale: Option<String>,
    pub attribution: String,
    pub measure_window: Option<String>,
    pub scope: Option<String>,
    pub citation: Citation,
    pub validation_state: ValidationState,
    pub codes: Vec<Code>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodInfo {
    pub period_id: Option<String>,
    pub fiscal_year: i64,
    pub period_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExpectedKpis {
    pub schema_version: i64,
    pub source: String,
    pub pack_version: Option<String>,
    pub keys: BTreeSet<String>,
}

/// The full manifest — WYPROWADZONE fields (`outcome`, every observation's
/// `validationState`, `counts`) are ordinary public fields, hand-settable
/// (the negative construction tests need that), but [`SealedManifest::seal`]
/// is the only path that treats one as trustworthy: it independently
/// RE-derives all three from `codes`/`runDiagnostics` and rejects any
/// mismatch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiIngestManifest {
    pub manifest_schema_version: u32,
    pub validator_version: String,
    pub run_id: String,
    pub revision: i64,
    pub outcome: Outcome,
    pub company_id: String,
    pub report_document_id: String,
    pub source_content_hash: Option<String>,
    pub scope: String,
    pub data_quality: String,
    pub period: PeriodInfo,
    pub expected_kpis: Option<ExpectedKpis>,
    pub observations: Vec<ManifestObservation>,
    pub run_diagnostics: Vec<Code>,
    pub completeness: Option<Completeness>,
    pub counts: Counts,
}

/// Assembles a [`KpiIngestManifest`] from [`evaluate`]'s output, deriving
/// every observation's `validationState`, the manifest `outcome` and
/// `counts` — the ONLY legitimate production construction path (sol r4/r5:
/// "outcome i counts są wyprowadzane, nie podawane").
pub fn build_manifest(
    run: &ManifestRunInput,
    observations: &[ManifestObservationInput],
) -> KpiIngestManifest {
    let evaluation = evaluate(run, observations);

    let mut sorted_inputs: Vec<&ManifestObservationInput> = observations.iter().collect();
    sorted_inputs.sort_by_key(|o| o.ordinal);

    let manifest_observations: Vec<ManifestObservation> = sorted_inputs
        .into_iter()
        .map(|obs| {
            let codes = evaluation
                .observation_codes
                .get(&obs.observation_id)
                .cloned()
                .unwrap_or_default();
            let validation_state = derive_validation_state(&codes);
            ManifestObservation {
                observation_id: obs.observation_id.clone(),
                ordinal: obs.ordinal,
                metric_key: obs.metric_key_candidate.clone().unwrap_or_default(),
                // Only a MAPPED observation carries a definition identity in the
                // manifest — mirrors `evaluate`'s completeness/present rule, so
                // `seal()`'s re-derivation cannot disagree with the validator.
                definition_id: (obs.mapping_status == "mapped")
                    .then(|| obs.definition.as_ref().map(|d| d.definition_id.clone()))
                    .flatten(),
                normalized_value: canonical_normalized_value(obs.normalized_value.as_deref()),
                currency: obs.currency.clone(),
                unit_scale: obs.unit_scale.clone(),
                attribution: obs
                    .attribution
                    .clone()
                    .unwrap_or_else(|| "total".to_owned()),
                measure_window: obs.measure_window.clone(),
                scope: obs.scope.clone(),
                citation: Citation {
                    page: obs.citation_page,
                    table: obs.citation_table.clone(),
                    row: obs.citation_row.clone(),
                    quote: obs.citation_quote.clone(),
                },
                validation_state,
                codes,
            }
        })
        .collect();

    let outcome = derive_outcome(&manifest_observations, &evaluation.run_diagnostics);
    let counts = derive_counts(&manifest_observations);

    KpiIngestManifest {
        manifest_schema_version: MANIFEST_SCHEMA_VERSION,
        validator_version: VALIDATOR_VERSION.to_owned(),
        run_id: run.run_id.clone(),
        revision: run.revision,
        outcome,
        company_id: run.company_id.clone(),
        report_document_id: run.report_document_id.clone(),
        source_content_hash: run.source_content_hash.clone(),
        scope: run.scope.clone(),
        data_quality: run.data_quality.clone(),
        period: PeriodInfo {
            period_id: run.period_id.clone(),
            fiscal_year: run.fiscal_year,
            period_type: run.period_type.clone(),
        },
        expected_kpis: run.expected.as_ref().map(|e| ExpectedKpis {
            schema_version: e.schema_version,
            source: e.source.clone(),
            pack_version: e.pack_version.clone(),
            keys: e.keys.clone(),
        }),
        observations: manifest_observations,
        run_diagnostics: evaluation.run_diagnostics,
        completeness: evaluation.completeness,
        counts,
    }
}

// ---------------------------------------------------------------------------
// SealedManifest — the sole hashed, verified construction path
// ---------------------------------------------------------------------------

/// Why [`SealedManifest::seal`] refused a manifest — every case is a
/// structural inconsistency between `codes`/`runDiagnostics` and the
/// derived-but-hand-settable `validationState`/`outcome`/`counts` fields.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestSealError {
    #[error(
        "observation {observation_id} validationState {stated} does not match its codes (derived {derived})"
    )]
    ObservationStateMismatch {
        observation_id: String,
        stated: &'static str,
        derived: &'static str,
    },
    #[error("manifest outcome {stated} does not match its observations/runDiagnostics (derived {derived})")]
    OutcomeMismatch {
        stated: &'static str,
        derived: &'static str,
    },
    #[error("manifest counts {{passed:{stated_passed}, unreviewed:{stated_unreviewed}, flagged:{stated_flagged}}} do not match the observations (derived {{passed:{derived_passed}, unreviewed:{derived_unreviewed}, flagged:{derived_flagged}}})")]
    CountsMismatch {
        stated_passed: u32,
        stated_unreviewed: u32,
        stated_flagged: u32,
        derived_passed: u32,
        derived_unreviewed: u32,
        derived_flagged: u32,
    },
    #[error("manifest observations contain a duplicate observationId: {observation_id}")]
    DuplicateObservationId { observation_id: String },
    #[error("manifest observations contain a duplicate ordinal: {ordinal}")]
    DuplicateOrdinal { ordinal: i64 },
    #[error(
        "code {code} carries a detail shape it does not admit (observation {observation_id:?})"
    )]
    InvalidCodeDetail {
        code: &'static str,
        observation_id: Option<String>,
    },
    #[error("manifest completeness is inconsistent with its observations: {reason}")]
    CompletenessInconsistent { reason: &'static str },
    #[error(
        "manifest carries schema/validator versions {stated_schema}/{stated_validator} but this validator produces {expected_schema}/{expected_validator}"
    )]
    VersionMismatch {
        stated_schema: u32,
        stated_validator: String,
        expected_schema: u32,
        expected_validator: &'static str,
    },
}

/// One observation's stored verdict — exactly the two columns
/// `storage::kpi_ingest_staging::apply_validation_outcome` writes to
/// `kpi_staged_observations`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationVerdict {
    pub observation_id: String,
    pub validation_state: &'static str,
    pub validation_codes_json: String,
}

/// The canonical staged-content projection `apply_validation_outcome`
/// re-compares against the LIVE `kpi_staged_observations` row in the same
/// transaction (sol r5-2 content-tamper guard): a manifest naming the right
/// observation ids but a different value/citation is refused, zero writes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationContentProjection {
    pub observation_id: String,
    pub ordinal: i64,
    pub metric_key_candidate: Option<String>,
    pub normalized_value: Option<String>,
    pub currency: Option<String>,
    pub unit_scale_eff: String,
    pub attribution_eff: String,
    pub measure_window_eff: Option<String>,
    pub scope: Option<String>,
    pub citation_page: Option<i64>,
    pub citation_table: Option<String>,
    pub citation_row: Option<String>,
    pub citation_quote: Option<String>,
}

/// A [`KpiIngestManifest`] frozen into canonical bytes: `manifest_hash` is
/// `sha256_hex(manifest_json)` BY CONSTRUCTION (no other path produces one),
/// and `seal()` independently re-verifies `outcome`/`counts`/every
/// observation's `validationState` against `codes`/`runDiagnostics` before
/// freezing — a hand-built inconsistent manifest never reaches a hash.
#[derive(Debug, Clone)]
pub struct SealedManifest {
    run_id: String,
    revision: i64,
    outcome: Outcome,
    manifest_json: String,
    manifest_hash: String,
    verdicts: Vec<ObservationVerdict>,
    content: Vec<ObservationContentProjection>,
    // Run-context fields (#361 finding 1b): the atom re-compares each of
    // these against the LIVE `kpi_ingest_runs` row in the same transaction
    // before writing anything — a manifest built for a different company/
    // document/hash/scope/quality/period/expected-snapshot must never bind.
    company_id: String,
    report_document_id: String,
    source_content_hash: Option<String>,
    scope: String,
    data_quality: String,
    period_id: Option<String>,
    fiscal_year: i64,
    period_type: String,
    expected_kpis_json: Option<String>,
}

/// Whether `code`'s detail matches the ONE shape that code admits (or `None`
/// for every code not listed here) — data-model.md § Rejestr kodów. Checked
/// by [`SealedManifest::seal`] over both observation `codes` and
/// `runDiagnostics` (#361 finding 1a).
fn detail_matches_code(code: DiagnosticCode, detail: &Option<DiagnosticDetail>) -> bool {
    use DiagnosticCode::*;
    use DiagnosticDetail::*;
    match code {
        PlausibilityImplausible => matches!(detail, Some(HistoryMedian { .. })),
        IdentityViolation => matches!(detail, Some(CheckResidual { .. })),
        DuplicateConflict | DuplicateRepeat => matches!(detail, Some(ObservationIds { .. })),
        PlausibilityAbstained => matches!(detail, Some(Reason { .. })),
        CompletenessMissingWithoutReason => matches!(detail, Some(MetricKeys { .. })),
        _ => detail.is_none(),
    }
}

/// Re-verifies `manifest.completeness` against `manifest.observations` and
/// `manifest.expectedKpis` when an expected snapshot is present (#361 finding
/// 1a): `present` is exactly the mapped metric keys among observations that
/// fall inside the expected set (mirrors [`evaluate`]'s own construction);
/// `missing` and `present` are disjoint; their union stays inside the
/// expected keys (which also covers "every missing entry's metricKey is in
/// the expected set"). A `None`/empty-keys snapshot has no completeness gate
/// (mirrors [`evaluate`]), so nothing to check.
fn validate_completeness_consistency(
    manifest: &KpiIngestManifest,
) -> Result<(), ManifestSealError> {
    let Some(expected) = manifest.expected_kpis.as_ref() else {
        return Ok(());
    };
    if expected.keys.is_empty() {
        return Ok(());
    }
    let Some(completeness) = manifest.completeness.as_ref() else {
        return Err(ManifestSealError::CompletenessInconsistent {
            reason: "expectedKpis is present but completeness is missing",
        });
    };
    if completeness.expected != expected.keys {
        return Err(ManifestSealError::CompletenessInconsistent {
            reason: "completeness.expected does not match expectedKpis.keys",
        });
    }
    // `o.metric_key` is the RAW candidate (`build_manifest` never re-derives
    // it from the resolved definition); the real resolver echoes back the
    // TRIMMED candidate it matched on (`storage::kpi_extraction::
    // resolve_kpi_definition`), so `present`'s membership -- built by
    // `evaluate` from the resolved definition's own metric_key -- can
    // legitimately diverge from an untrimmed candidate by whitespace alone.
    // Trim here to avoid a false CompletenessInconsistent on an otherwise
    // consistent manifest.
    let mapped_metric_keys: BTreeSet<String> = manifest
        .observations
        .iter()
        .filter(|o| o.definition_id.is_some() && !o.metric_key.trim().is_empty())
        .map(|o| o.metric_key.trim().to_owned())
        .collect();
    let expected_present: BTreeSet<String> = mapped_metric_keys
        .into_iter()
        .filter(|key| expected.keys.contains(key))
        .collect();
    if completeness.present != expected_present {
        return Err(ManifestSealError::CompletenessInconsistent {
            reason: "present does not match the mapped metric keys among observations",
        });
    }
    let missing_keys: BTreeSet<String> = completeness
        .missing
        .iter()
        .map(|m| m.metric_key.clone())
        .collect();
    if !missing_keys.is_disjoint(&completeness.present) {
        return Err(ManifestSealError::CompletenessInconsistent {
            reason: "missing and present overlap",
        });
    }
    if !missing_keys
        .union(&completeness.present)
        .all(|key| expected.keys.contains(key))
    {
        return Err(ManifestSealError::CompletenessInconsistent {
            reason: "missing/present contain a key outside the expected snapshot",
        });
    }
    Ok(())
}

impl SealedManifest {
    pub fn seal(manifest: KpiIngestManifest) -> Result<Self, ManifestSealError> {
        if manifest.manifest_schema_version != MANIFEST_SCHEMA_VERSION
            || manifest.validator_version != VALIDATOR_VERSION
        {
            return Err(ManifestSealError::VersionMismatch {
                stated_schema: manifest.manifest_schema_version,
                stated_validator: manifest.validator_version.clone(),
                expected_schema: MANIFEST_SCHEMA_VERSION,
                expected_validator: VALIDATOR_VERSION,
            });
        }
        let mut seen_ids = HashSet::with_capacity(manifest.observations.len());
        let mut seen_ordinals = HashSet::with_capacity(manifest.observations.len());
        for obs in &manifest.observations {
            if !seen_ids.insert(obs.observation_id.as_str()) {
                return Err(ManifestSealError::DuplicateObservationId {
                    observation_id: obs.observation_id.clone(),
                });
            }
            if !seen_ordinals.insert(obs.ordinal) {
                return Err(ManifestSealError::DuplicateOrdinal {
                    ordinal: obs.ordinal,
                });
            }
            for c in &obs.codes {
                if !detail_matches_code(c.code, &c.detail) {
                    return Err(ManifestSealError::InvalidCodeDetail {
                        code: c.code.as_str(),
                        observation_id: Some(obs.observation_id.clone()),
                    });
                }
            }
        }
        for c in &manifest.run_diagnostics {
            if !detail_matches_code(c.code, &c.detail) {
                return Err(ManifestSealError::InvalidCodeDetail {
                    code: c.code.as_str(),
                    observation_id: None,
                });
            }
        }

        for obs in &manifest.observations {
            let derived = derive_validation_state(&obs.codes);
            if derived != obs.validation_state {
                return Err(ManifestSealError::ObservationStateMismatch {
                    observation_id: obs.observation_id.clone(),
                    stated: obs.validation_state.as_str(),
                    derived: derived.as_str(),
                });
            }
        }
        let derived_outcome = derive_outcome(&manifest.observations, &manifest.run_diagnostics);
        if derived_outcome != manifest.outcome {
            return Err(ManifestSealError::OutcomeMismatch {
                stated: manifest.outcome.as_str(),
                derived: derived_outcome.as_str(),
            });
        }
        let derived_counts = derive_counts(&manifest.observations);
        if derived_counts != manifest.counts {
            return Err(ManifestSealError::CountsMismatch {
                stated_passed: manifest.counts.passed,
                stated_unreviewed: manifest.counts.unreviewed,
                stated_flagged: manifest.counts.flagged,
                derived_passed: derived_counts.passed,
                derived_unreviewed: derived_counts.unreviewed,
                derived_flagged: derived_counts.flagged,
            });
        }
        validate_completeness_consistency(&manifest)?;

        let manifest_json =
            serde_json::to_string(&manifest).expect("KpiIngestManifest serialization is total");
        let manifest_hash = sha256_hex(manifest_json.as_bytes());

        let verdicts = manifest
            .observations
            .iter()
            .map(|obs| ObservationVerdict {
                observation_id: obs.observation_id.clone(),
                validation_state: obs.validation_state.as_str(),
                validation_codes_json: serde_json::to_string(&obs.codes)
                    .expect("codes serialization is total"),
            })
            .collect();
        let content = manifest
            .observations
            .iter()
            .map(|obs| ObservationContentProjection {
                observation_id: obs.observation_id.clone(),
                ordinal: obs.ordinal,
                metric_key_candidate: Some(obs.metric_key.clone()).filter(|s| !s.is_empty()),
                normalized_value: obs.normalized_value.clone(),
                currency: obs.currency.clone(),
                unit_scale_eff: obs.unit_scale.clone().unwrap_or_else(|| "ones".to_owned()),
                attribution_eff: obs.attribution.clone(),
                measure_window_eff: obs.measure_window.clone(),
                scope: obs.scope.clone(),
                citation_page: obs.citation.page,
                citation_table: obs.citation.table.clone(),
                citation_row: obs.citation.row.clone(),
                citation_quote: obs.citation.quote.clone(),
            })
            .collect();
        let expected_kpis_json = manifest
            .expected_kpis
            .as_ref()
            .map(|e| serde_json::to_string(e).expect("ExpectedKpis serialization is total"));

        Ok(Self {
            run_id: manifest.run_id,
            revision: manifest.revision,
            outcome: manifest.outcome,
            manifest_json,
            manifest_hash,
            verdicts,
            content,
            company_id: manifest.company_id,
            report_document_id: manifest.report_document_id,
            source_content_hash: manifest.source_content_hash,
            scope: manifest.scope,
            data_quality: manifest.data_quality,
            period_id: manifest.period.period_id,
            fiscal_year: manifest.period.fiscal_year,
            period_type: manifest.period.period_type,
            expected_kpis_json,
        })
    }

    pub fn run_id(&self) -> &str {
        &self.run_id
    }

    pub fn revision(&self) -> i64 {
        self.revision
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn manifest_hash(&self) -> &str {
        &self.manifest_hash
    }

    pub fn manifest_json(&self) -> &str {
        &self.manifest_json
    }

    pub fn observation_verdicts(&self) -> &[ObservationVerdict] {
        &self.verdicts
    }

    /// Run-context accessors (#361 finding 1b) — `KpiIngestStagingStore::
    /// apply_validation_outcome` re-compares each against the LIVE run row
    /// in the same transaction before writing anything.
    pub fn company_id(&self) -> &str {
        &self.company_id
    }

    pub fn report_document_id(&self) -> &str {
        &self.report_document_id
    }

    pub fn source_content_hash(&self) -> Option<&str> {
        self.source_content_hash.as_deref()
    }

    pub fn scope(&self) -> &str {
        &self.scope
    }

    pub fn data_quality(&self) -> &str {
        &self.data_quality
    }

    pub fn period_id(&self) -> Option<&str> {
        self.period_id.as_deref()
    }

    pub fn fiscal_year(&self) -> i64 {
        self.fiscal_year
    }

    pub fn period_type(&self) -> &str {
        &self.period_type
    }

    /// The canonical `expected_kpis_json` this manifest was sealed with,
    /// `None` when the manifest carries no expected-KPI snapshot —
    /// byte-compared against `kpi_ingest_runs.expected_kpis_json` by the atom.
    pub fn expected_kpis_json(&self) -> Option<&str> {
        self.expected_kpis_json.as_deref()
    }

    pub fn observation_content(&self) -> &[ObservationContentProjection] {
        &self.content
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in digest.as_slice() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

#[cfg(test)]
mod tests;
