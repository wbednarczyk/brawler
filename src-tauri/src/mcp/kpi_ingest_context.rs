//! Acquisition-workflow context read model (#385, ADR 0099): the two pure
//! reads of the nine-tool surface — `get_kpi_ingest_context` (everything one
//! report's extraction needs, within hard response budgets: run state,
//! document metadata, the hash-guarded derived-period hint, the expected+
//! minted definition catalog, validator-equivalent plausibility evidence,
//! profile doctrine and the paginated repair manifest) and
//! `get_kpi_ingest_document` (chunked bytes from the run's content-addressed
//! blob, verified against the frozen `source_content_hash` — the only portable
//! byte channel).
//!
//! Budgets are runtime mechanisms (ADR 0099 dec. 7): sections are capped and
//! keyset-paginated, output strings are byte-bounded with `…` truncation, the
//! default call dynamically shrinks its pageable sections to stay ≤256 KiB
//! (overflow always leaves a cursor, never a dead end), and unsatisfiable
//! requests refuse with `response_budget_exceeded`.

use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine as _;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::kpi_ingest::{
    get_existing_run, is_content_hash, period_dto, reject_control_chars, run_status_dto,
    RunStatusDto, SNAPSHOT_DIR,
};
use super::tools::{run, ToolCallError, ToolOutcome};
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::fundamentals::kpi_manifest::{abstention_reason_str, decimal_str};
use crate::fundamentals::metrics::measure_window_for;
use crate::fundamentals::validation::{
    history_median, is_split_sensitive_metric, AbstentionReason,
};
use crate::storage::{
    AppState, HistoryPoint, HistorySlotKey, KpiDefinition, KpiIngestRun, KpiIngestRunState,
    ListKpiDefinitionsInput,
};

// ============================================================================
// Budgets (contracts.md § Budgets — frozen numbers)
// ============================================================================

const CATALOG_PAGE_MAX: usize = 64;
const PLAUSIBILITY_PAGE_MAX: usize = 64;
const MANIFEST_PAGE_MAX: usize = 50;
const RECENT_POINTS_MAX: usize = 8;
/// Every context response is ≤256 KiB; a document chunk is ≤256 KiB of RAW
/// bytes (its base64 envelope may exceed this — the chunk cap is the budget).
const RESPONSE_BUDGET_BYTES: usize = 262_144;
const DOCUMENT_CHUNK_MAX: u64 = 262_144;

// Output-string byte caps (per-field `…` truncation, contracts.md § Budgets).
const LABEL_MAX: usize = 256;
const UNIT_MAX: usize = 64;
const STATEMENT_GROUP_MAX: usize = 64;
const PROFILE_RULE_MAX: usize = 512;
const ABSTENTION_REASON_MAX: usize = 256;
const URL_MAX: usize = 512;
const TITLE_MAX: usize = 256;
const CONTENT_TYPE_MAX: usize = 128;
const LOCAL_PATH_MAX: usize = 512;

// ============================================================================
// Inputs
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSection {
    Catalog,
    Plausibility,
    Manifest,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GetKpiIngestContextInput {
    pub run_id: String,
    /// Omit for the full default context; name a section to paginate it.
    #[serde(default)]
    pub section: Option<ContextSection>,
    /// Continuation cursor from a previous truncated response (section calls
    /// only).
    #[serde(default)]
    pub cursor: Option<String>,
    /// Page size for a section call (≤ the section's cap; default = the cap).
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GetKpiIngestDocumentInput {
    pub run_id: String,
    pub offset: u64,
    #[schemars(range(min = 1, max = 262_144))]
    pub length: u64,
}

// ============================================================================
// Wire DTOs (MCP-only — no TS consumer, no ts_rs; contracts.md § KPI acquisition workflow tools)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentMetaDto {
    pub url: String,
    pub title: Option<String>,
    pub content_type: Option<String>,
    /// Size of the run's PINNED BLOB (never the mutable document row's
    /// `byte_size` — recapture must not change what this run describes).
    pub byte_size: Option<i64>,
    /// Local-client convenience only, never the delivery contract.
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedPeriodDto {
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
}

/// Two-tier wire shape (ADR 0101 dec. 7/9): `Full` is today's shape verbatim
/// (byte-identical) for resolved-expected and company-agent rows; `Compact`
/// carries only `{metricKey, label}` for the remaining ~373 canonical rows —
/// enough for an agent's reuse-or-propose decision without the ≈85 KiB a full
/// catalog would cost (server-side propose validation is the real guard, not
/// what the agent can see). `#[serde(untagged)]` keeps `Full` wire-identical
/// to the pre-widening struct; `definitionId` is kept internally on `Compact`
/// (never serialized) for stable keyset pagination.
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum CatalogEntryDto {
    #[serde(rename_all = "camelCase")]
    Full {
        definition_id: String,
        metric_key: String,
        label: String,
        unit: Option<String>,
        statement_group: String,
        value_kind: String,
        origin: String,
    },
    #[serde(rename_all = "camelCase")]
    Compact {
        #[serde(skip)]
        definition_id: String,
        metric_key: String,
        label: String,
    },
}

impl CatalogEntryDto {
    fn metric_key(&self) -> &str {
        match self {
            Self::Full { metric_key, .. } | Self::Compact { metric_key, .. } => metric_key,
        }
    }

    fn definition_id(&self) -> &str {
        match self {
            Self::Full { definition_id, .. } | Self::Compact { definition_id, .. } => definition_id,
        }
    }

    /// Sort/pagination tier: `Full` (0) always precedes `Compact` (1) so the
    /// default call's truncated first page is dominated by the entries an
    /// agent needs immediately (ADR 0101 dec. 7).
    fn tier(&self) -> u8 {
        match self {
            Self::Full { .. } => 0,
            Self::Compact { .. } => 1,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotDto {
    pub definition_id: String,
    pub scope: String,
    pub attribution: String,
    pub measure_window: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentPointDto {
    pub fiscal_year: i64,
    pub period_type: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlausibilityEntryDto {
    pub metric_key: String,
    pub slot: SlotDto,
    /// `observed` = this exact slot exists in the fact store; `candidate` =
    /// the recommended default slot a staged observation would land in
    /// (contracts.md § KPI acquisition workflow tools, dated addition #385) — a recommendation, not
    /// observed validator evidence.
    pub slot_origin: &'static str,
    /// `history_median` over the exact vector the validator would read —
    /// `null` under `thin_history` (dated `|null` clarification, #385).
    pub median: Option<String>,
    pub non_zero_count: usize,
    pub abstention_reason: Option<String>,
    pub recent_points: Vec<RecentPointDto>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TruncatedDto {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plausibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextDto {
    pub run: RunStatusDto,
    pub document: DocumentMetaDto,
    pub derived_period: Option<DerivedPeriodDto>,
    pub catalog: Vec<CatalogEntryDto>,
    /// Metric keys present in `catalog` (this page) that are not members of
    /// the run's expected set — ADR 0101 dec. 8: explicit, so their absence
    /// from `plausibility` is never misread as "no history exists" for a
    /// metric nobody asked this run to observe. Cheap: a set-difference
    /// against `run.expectedKpis.keys`, zero additional queries.
    pub not_requested: Vec<String>,
    pub plausibility: Vec<PlausibilityEntryDto>,
    pub profile_rules: Vec<String>,
    pub manifest_available: bool,
    pub truncated: TruncatedDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionDto {
    pub run_id: String,
    pub section: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog: Option<Vec<CatalogEntryDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plausibility: Option<Vec<PlausibilityEntryDto>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub manifest: Option<Value>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentChunkDto {
    pub bytes_base64: String,
    pub offset: u64,
    pub length: u64,
    pub total_bytes: u64,
    /// The run's frozen `source_content_hash` the served bytes were verified
    /// against.
    pub sha256: String,
    pub eof: bool,
}

// ============================================================================
// Cursors — opaque base64url(JSON); `{}` = start-of-section sentinel
// (catalog/plausibility only)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CatalogCursor {
    t: u8,
    m: String,
    d: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlausibilityCursor {
    m: String,
    d: String,
    s: String,
    a: String,
    w: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct ManifestCursor {
    attempt_id: String,
    offset: usize,
}

/// A decoded keyset cursor: `Start` is the `{}` sentinel (emitted by the
/// default call's dynamic shrink when zero entries were retained; also
/// accepted from clients), `After` resumes strictly after the named keyset.
enum SectionCursor<T> {
    Start,
    After(T),
}

fn invalid_cursor() -> CommandError {
    CommandError::new(
        CommandErrorCode::InvalidInput,
        "cursor is not a cursor this tool issued",
    )
}

fn decode_cursor_value(cursor: &str) -> Result<Value, CommandError> {
    reject_control_chars("cursor", cursor)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor.as_bytes())
        .map_err(|_| invalid_cursor())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid_cursor())
}

/// No input length cap of our own: the 1 MiB transport bound is the limit — a
/// smaller cap could reject a cursor this tool itself emitted from a legally
/// long stored metric key (a dead end ADR 0099 forbids).
fn decode_section_cursor<T: serde::de::DeserializeOwned>(
    cursor: &str,
) -> Result<SectionCursor<T>, CommandError> {
    let value = decode_cursor_value(cursor)?;
    if value.as_object().is_some_and(|map| map.is_empty()) {
        return Ok(SectionCursor::Start);
    }
    serde_json::from_value(value)
        .map(SectionCursor::After)
        .map_err(|_| invalid_cursor())
}

fn encode_cursor<T: Serialize>(cursor: &T) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).expect("cursor serialization is total"))
}

fn start_sentinel_cursor() -> String {
    URL_SAFE_NO_PAD.encode(b"{}")
}

// ============================================================================
// Shared helpers
// ============================================================================

fn internal(message: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, message.into())
}

fn budget_refusal(message: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::ResponseBudgetExceeded, message.into())
}

/// Byte-bound an output string on a char boundary, marking truncation with a
/// trailing `…` (total stays ≤ `max`).
fn truncate_bytes(value: &str, max: usize) -> String {
    const MARKER: &str = "…";
    if value.len() <= max {
        return value.to_owned();
    }
    let mut end = max.saturating_sub(MARKER.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{MARKER}", &value[..end])
}

fn serialized_len<T: Serialize>(value: &T) -> Result<usize, CommandError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| internal(format!("response serialization failed: {error}")))
}

/// The run's expected metric keys: the creation-time stamp verbatim; a legacy
/// NULL row computes the validator's live fallback (`kpi_relevance` only) —
/// WITHOUT stamping, this is a pure read.
fn expected_keys(state: &AppState, run: &KpiIngestRun) -> Result<BTreeSet<String>, CommandError> {
    if let Some(stored) = run.expected_kpis_json.as_deref() {
        let parsed: Value = serde_json::from_str(stored)
            .map_err(|_| internal("stored expected_kpis_json is not valid JSON"))?;
        let Some(keys) = parsed.get("keys").and_then(Value::as_array) else {
            return Err(internal("stored expected_kpis_json has no keys array"));
        };
        return keys
            .iter()
            .map(|key| {
                key.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| internal("stored expected_kpis_json carries a non-string key"))
            })
            .collect();
    }
    state
        .financials()
        .expected_primary_metric_keys(&run.company_id)
        .map_err(CommandError::from)
        .map(Option::unwrap_or_default)
}

/// Expected keys resolved to their full catalog definitions. An expected key
/// that does not resolve — or resolves to a definition the visible catalog
/// does not carry — is a data error (`internal`), never a silent omission:
/// the missing entry is exactly the repair context the agent needs.
fn resolved_expected_definitions(
    state: &AppState,
    run: &KpiIngestRun,
    expected: &BTreeSet<String>,
    definitions: &[KpiDefinition],
) -> Result<ResolvedExpected, CommandError> {
    let by_id: HashMap<&str, &KpiDefinition> = definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition))
        .collect();
    let mut resolved = Vec::with_capacity(expected.len());
    for key in expected {
        let hit = state
            .kpi_extraction()
            .resolve_kpi_definition(&run.company_id, key)
            .map_err(CommandError::from)?
            .ok_or_else(|| {
                internal(format!(
                    "expected metric key {key} does not resolve to any KPI definition"
                ))
            })?;
        let definition = by_id.get(hit.definition_id.as_str()).ok_or_else(|| {
            internal(format!(
                "expected metric key {key} resolved to definition {} outside the visible catalog",
                hit.definition_id
            ))
        })?;
        resolved.push((key.clone(), (*definition).clone()));
    }
    Ok(resolved)
}

fn catalog_entry_full(definition: &KpiDefinition) -> CatalogEntryDto {
    CatalogEntryDto::Full {
        definition_id: definition.id.clone(),
        metric_key: definition.metric_key.clone(),
        label: truncate_bytes(&definition.label, LABEL_MAX),
        unit: definition
            .unit
            .as_deref()
            .map(|unit| truncate_bytes(unit, UNIT_MAX)),
        statement_group: truncate_bytes(&definition.statement_group, STATEMENT_GROUP_MAX),
        value_kind: definition.value_kind.clone(),
        origin: definition.origin.clone(),
    }
}

fn catalog_entry_compact(definition: &KpiDefinition) -> CatalogEntryDto {
    CatalogEntryDto::Compact {
        definition_id: definition.id.clone(),
        metric_key: definition.metric_key.clone(),
        label: truncate_bytes(&definition.label, LABEL_MAX),
    }
}

/// The full sorted catalog (ADR 0101 dec. 7): FULL metadata for the expected
/// keys plus the company's minted extras (`origin == "agent"` — ADR 0093
/// dec. 4 vocabulary), COMPACT `{metricKey, label}` for every other canonical
/// (shared, `company_id IS NULL`) definition — the full canon, not just what
/// this run happened to ask for. Deduped by definition id, sorted by
/// `(tier, metricKey, definitionId)`: tier keeps every Full entry ahead of
/// every Compact entry so the default call's truncated first page is never
/// dominated by canon the agent didn't ask about.
fn build_catalog(
    resolved_expected: &[(String, KpiDefinition)],
    definitions: &[KpiDefinition],
    company_id: &str,
) -> Vec<CatalogEntryDto> {
    let mut by_id: HashMap<String, CatalogEntryDto> = HashMap::new();
    for (_, definition) in resolved_expected {
        by_id
            .entry(definition.id.clone())
            .or_insert_with(|| catalog_entry_full(definition));
    }
    for definition in definitions {
        if definition.origin == "agent" && definition.company_id.as_deref() == Some(company_id) {
            by_id
                .entry(definition.id.clone())
                .or_insert_with(|| catalog_entry_full(definition));
        }
    }
    for definition in definitions {
        if definition.company_id.is_none() {
            by_id
                .entry(definition.id.clone())
                .or_insert_with(|| catalog_entry_compact(definition));
        }
    }
    let mut entries: Vec<CatalogEntryDto> = by_id.into_values().collect();
    entries.sort_by(|a, b| {
        (a.tier(), a.metric_key(), a.definition_id()).cmp(&(
            b.tier(),
            b.metric_key(),
            b.definition_id(),
        ))
    });
    entries
}

/// The candidate default slot's measure window (ADR 0100 decision 6): the
/// definition's `period_nature` decides the axis -- `instant` is always
/// `point_in_time`, regardless of `value_kind` -- and the PROFILE picks the
/// duration window: interim publications are cumulative (ADR 0098 dec. 3),
/// everything else stages plain `flow`. TTM eligibility (ratio/percentage)
/// plays no role here, a distinct axis (`is_ttm_eligible`): a ratio is still
/// duration-reported and gets a flow/cumulative candidate slot.
fn candidate_window(profile_version: &str, period_nature: &str) -> &'static str {
    measure_window_for(Some(period_nature), Some(profile_version))
}

fn abstention_for(metric_key: &str, median: Option<&Decimal>) -> Option<&'static str> {
    if is_split_sensitive_metric(metric_key) {
        return Some(abstention_reason_str(AbstentionReason::SplitSensitive));
    }
    if median.is_none() {
        return Some(abstention_reason_str(AbstentionReason::ThinHistory));
    }
    None
}

/// Total chronology for `recentPoints`: effective period end (stored
/// `period_end_date`, else the calendar-year derivation the comparison module
/// uses), then fiscal year, then an intra-year period rank, then the period
/// label itself — deterministic under ties.
fn point_sort_key(point: &HistoryPoint) -> (String, i64, u8, String) {
    let effective_end = point.period_end.clone().unwrap_or_else(|| {
        let suffix = match point.period_type.as_str() {
            "Q1" => "-03-31",
            "Q2" | "H1" => "-06-30",
            "Q3" | "9M" => "-09-30",
            _ => "-12-31",
        };
        format!("{}{}", point.fiscal_year, suffix)
    });
    let rank = match point.period_type.as_str() {
        "Q1" => 1,
        "Q2" => 2,
        "H1" => 3,
        "Q3" => 4,
        "9M" => 5,
        "Q4" => 6,
        "H2" => 7,
        "FY" => 8,
        _ => 0,
    };
    (
        effective_end,
        point.fiscal_year,
        rank,
        point.period_type.clone(),
    )
}

fn plausibility_entry(
    metric_key: &str,
    slot: &HistorySlotKey,
    slot_origin: &'static str,
    points: &[HistoryPoint],
) -> PlausibilityEntryDto {
    let values: Vec<Decimal> = points.iter().map(|point| point.value).collect();
    let median = history_median(&values);
    let non_zero_count = values.iter().filter(|value| !value.is_zero()).count();
    let mut recent: Vec<&HistoryPoint> = points.iter().collect();
    recent.sort_by_key(|point| point_sort_key(point));
    let recent_points = recent
        .iter()
        .rev()
        .take(RECENT_POINTS_MAX)
        .rev()
        .map(|point| RecentPointDto {
            fiscal_year: point.fiscal_year,
            period_type: point.period_type.clone(),
            value: decimal_str(point.value),
        })
        .collect();
    PlausibilityEntryDto {
        metric_key: metric_key.to_owned(),
        slot: SlotDto {
            definition_id: slot.definition_id.clone(),
            scope: slot.statement_basis.clone(),
            attribution: slot.attribution_eff.clone(),
            measure_window: slot
                .measure_window_eff
                .clone()
                .unwrap_or_else(|| "flow".to_owned()),
        },
        slot_origin,
        median: median.map(decimal_str),
        non_zero_count,
        abstention_reason: abstention_for(metric_key, median.as_ref())
            .map(|reason| truncate_bytes(reason, ABSTENTION_REASON_MAX)),
        recent_points,
    }
}

/// Validator-equivalent plausibility evidence: per expected definition, every
/// slot the fact store actually realizes (`observed`, filtered to the run's
/// scope — both bases while the scope is unattached, which is evidence FOR the
/// scope choice) plus the recommended default `candidate` slot a staged
/// observation would land in. A candidate that matches an observed slot is the
/// observed entry; a pure candidate always has an empty history.
fn build_plausibility(
    state: &AppState,
    run: &KpiIngestRun,
    resolved_expected: &[(String, KpiDefinition)],
) -> Result<Vec<PlausibilityEntryDto>, CommandError> {
    let definition_ids: BTreeSet<String> = resolved_expected
        .iter()
        .map(|(_, definition)| definition.id.clone())
        .collect();
    if definition_ids.is_empty() {
        return Ok(Vec::new());
    }
    let (exclude_fy, exclude_pt) = match period_dto(state, run)? {
        Some(period) => (period.fiscal_year, period.period_type),
        // No period attached yet: exclude nothing (fy 0 matches no period).
        None => (0, String::new()),
    };
    let points_by_slot = state
        .financials()
        .slot_history_points(&run.company_id, &definition_ids, exclude_fy, &exclude_pt)
        .map_err(CommandError::from)?;
    let scopes: Vec<&str> = match run.scope.as_deref() {
        Some(scope) => vec![scope],
        None => vec!["standalone", "consolidated"],
    };

    let mut entries: Vec<PlausibilityEntryDto> = Vec::new();
    for (metric_key, definition) in resolved_expected {
        let mut seen_slots: BTreeSet<HistorySlotKey> = BTreeSet::new();
        for (slot, points) in &points_by_slot {
            if slot.definition_id != definition.id {
                continue;
            }
            if !scopes.contains(&slot.statement_basis.as_str()) {
                continue;
            }
            seen_slots.insert(slot.clone());
            entries.push(plausibility_entry(metric_key, slot, "observed", points));
        }
        for scope in &scopes {
            let candidate = HistorySlotKey {
                definition_id: definition.id.clone(),
                statement_basis: (*scope).to_owned(),
                attribution_eff: "total".to_owned(),
                measure_window_eff: Some(
                    candidate_window(&run.profile_version, &definition.period_nature).to_owned(),
                ),
            };
            if seen_slots.contains(&candidate) {
                continue;
            }
            entries.push(plausibility_entry(metric_key, &candidate, "candidate", &[]));
        }
    }
    entries.sort_by(|a, b| {
        (
            a.metric_key.as_str(),
            a.slot.definition_id.as_str(),
            a.slot.scope.as_str(),
            a.slot.attribution.as_str(),
            a.slot.measure_window.as_str(),
        )
            .cmp(&(
                b.metric_key.as_str(),
                b.slot.definition_id.as_str(),
                b.slot.scope.as_str(),
                b.slot.attribution.as_str(),
                b.slot.measure_window.as_str(),
            ))
    });
    Ok(entries)
}

/// The derived-period HINT: served only from the provenance-bound cache
/// (migration 0140) and only when the cached hash equals the run's frozen
/// source hash — both present (`None == None` never counts). Anything else is
/// `null`; the read path never derives (and never writes).
fn derived_period_hint(
    state: &AppState,
    run: &KpiIngestRun,
) -> Result<Option<DerivedPeriodDto>, CommandError> {
    let Some(run_hash) = run.source_content_hash.as_deref() else {
        return Ok(None);
    };
    let Some(cached) = state
        .financials()
        .cached_derived_period(&run.report_document_id)
        .map_err(CommandError::from)?
    else {
        return Ok(None);
    };
    if cached.content_hash.as_deref() != Some(run_hash) {
        return Ok(None);
    }
    if !cached.has_period {
        return Ok(None);
    }
    match (cached.fiscal_year, cached.period_type, cached.period_end) {
        (Some(fiscal_year), Some(period_type), Some(period_end)) => Ok(Some(DerivedPeriodDto {
            fiscal_year,
            period_type,
            period_end,
        })),
        _ => Ok(None),
    }
}

fn document_meta(state: &AppState, run: &KpiIngestRun) -> Result<DocumentMetaDto, CommandError> {
    let document =
        state
            .get_report_document(&run.report_document_id)
            .map_err(|error| match error {
                crate::storage::StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows) => {
                    internal("the run's report document row is missing")
                }
                other => CommandError::from(other),
            })?;
    let byte_size = run
        .source_content_hash
        .as_deref()
        .filter(|hash| is_content_hash(hash))
        .and_then(|hash| {
            std::fs::metadata(state.data_dir().join(SNAPSHOT_DIR).join(hash))
                .ok()
                .map(|meta| meta.len() as i64)
        });
    Ok(DocumentMetaDto {
        url: truncate_bytes(&document.url, URL_MAX),
        title: document
            .title
            .as_deref()
            .map(|title| truncate_bytes(title, TITLE_MAX)),
        content_type: document
            .content_type
            .as_deref()
            .map(|value| truncate_bytes(value, CONTENT_TYPE_MAX)),
        byte_size,
        local_path: document
            .local_path
            .as_deref()
            .map(|path| truncate_bytes(path, LOCAL_PATH_MAX)),
    })
}

fn profile_rules(run: &KpiIngestRun) -> Result<Vec<String>, CommandError> {
    Ok(
        crate::storage::kpi_ingest_profile_rules(&run.profile_version)?
            .iter()
            .map(|rule| truncate_bytes(rule, PROFILE_RULE_MAX))
            .collect(),
    )
}

// ============================================================================
// get_kpi_ingest_context
// ============================================================================

fn catalog_cursor_for(entry: &CatalogEntryDto) -> String {
    encode_cursor(&CatalogCursor {
        t: entry.tier(),
        m: entry.metric_key().to_owned(),
        d: entry.definition_id().to_owned(),
    })
}

fn plausibility_cursor_for(entry: &PlausibilityEntryDto) -> String {
    encode_cursor(&PlausibilityCursor {
        m: entry.metric_key.clone(),
        d: entry.slot.definition_id.clone(),
        s: entry.slot.scope.clone(),
        a: entry.slot.attribution.clone(),
        w: Some(entry.slot.measure_window.clone()),
    })
}

fn catalog_start_index(
    entries: &[CatalogEntryDto],
    cursor: &SectionCursor<CatalogCursor>,
) -> usize {
    match cursor {
        SectionCursor::Start => 0,
        SectionCursor::After(after) => entries.partition_point(|entry| {
            (entry.tier(), entry.metric_key(), entry.definition_id())
                <= (after.t, after.m.as_str(), after.d.as_str())
        }),
    }
}

fn plausibility_start_index(
    entries: &[PlausibilityEntryDto],
    cursor: &SectionCursor<PlausibilityCursor>,
) -> usize {
    match cursor {
        SectionCursor::Start => 0,
        SectionCursor::After(after) => {
            let after_window = after.w.clone().unwrap_or_default();
            entries.partition_point(|entry| {
                (
                    entry.metric_key.as_str(),
                    entry.slot.definition_id.as_str(),
                    entry.slot.scope.as_str(),
                    entry.slot.attribution.as_str(),
                    entry.slot.measure_window.as_str(),
                ) <= (
                    after.m.as_str(),
                    after.d.as_str(),
                    after.s.as_str(),
                    after.a.as_str(),
                    after_window.as_str(),
                )
            })
        }
    }
}

fn validate_section_limit(
    limit: Option<i64>,
    cap: usize,
    section: &str,
) -> Result<usize, CommandError> {
    let limit = limit.unwrap_or(cap as i64);
    if limit < 1 || limit > cap as i64 {
        return Err(budget_refusal(format!(
            "limit {limit} is outside 1..={cap} for the {section} section"
        )));
    }
    Ok(limit as usize)
}

/// Shrink a section page until the response fits the budget: drop trailing
/// entries (they reappear on the next page via the cursor); a single entry
/// that cannot fit alone is a typed refusal — the documented floor, reachable
/// only with pre-bound legacy identities.
fn fit_section_page<T: Clone>(
    mut page: Vec<T>,
    mut has_more: bool,
    cursor_for: impl Fn(&T) -> String,
    render: impl Fn(&[T], Option<String>) -> Result<usize, CommandError>,
    describe: impl Fn(&T) -> String,
) -> Result<(Vec<T>, Option<String>), CommandError> {
    loop {
        let cursor = match (has_more, page.last()) {
            (true, Some(last)) => Some(cursor_for(last)),
            _ => None,
        };
        if render(&page, cursor.clone())? <= RESPONSE_BUDGET_BYTES {
            return Ok((page, cursor));
        }
        if page.len() <= 1 {
            let offender = page
                .first()
                .map(&describe)
                .unwrap_or_else(|| "the section baseline".to_owned());
            return Err(budget_refusal(format!(
                "a single entry ({offender}) exceeds the 256 KiB response budget on its own — \
                 pre-bound legacy data; repair the stored row"
            )));
        }
        page.pop();
        has_more = true;
    }
}

fn get_kpi_ingest_context(
    state: &AppState,
    input: GetKpiIngestContextInput,
) -> Result<Value, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    let run = get_existing_run(state, &input.run_id)?;

    match input.section {
        None => {
            if input.cursor.is_some() || input.limit.is_some() {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    "cursor and limit apply to section calls only",
                ));
            }
            default_context(state, &run).and_then(|dto| {
                serde_json::to_value(dto)
                    .map_err(|error| internal(format!("serialization failed: {error}")))
            })
        }
        Some(section) => {
            section_context(state, &run, section, input.cursor, input.limit).and_then(|dto| {
                serde_json::to_value(dto)
                    .map_err(|error| internal(format!("serialization failed: {error}")))
            })
        }
    }
}

/// One expected metric key paired with its resolved catalog definition.
type ResolvedExpected = Vec<(String, KpiDefinition)>;

/// The expected keys resolved against the company's visible definitions —
/// the shared substrate of the catalog and plausibility sections.
fn resolved_catalog_inputs(
    state: &AppState,
    run: &KpiIngestRun,
) -> Result<(ResolvedExpected, Vec<KpiDefinition>), CommandError> {
    let expected = expected_keys(state, run)?;
    let definitions = state
        .financials()
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: Some(run.company_id.clone()),
        })
        .map_err(CommandError::from)?;
    let resolved = resolved_expected_definitions(state, run, &expected, &definitions)?;
    Ok((resolved, definitions))
}

fn default_context(state: &AppState, run: &KpiIngestRun) -> Result<ContextDto, CommandError> {
    let run_dto = run_status_dto(state, run)?;
    let document = document_meta(state, run)?;
    let derived_period = derived_period_hint(state, run)?;
    let (resolved_expected, definitions) = resolved_catalog_inputs(state, run)?;
    let catalog_all = build_catalog(&resolved_expected, &definitions, &run.company_id);
    let plausibility_all = build_plausibility(state, run, &resolved_expected)?;
    let expected_metric_keys: BTreeSet<&str> = resolved_expected
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    let rules = profile_rules(run)?;
    let manifest_available = state
        .kpi_ingest_runs()
        .latest_validation_attempt(&run.id)
        .map_err(CommandError::from)?
        .is_some();

    // Cap, then dynamically shrink to the byte budget: pop the last entry of
    // the larger section until the serialized response fits. Dropped entries
    // reappear on the section pages named by the emitted cursors; zero
    // retained entries emit the `{}` start-of-section sentinel (the section
    // call's baseline is small, so its pages fit normally).
    let mut catalog_kept = catalog_all.len().min(CATALOG_PAGE_MAX);
    let mut plausibility_kept = plausibility_all.len().min(PLAUSIBILITY_PAGE_MAX);
    loop {
        let truncated = TruncatedDto {
            catalog: section_cursor_or_sentinel(&catalog_all, catalog_kept, catalog_cursor_for),
            plausibility: section_cursor_or_sentinel(
                &plausibility_all,
                plausibility_kept,
                plausibility_cursor_for,
            ),
            manifest: manifest_available.then_some(true),
        };
        let catalog_page = &catalog_all[..catalog_kept];
        let not_requested: Vec<String> = catalog_page
            .iter()
            .filter(|entry| !expected_metric_keys.contains(entry.metric_key()))
            .map(|entry| entry.metric_key().to_owned())
            .collect();
        let dto = ContextDto {
            run: run_dto.clone(),
            document: document.clone(),
            derived_period: derived_period.clone(),
            catalog: catalog_page.to_vec(),
            not_requested,
            plausibility: plausibility_all[..plausibility_kept].to_vec(),
            profile_rules: rules.clone(),
            manifest_available,
            truncated,
        };
        let size = serialized_len(&dto)?;
        if size <= RESPONSE_BUDGET_BYTES {
            return Ok(dto);
        }
        if catalog_kept == 0 && plausibility_kept == 0 {
            // Defensive gate: unreachable through current writers (every
            // baseline field is write-time bounded); refusing beats silently
            // truncating the frozen shape.
            return Err(budget_refusal(
                "the context baseline exceeds the 256 KiB response budget — pre-bound legacy \
                 data; repair the stored row",
            ));
        }
        if plausibility_kept >= catalog_kept {
            plausibility_kept -= 1;
        } else {
            catalog_kept -= 1;
        }
    }
}

/// The default call's `truncated` cursor for one pageable section: absent when
/// everything fit, the keyset of the last retained entry when trimmed, and the
/// `{}` start-of-section sentinel when nothing was retained.
fn section_cursor_or_sentinel<T>(
    all: &[T],
    kept: usize,
    cursor_for: impl Fn(&T) -> String,
) -> Option<String> {
    if kept >= all.len() {
        return None;
    }
    if kept == 0 {
        return Some(start_sentinel_cursor());
    }
    Some(cursor_for(&all[kept - 1]))
}

fn section_context(
    state: &AppState,
    run: &KpiIngestRun,
    section: ContextSection,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<SectionDto, CommandError> {
    match section {
        ContextSection::Catalog => {
            let limit = validate_section_limit(limit, CATALOG_PAGE_MAX, "catalog")?;
            let decoded: SectionCursor<CatalogCursor> = match cursor.as_deref() {
                Some(cursor) => decode_section_cursor(cursor)?,
                None => SectionCursor::Start,
            };
            let (resolved, definitions) = resolved_catalog_inputs(state, run)?;
            let all = build_catalog(&resolved, &definitions, &run.company_id);
            let start = catalog_start_index(&all, &decoded);
            let end = (start + limit).min(all.len());
            let page: Vec<CatalogEntryDto> = all[start..end].to_vec();
            let has_more = end < all.len();
            let run_id = run.id.clone();
            let (page, next_cursor) = fit_section_page(
                page,
                has_more,
                catalog_cursor_for,
                |entries, next_cursor| {
                    serialized_len(&SectionDto {
                        run_id: run_id.clone(),
                        section: "catalog",
                        catalog: Some(entries.to_vec()),
                        plausibility: None,
                        manifest: None,
                        next_cursor,
                    })
                },
                |entry| format!("catalog entry {}", entry.definition_id()),
            )?;
            Ok(SectionDto {
                run_id: run.id.clone(),
                section: "catalog",
                catalog: Some(page),
                plausibility: None,
                manifest: None,
                next_cursor,
            })
        }
        ContextSection::Plausibility => {
            let limit = validate_section_limit(limit, PLAUSIBILITY_PAGE_MAX, "plausibility")?;
            let decoded: SectionCursor<PlausibilityCursor> = match cursor.as_deref() {
                Some(cursor) => decode_section_cursor(cursor)?,
                None => SectionCursor::Start,
            };
            let (resolved, _definitions) = resolved_catalog_inputs(state, run)?;
            let all = build_plausibility(state, run, &resolved)?;
            let start = plausibility_start_index(&all, &decoded);
            let end = (start + limit).min(all.len());
            let page: Vec<PlausibilityEntryDto> = all[start..end].to_vec();
            let has_more = end < all.len();
            let run_id = run.id.clone();
            let (page, next_cursor) = fit_section_page(
                page,
                has_more,
                plausibility_cursor_for,
                |entries, next_cursor| {
                    serialized_len(&SectionDto {
                        run_id: run_id.clone(),
                        section: "plausibility",
                        catalog: None,
                        plausibility: Some(entries.to_vec()),
                        manifest: None,
                        next_cursor,
                    })
                },
                |entry| {
                    format!(
                        "plausibility slot {}/{}",
                        entry.metric_key, entry.slot.definition_id
                    )
                },
            )?;
            Ok(SectionDto {
                run_id: run.id.clone(),
                section: "plausibility",
                catalog: None,
                plausibility: Some(page),
                manifest: None,
                next_cursor,
            })
        }
        ContextSection::Manifest => manifest_section(state, run, cursor, limit),
    }
}

/// The manifest section (repair context): page 1 pins the LATEST validation
/// attempt (including `failed` — the run row's `manifest_hash` is NULL after a
/// failed validation by design) and serves the full manifest header with the
/// first observation page; continuation cursors carry the pinned `attemptId`
/// plus an observation offset, so a newer attempt appearing between pages
/// never splices two manifests together.
fn manifest_section(
    state: &AppState,
    run: &KpiIngestRun,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<SectionDto, CommandError> {
    let limit = validate_section_limit(limit, MANIFEST_PAGE_MAX, "manifest")?;
    let (attempt, offset) = match cursor.as_deref() {
        None => {
            let attempt = state
                .kpi_ingest_runs()
                .latest_validation_attempt(&run.id)
                .map_err(CommandError::from)?
                .ok_or_else(|| {
                    CommandError::new(
                        CommandErrorCode::Conflict,
                        "no validation attempt exists for this run yet — check manifestAvailable",
                    )
                })?;
            (attempt, 0usize)
        }
        Some(cursor) => {
            let decoded: ManifestCursor = match decode_section_cursor(cursor)? {
                // `{}` is a catalog/plausibility-only sentinel: a manifest
                // continuation must pin its attempt.
                SectionCursor::Start => return Err(invalid_cursor()),
                SectionCursor::After(decoded) => decoded,
            };
            let attempt = state
                .kpi_ingest_runs()
                .validation_attempt_by_id(&run.id, &decoded.attempt_id)
                .map_err(CommandError::from)?
                .ok_or_else(invalid_cursor)?;
            (attempt, decoded.offset)
        }
    };

    let mut manifest: Value = serde_json::from_str(&attempt.manifest_json)
        .map_err(|_| internal("stored manifest bytes are not valid JSON"))?;
    let Some(manifest_object) = manifest.as_object_mut() else {
        return Err(internal("stored manifest is not a JSON object"));
    };
    let observations = match manifest_object.remove("observations") {
        Some(Value::Array(observations)) => observations,
        Some(_) => return Err(internal("stored manifest observations is not an array")),
        None => Vec::new(),
    };

    let start = offset.min(observations.len());
    let end = (start + limit).min(observations.len());
    let mut page: Vec<Value> = observations[start..end].to_vec();
    let mut has_more = end < observations.len();
    let attempt_id = attempt.id.clone();

    loop {
        let next_cursor = has_more.then(|| {
            encode_cursor(&ManifestCursor {
                attempt_id: attempt_id.clone(),
                offset: start + page.len(),
            })
        });
        let manifest_value = if start == 0 {
            let mut header = manifest_object.clone();
            header.insert("observations".to_owned(), Value::Array(page.clone()));
            Value::Object(header)
        } else {
            serde_json::json!({ "observations": page })
        };
        let dto = SectionDto {
            run_id: run.id.clone(),
            section: "manifest",
            catalog: None,
            plausibility: None,
            manifest: Some(manifest_value),
            next_cursor: next_cursor.clone(),
        };
        if serialized_len(&dto)? <= RESPONSE_BUDGET_BYTES {
            return Ok(dto);
        }
        if page.is_empty() {
            return Err(budget_refusal(
                "the manifest header alone exceeds the 256 KiB response budget — pre-bound \
                 legacy data; invalidate and re-validate the run",
            ));
        }
        page.pop();
        has_more = true;
    }
}

// ============================================================================
// get_kpi_ingest_document
// ============================================================================

/// Process-wide verified-blob cache: full-buffer hash verification happens
/// once per (canonical path, hash, size, mtime); later chunk reads seek. Keyed
/// by the canonical path so one data dir can never authorize another's
/// same-named blob. Metadata-preserving external replacement inside the
/// app-owned content-addressed store is outside this boundary (documented,
/// data-model § blobs).
/// Cache key: (canonical blob path, frozen hash) → verified (size, mtime).
type VerifiedBlobKey = (PathBuf, String);
type VerifiedBlobStamp = (u64, SystemTime);
static VERIFIED_BLOBS: OnceLock<Mutex<HashMap<VerifiedBlobKey, VerifiedBlobStamp>>> =
    OnceLock::new();

#[cfg(test)]
thread_local! {
    /// Per-thread so a parallel test run can measure its own delta: registry
    /// dispatch is synchronous on the calling test thread.
    pub(crate) static BLOB_HASH_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn get_kpi_ingest_document(
    state: &AppState,
    input: GetKpiIngestDocumentInput,
) -> Result<DocumentChunkDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    if input.length < 1 || input.length > DOCUMENT_CHUNK_MAX {
        return Err(budget_refusal(format!(
            "length {} is outside 1..={DOCUMENT_CHUNK_MAX}",
            input.length
        )));
    }
    let run = get_existing_run(state, &input.run_id)?;
    // Availability = SOURCE availability, not status: a run cancelled/failed
    // straight from `discovered` never captured bytes (conflict), while any
    // hash-bearing run — terminal included — stays readable.
    let Some(hash) = run.source_content_hash.clone() else {
        return Err(CommandError::new(
            CommandErrorCode::Conflict,
            "the run has not captured its source yet — finish start_kpi_ingest first",
        ));
    };
    if run.status == KpiIngestRunState::Discovered {
        return Err(internal(
            "invariant violated: a discovered run carries a source hash",
        ));
    }
    if !is_content_hash(&hash) {
        return Err(internal(
            "stored source_content_hash is not 64 lowercase hex bytes",
        ));
    }

    let path = state.data_dir().join(SNAPSHOT_DIR).join(&hash);
    let metadata = std::fs::metadata(&path)
        .map_err(|_| internal("the run's pinned source blob is missing on disk"))?;
    let total_bytes = metadata.len();
    let mtime = metadata
        .modified()
        .map_err(|_| internal("the blob's modification time is unreadable"))?;
    let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let cache_key = (canonical, hash.clone());

    // Poison recovery: every critical section leaves the map consistent (one
    // get / one insert), so a panicked writer must not wedge the dispatcher.
    let cache = VERIFIED_BLOBS.get_or_init(|| Mutex::new(HashMap::new()));
    let verified = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&cache_key)
        .is_some_and(|&(size, cached_mtime)| size == total_bytes && cached_mtime == mtime);

    let chunk: Vec<u8> = if verified {
        // Verified once already — chunk-only IO.
        let mut file = std::fs::File::open(&path)
            .map_err(|_| internal("the run's pinned source blob is unreadable"))?;
        let start = input.offset.min(total_bytes);
        file.seek(SeekFrom::Start(start))
            .map_err(|error| internal(format!("blob seek failed: {error}")))?;
        let mut chunk = Vec::new();
        file.take(input.length)
            .read_to_end(&mut chunk)
            .map_err(|error| internal(format!("blob read failed: {error}")))?;
        chunk
    } else {
        // First (or invalidated) access: read the whole blob, verify against
        // the frozen hash, cache the verification, serve from the buffer.
        #[cfg(test)]
        BLOB_HASH_COUNT.with(|count| count.set(count.get() + 1));
        let bytes = std::fs::read(&path)
            .map_err(|_| internal("the run's pinned source blob is unreadable"))?;
        let actual = crate::report_documents_capture::content_hash_hex(&bytes);
        if actual != hash {
            return Err(internal(
                "the pinned source blob no longer matches the run's frozen content hash",
            ));
        }
        cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_key, (total_bytes, mtime));
        let start = usize::try_from(input.offset.min(total_bytes)).expect("≤ file size");
        let end = usize::try_from(input.offset.saturating_add(input.length).min(total_bytes))
            .expect("≤ file size");
        bytes[start..end].to_vec()
    };

    let read_len = chunk.len() as u64;
    Ok(DocumentChunkDto {
        bytes_base64: STANDARD.encode(&chunk),
        offset: input.offset,
        length: read_len,
        total_bytes,
        sha256: hash,
        eof: input.offset.saturating_add(read_len) >= total_bytes,
    })
}

// ============================================================================
// Registered handlers
// ============================================================================

pub fn get_kpi_ingest_context_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_kpi_ingest_context(state, input))
}

pub fn get_kpi_ingest_document_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_kpi_ingest_document(state, input))
}

#[cfg(test)]
mod tests {
    use super::super::kpi_ingest::test_support::*;
    use super::*;
    use crate::report_documents_capture::content_hash_hex;
    use serde_json::json;

    fn doc_hash() -> String {
        content_hash_hex(DOC_BYTES)
    }

    /// Start the shared `doc1` run with full context → `extracting`, hash
    /// frozen, blob pinned.
    fn started_run(state: &AppState) -> String {
        let payload = success(acquisition_call(
            state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        payload["runId"].as_str().expect("runId").to_owned()
    }

    fn context(state: &AppState, args: serde_json::Value) -> ToolOutcome {
        acquisition_call(state, "get_kpi_ingest_context", &args)
    }

    fn document_chunk(state: &AppState, args: serde_json::Value) -> ToolOutcome {
        acquisition_call(state, "get_kpi_ingest_document", &args)
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_definition_raw(
        state: &AppState,
        id: &str,
        scope: &str,
        company_id: Option<&str>,
        metric_key: &str,
        label: &str,
        value_kind: &str,
        origin: &str,
    ) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "INSERT INTO kpi_definitions
                    (id, scope, company_id, metric_key, label, value_kind, origin)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, scope, company_id, metric_key, label, value_kind, origin],
            )
            .expect("definition row");
    }

    fn seed_attempt_raw(
        state: &AppState,
        id: &str,
        run_id: &str,
        revision: i64,
        attempt: i64,
        outcome: &str,
        observation_count: usize,
    ) {
        let observations: Vec<Value> = (0..observation_count)
            .map(|ordinal| json!({ "ordinal": ordinal, "metricKey": format!("m{ordinal}") }))
            .collect();
        let manifest = json!({
            "manifestSchemaVersion": 1,
            "runId": run_id,
            "revision": revision,
            "outcome": outcome,
            "runDiagnostics": [],
            "completeness": { "expected": [], "present": [], "missing": [] },
            "observations": observations,
        });
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "INSERT INTO kpi_ingest_validation_attempts
                    (id, run_id, revision, attempt, outcome, manifest_hash, manifest_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    id,
                    run_id,
                    revision,
                    attempt,
                    outcome,
                    format!("hash-{id}"),
                    manifest.to_string(),
                ],
            )
            .expect("attempt row");
    }

    // ------------------------------------------------------------------
    // Default call
    // ------------------------------------------------------------------

    #[test]
    fn default_context_golden_shape() {
        let state = test_state();
        let run_id = started_run(&state);
        let payload = success(context(&state, json!({ "runId": run_id })));
        let pretty = serde_json::to_string_pretty(&payload).expect("serializable");
        let redacted = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z")
            .expect("regex")
            .replace_all(&pretty, "[timestamp]")
            .into_owned();
        let redacted = regex::Regex::new(r"kpiing_[0-9a-f]+")
            .expect("regex")
            .replace_all(&redacted, "kpiing_[uid]")
            .into_owned();
        insta::assert_snapshot!("context_default_wire_shape", redacted);
    }

    #[test]
    fn cursor_or_limit_without_section_is_invalid_input() {
        let state = test_state();
        let run_id = started_run(&state);
        for args in [
            json!({ "runId": run_id, "cursor": "abc" }),
            json!({ "runId": run_id, "limit": 5 }),
        ] {
            assert_eq!(
                failure_code(context(&state, args)),
                CommandErrorCode::InvalidInput
            );
        }
    }

    #[test]
    fn unknown_run_is_not_found_and_control_chars_are_invalid_input() {
        let state = test_state();
        assert_eq!(
            failure_code(context(
                &state,
                json!({ "runId": "kpiing_ffffffffffffffffffffffffffffffff" })
            )),
            CommandErrorCode::NotFound
        );
        assert_eq!(
            failure_code(context(&state, json!({ "runId": "bad\u{0001}id" }))),
            CommandErrorCode::InvalidInput
        );
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": "bad\u{0001}id", "offset": 0, "length": 1 })
            )),
            CommandErrorCode::InvalidInput
        );
    }

    // ------------------------------------------------------------------
    // Catalog
    // ------------------------------------------------------------------

    #[test]
    fn catalog_carries_expected_keys_plus_minted_extras_only() {
        let state = test_state();
        seed_definition_raw(
            &state,
            "kdmint",
            "company",
            Some("c1"),
            "custom_pipeline_yield",
            "Custom pipeline yield",
            "currency",
            "agent",
        );
        seed_definition_raw(
            &state,
            "kduser",
            "company",
            Some("c1"),
            "user_only_metric",
            "User-created",
            "currency",
            "user",
        );
        let run_id = started_run(&state);
        let payload = success(context(&state, json!({ "runId": run_id })));
        let keys: Vec<&str> = payload["catalog"]
            .as_array()
            .expect("catalog")
            .iter()
            .map(|entry| entry["metricKey"].as_str().expect("key"))
            .collect();

        assert!(keys.contains(&"net_profit"), "expected floor key present");
        assert!(
            keys.contains(&"custom_pipeline_yield"),
            "agent-minted company extra present"
        );
        assert!(
            !keys.contains(&"user_only_metric"),
            "a user-origin company definition is not a minted extra"
        );
        // Full entries (expected + agent-minted) sort ahead of every compact
        // canonical entry (ADR 0101 dec. 7/9 tier order), not plain alphabetical
        // — the two Full keys here stay internally sorted.
        let full_keys: Vec<&str> = keys
            .iter()
            .copied()
            .filter(|key| *key == "net_profit" || *key == "custom_pipeline_yield")
            .collect();
        let mut sorted_full = full_keys.clone();
        sorted_full.sort_unstable();
        assert_eq!(
            full_keys, sorted_full,
            "full-tier entries sort by metric key"
        );
    }

    /// ADR 0101 dec. 7/9: `build_catalog` now widens to every canonical
    /// definition (`company_id IS NULL`), not just what this run expected —
    /// crossing the `CATALOG_PAGE_MAX` boundary, so the default call's
    /// `catalog` is always a truncated prefix; walking the `catalog` section
    /// cursor to exhaustion must reach the full canon exactly once each.
    #[test]
    fn catalog_section_pagination_reaches_full_canon_without_duplicates() {
        let state = test_state();
        let run_id = started_run(&state);

        let canonical_count: i64 = {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .query_row(
                    "SELECT COUNT(*) FROM kpi_definitions WHERE company_id IS NULL",
                    [],
                    |row| row.get(0),
                )
                .expect("count")
        };
        assert!(
            canonical_count as usize > CATALOG_PAGE_MAX,
            "the widened canon crosses the {CATALOG_PAGE_MAX}-entry page boundary: \
             {canonical_count}"
        );

        let mut walked: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut args = json!({ "runId": run_id, "section": "catalog", "limit": 64 });
            if let Some(cursor) = &cursor {
                args["cursor"] = json!(cursor);
            }
            let page = success(context(&state, args));
            assert_eq!(page["section"], "catalog");
            for entry in page["catalog"].as_array().expect("page") {
                walked.push(entry["metricKey"].as_str().expect("key").to_owned());
            }
            match page["nextCursor"].as_str() {
                Some(next) => cursor = Some(next.to_owned()),
                None => break,
            }
        }
        assert_eq!(
            walked.len(),
            canonical_count as usize,
            "every canonical row reached exactly once"
        );
        let mut deduped = walked.clone();
        deduped.sort_unstable();
        deduped.dedup();
        assert_eq!(deduped.len(), walked.len(), "no duplicates across pages");

        // The default call's own catalog is a prefix of the full walk, with a
        // cursor signalling truncation (the canon now always exceeds one page).
        let full = success(context(&state, json!({ "runId": run_id })));
        let default_page: Vec<String> = full["catalog"]
            .as_array()
            .expect("catalog")
            .iter()
            .map(|entry| entry["metricKey"].as_str().expect("key").to_owned())
            .collect();
        assert_eq!(
            &walked[..default_page.len()],
            default_page.as_slice(),
            "the default call's catalog page is a prefix of the full walk"
        );
        assert!(
            full["truncated"]["catalog"].is_string(),
            "the default call's catalog is always truncated once the canon exceeds one page"
        );
    }

    /// ADR 0101 dec. 7: the catalog now carries every canonical definition,
    /// not only what this run expected — an agent can check "does this
    /// already exist" before proposing.
    #[test]
    fn catalog_includes_compact_canonical_beyond_expected() {
        let state = test_state();
        let run_id = started_run(&state);
        let payload = success(context(&state, json!({ "runId": run_id })));
        let expected: BTreeSet<String> = payload["run"]["expectedKpis"]["keys"]
            .as_array()
            .expect("expected keys")
            .iter()
            .map(|key| key.as_str().expect("key").to_owned())
            .collect();
        let catalog = payload["catalog"].as_array().expect("catalog");
        assert!(
            catalog.len() > expected.len(),
            "the widened catalog carries canon beyond the expected floor: {} vs {}",
            catalog.len(),
            expected.len()
        );
        let compact = catalog
            .iter()
            .find(|entry| !expected.contains(entry["metricKey"].as_str().expect("key")))
            .expect("a canonical entry beyond expected");
        let mut keys: Vec<&str> = compact
            .as_object()
            .expect("object")
            .keys()
            .map(String::as_str)
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["label", "metricKey"],
            "a canon entry beyond expected is the compact shape: {compact}"
        );
    }

    /// ADR 0101 dec. 7/9: full metadata is reserved for the run's expected
    /// keys and the company's agent-minted extras; every other canonical row
    /// is the compact `{metricKey, label}` projection.
    #[test]
    fn expected_and_agent_entries_carry_full_metadata_compact_rest() {
        let state = test_state();
        seed_definition_raw(
            &state,
            "kdmint2",
            "company",
            Some("c1"),
            "custom_pipeline_yield_2",
            "Custom pipeline yield 2",
            "currency",
            "agent",
        );
        let run_id = started_run(&state);
        let payload = success(context(&state, json!({ "runId": run_id })));
        let catalog = payload["catalog"].as_array().expect("catalog");

        for key in ["net_profit", "custom_pipeline_yield_2"] {
            let entry = catalog
                .iter()
                .find(|entry| entry["metricKey"] == key)
                .unwrap_or_else(|| panic!("{key} present in catalog"));
            assert!(
                entry["definitionId"].is_string(),
                "{key} carries definitionId: {entry}"
            );
            assert!(
                entry["statementGroup"].is_string(),
                "{key} carries statementGroup: {entry}"
            );
            assert!(
                entry["valueKind"].is_string(),
                "{key} carries valueKind: {entry}"
            );
            assert!(entry["origin"].is_string(), "{key} carries origin: {entry}");
        }

        let expected: BTreeSet<String> = payload["run"]["expectedKpis"]["keys"]
            .as_array()
            .expect("expected keys")
            .iter()
            .map(|key| key.as_str().expect("key").to_owned())
            .collect();
        let compact = catalog
            .iter()
            .find(|entry| {
                let key = entry["metricKey"].as_str().expect("key");
                key != "custom_pipeline_yield_2" && !expected.contains(key)
            })
            .expect("a compact canonical entry");
        assert!(
            compact["definitionId"].is_null(),
            "compact entry omits definitionId: {compact}"
        );
    }

    #[test]
    fn cursors_round_trip_pipes_unicode_and_max_length_legacy_keys() {
        let state = test_state();
        // Pre-guard legacy identities seeded raw: a pipe+unicode key and a
        // 300-byte key (the write bound is 256 B — these rows predate it).
        seed_definition_raw(
            &state,
            "kdpipe",
            "company",
            Some("c1"),
            "weird|key_π",
            "Pipe key",
            "currency",
            "agent",
        );
        let long_key = "x".repeat(300);
        seed_definition_raw(
            &state,
            "kdlong",
            "company",
            Some("c1"),
            &long_key,
            "Long key",
            "currency",
            "agent",
        );
        let run_id = started_run(&state);

        let mut seen: Vec<String> = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let mut args = json!({ "runId": run_id, "section": "catalog", "limit": 1 });
            if let Some(cursor) = &cursor {
                args["cursor"] = json!(cursor);
            }
            let page = success(context(&state, args));
            for entry in page["catalog"].as_array().expect("page") {
                seen.push(entry["metricKey"].as_str().expect("key").to_owned());
            }
            match page["nextCursor"].as_str() {
                Some(next) => cursor = Some(next.to_owned()),
                None => break,
            }
        }
        assert!(seen.contains(&"weird|key_π".to_owned()));
        assert!(seen.contains(&long_key));
        let mut deduped = seen.clone();
        deduped.dedup();
        assert_eq!(seen, deduped, "no entry repeats across limit=1 pages");
    }

    #[test]
    fn malformed_cursors_are_invalid_input() {
        let state = test_state();
        let run_id = started_run(&state);
        for cursor in [
            "not-base64url-!!!",
            &URL_SAFE_NO_PAD.encode(b"not json"),
            &URL_SAFE_NO_PAD.encode(br#"{"wrong":"fields"}"#),
        ] {
            assert_eq!(
                failure_code(context(
                    &state,
                    json!({ "runId": run_id, "section": "catalog", "cursor": cursor })
                )),
                CommandErrorCode::InvalidInput,
                "cursor {cursor:?}"
            );
        }
    }

    #[test]
    fn the_start_sentinel_reads_from_the_beginning_and_is_refused_for_manifest() {
        let state = test_state();
        let run_id = started_run(&state);
        let sentinel = start_sentinel_cursor();
        let page = success(context(
            &state,
            json!({ "runId": run_id, "section": "catalog", "cursor": sentinel, "limit": 2 }),
        ));
        assert_eq!(
            page["catalog"].as_array().expect("page").len(),
            2,
            "the {{}} sentinel starts from the beginning"
        );
        assert_eq!(
            failure_code(context(
                &state,
                json!({ "runId": run_id, "section": "manifest", "cursor": sentinel })
            )),
            CommandErrorCode::InvalidInput,
            "a manifest continuation must pin its attempt"
        );
    }

    #[test]
    fn section_limits_outside_the_cap_are_budget_refusals() {
        let state = test_state();
        let run_id = started_run(&state);
        for (section, limit) in [
            ("catalog", 0),
            ("catalog", 65),
            ("plausibility", 65),
            ("manifest", 51),
        ] {
            assert_eq!(
                failure_code(context(
                    &state,
                    json!({ "runId": run_id, "section": section, "limit": limit })
                )),
                CommandErrorCode::ResponseBudgetExceeded,
                "{section} limit {limit}"
            );
        }
    }

    #[test]
    fn overlong_stored_labels_are_byte_truncated_with_a_marker() {
        let state = test_state();
        // 150 two-byte chars = 300 bytes — over the 256-byte label cap.
        let label = "ł".repeat(150);
        seed_definition_raw(
            &state,
            "kdlab",
            "company",
            Some("c1"),
            "labelled_metric",
            &label,
            "currency",
            "agent",
        );
        let run_id = started_run(&state);
        let payload = success(context(&state, json!({ "runId": run_id })));
        let entry = payload["catalog"]
            .as_array()
            .expect("catalog")
            .iter()
            .find(|entry| entry["metricKey"] == "labelled_metric")
            .expect("entry");
        let label = entry["label"].as_str().expect("label");
        assert!(
            label.len() <= 256,
            "label stays ≤256 bytes: {}",
            label.len()
        );
        assert!(label.ends_with('…'), "truncation carries the marker");
    }

    // ------------------------------------------------------------------
    // Plausibility
    // ------------------------------------------------------------------

    fn seed_period_and_fact(
        state: &AppState,
        period_id: &str,
        fiscal_year: i64,
        period_type: &str,
        definition_id: &str,
        value: &str,
    ) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "INSERT OR IGNORE INTO financial_periods
                    (id, company_id, fiscal_year, period_type, period_end_date)
                 VALUES (?1, 'c1', ?2, ?3, ?4)",
                rusqlite::params![
                    period_id,
                    fiscal_year,
                    period_type,
                    format!("{fiscal_year}-12-31"),
                ],
            )
            .expect("period");
        connection
            .execute(
                "INSERT INTO financial_facts
                    (id, company_id, period_id, definition_id, value_numeric, statement_basis,
                     attribution, variant, measure_window, data_quality)
                 VALUES (?1, 'c1', ?2, ?3, ?4, 'consolidated', 'total', 'reported', 'flow',
                         'final')",
                rusqlite::params![
                    format!("f-{period_id}-{definition_id}"),
                    period_id,
                    definition_id,
                    value,
                ],
            )
            .expect("fact");
    }

    fn canonical_definition_id(state: &AppState, metric_key: &str) -> String {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .query_row(
                "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'canonical'",
                [metric_key],
                |row| row.get(0),
            )
            .expect("canonical definition")
    }

    #[test]
    fn plausibility_observed_slot_matches_the_validator_history() {
        let state = test_state();
        let revenue = canonical_definition_id(&state, "revenue");
        seed_period_and_fact(&state, "p2022", 2022, "FY", &revenue, "100");
        seed_period_and_fact(&state, "p2023", 2023, "FY", &revenue, "300");
        seed_period_and_fact(&state, "p2024", 2024, "FY", &revenue, "500");
        // Start with consolidated scope so the seeded slots are in-basis.
        let payload = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &json!({
                "documentId": "doc1",
                "profileId": "gpw_ifrs_annual",
                "scope": "consolidated",
                "dataQuality": "final",
                "period": { "fiscalYear": 2026, "periodType": "FY" }
            }),
        ));
        let run_id = payload["runId"].as_str().expect("runId").to_owned();

        let full = success(context(&state, json!({ "runId": run_id })));
        let entries = full["plausibility"].as_array().expect("plausibility");
        let observed = entries
            .iter()
            .find(|entry| entry["metricKey"] == "revenue" && entry["slotOrigin"] == "observed")
            .expect("observed revenue slot");

        // history_median of [100, 300, 500] = 300 (upper middle); every point
        // is non-zero; chronological recent points.
        assert_eq!(observed["median"], "300");
        assert_eq!(observed["nonZeroCount"], 3);
        assert_eq!(observed["abstentionReason"], Value::Null);
        assert_eq!(observed["slot"]["scope"], "consolidated");
        assert_eq!(observed["slot"]["attribution"], "total");
        assert_eq!(observed["slot"]["measureWindow"], "flow");
        let years: Vec<i64> = observed["recentPoints"]
            .as_array()
            .expect("points")
            .iter()
            .map(|point| point["fiscalYear"].as_i64().expect("year"))
            .collect();
        assert_eq!(years, vec![2022, 2023, 2024], "chronological order");

        // The observed slot equals the candidate default slot here, so no
        // duplicate candidate entry exists for revenue/consolidated.
        let revenue_entries: Vec<_> = entries
            .iter()
            .filter(|entry| entry["metricKey"] == "revenue")
            .collect();
        assert_eq!(
            revenue_entries.len(),
            1,
            "an observed default slot suppresses its candidate twin: {revenue_entries:?}"
        );

        // A history-less expected definition still gets candidate evidence.
        let candidate = entries
            .iter()
            .find(|entry| entry["metricKey"] == "net_profit")
            .expect("net_profit candidate");
        assert_eq!(candidate["slotOrigin"], "candidate");
        assert_eq!(candidate["median"], Value::Null);
        assert_eq!(candidate["nonZeroCount"], 0);
        assert_eq!(candidate["abstentionReason"], "thin_history");
        assert_eq!(
            candidate["recentPoints"].as_array().expect("points").len(),
            0
        );

        // Stocks classify point_in_time via the validator's own classifier.
        let stock = entries
            .iter()
            .find(|entry| entry["metricKey"] == "total_assets")
            .expect("total_assets candidate");
        assert_eq!(stock["slot"]["measureWindow"], "point_in_time");
    }

    #[test]
    fn unattached_scope_serves_candidates_for_both_bases() {
        let state = test_state();
        // Two-phase fresh start: no scope/quality/period → source_captured.
        let payload = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &json!({ "documentId": "doc1", "profileId": "gpw_ifrs_annual" }),
        ));
        assert_eq!(payload["status"], "source_captured");
        let run_id = payload["runId"].as_str().expect("runId").to_owned();

        let full = success(context(&state, json!({ "runId": run_id })));
        let scopes: std::collections::BTreeSet<&str> = full["plausibility"]
            .as_array()
            .expect("plausibility")
            .iter()
            .filter(|entry| entry["metricKey"] == "revenue")
            .map(|entry| entry["slot"]["scope"].as_str().expect("scope"))
            .collect();
        assert_eq!(
            scopes.into_iter().collect::<Vec<_>>(),
            vec!["consolidated", "standalone"],
            "no attached scope → evidence for both bases"
        );
    }

    /// ADR 0101 dec. 8: plausibility stays computed only from `expected`
    /// (cost unchanged — `build_plausibility` is untouched by this slice);
    /// a catalog key outside `expected` is `notRequested`, an explicit signal
    /// on the response rather than a silent absence a caller could misread
    /// as "no history exists".
    #[test]
    fn plausibility_not_requested_for_unexpected_key() {
        let state = test_state();
        let run_id = started_run(&state);
        let payload = success(context(&state, json!({ "runId": run_id })));
        let expected: BTreeSet<String> = payload["run"]["expectedKpis"]["keys"]
            .as_array()
            .expect("expected keys")
            .iter()
            .map(|key| key.as_str().expect("key").to_owned())
            .collect();
        let unexpected_key = payload["catalog"]
            .as_array()
            .expect("catalog")
            .iter()
            .map(|entry| entry["metricKey"].as_str().expect("key").to_owned())
            .find(|key| !expected.contains(key))
            .expect("a catalog key outside expected");

        assert!(
            payload["notRequested"]
                .as_array()
                .expect("notRequested")
                .iter()
                .any(|key| key.as_str() == Some(unexpected_key.as_str())),
            "the unexpected key reads as notRequested: {:?}",
            payload["notRequested"]
        );
        assert!(
            !payload["plausibility"]
                .as_array()
                .expect("plausibility")
                .iter()
                .any(|entry| entry["metricKey"] == unexpected_key),
            "an unrequested key never gets plausibility evidence — absence, not a computed \
             abstention"
        );
    }

    #[test]
    fn candidate_window_is_profile_aware_and_classifier_driven() {
        // `period_nature` decides instant/duration; the profile decides the
        // duration window (interim = cumulative, ADR 0098 dec. 3). `instant`
        // (e.g. `total_assets`, `wdf_book_value_per_share`,
        // `shares_outstanding`) always short-circuits to `point_in_time`,
        // whatever the profile.
        assert_eq!(candidate_window("gpw_ifrs_annual@v1", "duration"), "flow");
        assert_eq!(candidate_window("gpw_interim@v1", "duration"), "cumulative");
        assert_eq!(
            candidate_window("gpw_interim@v1", "instant"),
            "point_in_time"
        );
        assert_eq!(
            candidate_window("gpw_ifrs_annual@v1", "instant"),
            "point_in_time"
        );
        // ADR 0100 decision 6 fix: `roe` is a ratio, never TTM-eligible, but
        // it is duration-REPORTED (not in STOCK_METRIC_KEYS) -- so its
        // candidate window is `flow`/`cumulative`, never `point_in_time` as
        // the old `is_flow_key`-based classifier (which conflated the
        // TTM-eligibility and window-kind axes) produced for it. TTM
        // eligibility is the separate question `is_ttm_eligible` answers.
        assert_eq!(
            candidate_window("gpw_ifrs_annual@v1", "duration"),
            "flow",
            "a duration ratio like roe gets a flow window, not point_in_time"
        );
    }

    // ------------------------------------------------------------------
    // derivedPeriod + document meta
    // ------------------------------------------------------------------

    #[test]
    fn derived_period_hint_requires_matching_provenance() {
        let state = test_state();
        let run_id = started_run(&state);
        let hash = doc_hash();
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE report_documents SET content_hash = ?1 WHERE id = 'doc1'",
                    [&hash],
                )
                .expect("stamp doc hash");
        }
        // Cache bound to the SAME bytes the run pinned → the hint serves.
        state
            .financials()
            .store_derived_period("doc1", Some((2025, "FY", "2025-12-31")), 2, Some(&hash))
            .expect("cache");
        let full = success(context(&state, json!({ "runId": run_id })));
        assert_eq!(full["derivedPeriod"]["fiscalYear"], 2025);
        assert_eq!(full["derivedPeriod"]["periodType"], "FY");
        assert_eq!(full["derivedPeriod"]["periodEnd"], "2025-12-31");

        // A→B→A: the cache now describes OTHER bytes — the hint must go null
        // even though the document row's hash still matches the run's.
        state
            .financials()
            .store_derived_period("doc1", Some((1999, "FY", "1999-12-31")), 2, Some("bbbb"))
            .expect("cache for other bytes");
        let full = success(context(&state, json!({ "runId": run_id })));
        assert_eq!(full["derivedPeriod"], Value::Null);

        // Legacy NULL-provenance row → null too.
        state
            .financials()
            .store_derived_period("doc1", Some((1999, "FY", "1999-12-31")), 2, None)
            .expect("legacy row");
        let full = success(context(&state, json!({ "runId": run_id })));
        assert_eq!(full["derivedPeriod"], Value::Null);
    }

    #[test]
    fn document_meta_reports_the_pinned_blob_not_the_recaptured_row() {
        let state = test_state();
        let run_id = started_run(&state);

        // Simulate a recapture: bigger file at local_path, bigger byte_size on
        // the row. The context must keep describing the run's frozen blob.
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE report_documents SET byte_size = 999999 WHERE id = 'doc1'",
                    [],
                )
                .expect("update row");
        }
        std::fs::write(
            state.data_dir().join("report_documents/doc1.pdf"),
            vec![0u8; 4096],
        )
        .expect("recapture bytes");

        let full = success(context(&state, json!({ "runId": run_id })));
        assert_eq!(
            full["document"]["byteSize"],
            DOC_BYTES.len() as i64,
            "byteSize is the pinned blob's size"
        );
        assert_eq!(full["document"]["url"], "https://x/doc1.pdf");
    }

    // ------------------------------------------------------------------
    // Manifest section
    // ------------------------------------------------------------------

    #[test]
    fn manifest_availability_flips_and_the_section_pins_its_attempt() {
        let state = test_state();
        let run_id = started_run(&state);

        let before = success(context(&state, json!({ "runId": run_id })));
        assert_eq!(before["manifestAvailable"], false);
        assert_eq!(before["truncated"].get("manifest"), None);
        assert_eq!(
            failure_code(context(
                &state,
                json!({ "runId": run_id, "section": "manifest" })
            )),
            CommandErrorCode::Conflict,
            "no attempt yet → conflict"
        );

        seed_attempt_raw(&state, "att1", &run_id, 1, 1, "failed", 7);
        let after = success(context(&state, json!({ "runId": run_id })));
        assert_eq!(after["manifestAvailable"], true);
        assert_eq!(after["truncated"]["manifest"], true);

        // Page 1: header + first observations; the FAILED attempt serves (the
        // run row's manifest_hash is NULL — this is the repair context).
        let page1 = success(context(
            &state,
            json!({ "runId": run_id, "section": "manifest", "limit": 5 }),
        ));
        let manifest = &page1["manifest"];
        assert_eq!(manifest["manifestSchemaVersion"], 1, "header on page 1");
        assert_eq!(manifest["outcome"], "failed");
        assert_eq!(manifest["observations"].as_array().expect("obs").len(), 5);
        let cursor = page1["nextCursor"].as_str().expect("cursor").to_owned();

        // A NEWER attempt lands between pages — the pinned cursor must keep
        // serving the ORIGINAL manifest, never splice two together.
        seed_attempt_raw(&state, "att2", &run_id, 2, 1, "ready", 1);
        let page2 = success(context(
            &state,
            json!({ "runId": run_id, "section": "manifest", "cursor": cursor, "limit": 5 }),
        ));
        let manifest2 = &page2["manifest"];
        assert_eq!(
            manifest2.get("manifestSchemaVersion"),
            None,
            "continuation pages carry observations only"
        );
        let ordinals: Vec<i64> = manifest2["observations"]
            .as_array()
            .expect("obs")
            .iter()
            .map(|observation| observation["ordinal"].as_i64().expect("ordinal"))
            .collect();
        assert_eq!(
            ordinals,
            vec![5, 6],
            "the pinned attempt's tail, not att2's"
        );
        assert_eq!(page2["nextCursor"], Value::Null);

        // A FRESH section call (no cursor) now serves the newer attempt.
        let fresh = success(context(
            &state,
            json!({ "runId": run_id, "section": "manifest" }),
        ));
        assert_eq!(fresh["manifest"]["outcome"], "ready");
    }

    // ------------------------------------------------------------------
    // Budget: dynamic shrink + defensive gate
    // ------------------------------------------------------------------

    fn plant_last_error(state: &AppState, run_id: &str, bytes: usize) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET last_error = ?1 WHERE id = ?2",
                rusqlite::params!["e".repeat(bytes), run_id],
            )
            .expect("plant last_error");
    }

    #[test]
    fn an_adversarial_baseline_shrinks_sections_instead_of_refusing() {
        let state = test_state();
        let run_id = started_run(&state);
        // A pre-bound legacy row: 259 KB of last_error — baseline + sections
        // would exceed the budget, sections alone fit easily.
        plant_last_error(&state, &run_id, 259_000);

        let full = success(context(&state, json!({ "runId": run_id })));
        let serialized = serde_json::to_vec(&full).expect("serialize");
        assert!(
            serialized.len() <= 262_144,
            "the shrunk response fits: {} bytes",
            serialized.len()
        );
        let truncated = &full["truncated"];
        assert!(
            truncated.get("catalog").is_some() || truncated.get("plausibility").is_some(),
            "at least one section was shrunk with a cursor: {truncated:?}"
        );

        // Every shrunk row is reachable via its section call (no dead end).
        let full_catalog_via_section = success(context(
            &state,
            json!({ "runId": run_id, "section": "catalog" }),
        ));
        assert!(
            !full_catalog_via_section["catalog"]
                .as_array()
                .expect("catalog")
                .is_empty(),
            "section calls have a small baseline and serve the entries"
        );
    }

    #[test]
    fn a_baseline_beyond_the_budget_is_a_defensive_typed_refusal() {
        let state = test_state();
        let run_id = started_run(&state);
        plant_last_error(&state, &run_id, 300_000);
        assert_eq!(
            failure_code(context(&state, json!({ "runId": run_id }))),
            CommandErrorCode::ResponseBudgetExceeded
        );
    }

    /// Mark a definition as a company's PRIMARY relevant KPI — the real source
    /// `expected_primary_metric_keys` (financials.rs) reads, so `start_kpi_ingest`
    /// stamps it into the run's expected set at creation exactly as production
    /// does (no hand-written stamp).
    fn seed_primary_relevance(state: &AppState, definition_id: &str) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
                 VALUES (?1, 'c1', ?2, 'active', 'manual', 'primary')",
                rusqlite::params![format!("krel-{definition_id}"), definition_id],
            )
            .expect("relevance row");
    }

    /// A REALISTIC populated default context (sol #387 B1): unlike the
    /// adversarial baseline (which plants a producer-invalid 259 KB `last_error`
    /// to force shrinking), this seeds a producer-valid run through the real
    /// read path — both pageable sections full to their caps, each slot carrying
    /// the maximum recent history, output strings near their byte caps — and
    /// proves the ≤256 KiB budget holds WITHOUT any shrink. Cardinalities are
    /// asserted first so a future fixture shrinkage cannot leave a vacuous green.
    #[test]
    fn a_realistic_full_context_fits_the_budget_without_shrinking() {
        let state = test_state();
        // 64 CANONICAL definitions, each marked a PRIMARY relevant KPI for the
        // company and given eight consolidated FY facts (the run's FY2025 is
        // excluded from history). `start_kpi_ingest` then stamps them into the
        // run's expected set through the real producer path — agent-minted
        // definitions never enter that set (ADR 0093 dec. 4). Labels near
        // LABEL_MAX; every slot carries RECENT_POINTS_MAX points.
        for idx in 0..CATALOG_PAGE_MAX {
            let id = format!("kdpop{idx:02}");
            let key = format!("mkey_{idx:02}");
            let label = format!(
                "Skonsolidowany wskaźnik operacyjny numer {idx:02} — pozycja sprawozdania z \
                 całkowitych dochodów grupy kapitałowej w ujęciu narastającym, wyrażona w tysiącach \
                 złotych, wraz z komentarzem zarządu o czynnikach zmiany rok do roku i sezonowości"
            );
            seed_definition_raw(
                &state,
                &id,
                "canonical",
                None,
                &key,
                &label,
                "currency",
                "system",
            );
            seed_primary_relevance(&state, &id);
            for year in 2016..2024 {
                seed_period_and_fact(
                    &state,
                    &format!("finper_c1_{year}_fy"),
                    year,
                    "FY",
                    &id,
                    &format!("{year}000000"),
                );
            }
        }

        // Start consolidated (matching the seeded facts' basis) with full context.
        // The expected-KPI stamp is now the natural union of the company's primary
        // relevance and the profile pack — no hand-written stamp.
        let run_id = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &json!({
                "documentId": "doc1",
                "profileId": "gpw_ifrs_annual",
                "scope": "consolidated",
                "dataQuality": "final",
                "period": { "fiscalYear": 2025, "periodType": "FY" }
            }),
        ))["runId"]
            .as_str()
            .expect("runId")
            .to_owned();

        let payload = success(context(&state, json!({ "runId": run_id })));
        let catalog = payload["catalog"].as_array().expect("catalog");
        let plausibility = payload["plausibility"].as_array().expect("plausibility");

        // Pinned cardinalities FIRST — both sections full to their page caps, and
        // at least one slot carries the maximum recent history.
        assert_eq!(
            catalog.len(),
            CATALOG_PAGE_MAX,
            "catalog full to its page cap"
        );
        assert_eq!(
            plausibility.len(),
            PLAUSIBILITY_PAGE_MAX,
            "plausibility full to its page cap (not byte-shrunk below it)"
        );
        let max_points = plausibility
            .iter()
            .filter_map(|entry| entry["recentPoints"].as_array().map(Vec::len))
            .max()
            .expect("a slot with history");
        assert_eq!(
            max_points, RECENT_POINTS_MAX,
            "a slot carries the max recent history"
        );

        // The realistic full-page response fits the budget with no shrink.
        let serialized = serde_json::to_vec(&payload).expect("serialize");
        assert!(
            serialized.len() <= RESPONSE_BUDGET_BYTES,
            "a realistic full context fits the budget: {} bytes",
            serialized.len()
        );
    }

    // ------------------------------------------------------------------
    // get_kpi_ingest_document
    // ------------------------------------------------------------------

    #[test]
    fn document_chunks_slice_verify_and_reassemble() {
        let state = test_state();
        let run_id = started_run(&state);
        let before = BLOB_HASH_COUNT.with(std::cell::Cell::get);

        let first = success(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 0, "length": 10 }),
        ));
        assert_eq!(first["totalBytes"], DOC_BYTES.len() as u64);
        assert_eq!(first["sha256"], doc_hash());
        assert_eq!(first["eof"], false);
        let second = success(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 10, "length": 262144 }),
        ));
        assert_eq!(second["eof"], true);
        assert_eq!(second["length"], (DOC_BYTES.len() - 10) as u64);

        let mut reassembled = STANDARD
            .decode(first["bytesBase64"].as_str().expect("b64"))
            .expect("decode");
        reassembled.extend(
            STANDARD
                .decode(second["bytesBase64"].as_str().expect("b64"))
                .expect("decode"),
        );
        assert_eq!(reassembled, DOC_BYTES, "chunks reassemble the exact bytes");

        let after = BLOB_HASH_COUNT.with(std::cell::Cell::get);
        assert_eq!(
            after - before,
            1,
            "verification hashes the blob ONCE; later chunks seek"
        );

        // Read at/past EOF: empty + eof, offset echoed (u64::MAX saturates).
        let at_end = success(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": DOC_BYTES.len() as u64, "length": 1 }),
        ));
        assert_eq!(at_end["bytesBase64"], "");
        assert_eq!(at_end["length"], 0);
        assert_eq!(at_end["eof"], true);
        let far = success(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": u64::MAX, "length": 1 }),
        ));
        assert_eq!(far["eof"], true);
        assert_eq!(far["length"], 0);
    }

    #[test]
    fn document_length_outside_the_cap_is_a_budget_refusal() {
        let state = test_state();
        let run_id = started_run(&state);
        for length in [0u64, 262_145] {
            assert_eq!(
                failure_code(document_chunk(
                    &state,
                    json!({ "runId": run_id, "offset": 0, "length": length })
                )),
                CommandErrorCode::ResponseBudgetExceeded,
                "length {length}"
            );
        }
    }

    #[test]
    fn document_availability_follows_the_source_not_the_status() {
        let state = test_state();
        // A run that never captured: created raw in `discovered`, then walked
        // to cancelled/failed — the document is a conflict in all three.
        {
            let connection = state.checkout_for_tests().expect("raw");
            for (id, status) in [
                ("kpiing_00000000000000000000000000000001", "discovered"),
                ("kpiing_00000000000000000000000000000002", "cancelled"),
                ("kpiing_00000000000000000000000000000003", "failed"),
            ] {
                connection
                    .execute(
                        "INSERT INTO kpi_ingest_runs
                            (id, report_document_id, company_id, profile_version, status)
                         VALUES (?1, 'doc1', 'c1', 'gpw_ifrs_annual@v1', ?2)",
                        rusqlite::params![id, status],
                    )
                    .expect("run row");
            }
        }
        for id in [
            "kpiing_00000000000000000000000000000001",
            "kpiing_00000000000000000000000000000002",
            "kpiing_00000000000000000000000000000003",
        ] {
            assert_eq!(
                failure_code(document_chunk(
                    &state,
                    json!({ "runId": id, "offset": 0, "length": 1 })
                )),
                CommandErrorCode::Conflict,
                "{id}: no captured source → conflict"
            );
        }

        // A terminal run WITH a pinned source stays readable.
        let run_id = started_run(&state);
        success(acquisition_call(
            &state,
            "cancel_kpi_ingest",
            &json!({ "runId": run_id }),
        ));
        success(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 0, "length": 8 }),
        ));

        // A `discovered` run that somehow carries a hash is a broken invariant.
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET status = 'discovered' WHERE id = ?1",
                    [&run_id],
                )
                .expect("force discovered");
        }
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": run_id, "offset": 0, "length": 1 })
            )),
            CommandErrorCode::Internal
        );
    }

    #[test]
    fn corrupt_missing_or_malformed_blobs_are_internal() {
        let state = test_state();
        let run_id = started_run(&state);
        let hash = doc_hash();
        let blob_path = state.data_dir().join(SNAPSHOT_DIR).join(&hash);

        // Corrupt the blob (content no longer matches the frozen hash).
        std::fs::write(&blob_path, b"tampered bytes").expect("corrupt");
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": run_id, "offset": 0, "length": 1 })
            )),
            CommandErrorCode::Internal,
            "hash mismatch"
        );

        // Remove it entirely.
        std::fs::remove_file(&blob_path).expect("remove");
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": run_id, "offset": 0, "length": 1 })
            )),
            CommandErrorCode::Internal,
            "missing blob"
        );

        // Malformed stored hash: validated BEFORE any path is built.
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "UPDATE kpi_ingest_runs SET source_content_hash = 'XYZ' WHERE id = ?1",
                    [&run_id],
                )
                .expect("malform hash");
        }
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": run_id, "offset": 0, "length": 1 })
            )),
            CommandErrorCode::Internal,
            "malformed stored hash"
        );
    }

    #[test]
    fn a_second_data_dir_never_borrows_anothers_verification() {
        // Two states, same document bytes → same blob NAME in two data dirs.
        // Verify in the first; corrupt the second's blob to the SAME SIZE —
        // the canonical-path cache key forces a fresh verification → internal.
        let state_a = test_state();
        let run_a = started_run(&state_a);
        success(document_chunk(
            &state_a,
            json!({ "runId": run_a, "offset": 0, "length": 4 }),
        ));

        let state_b = test_state();
        let run_b = started_run(&state_b);
        let hash = doc_hash();
        let blob_b = state_b.data_dir().join(SNAPSHOT_DIR).join(&hash);
        let mut corrupt = DOC_BYTES.to_vec();
        corrupt[0] ^= 0xFF; // same size, different content
        std::fs::write(&blob_b, &corrupt).expect("corrupt same-size");
        assert_eq!(
            failure_code(document_chunk(
                &state_b,
                json!({ "runId": run_b, "offset": 0, "length": 4 })
            )),
            CommandErrorCode::Internal,
            "a different data dir's blob is verified on its own"
        );
    }
}
