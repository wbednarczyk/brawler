use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

use super::{
    empty_string_to_none, notebooks, slug_part, CompanyLookupResult, NewNotebookEntry,
    NewNotebookOrigin, NotebookEntry, StorageError, StorageResult,
};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CreateNoteFromTranscriptSelectionInput {
    pub transcript_job_id: String,
    pub transcript_segment_ids: Vec<String>,
    pub note_draft: TranscriptNoteDraft,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptNoteDraft {
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptJob {
    pub id: String,
    pub company_id: Option<String>,
    pub company: Option<String>,
    pub company_name: Option<String>,
    pub provider_id: String,
    pub source_type: String,
    pub source_url: String,
    pub source_label: Option<String>,
    pub company_resolution_status: String,
    pub recognized_company_candidates: Vec<CompanyLookupResult>,
    pub status: String,
    pub error_code: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "ListVideoTranscriptJobsInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptJobListInput {
    pub company_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "CreateVideoTranscriptJobInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewTranscriptJob {
    pub company_id: Option<String>,
    pub provider_id: Option<String>,
    pub source_url: String,
    pub source_label: Option<String>,
    pub recognized_company_candidates: Option<Vec<CompanyLookupResult>>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "UpdateVideoTranscriptJobInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTranscriptJobInput {
    pub job_id: String,
    pub source_label: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResolveTranscriptJobCompanyInput {
    pub job_id: String,
    pub company_id: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub transcript_job_id: String,
    pub company_id: Option<String>,
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub speaker: Option<String>,
    pub text: String,
    pub language: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTranscriptSegment {
    pub transcript_job_id: String,
    pub company_id: Option<String>,
    pub start_seconds: Option<i64>,
    pub end_seconds: Option<i64>,
    pub speaker: Option<String>,
    pub text: String,
    pub language: Option<String>,
}

pub(crate) fn create_note_from_transcript_selection(
    connection: &Connection,
    input: CreateNoteFromTranscriptSelectionInput,
) -> StorageResult<NotebookEntry> {
    let transcript_job_id = input.transcript_job_id.trim().to_owned();
    let selected_segment_ids = input
        .transcript_segment_ids
        .into_iter()
        .map(|segment_id| segment_id.trim().to_owned())
        .filter(|segment_id| !segment_id.is_empty())
        .collect::<Vec<_>>();

    if selected_segment_ids.is_empty() {
        return Err(StorageError::InvalidTranscriptValue {
            key: "transcript_segment_ids",
            value: "empty".to_owned(),
        });
    }

    let job = get_transcript_job(connection, &transcript_job_id)?;
    let company_id =
        job.company_id
            .clone()
            .ok_or_else(|| StorageError::InvalidTranscriptValue {
                key: "company_id",
                value: "unresolved".to_owned(),
            })?;

    if job.status != "completed" {
        return Err(StorageError::InvalidTranscriptValue {
            key: "status",
            value: job.status,
        });
    }

    let all_segments = list_transcript_segments(connection, &transcript_job_id)?;
    let selected_id_set = selected_segment_ids
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let selected_segments = all_segments
        .into_iter()
        .filter(|segment| selected_id_set.contains(&segment.id))
        .collect::<Vec<_>>();

    if selected_segments.len() != selected_id_set.len() {
        return Err(StorageError::InvalidTranscriptValue {
            key: "transcript_segment_ids",
            value: "unknown segment".to_owned(),
        });
    }

    let origins = selected_segments
        .iter()
        .map(|segment| NewNotebookOrigin {
            source_type: "transcript_segment".to_owned(),
            source_id: Some(segment.id.clone()),
            source_url: Some(job.source_url.clone()),
            label: Some(transcript_origin_label(&job, segment)),
        })
        .collect::<Vec<_>>();

    notebooks::create_notebook_entry(
        connection,
        NewNotebookEntry {
            company_id,
            title: input.note_draft.title,
            body: input.note_draft.body,
            body_format: Some("markdown".to_owned()),
            tags: input.note_draft.tags,
            kind: input.note_draft.kind,
            claim_status: input.note_draft.claim_status,
            event_date: input.note_draft.event_date,
            follow_up_after: input.note_draft.follow_up_after,
            follow_up_date: input.note_draft.follow_up_date,
            origins,
        },
    )
}

pub(crate) fn list_transcript_jobs(
    connection: &Connection,
    input: TranscriptJobListInput,
) -> StorageResult<Vec<TranscriptJob>> {
    let mut statement = connection.prepare(
        "
        SELECT
            transcript_jobs.id,
            transcript_jobs.company_id,
            companies.qualified_ticker,
            companies.display_name,
            transcript_jobs.provider_id,
            transcript_jobs.source_type,
            transcript_jobs.source_url,
            transcript_jobs.source_label,
            transcript_jobs.company_resolution_status,
            transcript_jobs.recognized_company_candidates_json,
            transcript_jobs.status,
            transcript_jobs.error_code,
            transcript_jobs.created_at,
            transcript_jobs.started_at,
            transcript_jobs.finished_at,
            transcript_jobs.error
        FROM transcript_jobs
        LEFT JOIN companies ON companies.id = transcript_jobs.company_id
        WHERE (?1 IS NULL OR transcript_jobs.company_id = ?1)
        ORDER BY transcript_jobs.created_at DESC, transcript_jobs.id DESC
        ",
    )?;

    let rows = statement.query_map([input.company_id], transcript_job_from_row)?;
    let jobs = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(jobs)
}

pub(crate) fn delete_transcript_job(connection: &Connection, job_id: &str) -> StorageResult<()> {
    connection.execute("DELETE FROM transcript_jobs WHERE id = ?1", [job_id])?;

    Ok(())
}

pub(crate) fn create_transcript_job(
    connection: &Connection,
    input: NewTranscriptJob,
) -> StorageResult<TranscriptJob> {
    let source_url = input.source_url.trim().to_owned();
    let provider_id = input
        .provider_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("provider_gemini")
        .to_owned();
    let source_type = "youtube_url".to_owned();
    let company_resolution_status = if input.company_id.is_some() {
        "provided"
    } else {
        "unresolved"
    };
    let status = "queued".to_owned();
    let recognized_company_candidates = input.recognized_company_candidates.unwrap_or_default();
    let recognized_company_candidates_json = serde_json::to_string(&recognized_company_candidates)
        .map_err(|error| StorageError::InvalidTranscriptValue {
            key: "recognized_company_candidates",
            value: error.to_string(),
        })?;
    let id = transcript_job_id(connection, input.company_id.as_deref(), &source_url)?;

    if source_url.is_empty() {
        return Err(StorageError::InvalidTranscriptValue {
            key: "source_url",
            value: source_url,
        });
    }

    if let Some(existing_job) =
        find_existing_transcript_job(connection, input.company_id.as_deref(), &source_url)?
    {
        return Ok(existing_job);
    }

    validate_allowed_transcript_value("provider_id", &provider_id, &["provider_gemini"])?;
    validate_allowed_transcript_value("source_type", &source_type, &["youtube_url"])?;
    validate_allowed_transcript_value(
        "company_resolution_status",
        company_resolution_status,
        &[
            "provided",
            "recognized",
            "unresolved",
            "needs_user_selection",
        ],
    )?;
    validate_allowed_transcript_value(
        "status",
        &status,
        &["queued", "running", "completed", "failed"],
    )?;

    connection.execute(
        "
        INSERT INTO transcript_jobs (
            id,
            company_id,
            provider_id,
            source_type,
            source_url,
            source_label,
            company_resolution_status,
            recognized_company_candidates_json,
            status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            id,
            input.company_id,
            provider_id,
            source_type,
            source_url,
            empty_string_to_none(input.source_label),
            company_resolution_status,
            recognized_company_candidates_json,
            status,
        ],
    )?;

    get_transcript_job(connection, &id)
}

pub(crate) fn update_transcript_job(
    connection: &Connection,
    input: UpdateTranscriptJobInput,
) -> StorageResult<TranscriptJob> {
    connection.execute(
        "
        UPDATE transcript_jobs
        SET source_label = ?2
        WHERE id = ?1
        ",
        params![
            input.job_id.as_str(),
            empty_string_to_none(input.source_label)
        ],
    )?;

    get_transcript_job(connection, &input.job_id)
}

fn find_existing_transcript_job(
    connection: &Connection,
    company_id: Option<&str>,
    source_url: &str,
) -> StorageResult<Option<TranscriptJob>> {
    let mut statement = connection.prepare(
        "
        SELECT
            transcript_jobs.id,
            transcript_jobs.company_id,
            companies.qualified_ticker,
            companies.display_name,
            transcript_jobs.provider_id,
            transcript_jobs.source_type,
            transcript_jobs.source_url,
            transcript_jobs.source_label,
            transcript_jobs.company_resolution_status,
            transcript_jobs.recognized_company_candidates_json,
            transcript_jobs.status,
            transcript_jobs.error_code,
            transcript_jobs.created_at,
            transcript_jobs.started_at,
            transcript_jobs.finished_at,
            transcript_jobs.error
        FROM transcript_jobs
        LEFT JOIN companies ON companies.id = transcript_jobs.company_id
        WHERE
            transcript_jobs.source_url = ?1
            AND (
                (?2 IS NULL AND transcript_jobs.company_id IS NULL)
                OR transcript_jobs.company_id = ?2
            )
        ORDER BY transcript_jobs.created_at DESC, transcript_jobs.id DESC
        LIMIT 1
        ",
    )?;

    let mut rows = statement.query(params![source_url, company_id])?;

    rows.next()?
        .map(transcript_job_from_row)
        .transpose()
        .map_err(StorageError::from)
}

pub(crate) fn list_transcript_segments(
    connection: &Connection,
    transcript_job_id: &str,
) -> StorageResult<Vec<TranscriptSegment>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            transcript_job_id,
            company_id,
            start_seconds,
            end_seconds,
            speaker,
            text,
            language,
            created_at
        FROM transcript_segments
        WHERE transcript_job_id = ?1
        ORDER BY
            CASE WHEN start_seconds IS NULL THEN 1 ELSE 0 END,
            start_seconds ASC,
            id ASC
        ",
    )?;

    let rows = statement.query_map([transcript_job_id], transcript_segment_from_row)?;
    let segments = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(segments)
}

pub(crate) fn create_transcript_segment(
    connection: &Connection,
    input: NewTranscriptSegment,
) -> StorageResult<TranscriptSegment> {
    let text = input.text;

    if text.trim().is_empty() {
        return Err(StorageError::InvalidTranscriptValue {
            key: "text",
            value: text,
        });
    }

    let parent_company_id = connection.query_row(
        "SELECT company_id FROM transcript_jobs WHERE id = ?1",
        [&input.transcript_job_id],
        |row| row.get::<_, Option<String>>(0),
    )?;
    let company_id = input.company_id.or(parent_company_id);
    let id = transcript_segment_id(connection, &input.transcript_job_id)?;

    connection.execute(
        "
        INSERT INTO transcript_segments (
            id,
            transcript_job_id,
            company_id,
            start_seconds,
            end_seconds,
            speaker,
            text,
            language
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            id,
            input.transcript_job_id,
            company_id,
            input.start_seconds,
            input.end_seconds,
            empty_string_to_none(input.speaker),
            text,
            empty_string_to_none(input.language),
        ],
    )?;

    get_transcript_segment(connection, &id)
}

pub(crate) fn resolve_transcript_job_company(
    connection: &Connection,
    input: ResolveTranscriptJobCompanyInput,
) -> StorageResult<TranscriptJob> {
    connection.execute(
        "
        UPDATE transcript_jobs
        SET
            company_id = ?2,
            company_resolution_status = 'provided'
        WHERE id = ?1
        ",
        params![input.job_id, input.company_id],
    )?;

    connection.execute(
        "
        UPDATE transcript_segments
        SET company_id = (
            SELECT company_id
            FROM transcript_jobs
            WHERE transcript_jobs.id = transcript_segments.transcript_job_id
        )
        WHERE transcript_job_id = ?1
        ",
        [input.job_id.as_str()],
    )?;

    get_transcript_job(connection, &input.job_id)
}

pub(crate) fn mark_transcript_job_running(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<TranscriptJob> {
    connection.execute(
        "
        UPDATE transcript_jobs
        SET
            status = 'running',
            started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            finished_at = NULL,
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;

    get_transcript_job(connection, job_id)
}

pub(crate) fn mark_transcript_job_completed(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<TranscriptJob> {
    connection.execute(
        "
        UPDATE transcript_jobs
        SET
            status = 'completed',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;

    get_transcript_job(connection, job_id)
}

pub(crate) fn mark_transcript_job_failed(
    connection: &Connection,
    job_id: &str,
    error_code: &str,
    error: &str,
) -> StorageResult<TranscriptJob> {
    validate_allowed_transcript_value(
        "error_code",
        error_code,
        &[
            "provider_not_configured",
            "provider_limit",
            "provider_unavailable",
            "provider_error",
            "network_error",
            "invalid_source_url",
            "parse_error",
            "unknown",
        ],
    )?;

    connection.execute(
        "
        UPDATE transcript_jobs
        SET
            status = 'failed',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = ?2,
            error = ?3
        WHERE id = ?1
        ",
        params![job_id, error_code, error],
    )?;

    get_transcript_job(connection, job_id)
}

fn validate_allowed_transcript_value(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidTranscriptValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn transcript_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TranscriptJob> {
    let candidates_json: String = row.get(9)?;
    let recognized_company_candidates =
        serde_json::from_str::<Vec<CompanyLookupResult>>(&candidates_json).unwrap_or_default();

    Ok(TranscriptJob {
        id: row.get(0)?,
        company_id: row.get(1)?,
        company: row.get(2)?,
        company_name: row.get(3)?,
        provider_id: row.get(4)?,
        source_type: row.get(5)?,
        source_url: row.get(6)?,
        source_label: row.get(7)?,
        company_resolution_status: row.get(8)?,
        recognized_company_candidates,
        status: row.get(10)?,
        error_code: row.get(11)?,
        created_at: row.get(12)?,
        started_at: row.get(13)?,
        finished_at: row.get(14)?,
        error: row.get(15)?,
    })
}

fn transcript_segment_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<TranscriptSegment> {
    Ok(TranscriptSegment {
        id: row.get(0)?,
        transcript_job_id: row.get(1)?,
        company_id: row.get(2)?,
        start_seconds: row.get(3)?,
        end_seconds: row.get(4)?,
        speaker: row.get(5)?,
        text: row.get(6)?,
        language: row.get(7)?,
        created_at: row.get(8)?,
    })
}

pub(crate) fn get_transcript_job(
    connection: &Connection,
    id: &str,
) -> StorageResult<TranscriptJob> {
    connection
        .query_row(
            "
            SELECT
                transcript_jobs.id,
                transcript_jobs.company_id,
                companies.qualified_ticker,
                companies.display_name,
                transcript_jobs.provider_id,
                transcript_jobs.source_type,
                transcript_jobs.source_url,
                transcript_jobs.source_label,
                transcript_jobs.company_resolution_status,
                transcript_jobs.recognized_company_candidates_json,
                transcript_jobs.status,
                transcript_jobs.error_code,
                transcript_jobs.created_at,
                transcript_jobs.started_at,
                transcript_jobs.finished_at,
                transcript_jobs.error
            FROM transcript_jobs
            LEFT JOIN companies ON companies.id = transcript_jobs.company_id
            WHERE transcript_jobs.id = ?1
            ",
            [id],
            transcript_job_from_row,
        )
        .map_err(StorageError::from)
}

fn get_transcript_segment(connection: &Connection, id: &str) -> StorageResult<TranscriptSegment> {
    connection
        .query_row(
            "
            SELECT
                id,
                transcript_job_id,
                company_id,
                start_seconds,
                end_seconds,
                speaker,
                text,
                language,
                created_at
            FROM transcript_segments
            WHERE id = ?1
            ",
            [id],
            transcript_segment_from_row,
        )
        .map_err(StorageError::from)
}

fn transcript_job_id(
    connection: &Connection,
    company_id: Option<&str>,
    source_url: &str,
) -> StorageResult<String> {
    let base_id = format!(
        "transcript_job_{}_{}",
        slug_part(company_id.unwrap_or("unresolved")),
        slug_part(source_url)
    );
    let existing_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM transcript_jobs WHERE id = ?1 OR id LIKE ?2",
        params![&base_id, format!("{base_id}_%")],
        |row| row.get(0),
    )?;

    if existing_count == 0 {
        Ok(base_id)
    } else {
        Ok(format!("{base_id}_{}", existing_count + 1))
    }
}

fn transcript_segment_id(
    connection: &Connection,
    transcript_job_id: &str,
) -> StorageResult<String> {
    let existing_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM transcript_segments WHERE transcript_job_id = ?1",
        [transcript_job_id],
        |row| row.get(0),
    )?;

    Ok(format!(
        "transcript_segment_{}_{}",
        slug_part(transcript_job_id),
        existing_count + 1
    ))
}

fn transcript_origin_label(job: &TranscriptJob, segment: &TranscriptSegment) -> String {
    let source_label = job.source_label.as_deref().unwrap_or(&job.source_url);
    let timestamp = transcript_segment_timestamp_label(segment);

    format!(
        "Transcript {} · job {} · segment {} · {} · {}",
        job.provider_id, job.id, segment.id, timestamp, source_label
    )
}

fn transcript_segment_timestamp_label(segment: &TranscriptSegment) -> String {
    match (segment.start_seconds, segment.end_seconds) {
        (Some(start_seconds), Some(end_seconds)) => format!("{start_seconds}s-{end_seconds}s"),
        (Some(start_seconds), None) => format!("{start_seconds}s"),
        (None, Some(end_seconds)) => format!("0s-{end_seconds}s"),
        (None, None) => "no timestamp".to_owned(),
    }
}
