//! Document metadata, the derived-period hint, and profile doctrine for
//! [`super`] — the three read helpers `default_context` assembles alongside
//! the catalog and plausibility sections.

use super::dto::{DerivedPeriodDto, DocumentMetaDto};
use super::paging::{internal, truncate_bytes};
use super::{CONTENT_TYPE_MAX, LOCAL_PATH_MAX, PROFILE_RULE_MAX, TITLE_MAX, URL_MAX};
use crate::commands::error::CommandError;
use crate::storage::{AppState, KpiIngestRun};

use super::super::kpi_ingest::{is_content_hash, SNAPSHOT_DIR};

/// The derived-period HINT: served only from the provenance-bound cache
/// (migration 0140) and only when the cached hash equals the run's frozen
/// source hash — both present (`None == None` never counts). Anything else is
/// `null`; the read path never derives (and never writes).
pub(super) fn derived_period_hint(
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

pub(super) fn document_meta(
    state: &AppState,
    run: &KpiIngestRun,
) -> Result<DocumentMetaDto, CommandError> {
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

pub(super) fn profile_rules(run: &KpiIngestRun) -> Result<Vec<String>, CommandError> {
    Ok(
        crate::storage::kpi_ingest_profile_rules(&run.profile_version)?
            .iter()
            .map(|rule| truncate_bytes(rule, PROFILE_RULE_MAX))
            .collect(),
    )
}
