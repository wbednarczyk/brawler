use super::sources;
use super::*;

const EVIDENCE_TYPES: &[&str] = &[
    "feed_item",
    "notebook_entry",
    "claim",
    "transcript_segment",
    "company_event",
    "ai_analysis",
    "research_question",
    "company_signal",
];

const RELATION_TYPES: &[&str] = &[
    "originates_from",
    "cites",
    "supports",
    "contradicts",
    "updates",
    "follows_up",
    "answers",
    "related",
];

const RESEARCH_QUESTION_STATUSES: &[&str] = &["open", "answered", "closed"];

pub(super) fn list_research_evidence(
    connection: &Connection,
    input: ResearchEvidenceInput,
) -> StorageResult<ResearchTimelineResult> {
    let limit = input.limit.unwrap_or(100).clamp(1, 500);
    let evidence_types = input
        .evidence_types
        .as_ref()
        .map(|values| {
            values
                .iter()
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty());
    if let Some(values) = evidence_types.as_ref() {
        for value in values {
            validate_allowed_research_value("evidence_type", value, EVIDENCE_TYPES)?;
        }
    }
    let company_checkpoint = input
        .company_id
        .as_deref()
        .and_then(|company_id| review_checkpoint_timestamp(connection, "company", company_id).ok())
        .flatten();
    let watchlist_checkpoint = input
        .watchlist_id
        .as_deref()
        .and_then(|watchlist_id| {
            review_checkpoint_timestamp(connection, "watchlist", watchlist_id).ok()
        })
        .flatten();
    let active_checkpoint = input
        .company_id
        .as_ref()
        .map(|_| company_checkpoint.clone())
        .or_else(|| {
            input
                .watchlist_id
                .as_ref()
                .map(|_| watchlist_checkpoint.clone())
        })
        .flatten();

    let mut statement = connection.prepare(
        "
        WITH scope_companies AS (
            SELECT companies.id AS company_id
            FROM companies
            WHERE (?1 IS NULL OR companies.id = ?1)
                AND (
                    ?2 IS NULL
                    OR companies.id IN (
                        SELECT company_id
                        FROM watchlist_companies
                        WHERE watchlist_id = ?2
                    )
                )
        ),
        evidence AS (
            SELECT
                'evidence_feed_' || feed_items.id AS id,
                'feed_item' AS evidence_type,
                'feed' AS source_domain,
                feed_items.id AS source_id,
                feed_item_companies.company_id AS company_id,
                COALESCE(feed_items.published_at, feed_items.fetched_at) AS occurred_at,
                feed_items.title AS title,
                NULLIF(feed_items.summary, '') AS summary,
                feed_items.source_url AS source_url,
                COALESCE(feed_items.attribution, feed_items.source_name) AS attribution,
                CASE
                    WHEN lower(feed_items.type) IN ('official_report', 'official report') THEN 'official_report'
                    WHEN lower(feed_items.type) IN ('public_media', 'public media') THEN 'public_media'
                    ELSE 'unknown'
                END AS trust_category
            FROM feed_items
            JOIN feed_item_companies ON feed_item_companies.feed_item_id = feed_items.id
            JOIN scope_companies ON scope_companies.company_id = feed_item_companies.company_id

            UNION ALL

            SELECT
                'evidence_note_' || notebook_entries.id AS id,
                'notebook_entry' AS evidence_type,
                'notebooks' AS source_domain,
                notebook_entries.id AS source_id,
                notebook_entries.company_id AS company_id,
                COALESCE(notebook_entries.event_date, notebook_entries.updated_at, notebook_entries.created_at) AS occurred_at,
                notebook_entries.title AS title,
                NULL AS summary,
                NULL AS source_url,
                NULL AS attribution,
                'user_note' AS trust_category
            FROM notebook_entries
            JOIN scope_companies ON scope_companies.company_id = notebook_entries.company_id
            -- Claims are a first-class entity (ADR 0040); they surface from
            -- management_claims below, not from the notebook-entry branch.
            WHERE notebook_entries.kind != 'claim' AND notebook_entries.claim_status IS NULL

            UNION ALL

            SELECT
                'evidence_claim_' || management_claims.id AS id,
                'claim' AS evidence_type,
                'notebooks' AS source_domain,
                management_claims.id AS source_id,
                management_claims.company_id AS company_id,
                COALESCE(management_claims.made_at, management_claims.updated_at, management_claims.created_at) AS occurred_at,
                management_claims.statement AS title,
                NULL AS summary,
                NULL AS source_url,
                NULL AS attribution,
                'user_note' AS trust_category
            FROM management_claims
            JOIN scope_companies ON scope_companies.company_id = management_claims.company_id

            UNION ALL

            SELECT
                'evidence_event_' || company_events.id AS id,
                'company_event' AS evidence_type,
                'events' AS source_domain,
                company_events.id AS source_id,
                company_events.company_id AS company_id,
                company_events.event_date AS occurred_at,
                company_events.title AS title,
                company_events.event_type AS summary,
                company_events.source_url AS source_url,
                company_events.attribution AS attribution,
                CASE
                    WHEN company_events.manual = 1 THEN 'user_note'
                    ELSE 'market_calendar'
                END AS trust_category
            FROM company_events
            JOIN scope_companies ON scope_companies.company_id = company_events.company_id

            UNION ALL

            SELECT
                'evidence_transcript_segment_' || transcript_segments.id AS id,
                'transcript_segment' AS evidence_type,
                'transcripts' AS source_domain,
                transcript_segments.id AS source_id,
                transcript_segments.company_id AS company_id,
                transcript_segments.created_at AS occurred_at,
                COALESCE(transcript_segments.speaker, 'Transcript segment') AS title,
                substr(transcript_segments.text, 1, 240) AS summary,
                transcript_jobs.source_url AS source_url,
                transcript_jobs.provider_id AS attribution,
                'transcript' AS trust_category
            FROM transcript_segments
            JOIN transcript_jobs ON transcript_jobs.id = transcript_segments.transcript_job_id
            JOIN scope_companies ON scope_companies.company_id = transcript_segments.company_id

            UNION ALL

            SELECT
                'evidence_ai_analysis_' || ai_analysis_results.id AS id,
                'ai_analysis' AS evidence_type,
                'ai_analysis' AS source_domain,
                ai_analysis_results.id AS source_id,
                feed_item_companies.company_id AS company_id,
                ai_analysis_results.created_at AS occurred_at,
                'AI analysis' AS title,
                ai_analysis_results.summary AS summary,
                feed_items.source_url AS source_url,
                ai_analysis_results.provider_id AS attribution,
                'ai_generated' AS trust_category
            FROM ai_analysis_results
            JOIN feed_items ON feed_items.id = ai_analysis_results.feed_item_id
            JOIN feed_item_companies ON feed_item_companies.feed_item_id = feed_items.id
            JOIN scope_companies ON scope_companies.company_id = feed_item_companies.company_id

            UNION ALL

            SELECT
                'evidence_research_question_' || research_questions.id AS id,
                'research_question' AS evidence_type,
                'research' AS source_domain,
                research_questions.id AS source_id,
                research_questions.scope_id AS company_id,
                COALESCE(research_questions.closed_at, research_questions.updated_at, research_questions.created_at) AS occurred_at,
                research_questions.title AS title,
                NULLIF(research_questions.body, '') AS summary,
                NULL AS source_url,
                NULL AS attribution,
                'user_note' AS trust_category
            FROM research_questions
            JOIN scope_companies ON scope_companies.company_id = research_questions.scope_id
            WHERE research_questions.scope_type = 'company'

            UNION ALL

            SELECT
                'evidence_signal_' || company_signals.id AS id,
                'company_signal' AS evidence_type,
                'signals' AS source_domain,
                company_signals.id AS source_id,
                company_signals.company_id AS company_id,
                COALESCE(company_signals.signal_date, company_signals.created_at) AS occurred_at,
                signal_categories.display_name AS title,
                feed_items.title AS summary,
                feed_items.source_url AS source_url,
                company_signals.classified_by AS attribution,
                CASE
                    WHEN company_signals.classified_by = 'ai' THEN 'ai_generated'
                    ELSE 'official_report'
                END AS trust_category
            FROM company_signals
            JOIN feed_items ON feed_items.id = company_signals.feed_item_id
            JOIN signal_categories ON signal_categories.key = company_signals.category
            JOIN scope_companies ON scope_companies.company_id = company_signals.company_id
            WHERE company_signals.status = 'confirmed'
        )
        SELECT
            id,
            evidence_type,
            source_domain,
            source_id,
            company_id,
            occurred_at,
            title,
            summary,
            source_url,
            attribution,
            trust_category
        FROM evidence
        ORDER BY datetime(occurred_at) DESC, id
        ",
    )?;

    let rows = statement.query_map(params![input.company_id, input.watchlist_id], |row| {
        let occurred_at: String = row.get(5)?;
        Ok(ResearchEvidenceItem {
            id: row.get(0)?,
            evidence_type: row.get(1)?,
            source_domain: row.get(2)?,
            source_id: row.get(3)?,
            company_id: row.get(4)?,
            review_state: ResearchEvidenceReviewState {
                changed_since_company_review: changed_since_checkpoint(
                    &occurred_at,
                    company_checkpoint.as_deref(),
                ),
                changed_since_watchlist_review: changed_since_checkpoint(
                    &occurred_at,
                    watchlist_checkpoint.as_deref(),
                ),
            },
            occurred_at,
            title: row.get(6)?,
            summary: row.get(7)?,
            source_url: row.get(8)?,
            attribution: row.get(9)?,
            trust_category: row.get(10)?,
        })
    })?;

    let mut items = rows
        .collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)?;

    if let Some(values) = evidence_types.as_ref() {
        items.retain(|item| values.contains(&item.evidence_type));
    }

    if input.changed_since_review_only.unwrap_or(false) {
        items.retain(|item| changed_since_active_review(item, &input));
    }

    let company_summaries = research_company_summaries(connection, &input, &items)?;
    let member_company_count = company_summaries.len();
    let companies_with_changed_evidence = company_summaries
        .iter()
        .filter(|summary| summary.changed_since_review > 0)
        .count();
    let total = items.len();
    let changed_since_review = items
        .iter()
        .filter(|item| changed_since_active_review(item, &input))
        .count();
    items.truncate(limit as usize);

    Ok(ResearchTimelineResult {
        items,
        summary: ResearchTimelineSummary {
            total,
            changed_since_review,
            last_reviewed_at: active_checkpoint,
            member_company_count,
            companies_with_changed_evidence,
            company_summaries,
        },
    })
}

pub(super) fn mark_research_scope_reviewed(
    connection: &Connection,
    input: ResearchReviewCheckpointInput,
) -> StorageResult<ResearchReviewCheckpoint> {
    let scope_type = input.scope_type.trim().to_owned();
    let scope_id = input.scope_id.trim().to_owned();
    validate_review_scope(connection, &scope_type, &scope_id)?;
    let reviewed_at = input
        .reviewed_at
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .map(Ok)
        .unwrap_or_else(|| sources::current_timestamp(connection))?;
    upsert_review_checkpoint(connection, &scope_type, &scope_id, &reviewed_at)?;

    if scope_type == "watchlist" && input.cascade_to_companies.unwrap_or(false) {
        for company_id in watchlist_company_ids(connection, &scope_id)? {
            upsert_review_checkpoint(connection, "company", &company_id, &reviewed_at)?;
        }
    }

    let id = research_review_checkpoint_id(&scope_type, &scope_id);
    get_research_review_checkpoint(connection, &id)
}

pub(super) fn list_research_review_state(
    connection: &Connection,
    input: ResearchReviewCheckpointInput,
) -> StorageResult<Option<ResearchReviewCheckpoint>> {
    validate_review_scope(connection, input.scope_type.trim(), input.scope_id.trim())?;

    connection
        .query_row(
            "
            SELECT
                id,
                scope_type,
                scope_id,
                reviewed_at,
                created_at,
                updated_at
            FROM research_review_checkpoints
            WHERE scope_type = ?1 AND scope_id = ?2
            ",
            params![input.scope_type.trim(), input.scope_id.trim()],
            research_review_checkpoint_from_row,
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn create_evidence_link(
    connection: &Connection,
    input: NewEvidenceLink,
) -> StorageResult<EvidenceLink> {
    let from_type = input.from_type.trim().to_owned();
    let from_id = input.from_id.trim().to_owned();
    let to_type = input.to_type.trim().to_owned();
    let to_id = input.to_id.trim().to_owned();
    let relation_type = input.relation_type.trim().to_owned();

    validate_allowed_research_value("evidence_type", &from_type, EVIDENCE_TYPES)?;
    validate_allowed_research_value("evidence_type", &to_type, EVIDENCE_TYPES)?;
    validate_allowed_research_value("relation_type", &relation_type, RELATION_TYPES)?;
    validate_evidence_reference(connection, &from_type, &from_id)?;
    validate_evidence_reference(connection, &to_type, &to_id)?;

    let id = evidence_link_id(&from_type, &from_id, &to_type, &to_id, &relation_type);

    connection.execute(
        "
        INSERT OR IGNORE INTO evidence_links (
            id,
            from_type,
            from_id,
            to_type,
            to_id,
            relation_type
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![id, from_type, from_id, to_type, to_id, relation_type],
    )?;

    get_evidence_link(connection, &id)
}

pub(super) fn delete_evidence_link(connection: &Connection, id: &str) -> StorageResult<()> {
    connection.execute("DELETE FROM evidence_links WHERE id = ?1", [id.trim()])?;

    Ok(())
}

pub(super) fn list_research_questions(
    connection: &Connection,
    input: ResearchQuestionListInput,
) -> StorageResult<Vec<ResearchQuestion>> {
    let scope_type = empty_string_to_none(input.scope_type);
    let scope_id = empty_string_to_none(input.scope_id);
    let status = empty_string_to_none(input.status);

    if let Some(scope_type) = scope_type.as_deref() {
        validate_allowed_research_value("scope_type", scope_type, &["company", "watchlist"])?;
    }
    if let Some(status) = status.as_deref() {
        validate_allowed_research_value("question_status", status, RESEARCH_QUESTION_STATUSES)?;
    }
    if let (Some(scope_type), Some(scope_id)) = (scope_type.as_deref(), scope_id.as_deref()) {
        validate_review_scope(connection, scope_type, scope_id)?;
    }

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            scope_type,
            scope_id,
            title,
            body,
            status,
            closed_at,
            created_at,
            updated_at
        FROM research_questions
        WHERE (?1 IS NULL OR scope_type = ?1)
            AND (?2 IS NULL OR scope_id = ?2)
            AND (?3 IS NULL OR status = ?3)
        ORDER BY
            CASE status
                WHEN 'open' THEN 0
                WHEN 'answered' THEN 1
                ELSE 2
            END,
            datetime(updated_at) DESC,
            title COLLATE NOCASE
        ",
    )?;
    let rows = statement.query_map(
        params![scope_type, scope_id, status],
        research_question_from_row,
    )?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn create_research_question(
    connection: &Connection,
    input: NewResearchQuestion,
) -> StorageResult<ResearchQuestion> {
    let scope_type = input.scope_type.trim().to_owned();
    let scope_id = input.scope_id.trim().to_owned();
    let title = input.title.trim().to_owned();
    let body = input
        .body
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_owned();

    if title.is_empty() {
        return Err(StorageError::InvalidResearchValue {
            key: "title",
            value: title,
        });
    }
    validate_visible_question_scope(connection, &scope_type, &scope_id)?;
    let id = next_research_question_id(connection, &scope_type, &scope_id, &title)?;

    connection.execute(
        "
        INSERT INTO research_questions (
            id,
            scope_type,
            scope_id,
            title,
            body,
            status
        ) VALUES (?1, ?2, ?3, ?4, ?5, 'open')
        ",
        params![id, scope_type, scope_id, title, body],
    )?;

    get_research_question(connection, &id)
}

pub(super) fn update_research_question(
    connection: &Connection,
    input: ResearchQuestionUpdate,
) -> StorageResult<ResearchQuestion> {
    let id = input.id.trim().to_owned();
    let current = get_research_question(connection, &id)?;
    let title = input
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.title)
        .to_owned();
    let body = input
        .body
        .as_deref()
        .map(str::trim)
        .unwrap_or(&current.body)
        .to_owned();
    let status = input
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&current.status)
        .to_owned();
    validate_allowed_research_value("question_status", &status, RESEARCH_QUESTION_STATUSES)?;
    let closed_at = if status == "closed" || status == "answered" {
        current
            .closed_at
            .or_else(|| sources::current_timestamp(connection).ok())
    } else {
        None
    };

    connection.execute(
        "
        UPDATE research_questions
        SET title = ?2,
            body = ?3,
            status = ?4,
            closed_at = ?5,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id, title, body, status, closed_at],
    )?;

    get_research_question(connection, &input.id)
}

pub(super) fn delete_research_question(connection: &Connection, id: &str) -> StorageResult<()> {
    let id = id.trim();
    get_research_question(connection, id)?;

    connection.execute(
        "
        DELETE FROM evidence_links
        WHERE (from_type = 'research_question' AND from_id = ?1)
            OR (to_type = 'research_question' AND to_id = ?1)
        ",
        [id],
    )?;
    connection.execute("DELETE FROM research_questions WHERE id = ?1", [id])?;

    Ok(())
}

pub(super) fn list_evidence_links(
    connection: &Connection,
    input: EvidenceLinkListInput,
) -> StorageResult<Vec<EvidenceLink>> {
    let endpoint_type = input.endpoint_type.trim().to_owned();
    let endpoint_id = input.endpoint_id.trim().to_owned();
    validate_allowed_research_value("evidence_type", &endpoint_type, EVIDENCE_TYPES)?;
    validate_evidence_reference(connection, &endpoint_type, &endpoint_id)?;

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            from_type,
            from_id,
            to_type,
            to_id,
            relation_type,
            created_at
        FROM evidence_links
        WHERE (from_type = ?1 AND from_id = ?2)
            OR (to_type = ?1 AND to_id = ?2)
        ORDER BY datetime(created_at) DESC, id
        ",
    )?;
    let rows = statement.query_map(params![endpoint_type, endpoint_id], evidence_link_from_row)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn get_research_review_checkpoint(
    connection: &Connection,
    id: &str,
) -> StorageResult<ResearchReviewCheckpoint> {
    connection
        .query_row(
            "
            SELECT
                id,
                scope_type,
                scope_id,
                reviewed_at,
                created_at,
                updated_at
            FROM research_review_checkpoints
            WHERE id = ?1
            ",
            [id],
            research_review_checkpoint_from_row,
        )
        .map_err(StorageError::from)
}

fn research_review_checkpoint_from_row(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<ResearchReviewCheckpoint> {
    Ok(ResearchReviewCheckpoint {
        id: row.get(0)?,
        scope_type: row.get(1)?,
        scope_id: row.get(2)?,
        reviewed_at: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
    })
}

fn get_evidence_link(connection: &Connection, id: &str) -> StorageResult<EvidenceLink> {
    connection
        .query_row(
            "
            SELECT
                id,
                from_type,
                from_id,
                to_type,
                to_id,
                relation_type,
                created_at
            FROM evidence_links
            WHERE id = ?1
            ",
            [id],
            evidence_link_from_row,
        )
        .map_err(StorageError::from)
}

fn evidence_link_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<EvidenceLink> {
    Ok(EvidenceLink {
        id: row.get(0)?,
        from_type: row.get(1)?,
        from_id: row.get(2)?,
        to_type: row.get(3)?,
        to_id: row.get(4)?,
        relation_type: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn get_research_question(connection: &Connection, id: &str) -> StorageResult<ResearchQuestion> {
    connection
        .query_row(
            "
            SELECT
                id,
                scope_type,
                scope_id,
                title,
                body,
                status,
                closed_at,
                created_at,
                updated_at
            FROM research_questions
            WHERE id = ?1
            ",
            [id.trim()],
            research_question_from_row,
        )
        .map_err(StorageError::from)
}

fn research_question_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchQuestion> {
    Ok(ResearchQuestion {
        id: row.get(0)?,
        scope_type: row.get(1)?,
        scope_id: row.get(2)?,
        title: row.get(3)?,
        body: row.get(4)?,
        status: row.get(5)?,
        closed_at: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn review_checkpoint_timestamp(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "
            SELECT reviewed_at
            FROM research_review_checkpoints
            WHERE scope_type = ?1 AND scope_id = ?2
            ",
            params![scope_type, scope_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

fn research_company_summaries(
    connection: &Connection,
    input: &ResearchEvidenceInput,
    items: &[ResearchEvidenceItem],
) -> StorageResult<Vec<ResearchCompanyTimelineSummary>> {
    let company_ids = if let Some(watchlist_id) = input.watchlist_id.as_deref() {
        watchlist_company_ids(connection, watchlist_id)?
    } else if let Some(company_id) = input.company_id.as_deref() {
        vec![company_id.to_owned()]
    } else {
        Vec::new()
    };

    company_ids
        .into_iter()
        .map(|company_id| {
            let company_items = items
                .iter()
                .filter(|item| item.company_id == company_id)
                .collect::<Vec<_>>();
            let changed_since_review = company_items
                .iter()
                .filter(|item| changed_since_active_review(item, input))
                .count();

            Ok(ResearchCompanyTimelineSummary {
                last_reviewed_at: review_checkpoint_timestamp(connection, "company", &company_id)?,
                total: company_items.len(),
                changed_since_review,
                company_id,
            })
        })
        .collect()
}

fn watchlist_company_ids(
    connection: &Connection,
    watchlist_id: &str,
) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT companies.id
        FROM watchlist_companies
        JOIN companies ON companies.id = watchlist_companies.company_id
        WHERE watchlist_companies.watchlist_id = ?1
        ORDER BY companies.display_name, companies.exchange, companies.ticker
        ",
    )?;
    let rows = statement.query_map([watchlist_id], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn upsert_review_checkpoint(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
    reviewed_at: &str,
) -> StorageResult<()> {
    let id = research_review_checkpoint_id(scope_type, scope_id);

    connection.execute(
        "
        INSERT INTO research_review_checkpoints (
            id,
            scope_type,
            scope_id,
            reviewed_at
        ) VALUES (?1, ?2, ?3, ?4)
        ON CONFLICT(scope_type, scope_id) DO UPDATE SET
            reviewed_at = excluded.reviewed_at,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![id, scope_type, scope_id, reviewed_at],
    )?;

    Ok(())
}

fn validate_review_scope(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<()> {
    match scope_type {
        "company" => validate_reference_exists(connection, "companies", scope_id),
        "watchlist" => validate_reference_exists(connection, "watchlists", scope_id),
        _ => Err(StorageError::InvalidResearchValue {
            key: "scope_type",
            value: scope_type.to_owned(),
        }),
    }
}

fn validate_evidence_reference(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
) -> StorageResult<()> {
    match evidence_type {
        "feed_item" => validate_reference_exists(connection, "feed_items", evidence_id),
        "notebook_entry" => validate_reference_exists(connection, "notebook_entries", evidence_id),
        // Claims are a first-class entity (ADR 0040); claim evidence resolves against
        // management_claims, not notebook_entries.
        "claim" => validate_reference_exists(connection, "management_claims", evidence_id),
        "transcript_segment" => {
            validate_reference_exists(connection, "transcript_segments", evidence_id)
        }
        "company_event" => validate_reference_exists(connection, "company_events", evidence_id),
        "ai_analysis" => validate_reference_exists(connection, "ai_analysis_results", evidence_id),
        "research_question" => {
            validate_reference_exists(connection, "research_questions", evidence_id)
        }
        _ => Err(StorageError::InvalidResearchValue {
            key: "evidence_type",
            value: evidence_type.to_owned(),
        }),
    }
}

fn validate_visible_question_scope(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<()> {
    if scope_type != "company" {
        return Err(StorageError::InvalidResearchValue {
            key: "scope_type",
            value: scope_type.to_owned(),
        });
    }

    validate_review_scope(connection, scope_type, scope_id)
}

fn validate_reference_exists(
    connection: &Connection,
    table_name: &str,
    id: &str,
) -> StorageResult<()> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)");
    let exists: bool = connection.query_row(&sql, [id], |row| row.get(0))?;

    if exists {
        Ok(())
    } else {
        Err(StorageError::MissingResearchReference {
            table: table_name.to_owned(),
            id: id.to_owned(),
        })
    }
}

fn validate_allowed_research_value(
    field_name: &str,
    value: &str,
    allowed_values: &[&str],
) -> StorageResult<()> {
    if allowed_values.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidResearchValue {
            key: match field_name {
                "evidence_type" => "evidence_type",
                "relation_type" => "relation_type",
                "scope_type" => "scope_type",
                "question_status" => "question_status",
                _ => "unknown",
            },
            value: value.to_owned(),
        })
    }
}

fn changed_since_checkpoint(occurred_at: &str, checkpoint: Option<&str>) -> bool {
    checkpoint
        .map(|reviewed_at| occurred_at > reviewed_at)
        .unwrap_or(true)
}

fn changed_since_active_review(item: &ResearchEvidenceItem, input: &ResearchEvidenceInput) -> bool {
    if input.company_id.is_some() {
        item.review_state.changed_since_company_review
    } else if input.watchlist_id.is_some() {
        item.review_state.changed_since_watchlist_review
    } else {
        item.review_state.changed_since_company_review
    }
}

fn research_review_checkpoint_id(scope_type: &str, scope_id: &str) -> String {
    format!("review_{scope_type}_{scope_id}")
}

fn evidence_link_id(
    from_type: &str,
    from_id: &str,
    to_type: &str,
    to_id: &str,
    relation_type: &str,
) -> String {
    format!("evidence_link_{from_type}_{from_id}_{relation_type}_{to_type}_{to_id}")
}

fn next_research_question_id(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
    title: &str,
) -> StorageResult<String> {
    let base_id = format!(
        "research_question_{}_{}_{}",
        slug_part(scope_type),
        slug_part(scope_id),
        slug_part(title)
    );
    let mut candidate = base_id.clone();
    let mut suffix = 2;

    while reference_exists(connection, "research_questions", &candidate)? {
        candidate = format!("{base_id}_{suffix}");
        suffix += 1;
    }

    Ok(candidate)
}

fn reference_exists(connection: &Connection, table_name: &str, id: &str) -> StorageResult<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)");
    connection
        .query_row(&sql, [id], |row| row.get(0))
        .map_err(StorageError::from)
}
