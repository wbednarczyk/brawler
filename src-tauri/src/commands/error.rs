//! Typed command error envelope ([ADR 0070](../../../docs/adr/0070-typed-command-error-envelope.md)).
//!
//! Every migrated Tauri command returns `Result<T, CommandError>` instead of
//! `Result<T, String>`, so the frontend can branch on a machine-readable `code`
//! (link to Settings on `missing_credential`, offer retry on `network`, prompt a
//! refresh on `conflict`) instead of pattern-matching opaque strings. The code
//! set is **closed and additive-only** — new kinds are appended, never removed or
//! repurposed, so the wire contract stays stable.
//!
//! The `From<StorageError>` mapping is the single, central place storage failures
//! acquire a code. Its `match` is **wildcard-free on purpose**: adding a
//! `StorageError` variant must fail compilation here, forcing a deliberate code
//! choice rather than a silent fall-through to `internal`.

use serde::Serialize;

use crate::storage::StorageError;

/// Closed set of machine-readable command failure kinds (ADR 0070). Serialized
/// as `snake_case` on the wire; additive-only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum CommandErrorCode {
    /// A referenced entity does not exist (soft reference or lookup miss).
    NotFound,
    /// Caller-supplied input failed validation (bad value, malformed expression).
    InvalidInput,
    /// A required credential/secret is absent from the OS keychain.
    MissingCredential,
    /// A network/HTTP call failed (timeout, DNS, connection reset).
    Network,
    /// An upstream AI/source provider rejected or failed the request.
    Provider,
    /// The operation conflicts with current state (uniqueness/constraint, stale write).
    Conflict,
    /// The MCP `act` (write) tier is disabled (ADR 0088 M3). Returned only over
    /// the MCP boundary when a write tool is called while `mcpWritesEnabled` is
    /// off; the user enables it in Settings → MCP server. `update_settings` is
    /// itself MCP-excluded, so a connected agent can never clear this itself.
    WritesDisabled,
    /// An MCP `act` write is missing its mandatory provenance carrier (ADR 0088
    /// dec. 3). Returned only over the MCP boundary; the message names the
    /// required field (e.g. a non-empty `origins` array).
    ProvenanceRequired,
    /// An unexpected internal failure with no more specific code.
    Internal,
}

/// Serializable command error envelope crossing the Tauri boundary (ADR 0070).
/// `code` is the machine-readable kind; `message` stays the human-readable detail.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: CommandErrorCode,
    pub message: String,
}

impl CommandError {
    /// Build an envelope from an explicit code and message.
    pub fn new(code: CommandErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Assign a [`CommandErrorCode`] to a [`StorageError`]. The `match` is
/// intentionally wildcard-free so a new storage variant fails to compile here,
/// forcing a deliberate code choice rather than a silent fall-through.
///
/// `StorageError` is storage-only, so it never yields `missing_credential`,
/// `network`, or `provider` — those codes are reserved for keychain, HTTP, and
/// AI-provider failures that migrate through their own `From` impls.
fn code_for(error: &StorageError) -> CommandErrorCode {
    use CommandErrorCode::*;
    match error {
        // A SQLite constraint violation (UNIQUE/FK/CHECK) is a state conflict;
        // any other SQLite failure is an unexpected internal error.
        StorageError::Sqlite(e) => sqlite_code(e),
        StorageError::Pool(_) => Internal,
        StorageError::Io(_) => Internal,
        StorageError::Json(_) => Internal,
        StorageError::Yaml(_) => Internal,
        // Validation failures on caller-supplied values.
        StorageError::InvalidSettingValue { .. } => InvalidInput,
        StorageError::InvalidNotebookValue { .. } => InvalidInput,
        StorageError::InvalidCompanyEventValue { .. } => InvalidInput,
        StorageError::InvalidTranscriptValue { .. } => InvalidInput,
        StorageError::InvalidAiAnalysisValue { .. } => InvalidInput,
        StorageError::InvalidDiagnosticValue { .. } => InvalidInput,
        StorageError::InvalidLicenseValue { .. } => InvalidInput,
        StorageError::InvalidSourceValue { .. } => InvalidInput,
        StorageError::InvalidResearchValue { .. } => InvalidInput,
        StorageError::InvalidFinancialsValue { .. } => InvalidInput,
        // data_quality is a uniqueness-slot dimension; changing it via update
        // conflicts with the ADR 0093 lifecycle (a new final fact supersedes a
        // preliminary/estimated one, never an in-place edit) — a state conflict,
        // not a shape problem with the requested value itself.
        StorageError::FinancialFactDataQualityLocked { .. } => Conflict,
        StorageError::InvalidClaimValue { .. } => InvalidInput,
        StorageError::InvalidReportSeasonValue { .. } => InvalidInput,
        StorageError::InvalidFrameworkValue { .. } => InvalidInput,
        StorageError::InvalidCriterionExpression { .. } => InvalidInput,
        StorageError::InvalidCockpitLayoutName { .. } => InvalidInput,
        // Renaming onto an existing layout name is caller input, not a bug.
        StorageError::DuplicateCockpitLayoutName { .. } => InvalidInput,
        StorageError::InvalidAlertRuleValue { .. } => InvalidInput,
        // Creating a rule identical to an existing one is caller input, not a bug.
        StorageError::DuplicateAlertRule { .. } => InvalidInput,
        // A reset request against a non-template framework is an invalid
        // operation on caller-named input (not a concurrency conflict).
        StorageError::NotATemplate { .. } => InvalidInput,
        // Soft-reference misses and lookup failures: the referenced row is gone.
        StorageError::MissingResearchReference { .. } => NotFound,
        StorageError::MissingFinancialsReference { .. } => NotFound,
        StorageError::MissingClaimReference { .. } => NotFound,
        StorageError::MissingReportSeasonReference { .. } => NotFound,
        // Editing a frozen report expectation conflicts with recorded state
        // (the period's facts already landed), not with the input's shape.
        StorageError::ReportExpectationFrozen { .. } => Conflict,
        StorageError::MissingFrameworkReference { .. } => NotFound,
        StorageError::CockpitLayoutNotFound { .. } => NotFound,
        StorageError::AlertRuleNotFound { .. } => NotFound,
        // Internal ML/derivation step failure — no more specific code.
        StorageError::Classification(_) => Internal,
        // A provenance write path stamped a source_tier/extraction_method
        // pair that does not cohere (bug #324 guard) — every caller passes
        // both internally (never as raw MCP/UI input), so tripping this is a
        // programming error in the write path, not a shape problem with
        // caller-supplied data.
        StorageError::IncoherentFactProvenance { .. } => Internal,
        // Same rationale: no caller ever passes `source_tier` as raw input,
        // so tripping the ADR 0095 retired-tier refusal is a programming
        // error in the write path, not a shape problem with caller data.
        StorageError::RetiredSourceTier { .. } => Internal,
        // Unlike the two above, `extraction_method` IS caller-suppliable on
        // the plain create path (UI/MCP manual entry) — a request naming the
        // retired positional method is a shape problem with the input.
        StorageError::RetiredExtractionMethod => InvalidInput,
        // Acting on a lease the caller does not (or no longer) hold conflicts
        // with current state — another worker's claim, or the caller's own
        // lease having expired — not a shape problem with the request.
        StorageError::KpiIngestRunNotFound { .. } => NotFound,
        StorageError::MissingIngestReference { .. } => NotFound,
        StorageError::InvalidKpiIngestRunValue { .. } => InvalidInput,
        StorageError::RunPeriodCompanyMismatch { .. } => InvalidInput,
        StorageError::RunLeaseNotHeld { .. } => Conflict,
        // Caller-supplied document/company ids that do not cohere.
        StorageError::RunDocumentCompanyMismatch { .. } => InvalidInput,
        // Overwriting the immutable source snapshot hash conflicts with the
        // already-recorded value.
        StorageError::RunSourceHashAlreadyRecorded { .. } => Conflict,
        StorageError::InvalidRunLeaseDuration { .. } => InvalidInput,
        // A `committing` row holding a non-null lease is a structural bug
        // (the transition to `committing` must clear it, #360) — never caller
        // input.
        StorageError::RunLeaseInvariantViolation { .. } => Internal,
        // A stored `status` token no caller ever supplies as raw input.
        StorageError::UnknownKpiIngestRunState { .. } => Internal,
        // A second `create_run_if_absent` naming a different period for the
        // same active triple conflicts with the already-running triple.
        StorageError::RunPeriodConflict { .. } => Conflict,
        // Staging a run outside {extracting, validation_failed} conflicts
        // with the run's current lifecycle state.
        StorageError::InvalidRunStateForStaging { .. } => Conflict,
        // Acting on a stale or already-frozen staging revision conflicts
        // with the run's current manifest state.
        StorageError::InvalidStagingRevision { .. } => Conflict,
        StorageError::StagedObservationNotFound { .. } => NotFound,
        // A second commit receipt for the same run conflicts with the
        // already-recorded (immutable) one.
        StorageError::CommitReceiptAlreadyRecorded { .. } => Conflict,
        // The document's bytes are under the retention protection contract —
        // the request conflicts with that contract, not a shape problem.
        StorageError::ReportDocumentBytesProtected { .. } => Conflict,
    }
}

/// A SQLite constraint violation is the one storage failure that maps to
/// `conflict`; everything else stays `internal`.
fn sqlite_code(error: &rusqlite::Error) -> CommandErrorCode {
    match error {
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            CommandErrorCode::Conflict
        }
        _ => CommandErrorCode::Internal,
    }
}

impl From<StorageError> for CommandError {
    fn from(error: StorageError) -> Self {
        let code = code_for(&error);
        CommandError {
            code,
            message: error.to_string(),
        }
    }
}

/// Keychain failures acquire their code here (ADR 0070: non-storage failure
/// families migrate through their own `From` impls). Wildcard-free on purpose,
/// like [`code_for`]: a new `CredentialError` variant must fail compilation
/// here, forcing a deliberate code choice.
impl From<crate::providers::credentials::CredentialError> for CommandError {
    fn from(error: crate::providers::credentials::CredentialError) -> Self {
        use crate::providers::credentials::CredentialError;

        let code = match &error {
            // Problems with what the caller supplied or named.
            CredentialError::EmptySecret => CommandErrorCode::InvalidInput,
            CredentialError::UnknownProvider(_) => CommandErrorCode::InvalidInput,
            // Keychain backend failures have no more specific code.
            CredentialError::PersistenceVerificationFailed => CommandErrorCode::Internal,
            CredentialError::Backend(_) => CommandErrorCode::Internal,
        };
        CommandError {
            code,
            message: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(error: StorageError) -> CommandErrorCode {
        CommandError::from(error).code
    }

    fn constraint_error() -> StorageError {
        // SQLITE_CONSTRAINT (19) → primary code ConstraintViolation.
        StorageError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CONSTRAINT),
            Some("UNIQUE constraint failed: companies.qualified_ticker".to_owned()),
        ))
    }

    fn non_constraint_sqlite_error() -> StorageError {
        // SQLITE_BUSY (5) → primary code DatabaseBusy, not a conflict.
        StorageError::Sqlite(rusqlite::Error::SqliteFailure(
            rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_BUSY),
            None,
        ))
    }

    #[test]
    fn not_found_variants_map_to_not_found() {
        use CommandErrorCode::NotFound;
        assert_eq!(
            code_of(StorageError::CockpitLayoutNotFound {
                id: "layout_x".to_owned()
            }),
            NotFound
        );
        assert_eq!(
            code_of(StorageError::MissingResearchReference {
                table: "notebook_entries".to_owned(),
                id: "entry_1".to_owned()
            }),
            NotFound
        );
        assert_eq!(
            code_of(StorageError::MissingClaimReference {
                table: "claims".to_owned(),
                id: "claim_1".to_owned()
            }),
            NotFound
        );
        assert_eq!(
            code_of(StorageError::MissingFinancialsReference {
                table: "financial_periods".to_owned(),
                id: "fp_1".to_owned()
            }),
            NotFound
        );
        assert_eq!(
            code_of(StorageError::MissingReportSeasonReference {
                table: "report_preparations".to_owned(),
                id: "rp_1".to_owned()
            }),
            NotFound
        );
        assert_eq!(
            code_of(StorageError::MissingFrameworkReference {
                table: "quality_frameworks".to_owned(),
                id: "qf_1".to_owned()
            }),
            NotFound
        );
    }

    #[test]
    fn validation_variants_map_to_invalid_input() {
        use CommandErrorCode::InvalidInput;
        assert_eq!(
            code_of(StorageError::InvalidSettingValue {
                key: "theme",
                value: "chartreuse".to_owned()
            }),
            InvalidInput
        );
        assert_eq!(
            code_of(StorageError::InvalidNotebookValue {
                key: "kind",
                value: "??".to_owned()
            }),
            InvalidInput
        );
        assert_eq!(
            code_of(StorageError::InvalidCriterionExpression {
                message: "unbalanced parens".to_owned()
            }),
            InvalidInput
        );
        assert_eq!(
            code_of(StorageError::InvalidCockpitLayoutName {
                name: "".to_owned()
            }),
            InvalidInput
        );
        assert_eq!(
            code_of(StorageError::NotATemplate {
                id: "qf_custom".to_owned()
            }),
            InvalidInput
        );
    }

    #[test]
    fn constraint_violation_maps_to_conflict() {
        assert_eq!(code_of(constraint_error()), CommandErrorCode::Conflict);
    }

    #[test]
    fn frozen_report_expectation_maps_to_conflict() {
        assert_eq!(
            code_of(StorageError::ReportExpectationFrozen {
                event_key: "evt_demo".into(),
            }),
            CommandErrorCode::Conflict
        );
    }

    #[test]
    fn generic_failures_map_to_internal() {
        use CommandErrorCode::Internal;
        assert_eq!(code_of(non_constraint_sqlite_error()), Internal);
        assert_eq!(
            code_of(StorageError::Io(std::io::Error::other("disk gone"))),
            Internal
        );
        assert_eq!(
            code_of(StorageError::Json(
                serde_json::from_str::<i32>("not json").unwrap_err()
            )),
            Internal
        );
        assert_eq!(
            code_of(StorageError::Classification("model unavailable".to_owned())),
            Internal
        );
    }

    #[test]
    fn envelope_serializes_to_code_and_message() {
        let envelope = CommandError::from(StorageError::CockpitLayoutNotFound {
            id: "layout_demo".to_owned(),
        });
        let json = serde_json::to_string_pretty(&envelope).unwrap();
        insta::assert_snapshot!("command_error_envelope", json);
    }

    #[test]
    fn credential_errors_map_to_codes() {
        // Keychain failures migrate through their own From impl (ADR 0070):
        // caller-input problems are invalid_input; backend/persistence
        // failures have no more specific code than internal.
        use crate::providers::credentials::CredentialError;

        assert_eq!(
            CommandError::from(CredentialError::EmptySecret).code,
            CommandErrorCode::InvalidInput
        );
        assert_eq!(
            CommandError::from(CredentialError::UnknownProvider("provider_x".to_owned())).code,
            CommandErrorCode::InvalidInput
        );
        assert_eq!(
            CommandError::from(CredentialError::PersistenceVerificationFailed).code,
            CommandErrorCode::Internal
        );
        assert_eq!(
            CommandError::from(CredentialError::Backend("keychain down".to_owned())).code,
            CommandErrorCode::Internal
        );
    }
}
