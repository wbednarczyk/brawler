//! Acquisition-workflow submission tools (#386, ADR 0099 dec. 1): the last
//! three of the nine — `stage_kpi_observations` (complete revision snapshot +
//! required `missingReasons` written in the SAME staging transaction),
//! `validate_kpi_ingest` (synchronous; the full manifest returns in the
//! response — a `failed` manifest IS the typed repair report; a raced loser
//! gets a typed `superseded` result carrying the current tuple) and
//! `commit_kpi_ingest` (synchronous atomic commit; idempotent replay returns
//! the stored receipt verbatim).
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
    AppState, CommitReceipt, KpiIngestRun, KpiIngestRunState, NewStagedObservation,
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
}

impl MappingStatusInput {
    fn as_str(self) -> &'static str {
        match self {
            MappingStatusInput::Unmapped => "unmapped",
            MappingStatusInput::Mapped => "mapped",
            MappingStatusInput::NoDefinition => "no_definition",
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
    #[serde(default)]
    pub citation: Option<CitationInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StageKpiObservationsInput {
    pub run_id: String,
    pub observations: Vec<ObservationInput>,
    /// REQUIRED: `{}` is the explicit "no declared omissions"/clear — there is
    /// no destructive default (contracts.md tool 5).
    pub missing_reasons: BTreeMap<String, String>,
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

// ============================================================================
// Wire DTOs (MCP-only — no TS consumer, no ts_rs)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageResultDto {
    pub run_id: String,
    pub revision: i64,
    pub observation_count: usize,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentTupleDto {
    pub status: String,
    pub revision: i64,
    pub manifest_hash: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateResultDto {
    pub outcome: &'static str,
    pub revision: i64,
    pub manifest_hash: Option<String>,
    pub manifest: Option<Value>,
    pub current: Option<CurrentTupleDto>,
}

/// One commit outcome, deserialized TYPED from the stored receipt — a closed
/// vocabulary plus the frozen conditional shape: `divergent` ⟺ (`detail`
/// present ∧ `factId` null); every other outcome ⟺ (`detail` absent ∧
/// `factId` non-null). Violations are `internal` — the stored receipt is the
/// durable contract and a drifted shape must not silently reach the wire.
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
    pub detail: Option<DivergentDetailDto>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommitOutcomeKind {
    Created,
    Reobserved,
    Upgraded,
    Divergent,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DivergentDetailDto {
    pub existing_fact_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitReceiptDto {
    pub run_id: String,
    pub terminal_status: String,
    pub period_id: Option<String>,
    pub accepted_count: i64,
    pub outcomes_schema_version: i64,
    pub outcomes: Vec<CommitOutcomeDto>,
    pub manifest_hash: String,
    pub manifest_revision: i64,
    pub committed_at: String,
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

fn stage_kpi_observations(
    state: &AppState,
    scope: McpScope,
    input: StageKpiObservationsInput,
) -> Result<StageResultDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    if input.observations.is_empty() {
        return Err(invalid(
            "observations is the complete revision snapshot and must carry at least one entry",
        ));
    }
    if input.observations.len() > OBSERVATIONS_MAX {
        return Err(CommandError::new(
            CommandErrorCode::ResponseBudgetExceeded,
            format!(
                "observations carries {} entries — the budget is {OBSERVATIONS_MAX}",
                input.observations.len()
            ),
        ));
    }
    for observation in &input.observations {
        validate_observation(observation)?;
    }
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
        revision,
        observation_count: inserted.len(),
        status: "staged",
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
        manifest: None,
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
            let manifest = serde_json::to_value(&outcome.manifest)
                .map_err(|error| internal(format!("manifest serialization failed: {error}")))?;
            Ok(ValidateResultDto {
                outcome: if outcome.outcome == "ready" {
                    "ready"
                } else {
                    "failed"
                },
                revision: input.revision,
                manifest_hash: Some(outcome.manifest_hash),
                manifest: Some(manifest),
                current: None,
            })
        }
        Err(original) => classify_validate_error(state, &input.run_id, input.revision, original),
    }
}

// ============================================================================
// commit_kpi_ingest
// ============================================================================

fn receipt_dto(receipt: CommitReceipt) -> Result<CommitReceiptDto, CommandError> {
    if receipt.outcomes_schema_version != 1 {
        return Err(internal(format!(
            "stored receipt outcomesSchemaVersion {} is unsupported by this build",
            receipt.outcomes_schema_version
        )));
    }
    // The durable writer serializes `factId` explicitly (`null` for
    // divergent) — a MISSING key is shape drift, not a legal null (luna P1).
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
        let divergent = outcome.outcome == CommitOutcomeKind::Divergent;
        let legal = if divergent {
            outcome.detail.is_some() && outcome.fact_id.is_none()
        } else {
            outcome.detail.is_none() && outcome.fact_id.is_some()
        };
        if !legal {
            return Err(internal(format!(
                "stored receipt outcome for {} violates the divergent/factId/detail invariant",
                outcome.observation_id
            )));
        }
    }
    Ok(CommitReceiptDto {
        run_id: receipt.run_id,
        terminal_status: receipt.terminal_status,
        period_id: receipt.period_id,
        accepted_count: receipt.accepted_count,
        outcomes_schema_version: receipt.outcomes_schema_version,
        outcomes,
        manifest_hash: receipt.manifest_hash,
        manifest_revision: receipt.manifest_revision,
        committed_at: receipt.committed_at,
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
        let manifest_text = failed["manifest"].to_string();
        assert!(
            manifest_text.contains("citation.missing"),
            "the repair report names the diagnostic: {manifest_text}"
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
        assert!(ready["manifest"]["observations"].is_array());
        assert_eq!(ready["current"], Value::Null);

        // The old generation is now superseded — pre-check path.
        let superseded = success(acquisition_call(
            &state,
            "validate_kpi_ingest",
            &json!({ "runId": run_id, "revision": 1 }),
        ));
        assert_eq!(superseded["outcome"], "superseded");
        assert_eq!(superseded["manifest"], Value::Null);
        assert_eq!(superseded["current"]["status"], "ready_to_commit");
        assert_eq!(superseded["current"]["revision"], 2);
        assert!(superseded["current"]["manifestHash"].is_string());
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
        assert!(receipt["acceptedCount"].as_i64().expect("count") > 0);

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
        let error = receipt_dto(receipt_with(&outcome_row("created", true, false), 2))
            .expect_err("schema version 2");
        assert_eq!(error.code, CommandErrorCode::Internal);

        // A MISSING factId key (vs the writer's explicit null) is shape drift.
        let missing_key = r#"[{"observationId":"o","revision":1,"ordinal":0,"metricKey":"m","outcome":"divergent","detail":{"existingFactId":"f"}}]"#;
        let error = receipt_dto(receipt_with(missing_key, 1)).expect_err("missing factId key");
        assert_eq!(error.code, CommandErrorCode::Internal);
    }
}
