use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
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
    #[error("missing financials reference for {table}: {id}")]
    MissingFinancialsReference { table: String, id: String },
}

pub type StorageResult<T> = Result<T, StorageError>;
