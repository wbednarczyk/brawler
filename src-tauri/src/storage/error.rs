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
        "fact {fact_id} provenance write refused: source_tier='pdf' is retired (ADR 0095) — no new write may produce it, it is a legacy read-only value"
    )]
    RetiredSourceTier { fact_id: String },
    #[error(
        "financial fact write refused: extraction_method='html_positional' is retired (ADR 0095) — no new fact row may carry the removed positional parser's method"
    )]
    RetiredExtractionMethod,
}

pub type StorageResult<T> = Result<T, StorageError>;
