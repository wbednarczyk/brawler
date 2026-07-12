use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{
    quality_frameworks, research, research_reminders, slug_part, NewResearchBriefCitation,
    ResearchEvidenceInput, ResearchEvidenceItem, ResearchReminderListInput, StorageError,
    StorageResult,
};

pub const RESEARCH_DIGEST_PROMPT_VERSION: &str = "m31.research_digest.v2";
pub const RESEARCH_DIGEST_COLLECTOR_VERSION: &str = "m31.digest_collector.v1";
pub const RESEARCH_DIGEST_RENDERER_VERSION: &str = "m31.digest_renderer.v1";

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDigestScopeInput {
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
pub struct ResearchDigestJob {
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
    pub digest: Option<ResearchDigest>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDigest {
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
    pub citations: Vec<ResearchDigestCitation>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchDigestCitation {
    pub id: String,
    pub digest_id: String,
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
pub struct NewResearchDigestJob {
    pub scope_type: String,
    pub scope_id: String,
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletedResearchDigest {
    pub job_id: String,
    pub title: String,
    pub summary: String,
    pub content_markdown: String,
    pub language: Option<String>,
    pub citations: Vec<NewResearchBriefCitation>,
}

#[derive(Debug, Clone)]
pub struct ResearchDigestEvidenceContext {
    pub scope_type: String,
    pub scope_id: String,
    pub evidence_items: Vec<ResearchEvidenceItem>,
}

pub(super) fn create_research_digest_job(
    connection: &Connection,
    input: NewResearchDigestJob,
) -> StorageResult<ResearchDigestJob> {
    validate_scope(connection, &input.scope_type, &input.scope_id)?;
    let job_id = next_research_digest_job_id(connection, &input.scope_type, &input.scope_id)?;
    connection.execute(
        "
        INSERT INTO ai_research_digest_jobs (
            id, scope_type, scope_id, provider_id, model, prompt_version,
            evidence_collector_version, renderer_version, status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued')
        ",
        params![
            job_id,
            input.scope_type,
            input.scope_id,
            input.provider_id,
            input.model,
            RESEARCH_DIGEST_PROMPT_VERSION,
            RESEARCH_DIGEST_COLLECTOR_VERSION,
            RESEARCH_DIGEST_RENDERER_VERSION
        ],
    )?;
    get_research_digest_job(connection, &job_id)
}

pub(super) fn list_research_digest_jobs(
    connection: &Connection,
    input: ResearchDigestScopeInput,
) -> StorageResult<Vec<ResearchDigestJob>> {
    validate_scope(connection, &input.scope_type, &input.scope_id)?;
    let mut statement = connection.prepare(
        "
        SELECT id, scope_type, scope_id, provider_id, model, prompt_version,
            evidence_collector_version, renderer_version, status, error_code, error,
            created_at, started_at, finished_at
        FROM ai_research_digest_jobs
        WHERE scope_type = ?1 AND scope_id = ?2
        ORDER BY created_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map(
        params![input.scope_type, input.scope_id],
        research_digest_job_from_row,
    )?;
    let mut jobs = rows.collect::<Result<Vec<_>, _>>()?;
    for job in &mut jobs {
        job.digest = get_research_digest_for_job(connection, &job.id)?;
    }
    Ok(jobs)
}

pub(super) fn get_research_digest_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ResearchDigestJob> {
    let mut job = connection.query_row(
        "
        SELECT id, scope_type, scope_id, provider_id, model, prompt_version,
            evidence_collector_version, renderer_version, status, error_code, error,
            created_at, started_at, finished_at
        FROM ai_research_digest_jobs
        WHERE id = ?1
        ",
        [job_id],
        research_digest_job_from_row,
    )?;
    job.digest = get_research_digest_for_job(connection, &job.id)?;
    Ok(job)
}

/// Build a synthetic digest evidence row — one not produced by the timeline SQL
/// but injected by the collector (open reminders and qualitative framework-verdict
/// changes). `source_url` is always `None` for synthetic items; `review_state` is
/// supplied by the caller because it differs by kind: reminders are always-visible,
/// while framework verdicts are gated on the company review checkpoint (F4).
#[allow(clippy::too_many_arguments)]
fn synthetic_evidence_item(
    id: String,
    evidence_type: &str,
    source_domain: &str,
    source_id: String,
    company_id: String,
    occurred_at: String,
    title: String,
    summary: Option<String>,
    attribution: Option<String>,
    trust_category: &str,
    review_state: super::ResearchEvidenceReviewState,
) -> ResearchEvidenceItem {
    ResearchEvidenceItem {
        id,
        evidence_type: evidence_type.to_owned(),
        source_domain: source_domain.to_owned(),
        source_id,
        company_id,
        occurred_at,
        title,
        summary,
        source_url: None,
        attribution,
        trust_category: trust_category.to_owned(),
        review_state,
    }
}

pub(super) fn collect_research_digest_evidence(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ResearchDigestEvidenceContext> {
    let job = get_research_digest_job(connection, job_id)?;
    let mut timeline = research::list_research_evidence(
        connection,
        match job.scope_type.as_str() {
            "company" => ResearchEvidenceInput {
                company_id: Some(job.scope_id.clone()),
                watchlist_id: None,
                evidence_types: None,
                changed_since_review_only: Some(true),
                limit: Some(80),
            },
            "watchlist" => ResearchEvidenceInput {
                company_id: None,
                watchlist_id: Some(job.scope_id.clone()),
                evidence_types: None,
                changed_since_review_only: Some(true),
                limit: Some(120),
            },
            _ => {
                return Err(StorageError::InvalidSettingValue {
                    key: "scope_type",
                    value: job.scope_type,
                })
            }
        },
    )?;
    let reminders = research_reminders::list_research_reminders(
        connection,
        ResearchReminderListInput {
            scope_type: job.scope_type.clone(),
            scope_id: job.scope_id.clone(),
            status: Some("open".to_owned()),
        },
    )?;
    timeline
        .items
        .extend(reminders.into_iter().take(40).map(|reminder| {
            synthetic_evidence_item(
                format!("evidence_reminder_{}", reminder.id),
                "reminder",
                "research",
                reminder.id,
                reminder
                    .company_id
                    .unwrap_or_else(|| reminder.scope_id.clone()),
                reminder
                    .due_at
                    .clone()
                    .unwrap_or_else(|| reminder.updated_at.clone()),
                reminder.title,
                Some(reminder.body),
                reminder.source_type,
                "user_note",
                // Reminders are always-visible in the digest (unbounded by review).
                super::ResearchEvidenceReviewState {
                    changed_since_company_review: true,
                    changed_since_watchlist_review: true,
                },
            )
        }));

    // Qualitative verdict changes (ADR 0075 Decision 5, §T6c): for a company
    // digest, surface each criterion whose agent verdict changed since its
    // previous assessment as a synthetic `framework_verdict` evidence item. Scope
    // is company-only (watchlist digests aggregate per-company evidence; a
    // watchlist roll-up of framework changes is a follow-up).
    if job.scope_type == "company" {
        // F4: bound synthetic `framework_verdict` items by the SAME review-checkpoint
        // semantics real timeline evidence uses. A verdict change is included only
        // when its `changed_at` is strictly newer than the company's active review
        // checkpoint — the very checkpoint `changed_since_active_review` consults via
        // `review_checkpoint_timestamp` / `changed_since_checkpoint`. No checkpoint
        // recorded ⇒ everything is unreviewed ⇒ include. Without this bound, a
        // one-time pass→fail change would re-emit into every subsequent (user-
        // rerunnable) digest and "mark reviewed" could never suppress it.
        let company_checkpoint =
            research::review_checkpoint_timestamp(connection, "company", &job.scope_id)?;
        for framework in
            quality_frameworks::frameworks_with_qualitative_assessments(connection, &job.scope_id)?
        {
            for change in quality_frameworks::qualitative_verdict_changes(
                connection,
                &framework.framework_id,
                &job.scope_id,
            )? {
                let changed_since_company_review = research::changed_since_checkpoint(
                    &change.changed_at,
                    company_checkpoint.as_deref(),
                );
                if !changed_since_company_review {
                    continue;
                }
                timeline.items.push(synthetic_evidence_item(
                    format!(
                        "evidence_framework_verdict_{}_{}",
                        framework.framework_id, change.criterion_id
                    ),
                    "framework_verdict",
                    "quality_frameworks",
                    change.criterion_id.clone(),
                    job.scope_id.clone(),
                    change.changed_at.clone(),
                    format!(
                        "{} verdict changed: {} → {}",
                        change.label, change.previous_verdict, change.current_verdict
                    ),
                    Some(format!(
                        "{} — {} changed from {} to {}.",
                        framework.framework_name,
                        change.label,
                        change.previous_verdict,
                        change.current_verdict
                    )),
                    Some(framework.framework_name.clone()),
                    "ai_generated",
                    super::ResearchEvidenceReviewState {
                        // The change survived the checkpoint filter above, so this is
                        // honestly `true` (computed, not hardcoded). The watchlist flag
                        // mirrors the reminder path (always-visible in the aggregate):
                        // framework verdicts are company-scoped and don't gate a
                        // watchlist digest.
                        changed_since_company_review,
                        changed_since_watchlist_review: true,
                    },
                ));
            }
        }
    }

    Ok(ResearchDigestEvidenceContext {
        scope_type: job.scope_type,
        scope_id: job.scope_id,
        evidence_items: timeline.items,
    })
}

pub(super) fn mark_research_digest_job_running(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ResearchDigestJob> {
    connection.execute(
        "
        UPDATE ai_research_digest_jobs
        SET status = 'running',
            started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;
    get_research_digest_job(connection, job_id)
}

pub(super) fn complete_research_digest_job(
    connection: &Connection,
    input: CompletedResearchDigest,
) -> StorageResult<ResearchDigestJob> {
    let job = get_research_digest_job(connection, &input.job_id)?;
    let digest_id = next_research_digest_id(connection, &job.scope_type, &job.scope_id)?;
    let transaction = connection.unchecked_transaction()?;
    transaction.execute(
        "
        INSERT INTO ai_research_digests (
            id, job_id, scope_type, scope_id, provider_id, model, prompt_version,
            evidence_collector_version, renderer_version, title, summary, content_markdown, language
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
        ",
        params![
            digest_id,
            input.job_id,
            job.scope_type,
            job.scope_id,
            job.provider_id,
            job.model,
            job.prompt_version,
            job.evidence_collector_version,
            job.renderer_version,
            input.title,
            input.summary,
            input.content_markdown,
            input.language
        ],
    )?;
    for citation in input.citations {
        transaction.execute(
            "
            INSERT INTO ai_research_digest_citations (
                id, digest_id, citation_key, evidence_type, evidence_id, label, snippet
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ",
            params![
                research_digest_citation_id(&digest_id, &citation.citation_key),
                digest_id,
                citation.citation_key,
                citation.evidence_type,
                citation.evidence_id,
                citation.label,
                citation.snippet
            ],
        )?;
    }
    transaction.execute(
        "
        UPDATE ai_research_digest_jobs
        SET status = 'succeeded',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [input.job_id.clone()],
    )?;
    transaction.commit()?;
    get_research_digest_job(connection, &input.job_id)
}

pub(super) fn mark_research_digest_job_failed(
    connection: &Connection,
    job_id: &str,
    error_code: &str,
    error: &str,
) -> StorageResult<ResearchDigestJob> {
    connection.execute(
        "
        UPDATE ai_research_digest_jobs
        SET status = 'failed',
            error_code = ?2,
            error = ?3,
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![job_id, error_code, error],
    )?;
    get_research_digest_job(connection, job_id)
}

fn get_research_digest_for_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<Option<ResearchDigest>> {
    let mut digest = connection
        .query_row(
            "
            SELECT id, job_id, scope_type, scope_id, provider_id, model, prompt_version,
                evidence_collector_version, renderer_version, title, summary, content_markdown,
                language, generated_at, created_at
            FROM ai_research_digests
            WHERE job_id = ?1
            ORDER BY generated_at DESC
            LIMIT 1
            ",
            [job_id],
            research_digest_from_row,
        )
        .optional()?;
    if let Some(digest) = digest.as_mut() {
        digest.citations = list_research_digest_citations(connection, &digest.id)?;
    }
    Ok(digest)
}

fn list_research_digest_citations(
    connection: &Connection,
    digest_id: &str,
) -> StorageResult<Vec<ResearchDigestCitation>> {
    let mut statement = connection.prepare(
        "
        SELECT id, digest_id, citation_key, evidence_type, evidence_id, label, snippet, created_at
        FROM ai_research_digest_citations
        WHERE digest_id = ?1
        ORDER BY citation_key
        ",
    )?;
    let rows = statement.query_map([digest_id], research_digest_citation_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub fn completed_digest_from_provider_output(
    job_id: &str,
    evidence_items: &[ResearchEvidenceItem],
    output: crate::providers::analysis::ResearchBriefProviderOutput,
) -> Result<CompletedResearchDigest, String> {
    // Shared citation-integrity reference set (single home: research module).
    let evidence_refs = research::supplied_evidence_refs(evidence_items);
    if output.sections.is_empty() {
        return Err("Research digest did not include sections.".to_owned());
    }
    let citations = output
        .citations
        .into_iter()
        .map(|citation| {
            if !research::citation_resolves(
                &evidence_refs,
                &citation.evidence_type,
                &citation.evidence_id,
            ) {
                return Err(format!(
                    "Research digest citation {} references unavailable evidence.",
                    citation.citation_key
                ));
            }
            Ok(NewResearchBriefCitation {
                citation_key: citation.citation_key,
                evidence_type: citation.evidence_type,
                evidence_id: citation.evidence_id,
                label: citation.label,
                snippet: citation.snippet,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if citations.is_empty() {
        return Err("Research digest did not include citations.".to_owned());
    }
    let content_markdown = output
        .sections
        .iter()
        .map(|section| {
            let citations = section
                .citation_keys
                .iter()
                .map(|key| format!("[{key}]"))
                .collect::<Vec<_>>()
                .join(" ");
            format!("## {}\n\n{} {}", section.heading, section.body, citations)
                .trim()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Ok(CompletedResearchDigest {
        job_id: job_id.to_owned(),
        title: output.title,
        summary: output.summary,
        content_markdown,
        language: output.language,
        citations,
    })
}

fn research_digest_job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchDigestJob> {
    Ok(ResearchDigestJob {
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
        digest: None,
    })
}

fn research_digest_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchDigest> {
    Ok(ResearchDigest {
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

fn research_digest_citation_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ResearchDigestCitation> {
    Ok(ResearchDigestCitation {
        id: row.get(0)?,
        digest_id: row.get(1)?,
        citation_key: row.get(2)?,
        evidence_type: row.get(3)?,
        evidence_id: row.get(4)?,
        label: row.get(5)?,
        snippet: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn validate_scope(connection: &Connection, scope_type: &str, scope_id: &str) -> StorageResult<()> {
    let table_name = match scope_type {
        "company" => "companies",
        "watchlist" => "watchlists",
        _ => {
            return Err(StorageError::InvalidSettingValue {
                key: "scope_type",
                value: scope_type.to_owned(),
            })
        }
    };
    let exists: bool = connection.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)"),
        [scope_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(StorageError::MissingResearchReference {
            table: table_name.to_owned(),
            id: scope_id.to_owned(),
        });
    }
    Ok(())
}

fn next_research_digest_job_id(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<String> {
    let prefix = format!(
        "research_digest_job_{}_{}",
        slug_part(scope_type),
        slug_part(scope_id)
    );
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM ai_research_digest_jobs WHERE id LIKE ?1",
        [format!("{prefix}_%")],
        |row| row.get(0),
    )?;
    Ok(format!("{prefix}_{:03}", count + 1))
}

fn next_research_digest_id(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<String> {
    let prefix = format!(
        "research_digest_{}_{}",
        slug_part(scope_type),
        slug_part(scope_id)
    );
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM ai_research_digests WHERE id LIKE ?1",
        [format!("{prefix}_%")],
        |row| row.get(0),
    )?;
    Ok(format!("{prefix}_{:03}", count + 1))
}

fn research_digest_citation_id(digest_id: &str, citation_key: &str) -> String {
    format!(
        "research_digest_citation_{}_{}",
        slug_part(digest_id),
        slug_part(citation_key)
    )
}

use super::database::Database;
/// research_digests domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::research_digests()`.
#[derive(Clone)]
pub struct ResearchDigestStore {
    db: Database,
}

impl ResearchDigestStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_research_digest_job(
        &self,
        input: NewResearchDigestJob,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.db.checkout()?;

        create_research_digest_job(&connection, input)
    }

    pub fn list_research_digest_jobs(
        &self,
        input: ResearchDigestScopeInput,
    ) -> StorageResult<Vec<ResearchDigestJob>> {
        let connection = self.db.checkout()?;

        list_research_digest_jobs(&connection, input)
    }

    pub fn get_research_digest_job(&self, job_id: &str) -> StorageResult<ResearchDigestJob> {
        let connection = self.db.checkout()?;

        get_research_digest_job(&connection, job_id)
    }

    pub fn collect_research_digest_evidence(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchDigestEvidenceContext> {
        let connection = self.db.checkout()?;

        collect_research_digest_evidence(&connection, job_id)
    }

    pub fn mark_research_digest_job_running(
        &self,
        job_id: &str,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.db.checkout()?;

        mark_research_digest_job_running(&connection, job_id)
    }

    pub fn complete_research_digest_job(
        &self,
        input: CompletedResearchDigest,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.db.checkout()?;

        complete_research_digest_job(&connection, input)
    }

    pub fn mark_research_digest_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<ResearchDigestJob> {
        let connection = self.db.checkout()?;

        mark_research_digest_job_failed(&connection, job_id, error_code, error)
    }
}
