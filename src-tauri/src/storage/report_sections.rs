//! Storage for the report-over-report diff's derived section index
//! (`report_document_extractions` + `report_document_sections`, ADR 0052, v0.47.0).
//!
//! Every row is derived from a report document's stored bytes by pure-Rust
//! extraction — never a source of truth. Dropping these tables loses zero
//! canonical data; it only forces re-extraction. One extraction row per document
//! records the outcome (`extracted` | `no_text_layer` | `extraction_failed`) and
//! its source format; section rows exist only for `extracted` documents.

use rusqlite::{params, Connection, OptionalExtension};

use super::database::Database;
use super::{StorageError, StorageResult};

/// The persisted extraction outcome for one report document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredExtraction {
    pub report_document_id: String,
    pub source_format: String,
    pub extraction_state: String,
    pub char_count: i64,
    pub content_hash: Option<String>,
    pub extractor_version: i64,
}

/// One stored section of an extracted document, in document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSection {
    pub ordinal: i64,
    pub heading: String,
    pub body: String,
}

/// Replace the extraction row and all section rows for one document atomically.
/// Idempotent: re-running with the same input converges on the same rows.
pub(crate) fn replace_extraction(
    connection: &mut Connection,
    extraction: &StoredExtraction,
    sections: &[StoredSection],
) -> StorageResult<()> {
    let tx = connection.transaction()?;
    tx.execute(
        "DELETE FROM report_document_sections WHERE report_document_id = ?1",
        params![extraction.report_document_id],
    )?;
    tx.execute(
        "
        INSERT INTO report_document_extractions
            (report_document_id, source_format, extraction_state, char_count,
             content_hash, extractor_version)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ON CONFLICT (report_document_id) DO UPDATE SET
            source_format = excluded.source_format,
            extraction_state = excluded.extraction_state,
            char_count = excluded.char_count,
            content_hash = excluded.content_hash,
            extractor_version = excluded.extractor_version,
            extracted_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            extraction.report_document_id,
            extraction.source_format,
            extraction.extraction_state,
            extraction.char_count,
            extraction.content_hash,
            extraction.extractor_version,
        ],
    )?;
    for section in sections {
        tx.execute(
            "
            INSERT INTO report_document_sections
                (report_document_id, ordinal, heading, body, content_hash, extractor_version)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                extraction.report_document_id,
                section.ordinal,
                section.heading,
                section.body,
                extraction.content_hash,
                extraction.extractor_version,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// The stored extraction outcome for a document, if extracted.
pub(crate) fn get_extraction(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Option<StoredExtraction>> {
    connection
        .query_row(
            "SELECT report_document_id, source_format, extraction_state, char_count,
                    content_hash, extractor_version
             FROM report_document_extractions WHERE report_document_id = ?1",
            params![report_document_id],
            |row| {
                Ok(StoredExtraction {
                    report_document_id: row.get(0)?,
                    source_format: row.get(1)?,
                    extraction_state: row.get(2)?,
                    char_count: row.get(3)?,
                    content_hash: row.get(4)?,
                    extractor_version: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

/// All stored sections for a document, in document order.
pub(crate) fn get_sections(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Vec<StoredSection>> {
    let mut statement = connection.prepare(
        "SELECT ordinal, heading, body FROM report_document_sections
         WHERE report_document_id = ?1 ORDER BY ordinal",
    )?;
    let rows = statement.query_map(params![report_document_id], |row| {
        Ok(StoredSection {
            ordinal: row.get(0)?,
            heading: row.get(1)?,
            body: row.get(2)?,
        })
    })?;
    let mut sections = Vec::new();
    for row in rows {
        sections.push(row?);
    }
    Ok(sections)
}

/// True when a current extraction exists for `(content_hash, extractor_version)`,
/// so re-extraction can skip unchanged documents.
pub(crate) fn extraction_is_current(
    connection: &Connection,
    report_document_id: &str,
    content_hash: &str,
    extractor_version: i64,
) -> StorageResult<bool> {
    let found: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM report_document_extractions
             WHERE report_document_id = ?1 AND content_hash = ?2 AND extractor_version = ?3",
            params![report_document_id, content_hash, extractor_version],
            |row| row.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Report-sections domain store (Architecture v2 / ADR 0050). Owns a [`Database`]
/// and exposes only this domain's operations. Reach it via `AppState::report_sections()`.
#[derive(Clone)]
pub struct ReportSectionStore {
    db: Database,
}

impl ReportSectionStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn replace_extraction(
        &self,
        extraction: &StoredExtraction,
        sections: &[StoredSection],
    ) -> StorageResult<()> {
        let mut connection = self.db.checkout()?;
        replace_extraction(&mut connection, extraction, sections)
    }

    pub fn extraction(&self, report_document_id: &str) -> StorageResult<Option<StoredExtraction>> {
        let connection = self.db.checkout()?;
        get_extraction(&connection, report_document_id)
    }

    pub fn sections(&self, report_document_id: &str) -> StorageResult<Vec<StoredSection>> {
        let connection = self.db.checkout()?;
        get_sections(&connection, report_document_id)
    }

    pub fn is_extraction_current(
        &self,
        report_document_id: &str,
        content_hash: &str,
        extractor_version: i64,
    ) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        extraction_is_current(
            &connection,
            report_document_id,
            content_hash,
            extractor_version,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    fn seed_company_and_document(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
                [],
            )
            .expect("company");
        connection
            .execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES ('doc1', 'c1', 'espi_attachment', 'https://x/doc1.pdf', 'fetched')",
                [],
            )
            .expect("document");
    }

    #[test]
    fn round_trips_extraction_and_sections() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection);

        let extraction = StoredExtraction {
            report_document_id: "doc1".to_owned(),
            source_format: "pdf".to_owned(),
            extraction_state: "extracted".to_owned(),
            char_count: 1234,
            content_hash: Some("hash1".to_owned()),
            extractor_version: 1,
        };
        let sections = vec![
            StoredSection {
                ordinal: 0,
                heading: "<preamble>".to_owned(),
                body: "intro".to_owned(),
            },
            StoredSection {
                ordinal: 1,
                heading: "bilans".to_owned(),
                body: "aktywa 100".to_owned(),
            },
        ];
        replace_extraction(&mut connection, &extraction, &sections).expect("replace");

        assert_eq!(
            get_extraction(&connection, "doc1").expect("get"),
            Some(extraction)
        );
        assert_eq!(get_sections(&connection, "doc1").expect("sec"), sections);
        assert!(extraction_is_current(&connection, "doc1", "hash1", 1).expect("cur"));
        assert!(!extraction_is_current(&connection, "doc1", "hash1", 2).expect("ver"));
    }

    #[test]
    fn replace_is_idempotent_and_replaces_sections() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection);
        let base = StoredExtraction {
            report_document_id: "doc1".to_owned(),
            source_format: "pdf".to_owned(),
            extraction_state: "extracted".to_owned(),
            char_count: 10,
            content_hash: Some("h".to_owned()),
            extractor_version: 1,
        };
        replace_extraction(
            &mut connection,
            &base,
            &[StoredSection {
                ordinal: 0,
                heading: "a".to_owned(),
                body: "1".to_owned(),
            }],
        )
        .expect("first");
        // Re-extract with fewer sections: old sections must not linger.
        replace_extraction(
            &mut connection,
            &base,
            &[StoredSection {
                ordinal: 0,
                heading: "b".to_owned(),
                body: "2".to_owned(),
            }],
        )
        .expect("second");

        let sections = get_sections(&connection, "doc1").expect("sec");
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].heading, "b");
    }

    #[test]
    fn no_text_layer_stores_state_without_sections() {
        let mut connection = open_in_memory_database().expect("db");
        seed_company_and_document(&connection);
        let extraction = StoredExtraction {
            report_document_id: "doc1".to_owned(),
            source_format: "pdf".to_owned(),
            extraction_state: "no_text_layer".to_owned(),
            char_count: 40,
            content_hash: Some("scan".to_owned()),
            extractor_version: 1,
        };
        replace_extraction(&mut connection, &extraction, &[]).expect("replace");
        assert_eq!(
            get_extraction(&connection, "doc1")
                .expect("get")
                .map(|e| e.extraction_state),
            Some("no_text_layer".to_owned())
        );
        assert!(get_sections(&connection, "doc1").expect("sec").is_empty());
    }
}
