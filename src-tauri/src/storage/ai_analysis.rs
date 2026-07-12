use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{StorageError, StorageResult};

const DEFAULT_PROMPT_PRESET_ID: &str = "default_summary";
const DEFAULT_PROMPT_VERSION: &str = "m13.source_grounded.v2";

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysisJob {
    pub id: String,
    pub feed_item_id: String,
    pub prompt_preset_id: String,
    pub custom_question: Option<String>,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"queued\" | \"running\" | \"succeeded\" | \"failed\" | \"cancelled\"")
    )]
    pub status: String,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub result: Option<AiAnalysisResult>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysisResult {
    pub id: String,
    pub ai_analysis_job_id: Option<String>,
    pub feed_item_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    pub summary: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"low\" | \"medium\" | \"high\" | \"unknown\"")
    )]
    pub significance: String,
    pub reasoning: String,
    pub language: Option<String>,
    pub tags: Vec<String>,
    pub source_references: Vec<AiAnalysisSourceReference>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct AiAnalysisSourceReference {
    pub id: String,
    pub source_url: String,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAiAnalysisJob {
    pub feed_item_id: String,
    pub prompt_preset_id: Option<String>,
    pub custom_question: Option<String>,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedAiAnalysis {
    pub job_id: String,
    pub summary: String,
    pub significance: String,
    pub reasoning: String,
    pub language: Option<String>,
    pub tags: Vec<String>,
    pub source_references: Vec<NewAiAnalysisSourceReference>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewAiAnalysisSourceReference {
    pub source_url: String,
    pub label: Option<String>,
}

pub(crate) fn create_ai_analysis_job(
    connection: &Connection,
    input: NewAiAnalysisJob,
) -> StorageResult<AiAnalysisJob> {
    validate_required_value("feed_item_id", &input.feed_item_id)?;
    validate_required_value("provider_id", &input.provider_id)?;
    validate_required_value("model", &input.model)?;

    let prompt_preset_id = input
        .prompt_preset_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PROMPT_PRESET_ID)
        .to_owned();
    let prompt_version = input
        .prompt_version
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(DEFAULT_PROMPT_VERSION)
        .to_owned();
    let custom_question = input
        .custom_question
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty());
    let job_id = ai_analysis_job_id(
        &input.feed_item_id,
        &prompt_preset_id,
        custom_question.as_deref(),
    );

    connection.execute(
        "
        INSERT INTO ai_analysis_jobs (
            id,
            feed_item_id,
            prompt_preset_id,
            custom_question,
            provider_id,
            model,
            prompt_version,
            status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued')
        ON CONFLICT(id) DO NOTHING
        ",
        params![
            job_id,
            input.feed_item_id,
            prompt_preset_id,
            custom_question,
            input.provider_id,
            input.model,
            prompt_version
        ],
    )?;

    get_ai_analysis_job(connection, &job_id)
}

pub(crate) fn list_ai_analysis_jobs(
    connection: &Connection,
    feed_item_id: &str,
) -> StorageResult<Vec<AiAnalysisJob>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            feed_item_id,
            prompt_preset_id,
            custom_question,
            provider_id,
            model,
            prompt_version,
            status,
            error_code,
            error,
            created_at,
            started_at,
            finished_at
        FROM ai_analysis_jobs
        WHERE feed_item_id = ?1
        ORDER BY created_at DESC, id DESC
        ",
    )?;

    let rows = statement.query_map([feed_item_id], ai_analysis_job_from_row)?;
    let mut jobs = rows.collect::<Result<Vec<_>, _>>()?;

    for job in &mut jobs {
        job.result = get_ai_analysis_result_for_job(connection, &job.id)?;
    }

    Ok(jobs)
}

pub(crate) fn mark_ai_analysis_job_running(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<AiAnalysisJob> {
    connection.execute(
        "
        UPDATE ai_analysis_jobs
        SET status = 'running',
            started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;

    get_ai_analysis_job(connection, job_id)
}

pub(crate) fn complete_ai_analysis_job(
    connection: &Connection,
    input: CompletedAiAnalysis,
) -> StorageResult<AiAnalysisJob> {
    validate_allowed_analysis_value(
        "significance",
        &input.significance,
        &["low", "medium", "high", "unknown"],
    )?;

    let job = get_ai_analysis_job(connection, &input.job_id)?;
    let result_id = ai_analysis_result_id(&job.id);

    connection.execute(
        "
        INSERT INTO ai_analysis_results (
            id,
            ai_analysis_job_id,
            feed_item_id,
            provider_id,
            model,
            prompt_version,
            summary,
            significance,
            reasoning,
            language
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            summary = excluded.summary,
            significance = excluded.significance,
            reasoning = excluded.reasoning,
            language = excluded.language
        ",
        params![
            result_id,
            job.id,
            job.feed_item_id,
            job.provider_id,
            job.model,
            job.prompt_version,
            input.summary,
            input.significance,
            input.reasoning,
            input.language
        ],
    )?;

    connection.execute(
        "DELETE FROM ai_analysis_tags WHERE ai_analysis_result_id = ?1",
        [&result_id],
    )?;
    for tag in input.tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        connection.execute(
            "
            INSERT OR IGNORE INTO ai_analysis_tags (ai_analysis_result_id, tag)
            VALUES (?1, ?2)
            ",
            params![result_id, tag],
        )?;
    }

    connection.execute(
        "DELETE FROM ai_analysis_source_references WHERE ai_analysis_result_id = ?1",
        [&result_id],
    )?;
    for reference in input.source_references {
        validate_required_value("source_url", &reference.source_url)?;
        let reference_id = ai_analysis_source_reference_id(&result_id, &reference.source_url);
        connection.execute(
            "
            INSERT INTO ai_analysis_source_references (
                id,
                ai_analysis_result_id,
                source_url,
                label
            ) VALUES (?1, ?2, ?3, ?4)
            ",
            params![
                reference_id,
                result_id,
                reference.source_url,
                reference.label
            ],
        )?;
    }

    connection.execute(
        "
        UPDATE ai_analysis_jobs
        SET status = 'succeeded',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [&input.job_id],
    )?;

    get_ai_analysis_job(connection, &input.job_id)
}

pub(crate) fn mark_ai_analysis_job_failed(
    connection: &Connection,
    job_id: &str,
    error_code: &str,
    error: &str,
) -> StorageResult<AiAnalysisJob> {
    validate_required_value("error_code", error_code)?;
    validate_required_value("error", error)?;

    connection.execute(
        "
        UPDATE ai_analysis_jobs
        SET status = 'failed',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = ?2,
            error = ?3
        WHERE id = ?1
        ",
        params![job_id, error_code, error],
    )?;

    get_ai_analysis_job(connection, job_id)
}

pub(crate) fn get_ai_analysis_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<AiAnalysisJob> {
    let mut job = connection.query_row(
        "
        SELECT
            id,
            feed_item_id,
            prompt_preset_id,
            custom_question,
            provider_id,
            model,
            prompt_version,
            status,
            error_code,
            error,
            created_at,
            started_at,
            finished_at
        FROM ai_analysis_jobs
        WHERE id = ?1
        ",
        [job_id],
        ai_analysis_job_from_row,
    )?;

    job.result = get_ai_analysis_result_for_job(connection, &job.id)?;

    Ok(job)
}

fn get_ai_analysis_result_for_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<Option<AiAnalysisResult>> {
    let result = connection
        .query_row(
            "
            SELECT
                id,
                ai_analysis_job_id,
                feed_item_id,
                provider_id,
                model,
                prompt_version,
                summary,
                significance,
                reasoning,
                language,
                created_at
            FROM ai_analysis_results
            WHERE ai_analysis_job_id = ?1
            ",
            [job_id],
            |row| {
                Ok(AiAnalysisResult {
                    id: row.get(0)?,
                    ai_analysis_job_id: row.get(1)?,
                    feed_item_id: row.get(2)?,
                    provider_id: row.get(3)?,
                    model: row.get(4)?,
                    prompt_version: row.get(5)?,
                    summary: row.get(6)?,
                    significance: row.get(7)?,
                    reasoning: row.get(8)?,
                    language: row.get(9)?,
                    tags: Vec::new(),
                    source_references: Vec::new(),
                    created_at: row.get(10)?,
                })
            },
        )
        .optional()?;

    match result {
        Some(mut result) => {
            result.tags = list_ai_analysis_tags(connection, &result.id)?;
            result.source_references = list_ai_analysis_source_references(connection, &result.id)?;
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

fn list_ai_analysis_tags(connection: &Connection, result_id: &str) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT tag
        FROM ai_analysis_tags
        WHERE ai_analysis_result_id = ?1
        ORDER BY tag
        ",
    )?;
    let rows = statement.query_map([result_id], |row| row.get(0))?;

    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn list_ai_analysis_source_references(
    connection: &Connection,
    result_id: &str,
) -> StorageResult<Vec<AiAnalysisSourceReference>> {
    let mut statement = connection.prepare(
        "
        SELECT id, source_url, label, created_at
        FROM ai_analysis_source_references
        WHERE ai_analysis_result_id = ?1
        ORDER BY created_at, id
        ",
    )?;
    let rows = statement.query_map([result_id], |row| {
        Ok(AiAnalysisSourceReference {
            id: row.get(0)?,
            source_url: row.get(1)?,
            label: row.get(2)?,
            created_at: row.get(3)?,
        })
    })?;

    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn ai_analysis_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiAnalysisJob> {
    Ok(AiAnalysisJob {
        id: row.get(0)?,
        feed_item_id: row.get(1)?,
        prompt_preset_id: row.get(2)?,
        custom_question: row.get(3)?,
        provider_id: row.get(4)?,
        model: row.get(5)?,
        prompt_version: row.get(6)?,
        status: row.get(7)?,
        error_code: row.get(8)?,
        error: row.get(9)?,
        created_at: row.get(10)?,
        started_at: row.get(11)?,
        finished_at: row.get(12)?,
        result: None,
    })
}

fn validate_required_value(key: &'static str, value: &str) -> StorageResult<()> {
    if value.trim().is_empty() {
        Err(StorageError::InvalidAiAnalysisValue {
            key,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_allowed_analysis_value(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidAiAnalysisValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn ai_analysis_job_id(
    feed_item_id: &str,
    prompt_preset_id: &str,
    custom_question: Option<&str>,
) -> String {
    let custom_question = custom_question.unwrap_or("default");
    format!(
        "ai_job_{}_{}_{}",
        super::slug_part(feed_item_id),
        super::slug_part(prompt_preset_id),
        super::slug_part(custom_question)
    )
}

fn ai_analysis_result_id(job_id: &str) -> String {
    format!("ai_result_{}", super::slug_part(job_id))
}

fn ai_analysis_source_reference_id(result_id: &str, source_url: &str) -> String {
    format!(
        "ai_ref_{}_{}",
        super::slug_part(result_id),
        super::slug_part(source_url)
    )
}

use super::database::Database;
/// ai_analysis domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::ai_analysis()`.
#[derive(Clone)]
pub struct AiAnalysisStore {
    db: Database,
}

impl AiAnalysisStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_ai_analysis_job(&self, input: NewAiAnalysisJob) -> StorageResult<AiAnalysisJob> {
        let connection = self.db.checkout()?;

        create_ai_analysis_job(&connection, input)
    }

    pub fn list_ai_analysis_jobs(&self, feed_item_id: &str) -> StorageResult<Vec<AiAnalysisJob>> {
        let connection = self.db.checkout()?;

        list_ai_analysis_jobs(&connection, feed_item_id)
    }

    pub fn get_ai_analysis_job(&self, job_id: &str) -> StorageResult<AiAnalysisJob> {
        let connection = self.db.checkout()?;

        get_ai_analysis_job(&connection, job_id)
    }

    pub fn mark_ai_analysis_job_running(&self, job_id: &str) -> StorageResult<AiAnalysisJob> {
        let connection = self.db.checkout()?;

        mark_ai_analysis_job_running(&connection, job_id)
    }

    pub fn complete_ai_analysis_job(
        &self,
        input: CompletedAiAnalysis,
    ) -> StorageResult<AiAnalysisJob> {
        let connection = self.db.checkout()?;

        complete_ai_analysis_job(&connection, input)
    }

    pub fn mark_ai_analysis_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<AiAnalysisJob> {
        let connection = self.db.checkout()?;

        mark_ai_analysis_job_failed(&connection, job_id, error_code, error)
    }
}
