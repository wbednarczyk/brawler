//! Acquisition-workflow submission tools (#386, ADR 0099 dec. 1): three of
//! the original nine — `stage_kpi_observations` (complete revision snapshot +
//! required `missingReasons` written in the SAME staging transaction),
//! `validate_kpi_ingest` (synchronous; the full manifest returns in the
//! response — a `failed` manifest IS the typed repair report; a raced loser
//! gets a typed `superseded` result carrying the current tuple) and
//! `commit_kpi_ingest` (synchronous atomic commit; idempotent replay returns
//! the stored receipt verbatim) — plus the tenth tool, `propose_kpi_definition`
//! (ADR 0101, epic #399 S4): lease-bound like stage, mints a company-scoped
//! `origin=agent` catalog entry through the narrow `get_or_create_kpi_definition`
//! helper, guarded by an exact-key check then a curated `kpi_aliases` redirect.
//!
//! Input byte caps live HERE, at the tool boundary (contracts.md tool 5) —
//! storage keeps its domain constraints (vocabularies, lease, state machine)
//! and no length caps. Execution metadata (`ExecutionMeta`, `client`
//! required) merges into `cost_json` inside the stage/commit transactions
//! only (ADR 0099 dec. 8).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::kpi_ingest::{
    get_existing_run, holder, is_content_hash, reject_control_chars, RunScopeInput,
};
use super::registry::McpScope;
use super::tools::{run, ToolCallError, ToolOutcome};
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::jobs::kpi_ingest_validation::validate_kpi_ingest_run;
use crate::storage::{
    AppState, CommitReceipt, KpiDefinition, KpiIngestRun, KpiIngestRunState, NewKpiDefinition,
    NewStagedObservation,
};

/// Contract budget (contracts.md tool 5): the complete revision snapshot is
/// 1..=100 observations; beyond the cap is a BUDGET refusal, zero is invalid
/// input (an empty snapshot is nonsense, not an oversized request).
const OBSERVATIONS_MAX: usize = 100;
const MISSING_REASONS_MAX: usize = 128;

// Input string byte caps (contracts.md tool 5, UTF-8 bytes).
const RAW_LABEL_MAX: usize = 256;
const RAW_VALUE_MAX: usize = 256;
const METRIC_KEY_CANDIDATE_MAX: usize = 256;
const NORMALIZED_VALUE_MAX: usize = 64;
const RAW_CURRENCY_MAX: usize = 128;
const RAW_UNIT_SCALE_MAX: usize = 128;
const CITATION_TABLE_MAX: usize = 128;
const CITATION_ROW_MAX: usize = 128;
const CITATION_QUOTE_MAX: usize = 1024;
const REASON_KEY_MAX: usize = 128;
const REASON_MAX: usize = 512;
const EXECUTION_STRING_MAX: usize = 128;

// `propose_kpi_definition` byte caps (ADR 0101, epic #399 S4) — label/unit
// mirror the context catalog's own output bounds (contracts.md § Budgets:
// "label ≤256 B, unit/statementGroup ≤64 B").
const DEFINITION_LABEL_MAX: usize = 256;
const DEFINITION_UNIT_MAX: usize = 64;
const DEFINITION_STATEMENT_GROUP_MAX: usize = 64;
const DEFINITION_DESCRIPTION_MAX: usize = 512;

// ============================================================================
// Inputs
// ============================================================================

/// `ExecutionMeta` (contracts.md shared shapes): diagnostic agent metadata.
/// `client` is REQUIRED in every supplied payload; numerics are totals /
/// snapshots, never deltas. Optional fields are skipped when absent so the
/// stored merge never overwrites a field with `null`.
#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExecutionMetaInput {
    pub client: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repair_rounds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_in: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_out: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(range(min = 0.0))]
    pub cost_usd: Option<f64>,
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnitScaleInput {
    Ones,
    Thousands,
    Millions,
}

impl UnitScaleInput {
    fn as_str(self) -> &'static str {
        match self {
            UnitScaleInput::Ones => "ones",
            UnitScaleInput::Thousands => "thousands",
            UnitScaleInput::Millions => "millions",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MeasureWindowInput {
    Flow,
    PointInTime,
    Trailing,
    Cumulative,
    Duration,
}

impl MeasureWindowInput {
    fn as_str(self) -> &'static str {
        match self {
            MeasureWindowInput::Flow => "flow",
            MeasureWindowInput::PointInTime => "point_in_time",
            MeasureWindowInput::Trailing => "trailing",
            MeasureWindowInput::Cumulative => "cumulative",
            MeasureWindowInput::Duration => "duration",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AttributionInput {
    Total,
    OwnersOfParent,
    Nci,
}

impl AttributionInput {
    fn as_str(self) -> &'static str {
        match self {
            AttributionInput::Total => "total",
            AttributionInput::OwnersOfParent => "owners_of_parent",
            AttributionInput::Nci => "nci",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MappingStatusInput {
    Unmapped,
    Mapped,
    NoDefinition,
    /// A deliberate, reasoned exclusion (ADR 0102 dec. 1) — legal only
    /// alongside a non-blank `exclusionReason`.
    Excluded,
}

impl MappingStatusInput {
    fn as_str(self) -> &'static str {
        match self {
            MappingStatusInput::Unmapped => "unmapped",
            MappingStatusInput::Mapped => "mapped",
            MappingStatusInput::NoDefinition => "no_definition",
            MappingStatusInput::Excluded => "excluded",
        }
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CitationInput {
    #[serde(default)]
    #[schemars(range(min = 1))]
    pub page: Option<i64>,
    #[serde(default)]
    pub table: Option<String>,
    #[serde(default)]
    pub row: Option<String>,
    #[serde(default)]
    pub quote: Option<String>,
}

/// One staged observation exactly as the tool contract freezes it
/// (contracts.md tool 5). Raw fields are stored verbatim; normalized fields
/// are the agent's own normalization for audit.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ObservationInput {
    pub raw_label: String,
    pub raw_value: String,
    #[serde(default)]
    pub raw_currency: Option<String>,
    #[serde(default)]
    pub raw_unit_scale: Option<String>,
    #[serde(default)]
    pub normalized_value: Option<String>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub unit_scale: Option<UnitScaleInput>,
    #[serde(default)]
    pub measure_window: Option<MeasureWindowInput>,
    #[serde(default)]
    pub attribution: Option<AttributionInput>,
    #[serde(default)]
    pub scope: Option<RunScopeInput>,
    #[serde(default)]
    pub metric_key_candidate: Option<String>,
    #[serde(default)]
    pub mapping_status: Option<MappingStatusInput>,
    /// Required exactly when `mappingStatus == "excluded"` (ADR 0102 dec.
    /// 1); a reason on any other disposition is a typed refusal.
    #[serde(default)]
    pub exclusion_reason: Option<String>,
    #[serde(default)]
    pub citation: Option<CitationInput>,
}

/// The explicit chunked-draft wire union (ADR 0102 dec. 14): `draft` ABSENT
/// selects [`SingleCallStageInput`] — today's shape, preserved byte-for-byte
/// (`observations` + `missingReasons` both required). `draft` PRESENT selects
/// [`DraftStageInput`], whose THREE legal sub-forms (open/append/finalize)
/// [`classify_draft`] validates — that finer split stays a typed runtime
/// refusal rather than three more `oneOf` schema branches (the same
/// mode-dependent-required-fields shape most single-endpoint APIs use).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum StageKpiObservationsInput {
    SingleCall(SingleCallStageInput),
    Draft(DraftStageInput),
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SingleCallStageInput {
    pub run_id: String,
    pub observations: Vec<ObservationInput>,
    /// REQUIRED: `{}` is the explicit "no declared omissions"/clear — there is
    /// no destructive default (contracts.md tool 5).
    pub missing_reasons: BTreeMap<String, String>,
    #[serde(default)]
    pub execution: Option<ExecutionMetaInput>,
}

/// One of three legal shapes (ADR 0102 dec. 6-9), disambiguated by
/// [`classify_draft`]: `{open:true, expectedObservations}` mints a
/// server-issued draft; `{draftId, chunkIndex}` appends a chunk (its content
/// travels in the OUTER `observations` field); `{draftId, final:true}`
/// finalizes (its `missingReasons` travels in the OUTER field too).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DraftInput {
    #[serde(default)]
    pub open: Option<bool>,
    #[serde(default)]
    pub expected_observations: Option<i64>,
    #[serde(default)]
    pub draft_id: Option<String>,
    #[serde(default)]
    pub chunk_index: Option<i64>,
    #[serde(default, rename = "final")]
    pub is_final: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DraftStageInput {
    pub run_id: String,
    pub draft: DraftInput,
    /// A chunk append's content; empty/absent on an open or finalize call.
    #[serde(default)]
    pub observations: Vec<ObservationInput>,
    /// Required exactly on a finalizing call (ADR 0102 dec. 14) — absent on
    /// open/append, where a value is a typed refusal (reasons belong to a
    /// complete revision).
    #[serde(default)]
    pub missing_reasons: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub execution: Option<ExecutionMetaInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ValidateKpiIngestInput {
    pub run_id: String,
    #[schemars(range(min = 1))]
    pub revision: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CommitKpiIngestInput {
    pub run_id: String,
    pub manifest_hash: String,
    #[schemars(range(min = 1))]
    pub revision: i64,
    #[serde(default)]
    pub execution: Option<ExecutionMetaInput>,
}

/// `propose_kpi_definition` (ADR 0101, epic #399 S4). Deliberately narrower
/// than `create_kpi_definition`'s full `NewKpiDefinition`: `scope`/
/// `company_id`/`sector`/`origin`/`computation` are never caller-suppliable
/// — they are forced to `company`/the run's own company/`agent`/`reported`
/// inside the storage layer, matching a full-capture agent's use case (a
/// disclosed number, not a derived ratio). `value_kind` IS caller-suppliable
/// but only from `{monetary, count}` (issue #403, amends ADR 0101 dec. 6):
/// the wider `kpi_definitions.value_kind` vocabulary (contracts.md § KPI
/// Definition) also allows `ratio`/`percentage`/`physical`/`duration`, but a
/// full-capture agent stages disclosed numbers, never derived ratios, so this
/// surface admits only the two disclosed-number kinds; absent defaults to
/// `monetary` for backward compatibility. `description`, when supplied, is
/// stored in the otherwise-unused `formula` column of a
/// `computation="reported"` row — free-text rationale, never evaluated as an
/// expression (the derivation engine only reads `formula` for
/// `computation="derived"` rows).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ProposeKpiDefinitionInput {
    pub run_id: String,
    pub metric_key: String,
    pub label: String,
    #[serde(default)]
    pub unit: Option<String>,
    pub statement_group: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub value_kind: Option<String>,
}

// ============================================================================
// Wire DTOs (MCP-only — no TS consumer, no ts_rs)
// ============================================================================

/// `status` ∈ `staged` (single-call or a draft finalize) | `draft_open` |
/// `draft_appended` (ADR 0102 dec. 6-9). Every field beyond `runId`/`status`
/// is populated only for the branches it applies to — the single-call/
/// finalize JSON stays byte-for-byte `{runId, revision, observationCount,
/// status}` (dec. 14: `#[serde(skip_serializing_if)]` omits every absent key).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageResultDto {
    pub run_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revision: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observation_count: Option<usize>,
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draft_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_observations: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_index: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunks_received: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTupleDto {
    pub status: String,
    pub revision: i64,
    pub manifest_hash: Option<String>,
}

/// Observation counts by `validationState` (ADR 0102 dec. 12): the bounded
/// substitute for the removed inline `manifest` — the full manifest is read
/// via `get_kpi_ingest_context section:"manifest"`.
#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeverityCountsDto {
    pub passed: usize,
    pub unreviewed: usize,
    pub flagged: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateResultDto {
    pub outcome: &'static str,
    pub revision: i64,
    pub manifest_hash: Option<String>,
    /// `None` only on `superseded` (no manifest was ever produced by this
    /// call). `manifest` itself no longer rides this response (ADR 0102 dec.
    /// 12, amends ADR 0099 dec. 1) — read it via
    /// `get_kpi_ingest_context section:"manifest"`.
    pub severity_counts: Option<SeverityCountsDto>,
    pub current: Option<CurrentTupleDto>,
}

/// One commit outcome, deserialized TYPED from the stored receipt — a closed
/// vocabulary plus the frozen conditional shape. **v1** (`outcomesSchemaVersion
/// 1`): `divergent` ⟺ (`detail` present ∧ `factId` null); every other outcome
/// ⟺ (`detail` absent ∧ `factId` non-null). **v2** (ADR 0102 dec. 3) adds a
/// THIRD legal case: `excluded` ⟺ (`detail` present, an
/// [`ExcludedObservationDetailDto`] ∧ `factId` null) — v1's own invariant is
/// never loosened, only extended. Violations are `internal` — the stored
/// receipt is the durable contract and a drifted shape must not silently
/// reach the wire.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CommitOutcomeDto {
    pub observation_id: String,
    pub revision: i64,
    pub ordinal: i64,
    pub metric_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fact_id: Option<String>,
    pub outcome: CommitOutcomeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<CommitOutcomeDetailDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitOutcomeKind {
    Created,
    Reobserved,
    Upgraded,
    Divergent,
    /// v2 only (ADR 0102 dec. 3) — never a written fact.
    Excluded,
}

/// Structurally disjoint field sets (mirrors [`crate::fundamentals::kpi_manifest::DiagnosticDetail`]'s
/// own untagged-enum idiom) — deserialization is unambiguous.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum CommitOutcomeDetailDto {
    Divergent(DivergentDetailDto),
    Excluded(ExcludedObservationDetailDto),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DivergentDetailDto {
    pub existing_fact_id: String,
}

/// The excluded ledger entry (ADR 0102 dec. 3/4): one per observation sealed
/// `excluded` — carried both on its own [`CommitOutcomeDto`] and rolled up
/// into [`CommitReceiptDto::excluded`].
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ExcludedObservationDetailDto {
    pub label: String,
    pub reason: String,
}

/// `propose_kpi_definition` success shape (ADR 0101 dec. 3): `created: false`
/// on an exact-key reuse, `true` on a fresh mint — never an error either way.
/// The `synonym_redirect` guard IS an error (`CommandErrorCode::SynonymRedirect`,
/// [`crate::storage::StorageError::KpiDefinitionSynonymRedirect`]), not a
/// variant of this shape.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposeKpiDefinitionResultDto {
    pub definition: KpiDefinition,
    pub created: bool,
}

/// Bounded (ADR 0102 dec. 12, amends ADR 0099 dec. 1): `acceptedCount` is the
/// installed-fact count, `excludedCount` is ALWAYS ledgered regardless of
/// terminal status (dec. 4; `0` under `outcomesSchemaVersion` 1).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitCountsDto {
    pub accepted_count: i64,
    pub excluded_count: usize,
}

/// The commit response is a BOUNDED summary (ADR 0102 dec. 12): the full
/// outcomes ledger — up to `AGGREGATE_OBSERVATIONS_MAX` (1000) rows, one
/// `{label, reason}` per `excluded` outcome — cannot ride an unpaged
/// response. Read it via `get_kpi_ingest_context section:"receipt"`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitReceiptDto {
    pub run_id: String,
    pub terminal_status: String,
    pub counts: CommitCountsDto,
    pub manifest_hash: String,
    pub manifest_revision: i64,
}

// ============================================================================
// Validation helpers (tool-boundary byte caps; storage stays capless)
// ============================================================================

fn invalid(message: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::InvalidInput, message.into())
}

fn internal(message: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, message.into())
}

fn bounded(field: &'static str, value: &str, max: usize) -> Result<(), CommandError> {
    reject_control_chars(field, value)?;
    if value.len() > max {
        return Err(invalid(format!("{field} exceeds {max} bytes")));
    }
    Ok(())
}

fn bounded_opt(field: &'static str, value: Option<&str>, max: usize) -> Result<(), CommandError> {
    match value {
        Some(value) => bounded(field, value, max),
        None => Ok(()),
    }
}

fn validate_execution(execution: &ExecutionMetaInput) -> Result<Value, CommandError> {
    bounded("execution.client", &execution.client, EXECUTION_STRING_MAX)?;
    if execution.client.trim().is_empty() {
        return Err(invalid("execution.client must be non-empty"));
    }
    bounded_opt(
        "execution.model",
        execution.model.as_deref(),
        EXECUTION_STRING_MAX,
    )?;
    bounded_opt(
        "execution.skillVersion",
        execution.skill_version.as_deref(),
        EXECUTION_STRING_MAX,
    )?;
    // NaN/±inf are refused alongside negatives — a cost must be a real total.
    if execution
        .cost_usd
        .is_some_and(|cost| !cost.is_finite() || cost < 0.0)
    {
        return Err(invalid("execution.costUsd must be non-negative"));
    }
    serde_json::to_value(execution)
        .map_err(|error| internal(format!("execution serialization failed: {error}")))
}

fn validate_observation(observation: &ObservationInput) -> Result<(), CommandError> {
    bounded("rawLabel", &observation.raw_label, RAW_LABEL_MAX)?;
    bounded("rawValue", &observation.raw_value, RAW_VALUE_MAX)?;
    if observation.raw_label.trim().is_empty() || observation.raw_value.trim().is_empty() {
        return Err(invalid("rawLabel and rawValue must be non-blank"));
    }
    bounded_opt(
        "metricKeyCandidate",
        observation.metric_key_candidate.as_deref(),
        METRIC_KEY_CANDIDATE_MAX,
    )?;
    bounded_opt(
        "normalizedValue",
        observation.normalized_value.as_deref(),
        NORMALIZED_VALUE_MAX,
    )?;
    bounded_opt(
        "rawCurrency",
        observation.raw_currency.as_deref(),
        RAW_CURRENCY_MAX,
    )?;
    bounded_opt(
        "rawUnitScale",
        observation.raw_unit_scale.as_deref(),
        RAW_UNIT_SCALE_MAX,
    )?;
    bounded_opt(
        "exclusionReason",
        observation.exclusion_reason.as_deref(),
        REASON_MAX,
    )?;
    if let Some(currency) = observation.currency.as_deref() {
        // The frozen INPUT contract is stricter than storage (which upcases):
        // exactly three UPPERCASE ASCII letters at the tool boundary.
        if currency.len() != 3 || !currency.bytes().all(|b| b.is_ascii_uppercase()) {
            return Err(invalid(format!(
                "currency must be exactly 3 uppercase ASCII letters, got {currency:?}"
            )));
        }
    }
    if let Some(citation) = &observation.citation {
        if citation.page.is_some_and(|page| page < 1) {
            return Err(invalid("citation.page must be >= 1"));
        }
        bounded_opt(
            "citation.table",
            citation.table.as_deref(),
            CITATION_TABLE_MAX,
        )?;
        bounded_opt("citation.row", citation.row.as_deref(), CITATION_ROW_MAX)?;
        bounded_opt(
            "citation.quote",
            citation.quote.as_deref(),
            CITATION_QUOTE_MAX,
        )?;
    }
    Ok(())
}

fn validate_missing_reasons(reasons: &BTreeMap<String, String>) -> Result<(), CommandError> {
    if reasons.len() > MISSING_REASONS_MAX {
        return Err(invalid(format!(
            "missingReasons carries {} entries — the cap is {MISSING_REASONS_MAX}",
            reasons.len()
        )));
    }
    for (key, reason) in reasons {
        bounded("missingReasons key", key, REASON_KEY_MAX)?;
        bounded("missingReasons reason", reason, REASON_MAX)?;
        if key.trim().is_empty() || reason.trim().is_empty() {
            // The canonical ledger schema requires non-empty reason strings
            // (data-model § missing_reasons_json) — refuse at the boundary
            // instead of staging a ledger the validator will call malformed.
            return Err(invalid("missingReasons keys and reasons must be non-blank"));
        }
    }
    Ok(())
}

fn to_staged(observation: ObservationInput) -> NewStagedObservation {
    NewStagedObservation {
        raw_label: observation.raw_label,
        raw_value: observation.raw_value,
        raw_currency: observation.raw_currency,
        raw_unit_scale: observation.raw_unit_scale,
        normalized_value: observation.normalized_value,
        currency: observation.currency,
        unit_scale: observation
            .unit_scale
            .map(|value| value.as_str().to_owned()),
        measure_window: observation
            .measure_window
            .map(|value| value.as_str().to_owned()),
        attribution: observation
            .attribution
            .map(|value| value.as_str().to_owned()),
        scope: observation.scope.map(|value| value.as_str().to_owned()),
        metric_key_candidate: observation.metric_key_candidate,
        mapping_status: observation
            .mapping_status
            .map(|value| value.as_str().to_owned()),
        exclusion_reason: observation.exclusion_reason,
        citation_page: observation
            .citation
            .as_ref()
            .and_then(|citation| citation.page),
        citation_table: observation
            .citation
            .as_ref()
            .and_then(|citation| citation.table.clone()),
        citation_row: observation
            .citation
            .as_ref()
            .and_then(|citation| citation.row.clone()),
        citation_quote: observation.citation.and_then(|citation| citation.quote),
    }
}

// ============================================================================
// stage_kpi_observations
// ============================================================================

/// The three legal `draft` sub-forms (ADR 0102 dec. 6-9) — the runtime
/// classification [`classify_draft`] enforces since the top-level wire union
/// only splits on `draft` present/absent, not on which sub-form it is.
enum DraftForm {
    Open { expected_observations: i64 },
    Append { draft_id: String, chunk_index: i64 },
    Finalize { draft_id: String },
}

fn classify_draft(draft: &DraftInput) -> Result<DraftForm, CommandError> {
    match (
        draft.open,
        draft.expected_observations,
        draft.draft_id.as_deref(),
        draft.chunk_index,
        draft.is_final,
    ) {
        (Some(true), Some(expected), None, None, None | Some(false)) => {
            if expected < 1 {
                return Err(invalid("draft.expectedObservations must be >= 1"));
            }
            Ok(DraftForm::Open {
                expected_observations: expected,
            })
        }
        (None | Some(false), None, Some(draft_id), Some(chunk_index), None | Some(false)) => {
            if chunk_index < 0 {
                return Err(invalid("draft.chunkIndex must be >= 0"));
            }
            Ok(DraftForm::Append {
                draft_id: draft_id.to_owned(),
                chunk_index,
            })
        }
        (None | Some(false), None, Some(draft_id), None, Some(true)) => Ok(DraftForm::Finalize {
            draft_id: draft_id.to_owned(),
        }),
        _ => Err(invalid(
            "draft must be exactly one of {open:true, expectedObservations}, \
             {draftId, chunkIndex}, or {draftId, final:true}",
        )),
    }
}

fn validate_observations_batch(observations: &[ObservationInput]) -> Result<(), CommandError> {
    if observations.len() > OBSERVATIONS_MAX {
        return Err(CommandError::new(
            CommandErrorCode::ResponseBudgetExceeded,
            format!(
                "observations carries {} entries — the per-call budget is {OBSERVATIONS_MAX}",
                observations.len()
            ),
        ));
    }
    for observation in observations {
        validate_observation(observation)?;
    }
    Ok(())
}

fn stage_kpi_observations(
    state: &AppState,
    scope: McpScope,
    input: StageKpiObservationsInput,
) -> Result<StageResultDto, CommandError> {
    match input {
        StageKpiObservationsInput::SingleCall(input) => stage_single_call(state, scope, input),
        StageKpiObservationsInput::Draft(input) => stage_draft(state, scope, input),
    }
}

/// `draft` absent — today's shape, byte-for-byte (ADR 0102 dec. 14).
fn stage_single_call(
    state: &AppState,
    scope: McpScope,
    input: SingleCallStageInput,
) -> Result<StageResultDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    if input.observations.is_empty() {
        return Err(invalid(
            "observations is the complete revision snapshot and must carry at least one entry",
        ));
    }
    validate_observations_batch(&input.observations)?;
    validate_missing_reasons(&input.missing_reasons)?;
    let execution = input
        .execution
        .as_ref()
        .map(validate_execution)
        .transpose()?;

    let observations: Vec<NewStagedObservation> =
        input.observations.into_iter().map(to_staged).collect();
    let (revision, inserted) = state
        .kpi_ingest_staging()
        .stage_observations(
            &input.run_id,
            holder(scope),
            observations,
            &input.missing_reasons,
            execution.as_ref(),
        )
        .map_err(CommandError::from)?;
    Ok(StageResultDto {
        run_id: input.run_id,
        revision: Some(revision),
        observation_count: Some(inserted.len()),
        status: "staged",
        draft_id: None,
        expected_observations: None,
        chunk_index: None,
        chunks_received: None,
    })
}

/// `draft` present — open/append/finalize (ADR 0102 dec. 6-9).
fn stage_draft(
    state: &AppState,
    scope: McpScope,
    input: DraftStageInput,
) -> Result<StageResultDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    match classify_draft(&input.draft)? {
        DraftForm::Open {
            expected_observations,
        } => {
            if !input.observations.is_empty() {
                return Err(invalid(
                    "draft.open must not carry observations in the same call — append a chunk separately",
                ));
            }
            if input.missing_reasons.is_some() {
                return Err(invalid(
                    "missingReasons is only accepted on a finalizing call",
                ));
            }
            let summary = state
                .kpi_ingest_drafts()
                .open_draft(&input.run_id, holder(scope), expected_observations)
                .map_err(CommandError::from)?;
            Ok(StageResultDto {
                run_id: input.run_id,
                revision: None,
                observation_count: None,
                status: "draft_open",
                draft_id: Some(summary.draft_id),
                expected_observations: Some(summary.expected_observations),
                chunk_index: None,
                chunks_received: Some(summary.chunks_received),
            })
        }
        DraftForm::Append {
            draft_id,
            chunk_index,
        } => {
            if input.observations.is_empty() {
                return Err(invalid(
                    "a chunk append must carry at least one observation",
                ));
            }
            if input.missing_reasons.is_some() {
                return Err(invalid(
                    "missingReasons is only accepted on a finalizing call, not a chunk append",
                ));
            }
            validate_observations_batch(&input.observations)?;
            let observations: Vec<NewStagedObservation> =
                input.observations.into_iter().map(to_staged).collect();
            let result = state
                .kpi_ingest_drafts()
                .append_chunk(
                    &input.run_id,
                    holder(scope),
                    &draft_id,
                    chunk_index,
                    observations,
                )
                .map_err(CommandError::from)?;
            Ok(StageResultDto {
                run_id: input.run_id,
                revision: None,
                observation_count: None,
                status: "draft_appended",
                draft_id: Some(result.draft_id),
                expected_observations: None,
                chunk_index: Some(result.chunk_index),
                chunks_received: Some(result.chunks_received),
            })
        }
        DraftForm::Finalize { draft_id } => {
            if !input.observations.is_empty() {
                return Err(invalid(
                    "a finalizing call must not carry observations — append every chunk first",
                ));
            }
            let Some(missing_reasons) = input.missing_reasons else {
                return Err(invalid("missingReasons is required to finalize a draft"));
            };
            validate_missing_reasons(&missing_reasons)?;
            let execution = input
                .execution
                .as_ref()
                .map(validate_execution)
                .transpose()?;
            let (revision, inserted) = state
                .kpi_ingest_drafts()
                .finalize_draft(
                    &input.run_id,
                    holder(scope),
                    &draft_id,
                    &missing_reasons,
                    execution.as_ref(),
                )
                .map_err(CommandError::from)?;
            Ok(StageResultDto {
                run_id: input.run_id,
                revision: Some(revision),
                observation_count: Some(inserted.len()),
                status: "staged",
                draft_id: None,
                expected_observations: None,
                chunk_index: None,
                chunks_received: None,
            })
        }
    }
}

// ============================================================================
// propose_kpi_definition
// ============================================================================

fn propose_kpi_definition(
    state: &AppState,
    scope: McpScope,
    input: ProposeKpiDefinitionInput,
) -> Result<ProposeKpiDefinitionResultDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    bounded("metricKey", &input.metric_key, METRIC_KEY_CANDIDATE_MAX)?;
    let metric_key = input.metric_key.trim().to_owned();
    if !super::acts::is_snake_case_ascii_metric_key(&metric_key) {
        return Err(invalid(format!(
            "metricKey must be snake_case ASCII matching ^[a-z][a-z0-9_]*$, got {metric_key:?}"
        )));
    }
    bounded("label", &input.label, DEFINITION_LABEL_MAX)?;
    if input.label.trim().is_empty() {
        return Err(invalid("label must be non-blank"));
    }
    bounded_opt("unit", input.unit.as_deref(), DEFINITION_UNIT_MAX)?;
    bounded(
        "statementGroup",
        &input.statement_group,
        DEFINITION_STATEMENT_GROUP_MAX,
    )?;
    bounded_opt(
        "description",
        input.description.as_deref(),
        DEFINITION_DESCRIPTION_MAX,
    )?;
    // Issue #403: the propose surface admits only the two disclosed-number
    // kinds — never the wider create_kpi_definition vocabulary's derived
    // ratio/percentage/physical/duration (contracts.md § KPI Definition).
    let value_kind = match input.value_kind.as_deref() {
        None => "monetary",
        Some("monetary") => "monetary",
        Some("count") => "count",
        Some(other) => {
            return Err(invalid(format!(
                "valueKind must be one of monetary, count, got {other:?}"
            )))
        }
    }
    .to_owned();

    // scope/company_id/sector/origin are forced from the run inside the
    // storage layer (ADR 0101 dec. 6) — every value set here is a caller-
    // meaningful placeholder, overwritten before the INSERT.
    let new_definition = NewKpiDefinition {
        scope: "company".to_owned(),
        company_id: None,
        sector: None,
        metric_key,
        label: input.label.trim().to_owned(),
        value_kind,
        unit: input.unit,
        computation: "reported".to_owned(),
        formula: input.description,
        display_format: None,
        origin: None,
        statement_group: Some(input.statement_group),
        period_nature: None,
    };

    let (definition, created) = state
        .kpi_ingest_runs()
        .propose_kpi_definition(&input.run_id, holder(scope), new_definition)
        .map_err(CommandError::from)?;
    Ok(ProposeKpiDefinitionResultDto {
        definition,
        created,
    })
}

// ============================================================================
// validate_kpi_ingest
// ============================================================================

fn superseded_result(requested_revision: i64, run: &KpiIngestRun) -> ValidateResultDto {
    ValidateResultDto {
        outcome: "superseded",
        revision: requested_revision,
        manifest_hash: None,
        severity_counts: None,
        current: Some(CurrentTupleDto {
            status: run.status.as_str().to_owned(),
            revision: run.manifest_revision,
            manifest_hash: run.manifest_hash.clone(),
        }),
    }
}

/// The synchronous loser's classifier (ADR 0099 dec. 1): after ANY validator
/// error, the correctness signal is the re-read tuple, never the error code —
/// the raced loser may surface `InvalidStagingRevision`, `SealedManifestRejected`
/// or others. The tuple still at `staged@requested` means the error is real
/// and propagates; a moved tuple means this call was superseded.
fn classify_validate_error(
    state: &AppState,
    run_id: &str,
    requested_revision: i64,
    original: CommandError,
) -> Result<ValidateResultDto, CommandError> {
    match state.kpi_ingest_runs().get_run(run_id) {
        Ok(Some(run))
            if !(run.status == KpiIngestRunState::Staged
                && run.manifest_revision == requested_revision) =>
        {
            Ok(superseded_result(requested_revision, &run))
        }
        _ => Err(original),
    }
}

fn validate_kpi_ingest(
    state: &AppState,
    input: ValidateKpiIngestInput,
) -> Result<ValidateResultDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    if input.revision < 1 {
        return Err(invalid("revision must be >= 1"));
    }
    let run = get_existing_run(state, &input.run_id)?;
    // Generation pre-check: a moved tuple is `superseded` without running the
    // validator at all.
    if !(run.status == KpiIngestRunState::Staged && run.manifest_revision == input.revision) {
        return Ok(superseded_result(input.revision, &run));
    }
    match validate_kpi_ingest_run(state, &input.run_id) {
        Ok(outcome) => {
            let mut counts = SeverityCountsDto::default();
            for observation in &outcome.manifest.observations {
                match observation.validation_state {
                    crate::fundamentals::kpi_manifest::ValidationState::Passed => {
                        counts.passed += 1
                    }
                    crate::fundamentals::kpi_manifest::ValidationState::Unreviewed => {
                        counts.unreviewed += 1
                    }
                    crate::fundamentals::kpi_manifest::ValidationState::Flagged => {
                        counts.flagged += 1
                    }
                }
            }
            Ok(ValidateResultDto {
                outcome: if outcome.outcome == "ready" {
                    "ready"
                } else {
                    "failed"
                },
                revision: input.revision,
                manifest_hash: Some(outcome.manifest_hash),
                severity_counts: Some(counts),
                current: None,
            })
        }
        Err(original) => classify_validate_error(state, &input.run_id, input.revision, original),
    }
}

// ============================================================================
// commit_kpi_ingest
// ============================================================================

/// The stored-receipt reader (ADR 0102 dec. 3): accepts BOTH
/// `outcomesSchemaVersion` 1 and 2 — v1's shape invariant (every
/// non-divergent outcome carries a `factId`, divergent never does) is never
/// loosened, only extended with a third legal case that v2 alone admits:
/// `excluded`, no `factId`, carrying its `{label, reason}` detail. Any other
/// version is `internal` — an old build reading a future schema, or a
/// corrupt column.
fn receipt_dto(receipt: CommitReceipt) -> Result<CommitReceiptDto, CommandError> {
    if receipt.outcomes_schema_version != 1 && receipt.outcomes_schema_version != 2 {
        return Err(internal(format!(
            "stored receipt outcomesSchemaVersion {} is unsupported by this build",
            receipt.outcomes_schema_version
        )));
    }
    // The durable writer serializes `factId` explicitly (`null` for
    // divergent/excluded) — a MISSING key is shape drift, not a legal null
    // (luna P1).
    let raw_outcomes: Vec<Value> = serde_json::from_str(&receipt.outcomes_json)
        .map_err(|error| internal(format!("stored receipt outcomes are malformed: {error}")))?;
    for raw in &raw_outcomes {
        if !raw
            .as_object()
            .is_some_and(|object| object.contains_key("factId"))
        {
            return Err(internal(
                "stored receipt outcome omits the explicit factId key",
            ));
        }
    }
    let outcomes: Vec<CommitOutcomeDto> = serde_json::from_value(Value::Array(raw_outcomes))
        .map_err(|error| internal(format!("stored receipt outcomes are malformed: {error}")))?;
    for outcome in &outcomes {
        let legal = match outcome.outcome {
            CommitOutcomeKind::Divergent => {
                matches!(outcome.detail, Some(CommitOutcomeDetailDto::Divergent(_)))
                    && outcome.fact_id.is_none()
            }
            // v2-only: `excluded` never legal under a v1 stored receipt —
            // that outcome member did not exist under v1 (v1 invariant is
            // extended, never loosened).
            CommitOutcomeKind::Excluded => {
                receipt.outcomes_schema_version == 2
                    && matches!(outcome.detail, Some(CommitOutcomeDetailDto::Excluded(_)))
                    && outcome.fact_id.is_none()
            }
            CommitOutcomeKind::Created
            | CommitOutcomeKind::Reobserved
            | CommitOutcomeKind::Upgraded => outcome.detail.is_none() && outcome.fact_id.is_some(),
        };
        if !legal {
            return Err(internal(format!(
                "stored receipt outcome for {} violates the outcome/factId/detail invariant",
                outcome.observation_id
            )));
        }
    }
    let excluded_count = outcomes
        .iter()
        .filter(|outcome| matches!(outcome.outcome, CommitOutcomeKind::Excluded))
        .count();
    Ok(CommitReceiptDto {
        run_id: receipt.run_id,
        terminal_status: receipt.terminal_status,
        counts: CommitCountsDto {
            accepted_count: receipt.accepted_count,
            excluded_count,
        },
        manifest_hash: receipt.manifest_hash,
        manifest_revision: receipt.manifest_revision,
    })
}

fn commit_kpi_ingest(
    state: &AppState,
    input: CommitKpiIngestInput,
) -> Result<CommitReceiptDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    reject_control_chars("manifestHash", &input.manifest_hash)?;
    if !is_content_hash(&input.manifest_hash) {
        return Err(invalid(
            "manifestHash must be exactly 64 lowercase hex bytes",
        ));
    }
    if input.revision < 1 {
        return Err(invalid("revision must be >= 1"));
    }
    let execution = input
        .execution
        .as_ref()
        .map(validate_execution)
        .transpose()?;
    let receipt = state
        .kpi_ingest_commit()
        .commit_manifest(
            &input.run_id,
            &input.manifest_hash,
            input.revision,
            execution.as_ref(),
        )
        .map_err(CommandError::from)?;
    receipt_dto(receipt)
}

// ============================================================================
// Registered handlers
// ============================================================================

pub fn stage_kpi_observations_handler(
    state: &AppState,
    scope: McpScope,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| {
        stage_kpi_observations(state, scope, input)
    })
}

pub fn propose_kpi_definition_handler(
    state: &AppState,
    scope: McpScope,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| {
        propose_kpi_definition(state, scope, input)
    })
}

pub fn validate_kpi_ingest_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| validate_kpi_ingest(state, input))
}

pub fn commit_kpi_ingest_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| commit_kpi_ingest(state, input))
}

#[cfg(test)]
mod tests {
    use super::super::kpi_ingest::test_support::*;
    use super::super::registry::call;
    use super::*;
    use crate::storage::AppState;
    use serde_json::json;

    fn start_characteristic_run(state: &AppState) -> String {
        let payload = success(acquisition_call(
            state,
            "start_kpi_ingest",
            &json!({
                "documentId": "doc1",
                "profileId": "company_characteristic",
                "scope": "standalone",
                "dataQuality": "final",
                "period": { "fiscalYear": 2025, "periodType": "FY" }
            }),
        ));
        assert_eq!(payload["status"], "extracting");
        payload["runId"].as_str().expect("runId").to_owned()
    }

    fn mapped_observation(with_citation: bool) -> Value {
        let mut observation = json!({
            "rawLabel": "revenue",
            "rawValue": "1000",
            "normalizedValue": "1000",
            "currency": "PLN",
            "unitScale": "ones",
            "measureWindow": "flow",
            "attribution": "total",
            "metricKeyCandidate": "revenue",
            "mappingStatus": "mapped"
        });
        if with_citation {
            observation["citation"] = json!({ "page": 3, "quote": "revenue quote" });
        }
        observation
    }

    fn stage_args(run_id: &str, observations: Vec<Value>) -> Value {
        json!({
            "runId": run_id,
            "observations": observations,
            "missingReasons": {}
        })
    }

    fn force_repairable(state: &AppState, run_id: &str) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'validation_failed', manifest_hash = NULL \
                 WHERE id = ?1",
                [run_id],
            )
            .expect("repairable");
    }

    // ------------------------------------------------------------------
    // stage_kpi_observations
    // ------------------------------------------------------------------

    #[test]
    fn stage_writes_the_snapshot_reasons_and_execution() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        let payload = success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &json!({
                "runId": run_id,
                "observations": [mapped_observation(true)],
                "missingReasons": { "net_profit": "not disclosed" },
                "execution": { "client": "a", "tokensIn": 5 }
            }),
        ));
        assert_eq!(payload["status"], "staged");
        assert_eq!(payload["revision"], 1);
        assert_eq!(payload["observationCount"], 1);

        let run = state
            .kpi_ingest_runs()
            .get_run(&run_id)
            .expect("get")
            .expect("run");
        assert_eq!(
            run.missing_reasons_json.as_deref(),
            Some(r#"{"net_profit":"not disclosed"}"#)
        );
        let cost: Value =
            serde_json::from_str(run.cost_json.as_deref().expect("cost")).expect("json");
        assert_eq!(cost["client"], "a");
        assert_eq!(cost["tokensIn"], 5);
    }

    /// ADR 0102 dec. 14: the single-call wire contract is byte-for-byte
    /// unchanged when `draft` is absent. The response carries EXACTLY the
    /// four keys it always has (no `draftId`/`chunkIndex`/`chunksReceived`/
    /// `expectedObservations` leaking in via the new `Option` fields'
    /// `skip_serializing_if`), and `deny_unknown_fields` still rejects an
    /// unrecognized top-level key exactly as before the wire union existed.
    #[test]
    fn single_call_path_unchanged() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let payload = success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(&run_id, vec![mapped_observation(true)]),
        ));
        let mut keys: Vec<&str> = payload
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["observationCount", "revision", "runId", "status"],
            "the single-call response carries exactly today's four keys"
        );
        assert_eq!(payload["status"], "staged");

        // `deny_unknown_fields` still rejects an unrecognized top-level field
        // at the protocol layer exactly as it did before the wire union
        // existed — a malformed request never silently matches the OTHER
        // union variant instead.
        let unknown_field = call(
            &state,
            McpScope::KpiAcquisition,
            "stage_kpi_observations",
            &json!({
                "runId": run_id,
                "observations": [mapped_observation(true)],
                "missingReasons": {},
                "notAWireField": true
            }),
        );
        match unknown_field {
            Err(ToolCallError::InvalidArguments(_)) => {}
            other => panic!("expected InvalidArguments, got {other:?}"),
        }
    }

    #[test]
    fn stage_budget_and_cap_refusals() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let many: Vec<Value> = (0..101).map(|_| mapped_observation(true)).collect();
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "stage_kpi_observations",
                &stage_args(&run_id, many)
            )),
            CommandErrorCode::ResponseBudgetExceeded,
            "101 observations"
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "stage_kpi_observations",
                &stage_args(&run_id, vec![])
            )),
            CommandErrorCode::InvalidInput,
            "empty snapshot"
        );

        let mut long_label = mapped_observation(true);
        long_label["rawLabel"] = json!("x".repeat(257));
        let mut long_quote = mapped_observation(true);
        long_quote["citation"] = json!({ "page": 1, "quote": "q".repeat(1025) });
        let mut control = mapped_observation(true);
        control["rawLabel"] = json!("bad\u{0001}label");
        let mut lowercase_currency = mapped_observation(true);
        lowercase_currency["currency"] = json!("pln");
        let mut page_zero = mapped_observation(true);
        page_zero["citation"] = json!({ "page": 0, "quote": "q" });
        for (name, observation) in [
            ("257-byte rawLabel", long_label),
            ("1025-byte quote", long_quote),
            ("control char", control),
            ("lowercase currency", lowercase_currency),
            ("page zero", page_zero),
        ] {
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "stage_kpi_observations",
                    &stage_args(&run_id, vec![observation])
                )),
                CommandErrorCode::InvalidInput,
                "{name}"
            );
        }

        // missingReasons caps: 129 entries, long key, long reason, control.
        let too_many: BTreeMap<String, String> = (0..129)
            .map(|i| (format!("k{i}"), "r".to_owned()))
            .collect();
        let long_key: BTreeMap<String, String> = [("k".repeat(129), "r".to_owned())].into();
        let long_reason: BTreeMap<String, String> = [("k".to_owned(), "r".repeat(513))].into();
        let control_reason: BTreeMap<String, String> =
            [("k".to_owned(), "bad\u{0001}".to_owned())].into();
        let blank_reason: BTreeMap<String, String> = [("k".to_owned(), "  ".to_owned())].into();
        for (name, reasons) in [
            ("129 entries", too_many),
            ("129-byte key", long_key),
            ("513-byte reason", long_reason),
            ("control char reason", control_reason),
            ("blank reason", blank_reason),
        ] {
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "stage_kpi_observations",
                    &json!({
                        "runId": run_id,
                        "observations": [mapped_observation(true)],
                        "missingReasons": reasons
                    })
                )),
                CommandErrorCode::InvalidInput,
                "{name}"
            );
        }
    }

    #[test]
    fn execution_meta_is_schema_required_and_byte_bounded() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        // Missing required `client` → strict deserialize → -32602.
        let error = call(
            &state,
            McpScope::KpiAcquisition,
            "stage_kpi_observations",
            &json!({
                "runId": run_id,
                "observations": [mapped_observation(true)],
                "missingReasons": {},
                "execution": { "tokensIn": 5 }
            }),
        )
        .expect_err("missing client is a schema violation");
        assert!(matches!(error, ToolCallError::InvalidArguments(_)));

        for (name, execution) in [
            ("129-byte client", json!({ "client": "c".repeat(129) })),
            (
                "negative costUsd",
                json!({ "client": "a", "costUsd": -0.5 }),
            ),
        ] {
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "stage_kpi_observations",
                    &json!({
                        "runId": run_id,
                        "observations": [mapped_observation(true)],
                        "missingReasons": {},
                        "execution": execution
                    })
                )),
                CommandErrorCode::InvalidInput,
                "{name}"
            );
        }
    }

    #[test]
    fn restage_replaces_the_snapshot_and_lease_and_state_refusals_are_typed() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(
                &run_id,
                vec![mapped_observation(true), mapped_observation(true)],
            ),
        ));

        // Staged is not a stageable state → conflict.
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "stage_kpi_observations",
                &stage_args(&run_id, vec![mapped_observation(true)])
            )),
            CommandErrorCode::Conflict
        );

        // Repair: the new revision is a COMPLETE replace; the old revision's
        // rows stay archived under their own revision.
        force_repairable(&state, &run_id);
        let payload = success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(&run_id, vec![mapped_observation(true)]),
        ));
        assert_eq!(payload["revision"], 2);
        assert_eq!(payload["observationCount"], 1);
        let rev1 = state
            .kpi_ingest_staging()
            .list_staged_observations(&run_id, Some(1))
            .expect("list");
        assert_eq!(rev1.len(), 2, "the old revision stays archived");

        // Expired lease → the refined typed refusal.
        force_repairable(&state, &run_id);
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET lease_expires_at = '2000-01-01T00:00:00Z' \
                     WHERE id = ?1",
                    [&run_id],
                )
                .expect("expire");
        }
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "stage_kpi_observations",
                &stage_args(&run_id, vec![mapped_observation(true)])
            )),
            CommandErrorCode::RunLeaseExpired
        );
    }

    // ------------------------------------------------------------------
    // propose_kpi_definition
    // ------------------------------------------------------------------

    fn propose_args(run_id: &str, metric_key: &str) -> Value {
        json!({
            "runId": run_id,
            "metricKey": metric_key,
            "label": "Broker client count",
            "statementGroup": "other"
        })
    }

    #[test]
    fn propose_mints_a_company_scoped_agent_definition() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        let run = state
            .kpi_ingest_runs()
            .get_run(&run_id)
            .expect("get")
            .expect("run");

        let payload = success(acquisition_call(
            &state,
            "propose_kpi_definition",
            &propose_args(&run_id, "broker_client_count"),
        ));
        assert_eq!(payload["created"], true);
        assert_eq!(payload["definition"]["metricKey"], "broker_client_count");
        assert_eq!(payload["definition"]["scope"], "company");
        assert_eq!(payload["definition"]["companyId"], run.company_id);
        assert_eq!(payload["definition"]["origin"], "agent");
        assert_eq!(payload["definition"]["valueKind"], "monetary");
    }

    #[test]
    fn propose_duplicate_exact_key_returns_existing() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let first = success(acquisition_call(
            &state,
            "propose_kpi_definition",
            &propose_args(&run_id, "broker_client_count"),
        ));
        assert_eq!(first["created"], true);
        let first_id = first["definition"]["id"].as_str().expect("id").to_owned();

        let second = success(acquisition_call(
            &state,
            "propose_kpi_definition",
            &propose_args(&run_id, "broker_client_count"),
        ));
        assert_eq!(second["created"], false);
        assert_eq!(second["definition"]["id"], first_id);
    }

    #[test]
    fn propose_synonym_returns_typed_redirect() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "propose_kpi_definition",
                &propose_args(&run_id, "inventory"),
            )),
            CommandErrorCode::SynonymRedirect
        );

        // The canonical target ("inventories") is untouched by the refusal —
        // no shadow definition was minted under either key.
        assert!(state
            .financials()
            .list_kpi_definitions(crate::storage::ListKpiDefinitionsInput {
                scope: Some("company".to_owned()),
                sector: None,
                company_id: None,
            })
            .expect("list")
            .into_iter()
            .all(|d| d.metric_key != "inventory"));
    }

    /// ADR 0101 dec. 3/4: the exact-key reuse is against the WHOLE catalog,
    /// not just this company's own minted rows — proposing a key the shared
    /// canon already carries returns the canonical definition (`created:
    /// false`), never mints a company-scoped shadow whose duplicate
    /// `metricKey` would fragment the catalog (the wdf_equity_parent /
    /// inventories disease this ADR exists to prevent).
    #[test]
    fn propose_canonical_key_reuses_canon_never_mints_a_shadow() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let payload = success(acquisition_call(
            &state,
            "propose_kpi_definition",
            &propose_args(&run_id, "total_assets"),
        ));
        assert_eq!(payload["created"], false);
        assert_eq!(payload["definition"]["metricKey"], "total_assets");
        assert!(
            payload["definition"]["companyId"].is_null(),
            "the canonical row is returned, not a company shadow: {:?}",
            payload["definition"]
        );

        assert!(
            state
                .financials()
                .list_kpi_definitions(crate::storage::ListKpiDefinitionsInput {
                    scope: Some("company".to_owned()),
                    sector: None,
                    company_id: None,
                })
                .expect("list")
                .into_iter()
                .all(|d| d.metric_key != "total_assets"),
            "no company-scoped shadow of a canonical key was minted"
        );
    }

    #[test]
    fn propose_forces_company_scope_and_agent_origin() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        let run = state
            .kpi_ingest_runs()
            .get_run(&run_id)
            .expect("get")
            .expect("run");

        let payload = success(acquisition_call(
            &state,
            "propose_kpi_definition",
            &propose_args(&run_id, "custom_broker_metric"),
        ));
        assert_eq!(payload["definition"]["scope"], "company");
        assert_eq!(payload["definition"]["companyId"], run.company_id);
        assert_eq!(payload["definition"]["origin"], "agent");
    }

    #[test]
    fn propose_requires_live_lease() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET lease_expires_at = '2000-01-01T00:00:00Z' \
                     WHERE id = ?1",
                    [&run_id],
                )
                .expect("expire");
        }
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "propose_kpi_definition",
                &propose_args(&run_id, "broker_client_count"),
            )),
            CommandErrorCode::RunLeaseExpired
        );
    }

    #[test]
    fn propose_rejects_malformed_metric_key_and_blank_label() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let mut bad_key = propose_args(&run_id, "BadKey");
        bad_key["metricKey"] = json!("BadKey");
        assert_eq!(
            failure_code(acquisition_call(&state, "propose_kpi_definition", &bad_key)),
            CommandErrorCode::InvalidInput,
            "PascalCase key"
        );

        let mut blank_label = propose_args(&run_id, "broker_client_count");
        blank_label["label"] = json!("   ");
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "propose_kpi_definition",
                &blank_label
            )),
            CommandErrorCode::InvalidInput,
            "blank label"
        );
    }

    /// Issue #403: a full-capture agent proposing a disclosed client COUNT
    /// must be able to mint `value_kind:"count"` — the canon already carries
    /// non-null `unit` on count rows (`shares`, `properties`), so `unit`
    /// stays allowed alongside `valueKind:"count"`.
    #[test]
    fn propose_value_kind_count_mints_count_definition() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let mut args = propose_args(&run_id, "active_clients");
        args["valueKind"] = json!("count");
        args["unit"] = json!("clients");
        let payload = success(acquisition_call(&state, "propose_kpi_definition", &args));
        assert_eq!(payload["created"], true);
        assert_eq!(payload["definition"]["valueKind"], "count");
        assert_eq!(payload["definition"]["unit"], "clients");
    }

    /// Absent `valueKind` still defaults to `monetary` (backward compatibility
    /// with every caller minted before issue #403).
    #[test]
    fn propose_value_kind_defaults_to_monetary() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let payload = success(acquisition_call(
            &state,
            "propose_kpi_definition",
            &propose_args(&run_id, "broker_client_count"),
        ));
        assert_eq!(payload["definition"]["valueKind"], "monetary");
    }

    /// The propose surface admits only `monetary`/`count` (a full-capture
    /// agent stages disclosed numbers, not derived ratios) even though the
    /// wider `kpi_definitions.value_kind` vocabulary (contracts.md § KPI
    /// Definition) allows `ratio`/`percentage`/`physical`/`duration`.
    #[test]
    fn propose_value_kind_outside_vocabulary_refused() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        let mut args = propose_args(&run_id, "broker_client_count");
        args["valueKind"] = json!("ratio");
        assert_eq!(
            failure_code(acquisition_call(&state, "propose_kpi_definition", &args)),
            CommandErrorCode::InvalidInput,
            "valueKind outside {{monetary, count}}"
        );
    }

    // ------------------------------------------------------------------
    // validate_kpi_ingest — the typed repair loop
    // ------------------------------------------------------------------

    #[test]
    fn validate_ready_failed_repair_and_superseded() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        // Revision 1 is deliberately broken: no citation → citation.missing
        // (Flagged) → outcome REQUIRED failed; the manifest IS the repair
        // report.
        success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(&run_id, vec![mapped_observation(false)]),
        ));
        let failed = success(acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 1 }),
        ));
        assert_eq!(failed["outcome"], "failed");
        assert!(failed["manifestHash"].is_string());
        // ADR 0102 dec. 12: the full manifest no longer rides this response —
        // `severityCounts` is the bounded substitute (`citation.missing` is
        // `Flagged`); the repair report itself is read via
        // `get_kpi_ingest_context section:"manifest"`
        // (`validate_returns_summary_not_manifest` covers both halves).
        assert!(failed["manifest"].is_null());
        assert_eq!(failed["severityCounts"]["flagged"], 1);
        let manifest_page = success(acquisition_call(
            &state,
            "get_kpi_ingest_context",
            &json!({ "runId": run_id, "section": "manifest" }),
        ));
        let manifest_text = manifest_page["manifest"].to_string();
        assert!(
            manifest_text.contains("citation.missing"),
            "the paged manifest names the diagnostic: {manifest_text}"
        );

        // Repair: revision 2 is the COMPLETE snapshot with the citation fixed.
        success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(&run_id, vec![mapped_observation(true)]),
        ));
        let ready = success(acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 2 }),
        ));
        assert_eq!(ready["outcome"], "ready");
        assert!(ready["manifest"].is_null());
        let counts = &ready["severityCounts"];
        let total = counts["passed"].as_i64().expect("passed")
            + counts["unreviewed"].as_i64().expect("unreviewed")
            + counts["flagged"].as_i64().expect("flagged");
        assert_eq!(total, 1, "one observation total: {counts}");
        assert_eq!(
            counts["flagged"], 0,
            "a ready outcome carries no flagged rows"
        );
        assert_eq!(ready["current"], Value::Null);

        // The old generation is now superseded — pre-check path.
        let superseded = success(acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 1 }),
        ));
        assert_eq!(superseded["outcome"], "superseded");
        assert_eq!(superseded["severityCounts"], Value::Null);
        assert_eq!(superseded["current"]["status"], "ready_to_commit");
        assert_eq!(superseded["current"]["revision"], 2);
        assert!(superseded["current"]["manifestHash"].is_string());
    }

    /// ADR 0102 dec. 12: `validate_kpi_ingest` returns a BOUNDED summary — no
    /// inline `manifest` key at all (not even `null` under a different name);
    /// the full manifest is read via `get_kpi_ingest_context
    /// section:"manifest"` (`validate_ready_failed_repair_and_superseded`
    /// exercises that paged read end to end).
    #[test]
    fn validate_returns_summary_not_manifest() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(&run_id, vec![mapped_observation(true)]),
        ));
        let payload = success(acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 1 }),
        ));
        let mut keys: Vec<&str> = payload
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "current",
                "manifestHash",
                "outcome",
                "revision",
                "severityCounts"
            ],
            "no inline manifest key at all — the bounded summary shape"
        );
    }

    #[test]
    fn validate_numeric_and_not_found_refusals() {
        let state = test_state();
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "validate_kpi_ingest",
                &json!({ "runId": "kpiing_ffffffffffffffffffffffffffffffff", "revision": 1 })
            )),
            CommandErrorCode::NotFound
        );
        let run_id = start_characteristic_run(&state);
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "validate_kpi_ingest",
                &json!({ "runId": run_id, "revision": 0 })
            )),
            CommandErrorCode::InvalidInput
        );
    }

    /// The seam (sol R1 B-1): after ANY validator error the re-read tuple is
    /// the signal — two distinct original errors, both branches.
    #[test]
    fn classify_validate_error_reads_the_tuple_not_the_error_code() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &stage_args(&run_id, vec![mapped_observation(true)]),
        ));

        // Tuple unchanged (staged@1): both originals PROPAGATE verbatim.
        for original in [
            CommandError::new(CommandErrorCode::Conflict, "invalid staging revision"),
            CommandError::new(CommandErrorCode::Internal, "sealed manifest rejected"),
        ] {
            let code = original.code;
            let propagated = classify_validate_error(&state, &run_id, 1, original)
                .expect_err("unchanged tuple must propagate");
            assert_eq!(propagated.code, code);
        }

        // Tuple moved: BOTH originals become superseded, regardless of code.
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET manifest_revision = 2 WHERE id = ?1",
                    [&run_id],
                )
                .expect("move");
        }
        for original in [
            CommandError::new(CommandErrorCode::Conflict, "invalid staging revision"),
            CommandError::new(CommandErrorCode::Internal, "sealed manifest rejected"),
        ] {
            let result = classify_validate_error(&state, &run_id, 1, original)
                .expect("moved tuple is superseded");
            assert_eq!(result.outcome, "superseded");
            assert_eq!(result.current.as_ref().expect("current").revision, 2);
        }
    }

    /// Integration proof that the handler routes a real validator error
    /// through the seam: an (artificially) staged run with EMPTY staging
    /// errors inside the validator while the tuple stays put — the original
    /// error must propagate, not masquerade as superseded.
    #[test]
    fn a_real_validator_error_with_an_unmoved_tuple_propagates() {
        let state = test_state();
        let run_id = start_characteristic_run(&state);
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET status = 'staged', manifest_revision = 1 \
                     WHERE id = ?1",
                    [&run_id],
                )
                .expect("force staged without observations");
        }
        let outcome = acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 1 }),
        );
        match outcome {
            ToolOutcome::Failure(error) => {
                assert_ne!(error.code, CommandErrorCode::NotFound);
            }
            ToolOutcome::Success(value) => {
                panic!("empty staging must error, got {value}")
            }
        }
    }

    // ------------------------------------------------------------------
    // commit_kpi_ingest
    // ------------------------------------------------------------------

    fn drive_to_ready_over_mcp(state: &AppState) -> (String, String, i64) {
        let run_id = start_characteristic_run(state);
        success(acquisition_call(
            state,
            "stage_kpi_observations",
            &json!({
                "runId": run_id,
                "observations": [mapped_observation(true)],
                "missingReasons": {},
                "execution": { "client": "a", "tokensIn": 5 }
            }),
        ));
        let ready = success(acquisition_call(
            state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 1 }),
        ));
        assert_eq!(ready["outcome"], "ready");
        let hash = ready["manifestHash"].as_str().expect("hash").to_owned();
        (run_id, hash, 1)
    }

    #[test]
    fn commit_receipt_golden_shape_replay_verbatim_and_execution_union() {
        let state = test_state();
        let (run_id, hash, revision) = drive_to_ready_over_mcp(&state);
        let receipt = success(acquisition_call(
            &state,
            "commit_kpi_ingest",
            &json!({
                "runId": run_id,
                "manifestHash": hash,
                "revision": revision,
                "execution": { "client": "a", "costUsd": 0.5 }
            }),
        ));
        assert_eq!(receipt["terminalStatus"], "complete");
        assert!(receipt["counts"]["acceptedCount"].as_i64().expect("count") > 0);

        let pretty = serde_json::to_string_pretty(&receipt).expect("serializable");
        let redacted = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z")
            .expect("regex")
            .replace_all(&pretty, "[timestamp]")
            .into_owned();
        let redacted = regex::Regex::new(r"[a-z]+_[0-9a-f]{16,}")
            .expect("regex")
            .replace_all(&redacted, "[id]")
            .into_owned();
        let redacted = regex::Regex::new(r"[0-9a-f]{64}")
            .expect("regex")
            .replace_all(&redacted, "[hash]")
            .into_owned();
        // Fact ids end in an 8-hex time-derived suffix (`ulid_suffix`).
        let redacted = regex::Regex::new(r#"_[0-9a-f]{8}""#)
            .expect("regex")
            .replace_all(&redacted, "_[uid]\"")
            .into_owned();
        insta::assert_snapshot!("commit_receipt_wire_shape", redacted);

        // Replay with DIFFERENT execution → byte-identical receipt, no merge.
        let replay = success(acquisition_call(
            &state,
            "commit_kpi_ingest",
            &json!({
                "runId": run_id,
                "manifestHash": hash,
                "revision": revision,
                "execution": { "client": "b", "costUsd": 9.9 }
            }),
        ));
        assert_eq!(replay, receipt, "idempotent replay is verbatim");

        // Execution union across stage → commit survives on the run.
        let status = success(acquisition_call(
            &state,
            "get_kpi_ingest_status",
            &json!({ "runId": run_id }),
        ));
        assert_eq!(status["execution"]["client"], "a");
        assert_eq!(status["execution"]["tokensIn"], 5);
        assert_eq!(status["execution"]["costUsd"], 0.5);
    }

    /// ADR 0102 dec. 12: `commit_kpi_ingest` returns a BOUNDED summary — no
    /// inline `outcomes` array (a 1000-row ledger, `AGGREGATE_OBSERVATIONS_MAX`,
    /// cannot ride this response), `excludedCount` moves under `counts`. The
    /// full receipt is read via `get_kpi_ingest_context section:"receipt"`
    /// (`receipt_section_serves_the_full_excluded_ledger`, kpi_ingest_context.rs).
    #[test]
    fn commit_returns_bounded_summary() {
        let state = test_state();
        let (run_id, hash, revision) = drive_to_ready_over_mcp(&state);
        let receipt = success(acquisition_call(
            &state,
            "commit_kpi_ingest",
            &json!({ "runId": run_id, "manifestHash": hash, "revision": revision }),
        ));
        let mut keys: Vec<&str> = receipt
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec![
                "counts",
                "manifestHash",
                "manifestRevision",
                "runId",
                "terminalStatus"
            ],
            "no inline outcomes ledger at all — the bounded summary shape"
        );
        let mut count_keys: Vec<&str> = receipt["counts"]
            .as_object()
            .expect("counts object")
            .keys()
            .map(String::as_str)
            .collect();
        count_keys.sort_unstable();
        assert_eq!(count_keys, vec!["acceptedCount", "excludedCount"]);
    }

    #[test]
    fn commit_refusals_are_typed() {
        let state = test_state();
        let (run_id, hash, revision) = drive_to_ready_over_mcp(&state);

        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "commit_kpi_ingest",
                &json!({ "runId": run_id, "manifestHash": "XYZ", "revision": revision })
            )),
            CommandErrorCode::InvalidInput,
            "malformed hash grammar"
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "commit_kpi_ingest",
                &json!({ "runId": run_id, "manifestHash": hash, "revision": 0 })
            )),
            CommandErrorCode::InvalidInput,
            "revision below 1"
        );
        let wrong_hash = "0".repeat(64);
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "commit_kpi_ingest",
                &json!({ "runId": run_id, "manifestHash": wrong_hash, "revision": revision })
            )),
            CommandErrorCode::Conflict,
            "stale tuple"
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "commit_kpi_ingest",
                &json!({
                    "runId": "kpiing_ffffffffffffffffffffffffffffffff",
                    "manifestHash": hash,
                    "revision": revision
                })
            )),
            CommandErrorCode::NotFound,
            "unknown run"
        );
    }

    // ------------------------------------------------------------------
    // stage_kpi_observations — chunked drafts (epic #399 S6)
    // ------------------------------------------------------------------

    /// One clean company-scoped `origin=agent` definition, mirroring
    /// `propose_args`'s proven shape — only the key/label vary per index.
    fn propose_test_definition(state: &AppState, run_id: &str, i: usize) {
        let payload = success(acquisition_call(
            state,
            "propose_kpi_definition",
            &json!({
                "runId": run_id,
                "metricKey": format!("acq_test_metric_{i:03}"),
                "label": format!("Acquisition Test Metric {i:03}"),
                "statementGroup": "other"
            }),
        ));
        assert_eq!(
            payload["created"], true,
            "definition {i} must be freshly minted"
        );
    }

    /// One observation targeting `acq_test_metric_{i:03}`, mirroring
    /// `mapped_observation`'s currency/unitScale/measureWindow/attribution
    /// verbatim — only rawLabel/rawValue/metricKeyCandidate/citation.page
    /// vary per index, so no coherence diagnostic beyond the proven single-
    /// observation baseline can trip.
    fn observation_for(i: usize) -> Value {
        json!({
            "rawLabel": format!("acquisition test metric {i:03}"),
            "rawValue": (1000 + i).to_string(),
            "normalizedValue": (1000 + i).to_string(),
            "currency": "PLN",
            "unitScale": "ones",
            "measureWindow": "flow",
            "attribution": "total",
            "metricKeyCandidate": format!("acq_test_metric_{i:03}"),
            "mappingStatus": "mapped",
            "citation": { "page": 1 + (i % 50), "quote": format!("metric {i:03} quote") }
        })
    }

    /// The epic's frozen CI acceptance proof: a run with 170 disclosed
    /// numbers — beyond the single-call 100-observation cap — driven
    /// draft-open → append×4 → finalize → validate → commit against real
    /// (in-memory) storage, proving the chunked->100 flow end to end.
    #[test]
    fn finalize_170_observations_stage_validate_commit() {
        const TOTAL: usize = 170;
        let state = test_state();
        let run_id = start_characteristic_run(&state);

        for i in 0..TOTAL {
            propose_test_definition(&state, &run_id, i);
        }

        let open = success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &json!({
                "runId": run_id,
                "draft": { "open": true, "expectedObservations": TOTAL }
            }),
        ));
        assert_eq!(open["status"], "draft_open");
        let draft_id = open["draftId"].as_str().expect("draftId").to_owned();

        let observations: Vec<Value> = (0..TOTAL).map(observation_for).collect();
        for (chunk_index, chunk) in observations.chunks(50).enumerate() {
            let appended = success(acquisition_call(
                &state,
                "stage_kpi_observations",
                &json!({
                    "runId": run_id,
                    "draft": { "draftId": draft_id, "chunkIndex": chunk_index as i64 },
                    "observations": chunk
                }),
            ));
            assert_eq!(appended["status"], "draft_appended");
            assert_eq!(appended["chunkIndex"], chunk_index as i64);
        }

        let finalized = success(acquisition_call(
            &state,
            "stage_kpi_observations",
            &json!({
                "runId": run_id,
                "draft": { "draftId": draft_id, "final": true },
                "missingReasons": {}
            }),
        ));
        assert_eq!(finalized["status"], "staged");
        assert_eq!(finalized["observationCount"], TOTAL);
        let revision = finalized["revision"].as_i64().expect("revision");

        let ready = success(acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": revision }),
        ));
        assert_eq!(ready["outcome"], "ready", "validation manifest: {ready:#}");
        let hash = ready["manifestHash"].as_str().expect("hash").to_owned();

        let receipt = success(acquisition_call(
            &state,
            "commit_kpi_ingest",
            &json!({
                "runId": run_id,
                "manifestHash": hash,
                "revision": revision,
                "execution": { "client": "acquisition-agent" }
            }),
        ));
        assert_eq!(receipt["terminalStatus"], "complete");
        assert!(
            receipt["counts"]["acceptedCount"].as_i64().expect("count") >= TOTAL as i64,
            "receipt: {receipt:#}"
        );
    }

    /// The outcomes truth table (sol R3): over `is_divergent × detail_present
    /// × factId_present`, EXACTLY two rows are legal; every other row — and
    /// every malformed/unknown/wrong-version shape — is `internal`.
    #[test]
    fn receipt_outcomes_truth_table_and_shape_guards() {
        fn receipt_with(outcomes_json: &str, schema_version: i64) -> CommitReceipt {
            CommitReceipt {
                id: "receipt1".to_owned(),
                run_id: "kpiing_00000000000000000000000000000001".to_owned(),
                manifest_hash: "0".repeat(64),
                manifest_revision: 1,
                terminal_status: "complete".to_owned(),
                period_id: None,
                accepted_count: 1,
                outcomes_schema_version: schema_version,
                outcomes_json: outcomes_json.to_owned(),
                committed_at: "2026-01-01T00:00:00Z".to_owned(),
            }
        }
        fn outcome_row(outcome: &str, fact_id: bool, detail: bool) -> String {
            let mut row = json!({
                "observationId": "kpiobs_1",
                "revision": 1,
                "ordinal": 0,
                "metricKey": "revenue",
                "outcome": outcome,
                // The durable writer always serializes the key — explicitly
                // null when absent (luna P1).
                "factId": if fact_id { json!("fact_1") } else { Value::Null },
            });
            if detail {
                row["detail"] = json!({ "existingFactId": "fact_2" });
            }
            json!([row]).to_string()
        }

        // The two legal rows.
        assert!(receipt_dto(receipt_with(&outcome_row("divergent", false, true), 1)).is_ok());
        assert!(receipt_dto(receipt_with(&outcome_row("created", true, false), 1)).is_ok());

        // Every other truth-table row is internal.
        let illegal = [
            ("divergent", true, true),
            ("divergent", true, false),
            ("divergent", false, false),
            ("created", true, true),
            ("created", false, true),
            ("created", false, false),
            ("reobserved", false, false),
            ("upgraded", true, true),
        ];
        for (outcome, fact_id, detail) in illegal {
            let error = receipt_dto(receipt_with(&outcome_row(outcome, fact_id, detail), 1))
                .expect_err("illegal combination");
            assert_eq!(
                error.code,
                CommandErrorCode::Internal,
                "{outcome}/factId={fact_id}/detail={detail}"
            );
        }

        // Shape guards: corrupt JSON, unknown outcome, unknown field, wrong
        // top level, unsupported schema version.
        let unknown_outcome = outcome_row("exploded", true, false);
        for (name, json_text) in [
            ("corrupt", "not json"),
            ("unknown outcome", unknown_outcome.as_str()),
            (
                "unknown field",
                r#"[{"observationId":"o","revision":1,"ordinal":0,"metricKey":"m","outcome":"created","factId":"f","extra":1}]"#,
            ),
            ("wrong top level", r#"{"observationId":"o"}"#),
        ] {
            let error = receipt_dto(receipt_with(json_text, 1)).expect_err(name);
            assert_eq!(error.code, CommandErrorCode::Internal, "{name}");
        }
        // v2 is a legal schema version now (ADR 0102 dec. 3) — a plain
        // `created` row still round-trips under it unchanged.
        assert!(receipt_dto(receipt_with(&outcome_row("created", true, false), 2)).is_ok());
        let error = receipt_dto(receipt_with(&outcome_row("created", true, false), 3))
            .expect_err("schema version 3 is unsupported");
        assert_eq!(error.code, CommandErrorCode::Internal);

        // A MISSING factId key (vs the writer's explicit null) is shape drift.
        let missing_key = r#"[{"observationId":"o","revision":1,"ordinal":0,"metricKey":"m","outcome":"divergent","detail":{"existingFactId":"f"}}]"#;
        let error = receipt_dto(receipt_with(missing_key, 1)).expect_err("missing factId key");
        assert_eq!(error.code, CommandErrorCode::Internal);
    }

    /// ADR 0102 dec. 3: `excluded` is legal ONLY under v2, no `factId`,
    /// carrying `{label, reason}` — and rolls up into
    /// [`CommitReceiptDto::excludedCount`]/`excluded`.
    #[test]
    fn receipt_v2_outcome_excluded_and_ledger() {
        fn receipt_with(outcomes_json: &str, schema_version: i64) -> CommitReceipt {
            CommitReceipt {
                id: "receipt1".to_owned(),
                run_id: "kpiing_00000000000000000000000000000001".to_owned(),
                manifest_hash: "0".repeat(64),
                manifest_revision: 1,
                terminal_status: "complete".to_owned(),
                period_id: None,
                accepted_count: 0,
                outcomes_schema_version: schema_version,
                outcomes_json: outcomes_json.to_owned(),
                committed_at: "2026-01-01T00:00:00Z".to_owned(),
            }
        }
        let excluded_row = json!([{
            "observationId": "kpiobs_1",
            "revision": 1,
            "ordinal": 0,
            "metricKey": "",
            "factId": Value::Null,
            "outcome": "excluded",
            "detail": { "label": "Liczba pracowników", "reason": "not a KPI" },
        }])
        .to_string();

        // The bounded wire DTO (ADR 0102 dec. 12) carries only the count — the
        // per-outcome `{label, reason}` ledger is internal-only here, exposed
        // over the wire via `get_kpi_ingest_context section:"receipt"`
        // (`receipt_section_serves_the_full_excluded_ledger`,
        // kpi_ingest_context.rs), which reads the SAME stored `outcomes_json`
        // this function validates.
        let dto =
            receipt_dto(receipt_with(&excluded_row, 2)).expect("v2 excluded outcome is legal");
        assert_eq!(dto.counts.excluded_count, 1);

        // The same shape under v1 is illegal — that outcome member did not
        // exist under v1 (the invariant is extended, never loosened).
        let error = receipt_dto(receipt_with(&excluded_row, 1)).expect_err("excluded is v2-only");
        assert_eq!(error.code, CommandErrorCode::Internal);
    }

    /// ADR 0102 dec. 3: the reader accepts both v1 and v2 stored receipts —
    /// old rows never get rewritten, new commits always write v2.
    #[test]
    fn receipt_reader_accepts_v1_and_v2() {
        let created_row = json!([{
            "observationId": "kpiobs_1",
            "revision": 1,
            "ordinal": 0,
            "metricKey": "revenue",
            "factId": "fact_1",
            "outcome": "created",
        }])
        .to_string();
        for version in [1, 2] {
            let receipt = CommitReceipt {
                id: "receipt1".to_owned(),
                run_id: "kpiing_00000000000000000000000000000001".to_owned(),
                manifest_hash: "0".repeat(64),
                manifest_revision: 1,
                terminal_status: "complete".to_owned(),
                period_id: None,
                accepted_count: 1,
                outcomes_schema_version: version,
                outcomes_json: created_row.clone(),
                committed_at: "2026-01-01T00:00:00Z".to_owned(),
            };
            let dto = receipt_dto(receipt).unwrap_or_else(|e| panic!("v{version}: {e:?}"));
            assert_eq!(dto.counts.accepted_count, 1);
            assert_eq!(dto.counts.excluded_count, 0);
        }
    }
}
