use super::*;

// ============================================================================
// Public Structs (DTO/serializable types)
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDocument {
    pub id: String,
    pub company_id: String,
    pub period_id: Option<String>,
    pub source_type: String,
    pub origin_ref: Option<String>,
    pub url: String,
    pub local_path: Option<String>,
    pub content_type: Option<String>,
    pub content_hash: Option<String>,
    pub byte_size: Option<i64>,
    pub title: Option<String>,
    pub attribution: Option<String>,
    pub fetch_status: String,
    pub fetch_error: Option<String>,
    pub fetched_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureReportDocumentInput {
    pub company_id: String,
    pub source_type: String,
    pub url: String,
    pub period_id: Option<String>,
    pub origin_ref: Option<String>,
    pub title: Option<String>,
    pub attribution: Option<String>,
}

// ============================================================================
// Public Storage Functions
// ============================================================================

pub(super) fn create_or_find_pending(
    connection: &Connection,
    input: CaptureReportDocumentInput,
) -> StorageResult<ReportDocument> {
    let company_id = input.company_id.trim().to_owned();
    let url = input.url.trim().to_owned();
    let source_type = input.source_type.trim().to_owned();
    let period_id = empty_string_to_none(input.period_id.map(|s| s.trim().to_owned()));
    let origin_ref = empty_string_to_none(input.origin_ref.map(|s| s.trim().to_owned()));
    let title = empty_string_to_none(input.title.map(|s| s.trim().to_owned()));
    let attribution = empty_string_to_none(input.attribution.map(|s| s.trim().to_owned()));

    validate_reference_exists(connection, "companies", &company_id)?;
    if let Some(ref p_id) = period_id {
        validate_reference_exists(connection, "financial_periods", p_id)?;
    }

    if url.is_empty() {
        return Err(StorageError::InvalidFinancialsValue {
            key: "url",
            value: url,
        });
    }

    if source_type.is_empty() {
        return Err(StorageError::InvalidFinancialsValue {
            key: "source_type",
            value: source_type,
        });
    }

    // Try to find existing document with same (company_id, url)
    let existing = get_by_company_and_url(connection, &company_id, &url)
        .ok()
        .flatten();
    if let Some(doc) = existing {
        return Ok(doc);
    }

    let id = report_document_id(&company_id, &url);

    connection.execute(
        "
        INSERT INTO report_documents (
            id,
            company_id,
            period_id,
            source_type,
            origin_ref,
            url,
            title,
            attribution,
            fetch_status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ",
        params![
            id,
            company_id,
            period_id,
            source_type,
            origin_ref,
            url,
            title,
            attribution,
            "pending"
        ],
    )?;

    get_report_document(connection, &id)
}

pub(super) fn mark_fetched(
    connection: &Connection,
    id: &str,
    local_path: Option<&str>,
    content_type: Option<&str>,
    content_hash: Option<&str>,
    byte_size: Option<i64>,
) -> StorageResult<ReportDocument> {
    let local_path = empty_string_to_none(local_path.map(|s| s.to_owned()));
    let content_type = empty_string_to_none(content_type.map(|s| s.to_owned()));
    let content_hash = empty_string_to_none(content_hash.map(|s| s.to_owned()));

    let _doc = get_report_document(connection, id)?;

    connection.execute(
        "
        UPDATE report_documents
        SET local_path = ?2,
            content_type = ?3,
            content_hash = ?4,
            byte_size = ?5,
            fetch_status = ?6,
            fetched_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            id,
            local_path,
            content_type,
            content_hash,
            byte_size,
            "fetched"
        ],
    )?;

    get_report_document(connection, id)
}

pub(super) fn mark_failed(
    connection: &Connection,
    id: &str,
    error: &str,
) -> StorageResult<ReportDocument> {
    let error = error.trim().to_owned();
    let _doc = get_report_document(connection, id)?;

    connection.execute(
        "
        UPDATE report_documents
        SET fetch_status = ?2,
            fetch_error = ?3,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id, "failed", error],
    )?;

    get_report_document(connection, id)
}

pub(super) fn get(connection: &Connection, id: &str) -> StorageResult<ReportDocument> {
    get_report_document(connection, id)
}

pub(super) fn list_by_company(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<ReportDocument>> {
    let company_id = company_id.trim();

    validate_reference_exists(connection, "companies", company_id)?;

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            company_id,
            period_id,
            source_type,
            origin_ref,
            url,
            local_path,
            content_type,
            content_hash,
            byte_size,
            title,
            attribution,
            fetch_status,
            fetch_error,
            fetched_at,
            created_at,
            updated_at
        FROM report_documents
        WHERE company_id = ?1
        ORDER BY created_at DESC
        ",
    )?;

    let rows = statement.query_map(params![company_id], report_document_from_row)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

// ============================================================================
// Private Helper Functions
// ============================================================================

fn get_report_document(connection: &Connection, id: &str) -> StorageResult<ReportDocument> {
    connection
        .query_row(
            "
        SELECT
            id,
            company_id,
            period_id,
            source_type,
            origin_ref,
            url,
            local_path,
            content_type,
            content_hash,
            byte_size,
            title,
            attribution,
            fetch_status,
            fetch_error,
            fetched_at,
            created_at,
            updated_at
        FROM report_documents
        WHERE id = ?1
        ",
            params![id],
            report_document_from_row,
        )
        .map_err(StorageError::from)
}

fn get_by_company_and_url(
    connection: &Connection,
    company_id: &str,
    url: &str,
) -> StorageResult<Option<ReportDocument>> {
    connection
        .query_row(
            "
            SELECT
                id,
                company_id,
                period_id,
                source_type,
                origin_ref,
                url,
                local_path,
                content_type,
                content_hash,
                byte_size,
                title,
                attribution,
                fetch_status,
                fetch_error,
                fetched_at,
                created_at,
                updated_at
            FROM report_documents
            WHERE company_id = ?1 AND url = ?2
            ",
            params![company_id, url],
            report_document_from_row,
        )
        .optional()
        .map_err(StorageError::from)
}

fn report_document_from_row(row: &rusqlite::Row) -> rusqlite::Result<ReportDocument> {
    Ok(ReportDocument {
        id: row.get(0)?,
        company_id: row.get(1)?,
        period_id: row.get(2)?,
        source_type: row.get(3)?,
        origin_ref: row.get(4)?,
        url: row.get(5)?,
        local_path: row.get(6)?,
        content_type: row.get(7)?,
        content_hash: row.get(8)?,
        byte_size: row.get(9)?,
        title: row.get(10)?,
        attribution: row.get(11)?,
        fetch_status: row.get(12)?,
        fetch_error: row.get(13)?,
        fetched_at: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
}

fn report_document_id(company_id: &str, url: &str) -> String {
    format!("doc_{}_{}", slug_part(company_id), slug_part(url))
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
        Err(StorageError::MissingFinancialsReference {
            table: table_name.to_owned(),
            id: id.to_owned(),
        })
    }
}
