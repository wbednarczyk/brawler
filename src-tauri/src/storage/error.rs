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
    #[error("cockpit layout not found: {id}")]
    CockpitLayoutNotFound { id: String },
    #[error("invalid cockpit layout name: {name}")]
    InvalidCockpitLayoutName { name: String },
    #[error("a cockpit layout named {name} already exists")]
    DuplicateCockpitLayoutName { name: String },
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
    #[error("kpi ingest run not found: {id}")]
    KpiIngestRunNotFound { id: String },
    #[error("kpi ingest run reference not found: {table} {id}")]
    MissingIngestReference { table: String, id: String },
    #[error("invalid kpi ingest run value for {key}: {value}")]
    InvalidKpiIngestRunValue { key: &'static str, value: String },
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
}

pub type StorageResult<T> = Result<T, StorageError>;
