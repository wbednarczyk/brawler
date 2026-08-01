use std::collections::BTreeMap;

use crate::fundamentals::extraction::classify::{classify_doc_kind_with_slug_trust, DocKind};

use super::*;

// ============================================================================
// Public Structs (DTO/serializable types)
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
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
    /// doc_kind taxonomy (ADR 0077 §1): periodic_ssf | periodic_jsf |
    /// auditor_opinion | presentation | governance | other. NULL = unclassified.
    pub doc_kind: Option<String>,
    /// What the stored bytes REALLY are (migration 0121, epic #229 T2), using the
    /// `Container::as_str` vocabulary: pdf | zip | xml | html | unknown. Stamped
    /// at store time and healed on startup; NULL = not yet sniffed (legacy row,
    /// or its file is currently missing). Routing prefers this over the
    /// extension/content-type, both of which the corpus proves unreliable.
    pub detected_container: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CaptureReportDocumentInput {
    pub company_id: String,
    pub source_type: String,
    pub url: String,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub period_id: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub origin_ref: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub title: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(optional))]
    pub attribution: Option<String>,
}

/// Outcome of a corpus-wide `doc_kind` reclassification (ADR 0077 §1, T1.2):
/// how many documents were scanned, how many changed kind, and the final count
/// per kind over every row. Idempotent — a second run reports `updated == 0`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReclassifyReportDocumentsSummary {
    pub total: usize,
    pub updated: usize,
    pub by_kind: BTreeMap<String, usize>,
}

// ============================================================================
// Public Storage Functions
// ============================================================================

pub(super) fn create_or_find_pending(
    connection: &Connection,
    input: CaptureReportDocumentInput,
) -> StorageResult<ReportDocument> {
    create_or_find_with_status(connection, input, "pending")
}

/// Classify a document for the stored taxonomy column with the URL slug
/// distrusted when it names a **foreign** tracked issuer (epic #229 T3, #171).
///
/// The attachment host reuses one issuer's filename across unrelated filings, so
/// a slug can classify another company's document; the owner's own title is the
/// signal that survives. Loading the (tiny, ~50-row) issuer index per write keeps
/// the decision at the single seam where `doc_kind` is written — set-on-write and
/// the corpus reclassify then cannot drift apart. A failure to load the index
/// degrades to today's behavior (trust the slug) rather than refusing the write.
fn classify_for_storage(
    connection: &Connection,
    company_id: &str,
    title: &str,
    url: &str,
) -> DocKind {
    let slug_trusted = match super::sources::TrackedIssuerIndex::load(connection) {
        Ok(issuers) => !issuers.url_slug_names_foreign_issuer(company_id, title, url),
        Err(error) => {
            log::warn!(
                "module=report_documents stage=classify company={company_id} \
                 tracked-issuer load failed, trusting the URL slug: {error}"
            );
            true
        }
    };
    classify_doc_kind_with_slug_trust(title, url, slug_trusted)
}

/// Create a report document with an explicit initial `fetch_status`, or return the
/// existing row for the same `(company_id, url)` (idempotent upsert on the UNIQUE key).
///
/// Used by attachment ingestion to register periodic-report attachments as `pending`
/// (a follow-up fetch stores the file) and other ESPI/EBI attachments as `metadata_only`
/// (URL + attribution preserved, no bytes stored). See ADR 0036.
pub(super) fn create_or_find_with_status(
    connection: &Connection,
    input: CaptureReportDocumentInput,
    initial_status: &str,
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
        // Approved T1.2 scope addition (ADR 0077 §1): a re-ingest carrying a
        // changed title reclassifies in place so a stored document never keeps a
        // stale `doc_kind`. A same-title re-ingest stays a pure no-op (the
        // idempotent upsert contract), preserving created_at/updated_at.
        if title.is_some() && title != doc.title {
            let doc_kind = classify_for_storage(
                connection,
                &company_id,
                title.as_deref().unwrap_or(""),
                &url,
            );
            connection.execute(
                "
                UPDATE report_documents
                SET title = ?2,
                    doc_kind = ?3,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
                params![doc.id, title, doc_kind.as_str()],
            )?;
            return get_report_document(connection, &doc.id);
        }
        return Ok(doc);
    }

    let id = report_document_id(&company_id, &url);
    // Classify at ingestion so a new document is never left unclassified (NULL).
    let doc_kind = classify_for_storage(
        connection,
        &company_id,
        title.as_deref().unwrap_or(""),
        &url,
    );

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
            fetch_status,
            doc_kind
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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
            initial_status,
            doc_kind.as_str()
        ],
    )?;

    get_report_document(connection, &id)
}

/// Downgrade a document to `metadata_only`: keep the URL/title/attribution row for
/// citation and the source ladder, but record that no file is stored locally. Used by
/// retention pruning and for non-periodic ESPI/EBI attachments (ADR 0036).
pub(super) fn mark_metadata_only(
    connection: &Connection,
    id: &str,
) -> StorageResult<ReportDocument> {
    let _doc = get_report_document(connection, id)?;

    connection.execute(
        "
        UPDATE report_documents
        SET fetch_status = 'metadata_only',
            local_path = NULL,
            fetch_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id],
    )?;

    get_report_document(connection, id)
}

/// Documents awaiting a file fetch: ESPI/EBI attachments registered as `pending` with no
/// stored file yet. Driven outside the ingestion transaction so network IO stays off the
/// write path (ADR 0036).
pub(super) fn list_pending_attachments(
    connection: &Connection,
) -> StorageResult<Vec<ReportDocument>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id, company_id, period_id, source_type, origin_ref, url, local_path,
            content_type, content_hash, byte_size, title, attribution, fetch_status,
            fetch_error, fetched_at, created_at, updated_at, doc_kind, detected_container
        FROM report_documents
        WHERE source_type = 'espi_attachment'
          AND fetch_status = 'pending'
          AND local_path IS NULL
        ORDER BY created_at ASC
        ",
    )?;

    let rows = statement.query_map([], report_document_from_row)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
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
            updated_at,
            doc_kind,
            detected_container
        FROM report_documents
        WHERE company_id = ?1
        ORDER BY created_at DESC
        ",
    )?;

    let rows = statement.query_map(params![company_id], report_document_from_row)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Recompute `doc_kind` for every stored report document from its title + URL
/// (ADR 0077 §1). Classification is deterministic Rust code, so this is a pure
/// self-heal: a row is written only when its stored kind differs from the
/// recomputed one, making the operation idempotent. Returns per-kind final
/// counts over all rows plus how many were changed.
pub(super) fn reclassify_all(
    connection: &Connection,
) -> StorageResult<ReclassifyReportDocumentsSummary> {
    /// `(id, company_id, title, url, stored doc_kind)` — the owner is read too
    /// because slug trust is per-owner (epic #229 T3).
    type ReclassifyRow = (String, String, Option<String>, String, Option<String>);
    let rows: Vec<ReclassifyRow> = {
        let mut statement = connection
            .prepare("SELECT id, company_id, title, url, doc_kind FROM report_documents")?;
        let mapped = statement.query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?;
        mapped.collect::<Result<Vec<_>, _>>()?
    };
    // One load for the whole corpus (epic #229 T3) — the per-write seam loads it
    // per row, but a reclassify walks every document.
    let issuers = super::sources::TrackedIssuerIndex::load(connection)?;

    let mut updated = 0usize;
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();

    for (id, company_id, title, url, current_kind) in &rows {
        let title = title.as_deref().unwrap_or("");
        let slug_trusted = !issuers.url_slug_names_foreign_issuer(company_id, title, url);
        let kind = classify_doc_kind_with_slug_trust(title, url, slug_trusted);
        let kind_str = kind.as_str();
        *by_kind.entry(kind_str.to_owned()).or_insert(0) += 1;

        if current_kind.as_deref() != Some(kind_str) {
            connection.execute(
                "
                UPDATE report_documents
                SET doc_kind = ?2,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
                params![id, kind_str],
            )?;
            updated += 1;
        }
    }

    Ok(ReclassifyReportDocumentsSummary {
        total: rows.len(),
        updated,
        by_kind,
    })
}

/// Stamp what a stored document's bytes really are (migration 0121, epic #229
/// T2). Kept separate from [`mark_fetched`] because the same write serves two
/// callers: the store-time sniff (right after the bytes land) and the startup
/// self-heal that back-fills legacy rows. Writing only when the value actually
/// changes keeps the self-heal idempotent — a second pass touches no row and
/// leaves `updated_at` alone.
pub(super) fn set_detected_container(
    connection: &Connection,
    id: &str,
    container: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE report_documents
        SET detected_container = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
          AND (detected_container IS NULL OR detected_container <> ?2)
        ",
        params![id, container],
    )?;
    Ok(())
}

/// Fetched documents with a stored file whose container was never sniffed —
/// the startup self-heal's work list. Returns `(id, local_path)` pairs; the
/// caller reads each file and stamps the result. A row whose file is missing is
/// deliberately left NULL by the caller so the next start retries it.
pub(super) fn list_needing_container_sniff(
    connection: &Connection,
) -> StorageResult<Vec<(String, String)>> {
    let mut statement = connection.prepare(
        "
        SELECT id, local_path
        FROM report_documents
        WHERE detected_container IS NULL
          AND local_path IS NOT NULL
          AND TRIM(local_path) <> ''
          AND fetch_status = 'fetched'
        ORDER BY id
        ",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
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
            updated_at,
            doc_kind,
            detected_container
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
                updated_at,
                doc_kind,
                detected_container
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
        doc_kind: row.get(17)?,
        detected_container: row.get(18)?,
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

use super::database::Database;
/// report_documents domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::report_documents()`.
#[derive(Clone)]
pub struct ReportDocumentStore {
    db: Database,
}

impl ReportDocumentStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_or_find_pending_report_document(
        &self,
        input: CaptureReportDocumentInput,
    ) -> StorageResult<ReportDocument> {
        let connection = self.db.checkout()?;

        create_or_find_pending(&connection, input)
    }

    pub fn mark_report_document_fetched(
        &self,
        id: &str,
        local_path: Option<&str>,
        content_type: Option<&str>,
        content_hash: Option<&str>,
        byte_size: Option<i64>,
    ) -> StorageResult<ReportDocument> {
        let connection = self.db.checkout()?;

        mark_fetched(
            &connection,
            id,
            local_path,
            content_type,
            content_hash,
            byte_size,
        )
    }

    /// Record the magic-byte container of a stored document's bytes (epic #229
    /// T2). `container` is a [`crate::fundamentals::extraction::container::Container`]
    /// marker (`pdf|zip|xml|html|unknown`) — never a filename guess.
    pub fn set_report_document_detected_container(
        &self,
        id: &str,
        container: &str,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        set_detected_container(&connection, id, container)
    }

    /// Fetched documents with a stored file and no sniffed container yet —
    /// the startup self-heal's work list, `(id, local_path)`.
    pub fn report_documents_needing_container_sniff(&self) -> StorageResult<Vec<(String, String)>> {
        let connection = self.db.checkout()?;

        list_needing_container_sniff(&connection)
    }

    pub fn mark_report_document_failed(
        &self,
        id: &str,
        error: &str,
    ) -> StorageResult<ReportDocument> {
        let connection = self.db.checkout()?;

        mark_failed(&connection, id, error)
    }

    pub fn mark_report_document_metadata_only(&self, id: &str) -> StorageResult<ReportDocument> {
        let connection = self.db.checkout()?;

        mark_metadata_only(&connection, id)
    }

    pub fn list_pending_attachment_documents(&self) -> StorageResult<Vec<ReportDocument>> {
        let connection = self.db.checkout()?;

        list_pending_attachments(&connection)
    }

    pub fn get_report_document(&self, id: &str) -> StorageResult<ReportDocument> {
        let connection = self.db.checkout()?;

        get(&connection, id)
    }

    /// FK-existence check for a report_document id (ADR 0093 decision 6): the
    /// MCP batch fact tool refuses with a typed `not_found` BEFORE any write
    /// when the caller's `reportDocumentId` doesn't exist — `get_report_document`
    /// would otherwise surface an unmapped `QueryReturnedNoRows` as `internal`.
    pub fn ensure_report_document_exists(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        validate_reference_exists(&connection, "report_documents", id)
    }

    pub fn list_report_documents_by_company(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<ReportDocument>> {
        let connection = self.db.checkout()?;

        list_by_company(&connection, company_id)
    }

    pub fn reclassify_report_documents(&self) -> StorageResult<ReclassifyReportDocumentsSummary> {
        let connection = self.db.checkout()?;

        reclassify_all(&connection)
    }

    /// Every report document registered from one originating feed item (`origin_ref`),
    /// newest first — the insider attachment tier's discovery of a filing's
    /// notification documents (T4b).
    pub fn list_report_documents_by_origin(
        &self,
        origin_ref: &str,
    ) -> StorageResult<Vec<ReportDocument>> {
        let connection = self.db.checkout()?;
        list_by_origin(&connection, origin_ref)
    }

    /// Tracked companies with NO fetched periodic (`periodic_ssf`/`periodic_jsf`)
    /// report document, each paired with its autopilot mode (`off` when the company
    /// has no settings row). Drives the automatic report-history backfill catch-up
    /// (v0.57, ADR 0077 amendment). `company_id = Some` narrows to one company; the
    /// caller enqueues a backfill for the automated ones and records the `off`-mode
    /// skips (never a silent drop).
    pub fn companies_lacking_periodic_coverage(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<(String, String)>> {
        let connection = self.db.checkout()?;
        companies_lacking_periodic_coverage(&connection, company_id)
    }
}

/// The coverage predicate for the backfill catch-up: a company "has report
/// history" iff it has at least one **fetched periodic** report document
/// (`periodic_ssf`/`periodic_jsf`) — the same eligibility the extraction pipeline
/// requires. Returns `(company_id, mode)` for those that do NOT, so the caller can
/// gate on automation. `NOT EXISTS` keeps it index-friendly on the doc-kind index.
fn companies_lacking_periodic_coverage(
    connection: &Connection,
    company_id: Option<&str>,
) -> StorageResult<Vec<(String, String)>> {
    let mut statement = connection.prepare(
        "
        SELECT companies.id,
               COALESCE(company_autopilot_settings.mode, 'off') AS mode
        FROM companies
        LEFT JOIN company_autopilot_settings
            ON company_autopilot_settings.company_id = companies.id
        WHERE (?1 IS NULL OR companies.id = ?1)
          AND NOT EXISTS (
              SELECT 1 FROM report_documents
              WHERE report_documents.company_id = companies.id
                AND report_documents.fetch_status = 'fetched'
                AND report_documents.doc_kind IN ('periodic_ssf', 'periodic_jsf')
          )
        ORDER BY companies.id
        ",
    )?;
    let rows = statement.query_map(params![company_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn list_by_origin(connection: &Connection, origin_ref: &str) -> StorageResult<Vec<ReportDocument>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id, company_id, period_id, source_type, origin_ref, url, local_path,
            content_type, content_hash, byte_size, title, attribution, fetch_status,
            fetch_error, fetched_at, created_at, updated_at, doc_kind, detected_container
        FROM report_documents
        WHERE origin_ref = ?1
        ORDER BY created_at DESC
        ",
    )?;
    let rows = statement.query_map(params![origin_ref], report_document_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}
