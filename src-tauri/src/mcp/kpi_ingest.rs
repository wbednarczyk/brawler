//! Acquisition-workflow MCP tools (ADR 0099, #384): the run-lifecycle four —
//! `start_kpi_ingest` (fresh/resume/keepalive), `list_pending_kpi_ingests`,
//! `get_kpi_ingest_status`, `cancel_kpi_ingest` — plus the content-addressed
//! source-blob pin (`report_snapshots/{sha256}`, ADR 0099 dec. 5).
//!
//! Leases are CREDENTIAL-owned (ADR 0099 dec. 4): the holder string derives
//! from the authenticated scope (`mcp:kpi_acquisition` / `mcp:full`), never an
//! agent session — two agents sharing one token are indistinguishable and
//! silently keepalive each other's runs (accepted: single-instance app, one
//! owner). The lease disambiguates SCOPES; takeover after expiry surfaces the
//! frozen `run_taken_over` remedy.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::registry::McpScope;
use super::tools::{run, ToolCallError, ToolOutcome};
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage::{
    is_registered_profile_version, resolve_profile_version, AppState, KpiIngestRun,
    KpiIngestRunState, NewKpiIngestRun, ReportDocument,
};

/// The credential-scoped lease TTL (ADR 0099 dec. 4): generous for an agent
/// turn, short enough that an abandoned run frees within the hour. A
/// constant, not a correctness mechanism — expiry classification handles the
/// rest.
const RUN_LEASE_SECONDS: i64 = 1800;

/// `kpi_ingest_runs.instruction_version` stamped at `begin_extracting`: the
/// version of the ACQUISITION ORCHESTRATION CONTRACT (the frozen ADR 0099
/// surface), not of any agent skill — the start input carries no skill
/// version and the server cannot know one. The #387 skill records its own
/// version in the run's execution metadata instead.
const ACQUISITION_CONTRACT_VERSION: &str = "acquisition-mcp@v1";

const SNAPSHOT_DIR: &str = "report_snapshots";

// ============================================================================
// Inputs
// ============================================================================

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunScopeInput {
    Standalone,
    Consolidated,
}

impl RunScopeInput {
    fn as_str(self) -> &'static str {
        match self {
            RunScopeInput::Standalone => "standalone",
            RunScopeInput::Consolidated => "consolidated",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunDataQualityInput {
    Final,
    Preliminary,
    Estimated,
}

impl RunDataQualityInput {
    fn as_str(self) -> &'static str {
        match self {
            RunDataQualityInput::Final => "final",
            RunDataQualityInput::Preliminary => "preliminary",
            RunDataQualityInput::Estimated => "estimated",
        }
    }
}

/// The START period vocabulary (contracts.md § Planned): `Q1|H1|9M|FY` only —
/// the validator refuses everything else via `run.unsupported_period_grammar`,
/// so a run doomed to fail cannot start. The schema IS the enforcement.
#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
pub enum StartPeriodType {
    Q1,
    H1,
    #[serde(rename = "9M")]
    NineMonths,
    FY,
}

impl StartPeriodType {
    fn as_str(self) -> &'static str {
        match self {
            StartPeriodType::Q1 => "Q1",
            StartPeriodType::H1 => "H1",
            StartPeriodType::NineMonths => "9M",
            StartPeriodType::FY => "FY",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartPeriodInput {
    pub fiscal_year: i64,
    pub period_type: StartPeriodType,
}

/// Fresh variant: `documentId` + `profileId` (+ optional context). Resume /
/// keepalive variant: `runId` (+ optional context). Exactly one variant —
/// enforced in the handler (`invalid_input`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StartKpiIngestInput {
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub profile_id: Option<String>,
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub scope: Option<RunScopeInput>,
    #[serde(default)]
    pub data_quality: Option<RunDataQualityInput>,
    #[serde(default)]
    pub period: Option<StartPeriodInput>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ListPendingKpiIngestsInput {
    #[serde(default)]
    pub company_id: Option<String>,
    #[serde(default)]
    #[schemars(range(min = 1, max = 50))]
    pub limit: Option<i64>,
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RunIdInput {
    pub run_id: String,
}

// ============================================================================
// Wire DTOs (MCP-only — no TS consumer, no ts_rs; contracts.md § Planned)
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodDto {
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_id: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaseDto {
    pub holder: String,
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunStatusDto {
    pub run_id: String,
    pub status: String,
    pub revision: i64,
    pub manifest_hash: Option<String>,
    pub document_id: String,
    pub company_id: String,
    pub profile_version: String,
    pub scope: Option<String>,
    pub data_quality: Option<String>,
    pub period: Option<PeriodDto>,
    pub source_content_hash: Option<String>,
    pub attempt_count: i64,
    pub expected_kpis: Option<Value>,
    pub missing_reasons: Value,
    pub lease: Option<LeaseDto>,
    pub progress: Option<Value>,
    pub execution: Option<Value>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeaseSummaryDto {
    pub expires_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummaryDto {
    pub run_id: String,
    pub document_id: String,
    pub company_id: String,
    pub status: String,
    pub profile_version: String,
    pub period: Option<PeriodDto>,
    pub attempt_count: i64,
    pub lease: Option<LeaseSummaryDto>,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPendingDto {
    pub items: Vec<RunSummaryDto>,
    pub next_cursor: Option<String>,
}

// ============================================================================
// Shared helpers
// ============================================================================

/// The lease holder derives from the CREDENTIAL, never an agent session
/// (ADR 0099 dec. 4).
fn holder(scope: McpScope) -> &'static str {
    match scope {
        McpScope::Full => "mcp:full",
        McpScope::KpiAcquisition => "mcp:kpi_acquisition",
    }
}

/// Control characters are rejected in every text field (contracts.md § Planned
/// conventions) — this bounds JSON-escaping expansion at ×2.
fn reject_control_chars(field: &'static str, value: &str) -> Result<(), CommandError> {
    if value.chars().any(|c| c < '\u{20}') {
        return Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            format!("{field} must not contain control characters"),
        ));
    }
    Ok(())
}

/// Resolves a `period_id`-only pin's natural key so the wire `period` object
/// is populated whenever the run HAS a period identity (contracts shape).
fn period_dto(state: &AppState, run: &KpiIngestRun) -> Result<Option<PeriodDto>, CommandError> {
    if let (Some(fiscal_year), Some(period_type)) =
        (run.period_fiscal_year, run.period_type.as_deref())
    {
        return Ok(Some(PeriodDto {
            fiscal_year,
            period_type: period_type.to_owned(),
            period_id: run.period_id.clone(),
        }));
    }
    if let Some(period_id) = run.period_id.as_deref() {
        let period = state.financials().get_financial_period(period_id)?;
        return Ok(Some(PeriodDto {
            fiscal_year: period.fiscal_year,
            period_type: period.period_type,
            period_id: Some(period.id),
        }));
    }
    Ok(None)
}

/// Parses a stored JSON column; a NULL is `None`, malformed bytes are a typed
/// `internal` (never a silent null).
fn parse_stored_json(
    column: &'static str,
    stored: Option<&str>,
) -> Result<Option<Value>, CommandError> {
    match stored {
        None => Ok(None),
        Some(raw) => serde_json::from_str(raw).map(Some).map_err(|error| {
            CommandError::new(
                CommandErrorCode::Internal,
                format!("stored {column} is malformed JSON: {error}"),
            )
        }),
    }
}

fn run_status_dto(state: &AppState, run: &KpiIngestRun) -> Result<RunStatusDto, CommandError> {
    // `missingReasons` normalizes stored NULL to {}; a stored non-object is
    // corrupt (the schema is an object ledger).
    let missing_reasons =
        match parse_stored_json("missing_reasons_json", run.missing_reasons_json.as_deref())? {
            None => serde_json::json!({}),
            Some(value @ Value::Object(_)) => value,
            Some(_) => {
                return Err(CommandError::new(
                    CommandErrorCode::Internal,
                    "stored missing_reasons_json is not a JSON object",
                ))
            }
        };
    let expected_kpis =
        match parse_stored_json("expected_kpis_json", run.expected_kpis_json.as_deref())? {
            Some(value @ Value::Object(_)) => Some(value),
            None => None,
            Some(_) => {
                return Err(CommandError::new(
                    CommandErrorCode::Internal,
                    "stored expected_kpis_json is not a JSON object",
                ))
            }
        };
    Ok(RunStatusDto {
        run_id: run.id.clone(),
        status: run.status.as_str().to_owned(),
        revision: run.manifest_revision,
        manifest_hash: run.manifest_hash.clone(),
        document_id: run.report_document_id.clone(),
        company_id: run.company_id.clone(),
        profile_version: run.profile_version.clone(),
        scope: run.scope.clone(),
        data_quality: run.data_quality.clone(),
        period: period_dto(state, run)?,
        source_content_hash: run.source_content_hash.clone(),
        attempt_count: run.attempt_count,
        expected_kpis,
        missing_reasons,
        lease: match (run.lease_holder.clone(), run.lease_expires_at.clone()) {
            (Some(holder), Some(expires_at)) => Some(LeaseDto { holder, expires_at }),
            _ => None,
        },
        progress: parse_stored_json("progress_json", run.progress_json.as_deref())?,
        execution: parse_stored_json("cost_json", run.cost_json.as_deref())?,
        last_error: run.last_error.clone(),
        created_at: run.created_at.clone(),
        updated_at: run.updated_at.clone(),
    })
}

fn run_summary_dto(state: &AppState, run: &KpiIngestRun) -> Result<RunSummaryDto, CommandError> {
    Ok(RunSummaryDto {
        run_id: run.id.clone(),
        document_id: run.report_document_id.clone(),
        company_id: run.company_id.clone(),
        status: run.status.as_str().to_owned(),
        profile_version: run.profile_version.clone(),
        period: period_dto(state, run)?,
        attempt_count: run.attempt_count,
        lease: run
            .lease_expires_at
            .clone()
            .map(|expires_at| LeaseSummaryDto { expires_at }),
        created_at: run.created_at.clone(),
    })
}

fn get_existing_run(state: &AppState, run_id: &str) -> Result<KpiIngestRun, CommandError> {
    state.kpi_ingest_runs().get_run(run_id)?.ok_or_else(|| {
        CommandError::new(
            CommandErrorCode::NotFound,
            format!("kpi ingest run not found: {run_id}"),
        )
    })
}

// ============================================================================
// Source-blob pin (ADR 0099 dec. 5)
// ============================================================================

/// Per-invocation staged-name uniqueness: two concurrent starts of the same
/// run (one shared credential holder) must never write the same staged file.
static STAGED_SEQ: AtomicU64 = AtomicU64::new(0);

/// Copies the document's CURRENT bytes into the content-addressed blob store
/// and returns their hash: read to memory → hash the buffer → write a unique
/// staged sibling → atomically rename to `report_snapshots/{sha256}`. The blob
/// is byte-for-byte the hashed buffer, so a concurrent recapture overwriting
/// `local_path` mid-read can at worst change WHICH bytes were pinned — never
/// the name↔content consistency. An existing target wins immediately
/// (content-addressed dedup; also the idempotent re-pin). Crash orphans
/// (`*.staged-*`, or a blob renamed but never marked on the run) are
/// unreferenced and harmless — blob GC does not exist (data-model.md).
fn pin_source_blob(state: &AppState, document: &ReportDocument) -> Result<String, CommandError> {
    let unavailable = |reason: &str| {
        CommandError::new(
            CommandErrorCode::InvalidInput,
            format!(
                "document {} has no local file to pin ({reason}) — capture it first",
                document.id
            ),
        )
    };
    if document.fetch_status != "fetched" {
        return Err(unavailable(&format!(
            "fetch_status={}",
            document.fetch_status
        )));
    }
    let Some(local_path) = document
        .local_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    else {
        return Err(unavailable("no local_path"));
    };
    let source = state.data_dir().join(local_path);
    if !source.is_file() {
        return Err(unavailable("file missing on disk"));
    }
    let bytes = std::fs::read(&source).map_err(|error| {
        CommandError::new(
            CommandErrorCode::Internal,
            format!("reading document {} failed: {error}", document.id),
        )
    })?;
    let hash = crate::report_documents_capture::content_hash_hex(&bytes);

    let snapshot_dir = state.data_dir().join(SNAPSHOT_DIR);
    std::fs::create_dir_all(&snapshot_dir).map_err(|error| {
        CommandError::new(
            CommandErrorCode::Internal,
            format!("creating {SNAPSHOT_DIR} failed: {error}"),
        )
    })?;
    let target = snapshot_dir.join(&hash);
    if target.exists() {
        return Ok(hash);
    }
    let staged: PathBuf = snapshot_dir.join(format!(
        "{hash}.staged-{}-{}",
        std::process::id(),
        STAGED_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let publish = std::fs::write(&staged, &bytes).and_then(|()| std::fs::rename(&staged, &target));
    if let Err(error) = publish {
        // A concurrent pin may have published the same content first — the
        // target existing makes this call a success; otherwise fail typed.
        let _ = std::fs::remove_file(&staged);
        if !target.exists() {
            return Err(CommandError::new(
                CommandErrorCode::Internal,
                format!("publishing source blob failed: {error}"),
            ));
        }
    }
    Ok(hash)
}

// ============================================================================
// Tools
// ============================================================================

fn start_kpi_ingest(
    state: &AppState,
    scope: McpScope,
    input: StartKpiIngestInput,
) -> Result<RunStatusDto, CommandError> {
    for (field, value) in [
        ("documentId", input.document_id.as_deref()),
        ("profileId", input.profile_id.as_deref()),
        ("runId", input.run_id.as_deref()),
    ] {
        if let Some(value) = value {
            reject_control_chars(field, value)?;
        }
    }
    let holder = holder(scope);
    let store = state.kpi_ingest_runs();

    // Variant exclusivity: runId XOR (documentId + profileId).
    let run = match (&input.run_id, &input.document_id, &input.profile_id) {
        (Some(run_id), None, None) => {
            let run = get_existing_run(state, run_id)?;
            if !is_registered_profile_version(&run.profile_version) {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    format!(
                        "run {run_id} carries profile_version '{}' outside the registry — \
                         cancel it and start a fresh run (ADR 0099 dec. 6)",
                        run.profile_version
                    ),
                ));
            }
            run
        }
        (None, Some(document_id), Some(profile_id)) => {
            let Some(profile_version) = resolve_profile_version(profile_id) else {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    format!("profileId '{profile_id}' is outside the extraction-profile registry"),
                ));
            };
            let document = match state.get_report_document(document_id) {
                Ok(document) => document,
                // The raw store read has no Option seam — a no-rows miss is
                // this tool's typed not_found, never an internal error.
                Err(crate::storage::StorageError::Sqlite(rusqlite::Error::QueryReturnedNoRows)) => {
                    return Err(CommandError::new(
                        CommandErrorCode::NotFound,
                        format!("report document not found: {document_id}"),
                    ))
                }
                Err(error) => return Err(error.into()),
            };
            store.create_run_if_absent(&NewKpiIngestRun {
                report_document_id: document.id.clone(),
                company_id: document.company_id.clone(),
                period_id: None,
                profile_version: profile_version.to_owned(),
                scope: input.scope.map(|s| s.as_str().to_owned()),
                data_quality: input.data_quality.map(|q| q.as_str().to_owned()),
                period_fiscal_year: input.period.map(|p| p.fiscal_year),
                period_type: input.period.map(|p| p.period_type.as_str().to_owned()),
            })?
        }
        _ => {
            return Err(CommandError::new(
                CommandErrorCode::InvalidInput,
                "provide either runId (resume/keepalive) or documentId + profileId (fresh), \
                 never a mix",
            ));
        }
    };

    // Claim (takeover/keepalive semantics live in the store primitive).
    let run = store.claim_run(&run.id, holder, RUN_LEASE_SECONDS)?;

    // Set-once context attach — all-None is a no-op.
    store.attach_run_context(
        &run.id,
        holder,
        &crate::storage::RunContextAttach {
            scope: input.scope.map(RunScopeInput::as_str),
            data_quality: input.data_quality.map(RunDataQualityInput::as_str),
            period: input
                .period
                .map(|p| (p.fiscal_year, p.period_type.as_str())),
        },
    )?;

    // Pin the source bytes exactly once, at `discovered`.
    if run.status == KpiIngestRunState::Discovered {
        let document = state.get_report_document(&run.report_document_id)?;
        let hash = pin_source_blob(state, &document)?;
        store.mark_source_captured(&run.id, holder, &hash)?;
    }

    // Enter extraction when the context is complete.
    let run = get_existing_run(state, &run.id)?;
    if run.status == KpiIngestRunState::SourceCaptured
        && run.scope.is_some()
        && run.data_quality.is_some()
        && (run.period_id.is_some()
            || (run.period_fiscal_year.is_some() && run.period_type.is_some()))
    {
        store.begin_extracting(&run.id, holder, ACQUISITION_CONTRACT_VERSION)?;
    }

    let run = get_existing_run(state, &run.id)?;
    run_status_dto(state, &run)
}

const PENDING_LIMIT_DEFAULT: i64 = 20;
const PENDING_LIMIT_MAX: i64 = 50;

fn list_pending_kpi_ingests(
    state: &AppState,
    input: ListPendingKpiIngestsInput,
) -> Result<ListPendingDto, CommandError> {
    if let Some(company_id) = input.company_id.as_deref() {
        reject_control_chars("companyId", company_id)?;
    }
    if let Some(cursor) = input.cursor.as_deref() {
        reject_control_chars("cursor", cursor)?;
    }
    let limit = input.limit.unwrap_or(PENDING_LIMIT_DEFAULT);
    if !(1..=PENDING_LIMIT_MAX).contains(&limit) {
        return Err(CommandError::new(
            CommandErrorCode::ResponseBudgetExceeded,
            format!("limit must be within 1..={PENDING_LIMIT_MAX} (got {limit})"),
        ));
    }
    let cursor = input.cursor.as_deref().map(decode_cursor).transpose()?;

    let runs = state.kpi_ingest_runs().list_pending_runs(
        input.company_id.as_deref(),
        cursor
            .as_ref()
            .map(|(created_at, id)| (created_at.as_str(), id.as_str())),
        (limit + 1) as usize,
    )?;
    let has_more = runs.len() as i64 > limit;
    let page = &runs[..runs.len().min(limit as usize)];
    let next_cursor = if has_more {
        page.last()
            .map(|run| format!("{}|{}", run.created_at, run.id))
    } else {
        None
    };
    Ok(ListPendingDto {
        items: page
            .iter()
            .map(|run| run_summary_dto(state, run))
            .collect::<Result<_, _>>()?,
        next_cursor,
    })
}

/// The opaque keyset cursor: `{created_at}|{run_id}`, exactly two non-empty
/// components with the generated grammar (ISO-8601Z millisecond timestamp,
/// `kpiing_` id). Tampering only moves the page window.
fn decode_cursor(cursor: &str) -> Result<(String, String), CommandError> {
    let malformed = || {
        CommandError::new(
            CommandErrorCode::InvalidInput,
            "malformed cursor — pass the nextCursor value verbatim",
        )
    };
    let (created_at, id) = cursor.split_once('|').ok_or_else(malformed)?;
    if created_at.is_empty()
        || id.is_empty()
        || id.contains('|')
        || !created_at.ends_with('Z')
        || created_at.len() < 20
        || !created_at.as_bytes()[..4].iter().all(u8::is_ascii_digit)
        || !id.starts_with("kpiing_")
    {
        return Err(malformed());
    }
    Ok((created_at.to_owned(), id.to_owned()))
}

fn get_kpi_ingest_status(
    state: &AppState,
    input: RunIdInput,
) -> Result<RunStatusDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    let run = get_existing_run(state, &input.run_id)?;
    run_status_dto(state, &run)
}

fn cancel_kpi_ingest(state: &AppState, input: RunIdInput) -> Result<RunStatusDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    state.kpi_ingest_runs().cancel_run(&input.run_id)?;
    let run = get_existing_run(state, &input.run_id)?;
    run_status_dto(state, &run)
}

// ============================================================================
// Registry handlers
// ============================================================================

pub fn start_kpi_ingest_handler(
    state: &AppState,
    scope: McpScope,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| start_kpi_ingest(state, scope, input))
}

pub fn list_pending_kpi_ingests_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| list_pending_kpi_ingests(state, input))
}

pub fn get_kpi_ingest_status_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_kpi_ingest_status(state, input))
}

pub fn cancel_kpi_ingest_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| cancel_kpi_ingest(state, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::registry::call;
    use crate::storage::{open_in_memory_database, SettingsUpdate};
    use serde_json::json;

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    const DOC_BYTES: &[u8] = b"%PDF-1.4 test report bytes v1";

    /// State with a real (per-test, throwaway) data dir, one company and one
    /// FETCHED document whose bytes exist on disk — the substrate every
    /// lifecycle tool runs against.
    fn test_state() -> AppState {
        let dir = std::env::temp_dir().join(format!(
            "brawler-kpi-mcp-{}-{}",
            std::process::id(),
            TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(dir.join("report_documents")).expect("mkdir");
        std::fs::write(dir.join("report_documents/doc1.pdf"), DOC_BYTES).expect("doc bytes");
        let connection = open_in_memory_database().expect("db");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'TST', 'GPW:TST', 'Test SA')",
                [],
            )
            .expect("company");
        connection
            .execute(
                "INSERT INTO report_documents
                    (id, company_id, source_type, url, fetch_status, local_path)
                 VALUES ('doc1', 'c1', 'espi_attachment', 'https://x/doc1.pdf', 'fetched',
                         'report_documents/doc1.pdf')",
                [],
            )
            .expect("document");
        AppState::with_data_dir(connection, dir)
    }

    fn seed_document_row(state: &AppState, id: &str, local_path: Option<&str>, status: &str) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "INSERT INTO report_documents
                    (id, company_id, source_type, url, fetch_status, local_path)
                 VALUES (?1, 'c1', 'espi_attachment', ?2, ?3, ?4)",
                rusqlite::params![id, format!("https://x/{id}"), status, local_path],
            )
            .expect("document row");
    }

    fn success(outcome: ToolOutcome) -> Value {
        match outcome {
            ToolOutcome::Success(value) => value,
            ToolOutcome::Failure(error) => panic!("expected Success, got Failure: {error:?}"),
        }
    }

    fn failure_code(outcome: ToolOutcome) -> CommandErrorCode {
        match outcome {
            ToolOutcome::Failure(error) => error.code,
            ToolOutcome::Success(value) => panic!("expected Failure, got Success: {value}"),
        }
    }

    fn acquisition_call(state: &AppState, name: &str, args: &Value) -> ToolOutcome {
        call(state, McpScope::KpiAcquisition, name, args).expect("domain outcome")
    }

    fn full_start_args() -> Value {
        json!({
            "documentId": "doc1",
            "profileId": "gpw_ifrs_annual",
            "scope": "standalone",
            "dataQuality": "final",
            "period": { "fiscalYear": 2025, "periodType": "FY" }
        })
    }

    fn expire_lease_raw(state: &AppState, run_id: &str) {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET lease_expires_at = \
                 strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 seconds') WHERE id = ?1",
                [run_id],
            )
            .expect("expire");
    }

    #[test]
    fn a_fresh_start_with_full_context_reaches_extracting_and_pins_the_blob() {
        let state = test_state();
        let payload = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));

        assert_eq!(payload["status"], "extracting");
        assert_eq!(payload["profileVersion"], "gpw_ifrs_annual@v1");
        assert_eq!(payload["scope"], "standalone");
        assert_eq!(payload["dataQuality"], "final");
        assert_eq!(payload["period"]["fiscalYear"], 2025);
        assert_eq!(payload["period"]["periodType"], "FY");
        assert_eq!(payload["attemptCount"], 1);
        assert_eq!(payload["lease"]["holder"], "mcp:kpi_acquisition");
        assert_eq!(payload["missingReasons"], json!({}));
        assert!(payload["expectedKpis"]["packVersion"].is_string());

        let expected_hash = crate::report_documents_capture::content_hash_hex(DOC_BYTES);
        assert_eq!(payload["sourceContentHash"], expected_hash.as_str());
        let blob = state.data_dir().join(SNAPSHOT_DIR).join(&expected_hash);
        assert_eq!(
            std::fs::read(&blob).expect("blob exists"),
            DOC_BYTES,
            "the blob is byte-for-byte the hashed content"
        );
    }

    #[test]
    fn a_two_phase_start_stops_at_source_captured_then_resume_completes() {
        let state = test_state();
        let partial = json!({ "documentId": "doc1", "profileId": "gpw_ifrs_annual",
                              "scope": "standalone", "dataQuality": "final" });
        let payload = success(acquisition_call(&state, "start_kpi_ingest", &partial));
        assert_eq!(payload["status"], "source_captured", "no period yet");
        assert!(payload["period"].is_null());
        let run_id = payload["runId"].as_str().expect("runId").to_owned();

        let resume = json!({ "runId": run_id,
                             "period": { "fiscalYear": 2025, "periodType": "FY" } });
        let payload = success(acquisition_call(&state, "start_kpi_ingest", &resume));
        assert_eq!(
            payload["status"], "extracting",
            "attach completed the context"
        );
        assert_eq!(
            payload["attemptCount"], 1,
            "same-holder live resume never increments"
        );
    }

    #[test]
    fn a_resume_with_a_conflicting_context_value_is_a_conflict() {
        let state = test_state();
        let payload = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        let run_id = payload["runId"].as_str().expect("runId").to_owned();

        let conflicting = json!({ "runId": run_id, "scope": "consolidated" });
        assert_eq!(
            failure_code(acquisition_call(&state, "start_kpi_ingest", &conflicting)),
            CommandErrorCode::Conflict
        );
    }

    #[test]
    fn a_bare_run_id_is_a_pure_keepalive() {
        let state = test_state();
        let payload = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        let run_id = payload["runId"].as_str().expect("runId").to_owned();
        let first_expiry = payload["lease"]["expiresAt"]
            .as_str()
            .expect("expiry")
            .to_owned();

        let keepalive = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &json!({ "runId": run_id }),
        ));
        assert_eq!(keepalive["attemptCount"], 1, "keepalive never increments");
        assert_eq!(keepalive["status"], "extracting", "state untouched");
        assert!(
            keepalive["lease"]["expiresAt"].as_str().expect("expiry") >= first_expiry.as_str(),
            "lease extended (or equal within the same millisecond)"
        );
    }

    /// The full 3-field presence matrix: only `runId`-only and
    /// `documentId+profileId` are legal shapes.
    #[test]
    fn the_variant_exclusivity_matrix_refuses_every_mixed_shape() {
        let state = test_state();
        // A real run so the legal resume combination succeeds.
        let created = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        let run_id = created["runId"].as_str().expect("runId").to_owned();

        for (with_run, with_doc, with_profile) in [
            (false, false, false),
            (false, false, true),
            (false, true, false),
            (true, false, true),
            (true, true, false),
            (true, true, true),
        ] {
            let mut args = serde_json::Map::new();
            if with_run {
                args.insert("runId".into(), json!(run_id));
            }
            if with_doc {
                args.insert("documentId".into(), json!("doc1"));
            }
            if with_profile {
                args.insert("profileId".into(), json!("gpw_ifrs_annual"));
            }
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "start_kpi_ingest",
                    &Value::Object(args.clone())
                )),
                CommandErrorCode::InvalidInput,
                "combination run={with_run} doc={with_doc} profile={with_profile}"
            );
        }
        // The two legal shapes succeed (fresh is the idempotent re-start).
        success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &json!({ "runId": run_id }),
        ));
        success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
    }

    #[test]
    fn control_characters_are_rejected_in_every_text_field() {
        let state = test_state();
        let bad = "bad\u{1}value";
        for args in [
            json!({ "documentId": bad, "profileId": "gpw_ifrs_annual" }),
            json!({ "documentId": "doc1", "profileId": bad }),
            json!({ "runId": bad }),
        ] {
            assert_eq!(
                failure_code(acquisition_call(&state, "start_kpi_ingest", &args)),
                CommandErrorCode::InvalidInput,
                "start args {args}"
            );
        }
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "list_pending_kpi_ingests",
                &json!({ "companyId": bad })
            )),
            CommandErrorCode::InvalidInput
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "list_pending_kpi_ingests",
                &json!({ "cursor": bad })
            )),
            CommandErrorCode::InvalidInput
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "get_kpi_ingest_status",
                &json!({ "runId": bad })
            )),
            CommandErrorCode::InvalidInput
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "cancel_kpi_ingest",
                &json!({ "runId": bad })
            )),
            CommandErrorCode::InvalidInput
        );
    }

    #[test]
    fn document_and_profile_refusals_are_typed() {
        let state = test_state();
        seed_document_row(&state, "doc-meta", None, "metadata_only");
        seed_document_row(&state, "doc-blank", Some("  "), "fetched");
        seed_document_row(
            &state,
            "doc-ghost",
            Some("report_documents/ghost.pdf"),
            "fetched",
        );

        let start = |doc: &str| {
            failure_code(acquisition_call(
                &state,
                "start_kpi_ingest",
                &json!({ "documentId": doc, "profileId": "gpw_ifrs_annual" }),
            ))
        };
        assert_eq!(
            start("doc-missing"),
            CommandErrorCode::NotFound,
            "unknown document"
        );
        assert_eq!(
            start("doc-meta"),
            CommandErrorCode::InvalidInput,
            "metadata_only"
        );
        assert_eq!(
            start("doc-blank"),
            CommandErrorCode::InvalidInput,
            "blank local_path"
        );
        assert_eq!(
            start("doc-ghost"),
            CommandErrorCode::InvalidInput,
            "file missing on disk"
        );
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "start_kpi_ingest",
                &json!({ "documentId": "doc1", "profileId": "not_a_profile" })
            )),
            CommandErrorCode::InvalidInput,
            "unknown profile id"
        );

        // A raw-seeded legacy run outside the registry refuses at resume.
        {
            let connection = state.checkout_for_tests().expect("raw");
            connection
                .execute(
                    "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id,
                        profile_version, status)
                     VALUES ('kpiing_legacy', 'doc1', 'c1', 'p1', 'discovered')",
                    [],
                )
                .expect("legacy run");
        }
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "start_kpi_ingest",
                &json!({ "runId": "kpiing_legacy" })
            )),
            CommandErrorCode::InvalidInput,
            "legacy profile refusal (ADR 0099 dec. 6)"
        );
    }

    #[test]
    fn a_takeover_after_expiry_succeeds_and_the_ousted_holder_is_told() {
        let state = test_state();
        // The Full credential starts the run (writes gate ON for Full acts).
        state
            .update_settings(SettingsUpdate {
                mcp_writes_enabled: Some(true),
                ..Default::default()
            })
            .expect("enable writes");
        let payload = success(
            call(
                &state,
                McpScope::Full,
                "start_kpi_ingest",
                &full_start_args(),
            )
            .expect("domain outcome"),
        );
        assert_eq!(payload["lease"]["holder"], "mcp:full");
        let run_id = payload["runId"].as_str().expect("runId").to_owned();

        expire_lease_raw(&state, &run_id);

        // The acquisition credential takes over after expiry.
        let taken = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &json!({ "runId": run_id }),
        ));
        assert_eq!(taken["lease"]["holder"], "mcp:kpi_acquisition");
        assert_eq!(taken["attemptCount"], 2, "takeover counts as a new attempt");

        // The ousted holder gets the frozen non-retryable remedy.
        assert_eq!(
            failure_code(
                call(
                    &state,
                    McpScope::Full,
                    "start_kpi_ingest",
                    &json!({ "runId": run_id })
                )
                .expect("domain outcome")
            ),
            CommandErrorCode::RunTakenOver
        );
    }

    #[test]
    fn a_recapture_never_touches_the_first_runs_blob() {
        let state = test_state();
        let first = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        let first_hash = first["sourceContentHash"]
            .as_str()
            .expect("hash")
            .to_owned();
        let run_id = first["runId"].as_str().expect("runId").to_owned();
        success(acquisition_call(
            &state,
            "cancel_kpi_ingest",
            &json!({ "runId": run_id }),
        ));

        // Recapture overwrites local_path in place; a NEW run pins NEW bytes.
        let new_bytes: &[u8] = b"%PDF-1.4 recaptured bytes v2";
        std::fs::write(
            state.data_dir().join("report_documents/doc1.pdf"),
            new_bytes,
        )
        .expect("recapture");
        let second = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        let second_hash = second["sourceContentHash"]
            .as_str()
            .expect("hash")
            .to_owned();
        assert_ne!(
            second["runId"], first["runId"],
            "terminal run frees the triple"
        );
        assert_ne!(second_hash, first_hash);

        // Both blobs coexist; the first is untouched.
        let snapshots = state.data_dir().join(SNAPSHOT_DIR);
        assert_eq!(
            std::fs::read(snapshots.join(&first_hash)).expect("first blob"),
            DOC_BYTES
        );
        assert_eq!(
            std::fs::read(snapshots.join(&second_hash)).expect("second blob"),
            new_bytes
        );
    }

    #[test]
    fn list_pending_paginates_and_refuses_bad_limits_and_cursors() {
        let state = test_state();
        // Three claimable runs over three documents.
        for doc in ["doc2", "doc3"] {
            let path = format!("report_documents/{doc}.pdf");
            std::fs::write(state.data_dir().join(&path), doc.as_bytes()).expect("bytes");
            seed_document_row(&state, doc, Some(&path), "fetched");
        }
        for doc in ["doc1", "doc2", "doc3"] {
            success(acquisition_call(
                &state,
                "start_kpi_ingest",
                &json!({ "documentId": doc, "profileId": "gpw_ifrs_annual" }),
            ));
        }

        let page1 = success(acquisition_call(
            &state,
            "list_pending_kpi_ingests",
            &json!({ "limit": 2 }),
        ));
        assert_eq!(page1["items"].as_array().expect("items").len(), 2);
        let cursor = page1["nextCursor"].as_str().expect("more pages").to_owned();
        let page2 = success(acquisition_call(
            &state,
            "list_pending_kpi_ingests",
            &json!({ "limit": 2, "cursor": cursor }),
        ));
        assert_eq!(page2["items"].as_array().expect("items").len(), 1);
        assert!(page2["nextCursor"].is_null(), "the tail page");
        let summary = &page2["items"][0];
        assert!(summary["lease"]["expiresAt"].is_string());
        assert!(
            summary.get("lastError").is_none(),
            "summary is the compact shape"
        );

        for bad_limit in [0, 51] {
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "list_pending_kpi_ingests",
                    &json!({ "limit": bad_limit }),
                )),
                CommandErrorCode::ResponseBudgetExceeded,
                "limit {bad_limit} refuses, never clamps"
            );
        }
        for bad_cursor in ["garbage", "a|b|c", "|x", "2026-01-01T00:00:00.000Z|"] {
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "list_pending_kpi_ingests",
                    &json!({ "cursor": bad_cursor }),
                )),
                CommandErrorCode::InvalidInput,
                "cursor {bad_cursor}"
            );
        }
    }

    #[test]
    fn get_status_of_an_unknown_run_is_not_found() {
        let state = test_state();
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "get_kpi_ingest_status",
                &json!({ "runId": "kpiing_missing" })
            )),
            CommandErrorCode::NotFound
        );
    }

    /// The frozen wire shape of `RunStatus` (contracts.md § Planned) — every
    /// key, stable order, timestamps redacted. The run id is deterministic
    /// (a content hash of the identity triple), so it snapshots verbatim.
    #[test]
    fn run_status_golden_shape() {
        let state = test_state();
        let payload = success(acquisition_call(
            &state,
            "start_kpi_ingest",
            &full_start_args(),
        ));
        let pretty = serde_json::to_string_pretty(&payload).expect("serializable");
        let redacted = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z")
            .expect("regex")
            .replace_all(&pretty, "[timestamp]")
            .into_owned();
        let redacted = regex::Regex::new(r"kpiing_[0-9a-f]+")
            .expect("regex")
            .replace_all(&redacted, "kpiing_[uid]")
            .into_owned();
        insta::assert_snapshot!("run_status_wire_shape", redacted);
    }

    #[test]
    fn cancel_works_from_every_pre_commit_state_and_refuses_the_rest() {
        let state = test_state();
        let pre_commit = [
            "discovered",
            "source_captured",
            "extracting",
            "staged",
            "validation_failed",
            "ready_to_commit",
        ];
        let refused = ["committing", "complete", "partial", "failed", "cancelled"];
        {
            let connection = state.checkout_for_tests().expect("raw");
            for (index, status) in pre_commit.iter().chain(refused.iter()).enumerate() {
                let doc = format!("doc-s{index}");
                connection
                    .execute(
                        "INSERT INTO report_documents
                            (id, company_id, source_type, url, fetch_status)
                         VALUES (?1, 'c1', 'espi_attachment', ?1, 'fetched')",
                        [&doc],
                    )
                    .expect("doc row");
                connection
                    .execute(
                        "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id,
                            profile_version, status, lease_holder, lease_expires_at,
                            last_heartbeat_at)
                         VALUES (?1, ?2, 'c1', 'p1', ?3,
                                 CASE WHEN ?3 IN ('discovered','source_captured','extracting',
                                                  'staged','validation_failed')
                                      THEN 'mcp:full' END,
                                 CASE WHEN ?3 IN ('discovered','source_captured','extracting',
                                                  'staged','validation_failed')
                                      THEN strftime('%Y-%m-%dT%H:%M:%fZ','now','+1 hour') END,
                                 CASE WHEN ?3 IN ('discovered','source_captured','extracting',
                                                  'staged','validation_failed')
                                      THEN strftime('%Y-%m-%dT%H:%M:%fZ','now') END)",
                        rusqlite::params![format!("kpiing_s{index}"), doc, status],
                    )
                    .expect("run row");
            }
        }
        for (index, status) in pre_commit.iter().enumerate() {
            let payload = success(acquisition_call(
                &state,
                "cancel_kpi_ingest",
                &json!({ "runId": format!("kpiing_s{index}") }),
            ));
            assert_eq!(payload["status"], "cancelled", "cancel from {status}");
            assert!(payload["lease"].is_null(), "lease cleared from {status}");
        }
        for (offset, status) in refused.iter().enumerate() {
            let index = pre_commit.len() + offset;
            assert_eq!(
                failure_code(acquisition_call(
                    &state,
                    "cancel_kpi_ingest",
                    &json!({ "runId": format!("kpiing_s{index}") }),
                )),
                CommandErrorCode::Conflict,
                "cancel from {status} refuses"
            );
        }
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "cancel_kpi_ingest",
                &json!({ "runId": "kpiing_missing" }),
            )),
            CommandErrorCode::NotFound
        );
    }

    /// ADR 0099 dec. 2: the acquisition scope's gate ran at authentication —
    /// its act tools bypass `mcpWritesEnabled`; the SAME tool through the Full
    /// scope stays writes-gated.
    #[test]
    fn the_acquisition_scope_bypasses_the_full_writes_gate() {
        let state = test_state();
        // Writes OFF (the default): acquisition reaches the handler…
        assert_eq!(
            failure_code(acquisition_call(
                &state,
                "start_kpi_ingest",
                &json!({ "runId": "kpiing_missing" })
            )),
            CommandErrorCode::NotFound,
            "the handler ran (domain not_found), no writes_disabled gate"
        );
        // …while the Full scope is gated like every act tool.
        assert_eq!(
            failure_code(
                call(
                    &state,
                    McpScope::Full,
                    "start_kpi_ingest",
                    &json!({ "runId": "kpiing_missing" })
                )
                .expect("domain outcome")
            ),
            CommandErrorCode::WritesDisabled
        );
    }

    /// Two concurrent pins of the same document (one shared credential
    /// holder) publish exactly one blob whose bytes match the hash — the
    /// unique staged nonce makes the race safe.
    #[test]
    fn two_threads_pin_the_same_document_concurrently() {
        let state = test_state();
        let document = state.get_report_document("doc1").expect("doc");
        let hashes: Vec<String> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..2)
                .map(|_| scope.spawn(|| pin_source_blob(&state, &document).expect("pin")))
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("join"))
                .collect()
        });
        assert_eq!(hashes[0], hashes[1]);
        let snapshots = state.data_dir().join(SNAPSHOT_DIR);
        assert_eq!(
            std::fs::read(snapshots.join(&hashes[0])).expect("blob"),
            DOC_BYTES
        );
        let staged_leftovers = std::fs::read_dir(&snapshots)
            .expect("dir")
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".staged-"))
            .count();
        assert_eq!(staged_leftovers, 0, "no staged orphans on the happy path");
    }
}
