use super::*;

pub(super) fn existing_companies_by_ticker(
    connection: &Connection,
) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare("SELECT qualified_ticker, id FROM companies")?;
    let rows = statement.query_map([], |row| {
        let qualified_ticker: String = row.get(0)?;
        let id: String = row.get(1)?;
        Ok((qualified_ticker.to_uppercase(), id))
    })?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn existing_ids(
    connection: &Connection,
    table_name: &str,
) -> StorageResult<HashSet<String>> {
    let sql = format!("SELECT id FROM {table_name}");
    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map([], |row| row.get::<_, String>(0))?;

    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn existing_watchlist_ids_by_name(
    connection: &Connection,
) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare("SELECT LOWER(name), id FROM watchlists")?;
    let rows = statement.query_map([], |row| {
        let name: String = row.get(0)?;
        let id: String = row.get(1)?;
        Ok((name, id))
    })?;

    rows.collect::<Result<HashMap<_, _>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn existing_memberships(
    connection: &Connection,
) -> StorageResult<HashSet<(String, String)>> {
    let mut statement = connection.prepare(
        "
        SELECT watchlist_companies.watchlist_id, companies.qualified_ticker
        FROM watchlist_companies
        INNER JOIN companies ON companies.id = watchlist_companies.company_id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        let watchlist_id: String = row.get(0)?;
        let qualified_ticker: String = row.get(1)?;
        Ok((watchlist_id, qualified_ticker.to_uppercase()))
    })?;

    rows.collect::<Result<HashSet<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn imported_or_existing_evidence_reference(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
    imported_note_ids: &HashSet<String>,
    imported_claim_ids: &HashSet<String>,
    imported_question_ids: &HashSet<String>,
) -> bool {
    match evidence_type {
        "notebook_entry" if imported_note_ids.contains(evidence_id) => true,
        "claim" if imported_claim_ids.contains(evidence_id) => true,
        "research_question" if imported_question_ids.contains(evidence_id) => true,
        _ => evidence_reference_exists_for_import(connection, evidence_type, evidence_id)
            .unwrap_or(false),
    }
}

pub(super) fn imported_or_existing_evidence_reference_with_reminders(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
    imported_note_ids: &HashSet<String>,
    imported_claim_ids: &HashSet<String>,
    imported_question_ids: &HashSet<String>,
    imported_reminder_ids: &HashSet<String>,
) -> bool {
    match evidence_type {
        "reminder" if imported_reminder_ids.contains(evidence_id) => true,
        _ => imported_or_existing_evidence_reference(
            connection,
            evidence_type,
            evidence_id,
            imported_note_ids,
            imported_claim_ids,
            imported_question_ids,
        ),
    }
}

pub(super) fn evidence_reference_exists_for_import(
    connection: &Connection,
    evidence_type: &str,
    evidence_id: &str,
) -> StorageResult<bool> {
    match evidence_type {
        "feed_item" => table_reference_exists(connection, "feed_items", evidence_id),
        "notebook_entry" => table_reference_exists(connection, "notebook_entries", evidence_id),
        "claim" => table_reference_exists(connection, "management_claims", evidence_id),
        "transcript_segment" => {
            table_reference_exists(connection, "transcript_segments", evidence_id)
        }
        "company_event" => table_reference_exists(connection, "company_events", evidence_id),
        "ai_analysis" => table_reference_exists(connection, "ai_analysis_results", evidence_id),
        "research_question" => {
            table_reference_exists(connection, "research_questions", evidence_id)
        }
        "reminder" => table_reference_exists(connection, "research_reminders", evidence_id),
        "digest" => table_reference_exists(connection, "ai_research_digests", evidence_id),
        _ => Ok(false),
    }
}

pub(super) fn table_reference_exists(
    connection: &Connection,
    table_name: &str,
    id: &str,
) -> StorageResult<bool> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)");
    connection
        .query_row(&sql, [id], |row| row.get(0))
        .map_err(StorageError::from)
}
