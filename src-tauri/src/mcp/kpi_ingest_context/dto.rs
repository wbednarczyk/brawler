//! Wire DTOs for [`super`]: tool inputs (`ContextSection`,
//! `GetKpiIngestContextInput`, `GetKpiIngestDocumentInput`) and the response
//! shapes they drive — all re-exported from the module root, external callers
//! use `kpi_ingest_context::…` paths, never this submodule directly.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::kpi_ingest::RunStatusDto;

// ============================================================================
// Inputs
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextSection {
    Catalog,
    Plausibility,
    Manifest,
    /// The full commit receipt (ADR 0102 dec. 12) — like `manifest`, NEVER in
    /// the default call; a `commit_kpi_ingest` v1000-row outcomes ledger
    /// cannot ride the tool's own bounded summary response.
    Receipt,
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
    pub(super) fn metric_key(&self) -> &str {
        match self {
            Self::Full { metric_key, .. } | Self::Compact { metric_key, .. } => metric_key,
        }
    }

    pub(super) fn definition_id(&self) -> &str {
        match self {
            Self::Full { definition_id, .. } | Self::Compact { definition_id, .. } => definition_id,
        }
    }

    /// Sort/pagination tier: `Full` (0) always precedes `Compact` (1) so the
    /// default call's truncated first page is dominated by the entries an
    /// agent needs immediately (ADR 0101 dec. 7).
    pub(super) fn tier(&self) -> u8 {
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt: Option<Value>,
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
