use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("connection pool error: {0}")]
    Pool(#[from] r2d2::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("invalid setting value for {key}: {value}")]
    InvalidSettingValue { key: &'static str, value: String },
    #[error("invalid notebook value for {key}: {value}")]
    InvalidNotebookValue { key: &'static str, value: String },
    #[error("invalid company event value for {key}: {value}")]
    InvalidCompanyEventValue { key: &'static str, value: String },
    #[error("invalid transcript value for {key}: {value}")]
    InvalidTranscriptValue { key: &'static str, value: String },
    #[error("invalid AI analysis value for {key}: {value}")]
    InvalidAiAnalysisValue { key: &'static str, value: String },
    #[error("invalid diagnostic value for {key}: {value}")]
    InvalidDiagnosticValue { key: &'static str, value: String },
    #[error("invalid license value for {key}: {value}")]
    InvalidLicenseValue { key: &'static str, value: String },
    #[error("invalid source value for {key}: {value}")]
    InvalidSourceValue { key: &'static str, value: String },
    #[error("invalid research value for {key}: {value}")]
    InvalidResearchValue { key: &'static str, value: String },
    #[error("missing research reference for {table}: {id}")]
    MissingResearchReference { table: String, id: String },
    #[error("invalid financials value for {key}: {value}")]
    InvalidFinancialsValue { key: &'static str, value: String },
    #[error(
        "fact {id} data_quality is immutable via update ({current} -> {requested}): data_quality is a uniqueness-slot dimension (ADR 0093) — create a new final fact instead so supersession records the change"
    )]
    FinancialFactDataQualityLocked {
        id: String,
        current: String,
        requested: String,
    },
    #[error("missing financials reference for {table}: {id}")]
    MissingFinancialsReference { table: String, id: String },
    /// `propose_kpi_definition`'s alias guard (ADR 0101 dec. 4, epic #399 S4):
    /// `requested_key` is a curated `kpi_aliases` source — redirect to the
    /// existing `canonical_key` definition instead of minting a near-duplicate.
    #[error(
        "metricKey \"{requested_key}\" is a curated synonym of \"{canonical_key}\" (kpi_aliases.rs) — reuse definition {definition_id} instead of proposing a duplicate"
    )]
    KpiDefinitionSynonymRedirect {
        requested_key: String,
        canonical_key: String,
        definition_id: String,
    },
    #[error("invalid claim value for {key}: {value}")]
    InvalidClaimValue { key: &'static str, value: String },
    #[error("missing claim reference for {table}: {id}")]
    MissingClaimReference { table: String, id: String },
    #[error("invalid report season value for {key}: {value}")]
    InvalidReportSeasonValue { key: &'static str, value: String },
    #[error("missing report season reference for {table}: {id}")]
    MissingReportSeasonReference { table: String, id: String },
    #[error(
        "report expectation for {event_key} is frozen: the period's facts are already recorded"
    )]
    ReportExpectationFrozen { event_key: String },
    #[error("invalid quality framework value for {key}: {value}")]
    InvalidFrameworkValue { key: &'static str, value: String },
    #[error("missing quality framework reference for {table}: {id}")]
    MissingFrameworkReference { table: String, id: String },
    #[error("invalid criterion expression: {message}")]
    InvalidCriterionExpression { message: String },
    #[error("framework {id} is not an app template and cannot be reset")]
    NotATemplate { id: String },
    #[error("classification error: {0}")]
    Classification(String),
    #[error("invalid alert rule value for {key}: {value}")]
    InvalidAlertRuleValue { key: &'static str, value: String },
    #[error("alert rule not found: {id}")]
    AlertRuleNotFound { id: String },
    #[error("an identical alert rule already exists: {id}")]
    DuplicateAlertRule { id: String },
    #[error(
        "fact {fact_id} provenance write refused: source_tier={source_tier:?} is incoherent with extraction_method={extraction_method:?} (bug #324 guard)"
    )]
    IncoherentFactProvenance {
        fact_id: String,
        source_tier: String,
        extraction_method: String,
    },
    #[error(
        "fact {fact_id} provenance write refused: source_tier='{source_tier}' is retired (ADR 0095 / ADR 0098 dec. 7) — no new write may produce it, it is a legacy read-only value"
    )]
    RetiredSourceTier {
        fact_id: String,
        source_tier: String,
    },
    #[error(
        "financial fact write refused: extraction_method='html_positional' is retired (ADR 0095) — no new fact row may carry the removed positional parser's method"
    )]
    RetiredExtractionMethod,
    #[error(
        "financial fact write refused: measure_window='{measure_window}' contradicts definition {definition_id}'s period_nature='{period_nature}' (ADR 0100 decision 6) — an instant metric only accepts 'point_in_time', a duration metric never accepts 'point_in_time'"
    )]
    MeasureWindowPeriodNatureMismatch {
        definition_id: String,
        measure_window: String,
        period_nature: String,
    },
    #[error("kpi ingest run not found: {id}")]
    KpiIngestRunNotFound { id: String },
    #[error("kpi ingest run reference not found: {table} {id}")]
    MissingIngestReference { table: String, id: String },
    #[error("invalid kpi ingest run value for {key}: {value}")]
    InvalidKpiIngestRunValue { key: &'static str, value: String },
    #[error(
        "kpi ingest run profile_version '{value}' read from a stored run is not a registered profile version — invariant corruption, not caller input (ADR 0102 dec. 13)"
    )]
    UnknownKpiIngestProfileVersion { value: String },
    #[error("kpi ingest run period {period} does not belong to company {company}")]
    RunPeriodCompanyMismatch { period: String, company: String },
    #[error("kpi ingest run {id} lease not held by {holder}")]
    RunLeaseNotHeld { id: String, holder: String },
    #[error("kpi ingest run document {run_document} does not belong to company {company}")]
    RunDocumentCompanyMismatch {
        run_document: String,
        company: String,
    },
    #[error(
        "kpi ingest run {id} source_content_hash is already recorded and cannot be overwritten"
    )]
    RunSourceHashAlreadyRecorded { id: String },
    #[error("invalid kpi ingest run lease duration: {seconds} seconds (must be > 0)")]
    InvalidRunLeaseDuration { seconds: i64 },
    #[error(
        "kpi ingest run {id} was claimed by holder {holder} after lease expiry — abandon the run"
    )]
    RunTakenOver { id: String, holder: String },
    #[error(
        "kpi ingest run {id} lease for {holder} has expired — re-claim via start_kpi_ingest(runId)"
    )]
    RunLeaseExpired { id: String, holder: String },
    #[error("kpi ingest run {id} is not claimable in status {status}")]
    RunNotClaimable { id: String, status: String },
    #[error(
        "kpi ingest run {id} context value {key} is already '{existing}' and cannot become '{requested}' (set-once)"
    )]
    RunContextValueConflict {
        id: String,
        key: &'static str,
        existing: String,
        requested: String,
    },
    #[error(
        "kpi ingest run {id} lease invariant violated: status={status} holds a non-null lease"
    )]
    RunLeaseInvariantViolation { id: String, status: String },
    #[error("unknown kpi ingest run state: {value}")]
    UnknownKpiIngestRunState { value: String },
    #[error(
        "kpi ingest run {id} has an active run for the same (document, company, profile) triple with a conflicting period"
    )]
    RunPeriodConflict { id: String },
    #[error("kpi ingest run {id} is not stageable in status {status}")]
    InvalidRunStateForStaging { id: String, status: String },
    #[error("kpi staging revision {revision} for run {run_id} is invalid: {reason}")]
    InvalidStagingRevision {
        run_id: String,
        revision: i64,
        reason: &'static str,
    },
    #[error("staged observation not found: {id}")]
    StagedObservationNotFound { id: String },
    #[error("commit receipt already recorded for run {run}")]
    CommitReceiptAlreadyRecorded { run: String },
    #[error("report document {id} bytes are protected and cannot be downgraded to metadata-only")]
    ReportDocumentBytesProtected { id: String },
    #[error(
        "kpi ingest run {id} transition from {from} to {to} is not permitted by the closed lifecycle (ADR 0098 dec. 6)"
    )]
    InvalidRunTransition {
        id: String,
        from: String,
        to: String,
    },
    #[error("kpi ingest run {id} transition prerequisite missing: {requirement}")]
    RunTransitionPrerequisiteMissing {
        id: String,
        requirement: &'static str,
    },
    #[error(
        "kpi ingest run {id} commit refused: manifest is stale relative to the run's current state"
    )]
    StaleManifestForCommit { id: String },
    #[error(
        "kpi ingest run {run} stored commit receipt disagrees with the run row (terminal status/hash/revision) — commit invariant violated"
    )]
    CommitReceiptRunMismatch { run: String },
    #[error(
        "kpi ingest run {run} commit could not acquire the write transaction under contention — retry"
    )]
    CommitContention { run: String },
    #[error("sealed manifest for run {run_id} revision {revision} refused: {reason}")]
    SealedManifestRejected {
        run_id: String,
        revision: i64,
        reason: &'static str,
    },
    #[error(
        "kpi ingest run {run} commit refused: no validation attempt row for revision {revision} (predates migration 0139 — invalidate and re-validate)"
    )]
    MissingValidationAttempt { run: String, revision: i64 },
    #[error(
        "kpi ingest run {run} commit refused: the stored manifest's schema/validator version is unsupported by this build (invalidate and re-validate)"
    )]
    UnsupportedManifestVersion { run: String },
    #[error(
        "kpi ingest run {run} commit refused: the stored validation attempt's manifest bytes are corrupt or internally inconsistent"
    )]
    CorruptStoredManifest { run: String },
    #[error(
        "kpi ingest run {run} commit refused: pinned kpi definition {definition} is missing, ineligible, or no longer matches the manifest's metric key (invalidate and re-validate)"
    )]
    PinnedDefinitionMissing { run: String, definition: String },
    #[error("kpi ingest run {run} commit refused: period conflict — {reason}")]
    CommitPeriodConflict { run: String, reason: &'static str },
    #[error("kpi ingest draft not found: {draft_id}")]
    KpiIngestDraftNotFound { draft_id: String },
    /// Second `draft:{open:true}` on a run that already has a LIVE-epoch
    /// active draft, or a single-call `stage_kpi_observations` while one is
    /// open (ADR 0102 dec. 11) — the same conflict either way: exactly one
    /// active draft, explicit abort required, never a silent orphan.
    #[error("kpi ingest run {run_id} already has an active draft: {draft_id}")]
    KpiIngestActiveDraftExists { run_id: String, draft_id: String },
    /// A lease takeover bumped the run's epoch since this draft was opened
    /// (ADR 0102 dec. 6), or the caller named a draft this build has already
    /// lazily marked `superseded` — never resumable.
    #[error("kpi ingest draft {draft_id} is superseded (its lease epoch is stale)")]
    KpiIngestDraftSuperseded { draft_id: String },
    /// Replaying `(draftId, chunkIndex)` with content whose server-computed
    /// hash disagrees with the stored chunk (ADR 0102 dec. 8) — a same-index
    /// replay with MATCHING content is an idempotent no-op, never this error.
    #[error("kpi ingest draft {draft_id} chunk {chunk_index} conflicts with a previously stored chunk of the same index")]
    KpiIngestDraftChunkConflict { draft_id: String, chunk_index: i64 },
    /// Finalize refused: zero chunks, a gap in the 0-based contiguous chunk
    /// index sequence, or the assembled total disagreeing with the draft's
    /// declared `expectedObservations` (ADR 0102 dec. 8/9).
    #[error("kpi ingest draft {draft_id} cannot finalize: {reason}")]
    KpiIngestDraftIncomplete { draft_id: String, reason: String },
    /// Finalize refused: the assembled aggregate (across every chunk) exceeds
    /// `AGGREGATE_OBSERVATIONS_MAX` or the frozen aggregate byte cap (ADR
    /// 0102 dec. 10, contracts.md tool 5).
    #[error("kpi ingest draft {draft_id} finalize refused: {reason}")]
    KpiIngestDraftAggregateBudgetExceeded { draft_id: String, reason: String },
    /// The `job_queue` row a settle call targeted was gone by the time the
    /// settle transaction ran (ADR 0109 dec. 2, sol diff R1 #2) — the
    /// occurrence still settles truthfully in the SAME transaction, but this
    /// distinct error tells `jobs::queue::dispatch` the queue-side transition
    /// did NOT happen, so it must not run terminal hooks as if it had.
    #[error("job queue row {id} vanished before its settle could complete — the occurrence closed truthfully, but the queue-side transition did NOT apply")]
    JobQueueRowMissingDuringSettle { id: String },
}

pub type StorageResult<T> = Result<T, StorageError>;
