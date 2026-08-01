use super::*;

pub(super) fn list_notebook_entries(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<NotebookEntry>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            company_id,
            title,
            body,
            body_format,
            kind,
            claim_status,
            event_date,
            follow_up_after,
            follow_up_date,
            created_at,
            updated_at
        FROM notebook_entries
        WHERE company_id = ?1
        ORDER BY updated_at DESC, created_at DESC, id
        ",
    )?;

    let rows = statement.query_map([company_id], |row| notebook_entry_from_row(connection, row))?;
    let entries = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(entries)
}

pub(super) fn create_notebook_entry(
    connection: &Connection,
    input: NewNotebookEntry,
) -> StorageResult<NotebookEntry> {
    let title = input.title.trim().to_owned();
    let body = input.body.trim().to_owned();
    let body_format = input
        .body_format
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("markdown")
        .to_owned();
    let kind = input.kind.trim().to_owned();
    let claim_status = empty_string_to_none(input.claim_status);
    let tags = normalize_tags(input.tags);
    let id = notebook_entry_id(connection, &input.company_id, &title)?;

    validate_allowed_notebook_value("body_format", &body_format, &["markdown"])?;
    validate_allowed_notebook_value(
        "kind",
        &kind,
        &["manual", "observation", "claim", "question", "follow_up"],
    )?;

    if let Some(status) = claim_status.as_deref() {
        validate_allowed_notebook_value(
            "claim_status",
            status,
            &[
                "open",
                "delivered",
                "partially_delivered",
                "missed",
                "unknown",
                "not_applicable",
            ],
        )?;
    }

    for origins in &input.origins {
        validate_allowed_notebook_value(
            "origins.source_type",
            origins.source_type.trim(),
            &[
                "feed_item",
                "transcript_segment",
                "ai_analysis",
                "manual",
                "external_url",
                // A stored `report_documents` row (closes #111) — the
                // document an agent (or the owner) actually read, as opposed
                // to `external_url`'s bare link. `source_id` is a soft
                // reference to `report_documents.id`: NOT FK-checked, the
                // same parity every other origin type's `source_id` keeps
                // (none of them are FK-checked either — see the loop above).
                "report_document",
            ],
        )?;
    }

    connection.execute(
        "
        INSERT INTO notebook_entries (
            id,
            company_id,
            title,
            body,
            body_format,
            kind,
            claim_status,
            event_date,
            follow_up_after,
            follow_up_date
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ",
        params![
            id,
            input.company_id,
            title,
            body,
            body_format,
            kind,
            claim_status,
            empty_string_to_none(input.event_date),
            empty_string_to_none(input.follow_up_after),
            empty_string_to_none(input.follow_up_date),
        ],
    )?;

    for tag in tags {
        connection.execute(
            "
            INSERT OR IGNORE INTO notebook_entry_tags (notebook_entry_id, tag)
            VALUES (?1, ?2)
            ",
            params![&id, tag],
        )?;
    }

    for (index, origins) in input.origins.into_iter().enumerate() {
        let source_type = origins.source_type.trim().to_owned();

        connection.execute(
            "
            INSERT INTO notebook_entry_origins (
                id,
                notebook_entry_id,
                source_type,
                source_id,
                source_url,
                label
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                notebook_origin_id(&id, &source_type, index),
                id,
                source_type,
                empty_string_to_none(origins.source_id),
                empty_string_to_none(origins.source_url),
                empty_string_to_none(origins.label),
            ],
        )?;
    }

    get_notebook_entry(connection, &id)
}

pub(super) fn update_notebook_entry(
    connection: &Connection,
    input: NotebookEntryUpdate,
) -> StorageResult<NotebookEntry> {
    let id = input.id;
    let title = input.title.trim().to_owned();
    let body = input.body.trim().to_owned();
    let kind = input.kind.trim().to_owned();
    let claim_status = empty_string_to_none(input.claim_status);
    let tags = normalize_tags(input.tags);

    validate_allowed_notebook_value(
        "kind",
        &kind,
        &["manual", "observation", "claim", "question", "follow_up"],
    )?;

    if let Some(status) = claim_status.as_deref() {
        validate_allowed_notebook_value(
            "claim_status",
            status,
            &[
                "open",
                "delivered",
                "partially_delivered",
                "missed",
                "unknown",
                "not_applicable",
            ],
        )?;
    }

    connection.execute(
        "
        UPDATE notebook_entries
        SET
            title = ?2,
            body = ?3,
            kind = ?4,
            claim_status = ?5,
            event_date = ?6,
            follow_up_after = ?7,
            follow_up_date = ?8,
            updated_at = datetime('now')
        WHERE id = ?1
        ",
        params![
            &id,
            title,
            body,
            kind,
            claim_status,
            empty_string_to_none(input.event_date),
            empty_string_to_none(input.follow_up_after),
            empty_string_to_none(input.follow_up_date),
        ],
    )?;

    connection.execute(
        "DELETE FROM notebook_entry_tags WHERE notebook_entry_id = ?1",
        [&id],
    )?;

    for tag in tags {
        connection.execute(
            "
            INSERT OR IGNORE INTO notebook_entry_tags (notebook_entry_id, tag)
            VALUES (?1, ?2)
            ",
            params![&id, tag],
        )?;
    }

    get_notebook_entry(connection, &id)
}

pub(super) fn delete_notebook_entry(
    connection: &Connection,
    notebook_entry_id: &str,
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM notebook_entries WHERE id = ?1",
        [notebook_entry_id.trim()],
    )?;

    Ok(())
}

pub(super) fn get_notebook_entry(
    connection: &Connection,
    notebook_entry_id: &str,
) -> StorageResult<NotebookEntry> {
    connection
        .query_row(
            "
            SELECT
                id,
                company_id,
                title,
                body,
                body_format,
                kind,
                claim_status,
                event_date,
                follow_up_after,
                follow_up_date,
                created_at,
                updated_at
            FROM notebook_entries
            WHERE id = ?1
            ",
            [notebook_entry_id],
            |row| notebook_entry_from_row(connection, row),
        )
        .map_err(StorageError::from)
}

pub(super) fn notebook_entry_from_row(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<NotebookEntry> {
    let id: String = row.get(0)?;

    Ok(NotebookEntry {
        tags: notebook_entry_tags(connection, &id)?,
        origins: notebook_entry_origins(connection, &id)?,
        id,
        company_id: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        body_format: row.get(4)?,
        kind: row.get(5)?,
        claim_status: row.get(6)?,
        event_date: row.get(7)?,
        follow_up_after: row.get(8)?,
        follow_up_date: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

pub(super) fn notebook_entry_tags(
    connection: &Connection,
    notebook_entry_id: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT tag
        FROM notebook_entry_tags
        WHERE notebook_entry_id = ?1
        ORDER BY tag
        ",
    )?;
    let rows = statement.query_map([notebook_entry_id], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
}

pub(super) fn notebook_entry_origins(
    connection: &Connection,
    notebook_entry_id: &str,
) -> rusqlite::Result<Vec<NotebookOrigin>> {
    let mut statement = connection.prepare(
        "
        SELECT id, source_type, source_id, source_url, label, created_at
        FROM notebook_entry_origins
        WHERE notebook_entry_id = ?1
        ORDER BY created_at, id
        ",
    )?;
    let rows = statement.query_map([notebook_entry_id], |row| {
        let source_type: String = row.get(1)?;
        let stored_label: Option<String> = row.get(4)?;
        Ok(NotebookOrigin {
            id: row.get(0)?,
            label: resolve_origin_label(&source_type, stored_label),
            source_id: row.get(2)?,
            source_url: row.get(3)?,
            created_at: row.get(5)?,
            source_type,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

/// The `ai_analysis_results` table backing `source_type = "ai_analysis"` origins
/// was dropped with the in-app AI layer (ADR 0084, migration 0102). Owner policy
/// keeps the saved notes and their origins readable as user data, so rather than
/// a dead lookup against the dropped table — or a bare, misleading `ai analysis`
/// rendered from the raw `source_type` — such an origin resolves to an honest
/// retired-source label when it carries none of its own.
pub(super) const RETIRED_AI_ANALYSIS_ORIGIN_LABEL: &str = "retired AI analysis (source removed)";

/// Resolves the display label for one origin: an explicit stored label always
/// wins; a labelless retired-AI-analysis origin falls back to the honest
/// retired-source label; every other type keeps its stored value (including
/// `None`, which callers render from the `source_type`).
pub(super) fn resolve_origin_label(
    source_type: &str,
    stored_label: Option<String>,
) -> Option<String> {
    let has_label = stored_label
        .as_deref()
        .is_some_and(|l| !l.trim().is_empty());
    if !has_label && source_type == "ai_analysis" {
        return Some(RETIRED_AI_ANALYSIS_ORIGIN_LABEL.to_owned());
    }
    stored_label
}

pub(super) fn notebook_entry_id(
    connection: &Connection,
    company_id: &str,
    title: &str,
) -> StorageResult<String> {
    let base_id = format!("note_{}_{}", slug_part(company_id), slug_part(title));
    let existing_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM notebook_entries WHERE id = ?1 OR id LIKE ?2",
        params![&base_id, format!("{base_id}_%")],
        |row| row.get(0),
    )?;

    if existing_count == 0 {
        Ok(base_id)
    } else {
        Ok(format!("{base_id}_{}", existing_count + 1))
    }
}

pub(super) fn notebook_origin_id(
    notebook_entry_id: &str,
    source_type: &str,
    index: usize,
) -> String {
    format!(
        "note_origin_{}_{}_{}",
        slug_part(notebook_entry_id),
        slug_part(source_type),
        index + 1
    )
}

pub(super) fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = tags
        .into_iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();

    normalized.sort();
    normalized.dedup();
    normalized
}

pub(super) fn validate_allowed_notebook_value(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidNotebookValue {
            key,
            value: value.to_owned(),
        })
    }
}

#[cfg(test)]
mod proptests {
    //! Invariant coverage of `normalize_tags` (ADR 0049) — a dedup transform, so
    //! it is the canonical place to prove **order-independence**: the same tags
    //! arriving in any order must collapse to the same canonical set.
    use super::*;
    use crate::transform_invariants::{assert_idempotent_vec, assert_order_independent};
    use proptest::prelude::*;

    /// A small tag vocabulary deliberately seeded with duplicates, case variants,
    /// surrounding whitespace, and blanks so generated inputs exercise the
    /// dedup / lowercase / trim / empty-filter paths densely.
    fn tag_vocab() -> impl Strategy<Value = String> {
        prop::sample::select(vec![
            "alpha", "Alpha", " alpha ", "BETA", "beta", "  ", "", "gamma", "Gamma ",
        ])
        .prop_map(str::to_owned)
    }

    proptest! {
        #[test]
        fn normalize_tags_dedups_idempotently_and_order_independently(
            tags in prop::collection::vec(tag_vocab(), 0..10)
        ) {
            assert_idempotent_vec(normalize_tags, tags.clone());
            assert_order_independent(normalize_tags, tags.clone());

            let out = normalize_tags(tags);
            // Sorted and strictly deduplicated.
            prop_assert!(out.windows(2).all(|w| w[0] < w[1]), "not sorted/deduped: {out:?}");
            // Every surviving tag is trimmed, lowercased, and non-empty.
            prop_assert!(
                out.iter().all(|t| !t.is_empty() && *t == t.trim().to_lowercase()),
                "tag not normalized: {out:?}"
            );
        }
    }
}

#[cfg(test)]
mod retired_origin_tests {
    //! ADR 0084 / migration 0102: the `ai_analysis` producer and its results
    //! table are gone, but saved notes and their origins stay readable. A
    //! retired-source origin with no explicit label must resolve to an honest
    //! label — never a bare, misleading `ai analysis` implying a live source.
    use crate::app_state::AppState;
    use crate::storage::{
        open_in_memory_database, NewCompany, NewNotebookEntry, NewNotebookOrigin,
    };

    fn state_with_company() -> (AppState, String) {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        (state, company.id)
    }

    #[test]
    fn labelless_ai_analysis_origin_reads_the_retired_source_label() {
        let (state, company_id) = state_with_company();
        let entry = state
            .create_notebook_entry(NewNotebookEntry {
                company_id: company_id.clone(),
                title: "Saved AI take".to_owned(),
                body: "A note whose origin was a since-retired AI analysis.".to_owned(),
                body_format: None,
                tags: vec![],
                kind: "observation".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
                origins: vec![NewNotebookOrigin {
                    source_type: "ai_analysis".to_owned(),
                    source_id: Some("ai_result_dropped_42".to_owned()),
                    source_url: None,
                    label: None,
                }],
            })
            .expect("entry");

        assert_eq!(entry.origins.len(), 1);
        assert_eq!(entry.origins[0].source_type, "ai_analysis");
        assert_eq!(
            entry.origins[0].label.as_deref(),
            Some(super::RETIRED_AI_ANALYSIS_ORIGIN_LABEL),
            "a labelless retired-AI-analysis origin must render an honest retired label"
        );
    }

    #[test]
    fn explicit_origin_label_is_preserved_for_ai_analysis() {
        let (state, company_id) = state_with_company();
        let entry = state
            .create_notebook_entry(NewNotebookEntry {
                company_id,
                title: "Saved AI take with label".to_owned(),
                body: "Origin carried its own label.".to_owned(),
                body_format: None,
                tags: vec![],
                kind: "observation".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
                origins: vec![NewNotebookOrigin {
                    source_type: "ai_analysis".to_owned(),
                    source_id: None,
                    source_url: None,
                    label: Some("Q1 thesis".to_owned()),
                }],
            })
            .expect("entry");

        assert_eq!(entry.origins[0].label.as_deref(), Some("Q1 thesis"));
    }
}

use super::database::Database;
/// notebooks domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::notebooks()`.
#[derive(Clone)]
pub struct NotebookStore {
    db: Database,
}

impl NotebookStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_notebook_entries(&self, company_id: &str) -> StorageResult<Vec<NotebookEntry>> {
        let connection = self.db.checkout()?;

        list_notebook_entries(&connection, company_id)
    }

    pub fn create_notebook_entry(&self, input: NewNotebookEntry) -> StorageResult<NotebookEntry> {
        let connection = self.db.checkout()?;

        create_notebook_entry(&connection, input)
    }

    pub fn update_notebook_entry(
        &self,
        input: NotebookEntryUpdate,
    ) -> StorageResult<NotebookEntry> {
        let connection = self.db.checkout()?;

        update_notebook_entry(&connection, input)
    }

    pub fn delete_notebook_entry(&self, notebook_entry_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        delete_notebook_entry(&connection, notebook_entry_id)
    }
}
