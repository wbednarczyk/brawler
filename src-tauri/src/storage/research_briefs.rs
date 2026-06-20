use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{
    research, slug_part, ResearchEvidenceInput, ResearchEvidenceItem, StorageError, StorageResult,
};

pub const RESEARCH_BRIEF_PROMPT_VERSION: &str = "m30.research_brief.v1";
pub const RESEARCH_BRIEF_COLLECTOR_VERSION: &str = "m30.collector.v1";
pub const RESEARCH_BRIEF_RENDERER_VERSION: &str = "m30.renderer.v1";

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchBriefScopeInput {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchBriefJob {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    pub evidence_collector_version: String,
    pub renderer_version: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchBriefJobStatus")
    )]
    pub status: String,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub brief: Option<ResearchBrief>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchBrief {
    pub id: String,
    pub job_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    pub evidence_collector_version: String,
    pub renderer_version: String,
    pub title: String,
    pub summary: String,
    pub content_markdown: String,
    pub language: Option<String>,
    pub generated_at: String,
    pub created_at: String,
    pub citations: Vec<ResearchBriefCitation>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchBriefCitation {
    pub id: String,
    pub brief_id: String,
    pub citation_key: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchEvidenceType")
    )]
    pub evidence_type: String,
    pub evidence_id: String,
    pub label: String,
    pub snippet: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewResearchBriefJob {
    pub scope_type: String,
    pub scope_id: String,
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedResearchBrief {
    pub job_id: String,
    pub title: String,
    pub summary: String,
    pub content_markdown: String,
    pub language: Option<String>,
    pub citations: Vec<NewResearchBriefCitation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewResearchBriefCitation {
    pub citation_key: String,
    pub evidence_type: String,
    pub evidence_id: String,
    pub label: String,
    pub snippet: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResearchBriefEvidenceContext {
    pub scope_type: String,
    pub scope_id: String,
    pub evidence_items: Vec<ResearchEvidenceItem>,
}

pub(super) fn create_research_brief_job(
    connection: &Connection,
    input: NewResearchBriefJob,
) -> StorageResult<ResearchBriefJob> {
    validate_scope(connection, &input.scope_type, &input.scope_id)?;
    validate_required_value("provider_id", &input.provider_id)?;
    validate_required_value("model", &input.model)?;

    let job_id = next_research_brief_job_id(connection, &input.scope_type, &input.scope_id)?;

    connection.execute(
        "
        INSERT INTO ai_research_brief_jobs (
            id,
            scope_type,
            scope_id,
            provider_id,
            model,
            prompt_version,
            evidence_collector_version,
            renderer_version,
            status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued')
        ",
        params![
            job_id,
            input.scope_type,
            input.scope_id,
            input.provider_id,
            input.model,
            RESEARCH_BRIEF_PROMPT_VERSION,
            RESEARCH_BRIEF_COLLECTOR_VERSION,
            RESEARCH_BRIEF_RENDERER_VERSION
        ],
    )?;

    get_research_brief_job(connection, &job_id)
}

pub(super) fn list_research_brief_jobs(
    connection: &Connection,
    input: ResearchBriefScopeInput,
) -> StorageResult<Vec<ResearchBriefJob>> {
    validate_scope(connection, &input.scope_type, &input.scope_id)?;
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            scope_type,
            scope_id,
            provider_id,
            model,
            prompt_version,
            evidence_collector_version,
            renderer_version,
            status,
            error_code,
            error,
            created_at,
            started_at,
            finished_at
        FROM ai_research_brief_jobs
        WHERE scope_type = ?1 AND scope_id = ?2
        ORDER BY created_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map(
        params![input.scope_type, input.scope_id],
        research_brief_job_from_row,
    )?;
    let mut jobs = rows.collect::<Result<Vec<_>, _>>()?;
    for job in &mut jobs {
        job.brief = get_research_brief_for_job(connection, &job.id)?;
    }
    Ok(jobs)
}

pub(super) fn get_research_brief_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ResearchBriefJob> {
    let mut job = connection.query_row(
        "
        SELECT
            id,
            scope_type,
            scope_id,
            provider_id,
            model,
            prompt_version,
            evidence_collector_version,
            renderer_version,
            status,
            error_code,
            error,
            created_at,
            started_at,
            finished_at
        FROM ai_research_brief_jobs
        WHERE id = ?1
        ",
        [job_id],
        research_brief_job_from_row,
    )?;
    job.brief = get_research_brief_for_job(connection, &job.id)?;
    Ok(job)
}

pub(super) fn collect_research_brief_evidence(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ResearchBriefEvidenceContext> {
    let job = get_research_brief_job(connection, job_id)?;
    let timeline = match job.scope_type.as_str() {
        "company" => research::list_research_evidence(
            connection,
            ResearchEvidenceInput {
                company_id: Some(job.scope_id.clone()),
                watchlist_id: None,
                evidence_types: Some(vec![
                    "feed_item".to_owned(),
                    "notebook_entry".to_owned(),
                    "claim".to_owned(),
                    "company_event".to_owned(),
                    "transcript_segment".to_owned(),
                    "ai_analysis".to_owned(),
                    "research_question".to_owned(),
                ]),
                changed_since_review_only: None,
                limit: Some(80),
            },
        )?,
        "watchlist" => research::list_research_evidence(
            connection,
            ResearchEvidenceInput {
                company_id: None,
                watchlist_id: Some(job.scope_id.clone()),
                evidence_types: Some(vec![
                    "feed_item".to_owned(),
                    "notebook_entry".to_owned(),
                    "claim".to_owned(),
                    "company_event".to_owned(),
                    "transcript_segment".to_owned(),
                    "ai_analysis".to_owned(),
                    "research_question".to_owned(),
                ]),
                changed_since_review_only: None,
                limit: Some(120),
            },
        )?,
        _ => {
            return Err(StorageError::InvalidResearchValue {
                key: "scope_type",
                value: job.scope_type,
            });
        }
    };

    Ok(ResearchBriefEvidenceContext {
        scope_type: job.scope_type,
        scope_id: job.scope_id,
        evidence_items: timeline.items,
    })
}

pub(super) fn mark_research_brief_job_running(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ResearchBriefJob> {
    connection.execute(
        "
        UPDATE ai_research_brief_jobs
        SET status = 'running',
            started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;

    get_research_brief_job(connection, job_id)
}

pub(super) fn complete_research_brief_job(
    connection: &Connection,
    input: CompletedResearchBrief,
) -> StorageResult<ResearchBriefJob> {
    validate_required_value("title", &input.title)?;
    validate_required_value("summary", &input.summary)?;
    validate_required_value("content_markdown", &input.content_markdown)?;
    if input.citations.is_empty() {
        return Err(StorageError::InvalidResearchValue {
            key: "citations",
            value: "empty".to_owned(),
        });
    }

    let job = get_research_brief_job(connection, &input.job_id)?;
    let brief_id = next_research_brief_id(connection, &job.scope_type, &job.scope_id)?;

    connection.execute(
        "
        INSERT INTO ai_research_briefs (
            id,
            job_id,
            scope_type,
            scope_id,
            provider_id,
            model,
            prompt_version,
            evidence_collector_version,
            renderer_version,
            title,
            summary,
            content_markdown,
            language
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ",
        params![
            brief_id,
            job.id,
            job.scope_type,
            job.scope_id,
            job.provider_id,
            job.model,
            job.prompt_version,
            job.evidence_collector_version,
            job.renderer_version,
            input.title.trim(),
            input.summary.trim(),
            input.content_markdown.trim(),
            input.language
        ],
    )?;

    for citation in input.citations {
        validate_required_value("citation_key", &citation.citation_key)?;
        validate_required_value("evidence_type", &citation.evidence_type)?;
        validate_required_value("evidence_id", &citation.evidence_id)?;
        validate_required_value("label", &citation.label)?;
        let citation_id = research_brief_citation_id(&brief_id, &citation.citation_key);
        connection.execute(
            "
            INSERT INTO ai_research_brief_citations (
                id,
                brief_id,
                citation_key,
                evidence_type,
                evidence_id,
                label,
                snippet
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                citation_id,
                brief_id,
                citation.citation_key.trim(),
                citation.evidence_type.trim(),
                citation.evidence_id.trim(),
                citation.label.trim(),
                citation.snippet
            ],
        )?;
    }

    connection.execute(
        "
        UPDATE ai_research_brief_jobs
        SET status = 'succeeded',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [&input.job_id],
    )?;

    get_research_brief_job(connection, &input.job_id)
}

pub(super) fn mark_research_brief_job_failed(
    connection: &Connection,
    job_id: &str,
    error_code: &str,
    error: &str,
) -> StorageResult<ResearchBriefJob> {
    validate_required_value("error_code", error_code)?;
    validate_required_value("error", error)?;

    connection.execute(
        "
        UPDATE ai_research_brief_jobs
        SET status = 'failed',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = ?2,
            error = ?3
        WHERE id = ?1
        ",
        params![job_id, error_code, error],
    )?;

    get_research_brief_job(connection, job_id)
}

fn get_research_brief_for_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<Option<ResearchBrief>> {
    let brief = connection
        .query_row(
            "
            SELECT
                id,
                job_id,
                scope_type,
                scope_id,
                provider_id,
                model,
                prompt_version,
                evidence_collector_version,
                renderer_version,
                title,
                summary,
                content_markdown,
                language,
                generated_at,
                created_at
            FROM ai_research_briefs
            WHERE job_id = ?1
            ",
            [job_id],
            research_brief_from_row,
        )
        .optional()?;

    match brief {
        Some(mut brief) => {
            brief.citations = list_research_brief_citations(connection, &brief.id)?;
            Ok(Some(brief))
        }
        None => Ok(None),
    }
}

fn list_research_brief_citations(
    connection: &Connection,
    brief_id: &str,
) -> StorageResult<Vec<ResearchBriefCitation>> {
    let mut statement = connection.prepare(
        "
        SELECT id, brief_id, citation_key, evidence_type, evidence_id, label, snippet, created_at
        FROM ai_research_brief_citations
        WHERE brief_id = ?1
        ORDER BY citation_key, id
        ",
    )?;
    let rows = statement.query_map([brief_id], research_brief_citation_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn research_brief_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchBriefJob> {
    Ok(ResearchBriefJob {
        id: row.get(0)?,
        scope_type: row.get(1)?,
        scope_id: row.get(2)?,
        provider_id: row.get(3)?,
        model: row.get(4)?,
        prompt_version: row.get(5)?,
        evidence_collector_version: row.get(6)?,
        renderer_version: row.get(7)?,
        status: row.get(8)?,
        error_code: row.get(9)?,
        error: row.get(10)?,
        created_at: row.get(11)?,
        started_at: row.get(12)?,
        finished_at: row.get(13)?,
        brief: None,
    })
}

fn research_brief_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchBrief> {
    Ok(ResearchBrief {
        id: row.get(0)?,
        job_id: row.get(1)?,
        scope_type: row.get(2)?,
        scope_id: row.get(3)?,
        provider_id: row.get(4)?,
        model: row.get(5)?,
        prompt_version: row.get(6)?,
        evidence_collector_version: row.get(7)?,
        renderer_version: row.get(8)?,
        title: row.get(9)?,
        summary: row.get(10)?,
        content_markdown: row.get(11)?,
        language: row.get(12)?,
        generated_at: row.get(13)?,
        created_at: row.get(14)?,
        citations: Vec::new(),
    })
}

fn research_brief_citation_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ResearchBriefCitation> {
    Ok(ResearchBriefCitation {
        id: row.get(0)?,
        brief_id: row.get(1)?,
        citation_key: row.get(2)?,
        evidence_type: row.get(3)?,
        evidence_id: row.get(4)?,
        label: row.get(5)?,
        snippet: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn validate_scope(connection: &Connection, scope_type: &str, scope_id: &str) -> StorageResult<()> {
    validate_required_value("scope_id", scope_id)?;
    match scope_type {
        "company" => validate_reference_exists(connection, "companies", scope_id),
        "watchlist" => validate_reference_exists(connection, "watchlists", scope_id),
        _ => Err(StorageError::InvalidResearchValue {
            key: "scope_type",
            value: scope_type.to_owned(),
        }),
    }
}

fn validate_reference_exists(
    connection: &Connection,
    table_name: &str,
    id: &str,
) -> StorageResult<()> {
    let exists: bool = connection.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)"),
        [id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::InvalidResearchValue {
            key: "scope_id",
            value: id.to_owned(),
        })
    }
}

fn validate_required_value(key: &'static str, value: &str) -> StorageResult<()> {
    if value.trim().is_empty() {
        Err(StorageError::InvalidResearchValue {
            key,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn next_research_brief_job_id(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<String> {
    let prefix = format!(
        "research_brief_job_{}_{}",
        slug_part(scope_type),
        slug_part(scope_id)
    );
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM ai_research_brief_jobs WHERE id LIKE ?1",
        [format!("{prefix}_%")],
        |row| row.get(0),
    )?;
    Ok(format!("{prefix}_{}", count + 1))
}

fn next_research_brief_id(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<String> {
    let prefix = format!(
        "research_brief_{}_{}",
        slug_part(scope_type),
        slug_part(scope_id)
    );
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM ai_research_briefs WHERE id LIKE ?1",
        [format!("{prefix}_%")],
        |row| row.get(0),
    )?;
    Ok(format!("{prefix}_{}", count + 1))
}

fn research_brief_citation_id(brief_id: &str, citation_key: &str) -> String {
    format!(
        "research_brief_citation_{}_{}",
        slug_part(brief_id),
        slug_part(citation_key)
    )
}

use super::database::Database;
/// research_briefs domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::research_briefs()`.
#[derive(Clone)]
pub struct ResearchBriefStore {
    db: Database,
}

impl ResearchBriefStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_research_brief_job(
        &self,
        input: NewResearchBriefJob,
    ) -> StorageResult<ResearchBriefJob> {
        let connection = self.db.checkout()?;

        create_research_brief_job(&connection, input)
    }

    pub fn list_research_brief_jobs(
        &self,
        input: ResearchBriefScopeInput,
    ) -> StorageResult<Vec<ResearchBriefJob>> {
        let connection = self.db.checkout()?;

        list_research_brief_jobs(&connection, input)
    }

    pub fn get_research_brief_job(&self, job_id: &str) -> StorageResult<ResearchBriefJob> {
        let connection = self.db.checkout()?;

        get_research_brief_job(&connection, job_id)
    }

    pub fn collect_research_brief_evidence(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchBriefEvidenceContext> {
        let connection = self.db.checkout()?;

        collect_research_brief_evidence(&connection, job_id)
    }

    pub fn mark_research_brief_job_running(&self, job_id: &str) -> StorageResult<ResearchBriefJob> {
        let connection = self.db.checkout()?;

        mark_research_brief_job_running(&connection, job_id)
    }

    pub fn complete_research_brief_job(
        &self,
        input: CompletedResearchBrief,
    ) -> StorageResult<ResearchBriefJob> {
        let connection = self.db.checkout()?;

        complete_research_brief_job(&connection, input)
    }

    pub fn mark_research_brief_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ResearchBriefJob> {
        let connection = self.db.checkout()?;

        mark_research_brief_job_failed(&connection, job_id, error_code, error)
    }
}
