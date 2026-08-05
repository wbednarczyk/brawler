//! Report section-extraction job (v0.47.0, ADR 0052).
//!
//! Loads a stored financial-statement document's bytes, extracts ordered text
//! sections (dual-format, panic-safe, density-flagged — see
//! [`crate::report_diff::extraction`]), and persists them to the derived
//! `report_document_sections` index. CPU-bound, so it runs off the UI thread
//! (the command offloads it via `spawn_blocking`). Idempotent: re-running a
//! document whose bytes + extractor version are unchanged is skipped.

use sha2::{Digest, Sha256};

use crate::report_diff::extraction::{extract_report, SourceFormat, EXTRACTOR_VERSION};
use crate::report_diff::ExtractionState;
use crate::storage::{AppState, StoredExtraction, StoredSection};

/// Outcome of one extraction run, surfaced to the caller/command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReportExtractionResult {
    pub state: ExtractionState,
    pub format: SourceFormat,
    pub section_count: usize,
    /// True when the document was already current and extraction was skipped.
    pub skipped: bool,
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    // digest 0.11 dropped LowerHex on the output array — hex-encode manually.
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut hex, byte| {
            let _ = write!(hex, "{byte:02x}");
            hex
        })
}

/// Extract and persist sections for one report document. Returns a structured
/// outcome; missing-file / unreadable cases are surfaced as a recoverable error
/// string (the document stays un-extracted, never crashing the worker).
pub fn run_report_extraction(
    state: &AppState,
    report_document_id: &str,
) -> Result<ReportExtractionResult, String> {
    let document = state
        .report_documents()
        .get_report_document(report_document_id)
        .map_err(|error| error.to_string())?;

    let local_path = document
        .local_path
        .as_deref()
        .filter(|p| !p.trim().is_empty())
        .ok_or_else(|| "report document has no stored file (no_local_file)".to_owned())?;

    let path = state.data_dir().join(local_path);
    let bytes = std::fs::read(&path)
        .map_err(|error| format!("could not read report file {}: {error}", path.display()))?;

    let content_hash = sha256_hex(&bytes);
    let store = state.report_sections();

    // Skip when the same bytes were already extracted by this heuristic version.
    if store
        .is_extraction_current(report_document_id, &content_hash, EXTRACTOR_VERSION)
        .map_err(|error| error.to_string())?
    {
        let existing = store
            .extraction(report_document_id)
            .map_err(|error| error.to_string())?;
        let (state_str, format_str) = existing
            .map(|e| (e.extraction_state, e.source_format))
            .unwrap_or_else(|| ("extracted".to_owned(), "pdf".to_owned()));
        return Ok(ReportExtractionResult {
            state: parse_state(&state_str),
            format: parse_format(&format_str),
            section_count: store
                .sections(report_document_id)
                .map(|s| s.len())
                .unwrap_or(0),
            skipped: true,
        });
    }

    // Container truth (epic #229 T2): the stored `detected_container` decides the
    // reader, so an XHTML render under a `.pdf` name is parsed as markup instead
    // of being handed to the PDF reader (where it fails 100%). A ZIP report
    // package or an unrecognised container has no readable text layer at all —
    // that is the same *unreadable* class as a missing file above, surfaced as a
    // recoverable error with the document left un-extracted, never a fake
    // `source_format` row.
    let Some(format) = crate::report_documents_container::resolved_source_format(&document) else {
        return Err(format!(
            "report document is a {} container, not readable text (unreadable_container)",
            crate::report_documents_container::resolved_container(&document).as_str()
        ));
    };
    let outcome = extract_report(&bytes, format);

    let stored_sections: Vec<StoredSection> = outcome
        .sections
        .iter()
        .map(|s| StoredSection {
            ordinal: s.ordinal,
            heading: s.heading.clone(),
            body: s.body.clone(),
        })
        .collect();

    let extraction = StoredExtraction {
        report_document_id: report_document_id.to_owned(),
        source_format: outcome.format.as_str().to_owned(),
        extraction_state: outcome.state.as_str().to_owned(),
        char_count: outcome.char_count as i64,
        content_hash: Some(content_hash),
        extractor_version: EXTRACTOR_VERSION,
    };
    store
        .replace_extraction(&extraction, &stored_sections)
        .map_err(|error| error.to_string())?;

    Ok(ReportExtractionResult {
        state: outcome.state,
        format: outcome.format,
        section_count: stored_sections.len(),
        skipped: false,
    })
}

fn parse_state(s: &str) -> ExtractionState {
    match s {
        "no_text_layer" => ExtractionState::NoTextLayer,
        "extraction_failed" => ExtractionState::ExtractionFailed,
        _ => ExtractionState::Extracted,
    }
}

fn parse_format(s: &str) -> SourceFormat {
    match s {
        "xhtml" => SourceFormat::Xhtml,
        _ => SourceFormat::Pdf,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{AppState, CaptureReportDocumentInput};
    use rusqlite::Connection;
    use std::path::PathBuf;

    /// The hand-rolled hex encoding (digest 0.11 dropped LowerHex) must keep
    /// producing canonical lowercase sha256 hex — provenance hashes depend on it.
    #[test]
    fn sha256_hex_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// A unique temp data dir per test invocation (mirrors the KPI job test).
    fn app_with_dir(tag: &str) -> AppState {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "brawler-report-extraction-{}-{tag}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("data dir");
        let connection = crate::storage::open_in_memory_database().expect("db");
        seed_company(&connection);
        AppState::with_data_dir(connection, dir)
    }

    fn seed_company(connection: &Connection) {
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
                [],
            )
            .expect("company");
    }

    fn register_doc(state: &AppState, file_name: &str, bytes: &[u8]) -> String {
        std::fs::write(state.data_dir().join(file_name), bytes).expect("write file");
        let doc = state
            .report_documents()
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: "c1".to_owned(),
                source_type: "espi_attachment".to_owned(),
                url: format!("https://x/{file_name}"),
                period_id: None,
                origin_ref: None,
                title: Some(file_name.to_owned()),
                attribution: None,
            })
            .expect("doc");
        let content_type = if file_name.ends_with(".xhtml") {
            "application/xhtml+xml"
        } else {
            "application/pdf"
        };
        state
            .report_documents()
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some(content_type),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        doc.id
    }

    #[test]
    fn extracts_xhtml_sections_and_is_idempotent() {
        let state = app_with_dir("xhtml");
        let body = format!(
            "<html><body><h1>Bilans</h1><p>{}</p><h1>Rachunek zysków</h1><p>przychody 100</p></body></html>",
            "aktywa ".repeat(600)
        );
        let id = register_doc(&state, "report.xhtml", body.as_bytes());

        let first = run_report_extraction(&state, &id).expect("extract");
        assert_eq!(first.state, ExtractionState::Extracted);
        assert!(!first.skipped);
        assert!(first.section_count >= 2);

        // Re-run: same bytes + version -> skipped.
        let second = run_report_extraction(&state, &id).expect("re-extract");
        assert!(second.skipped);
    }

    /// Epic #229 T2: 24 of the maintainer's stored files are markup under a `.pdf`
    /// name; the sniffed container picks the markup reader and the sections land.
    #[test]
    fn markup_stored_under_a_pdf_name_extracts_as_markup() {
        let state = app_with_dir("liar");
        let body = format!(
            "<html><body><h1>Bilans</h1><p>{}</p><h1>Rachunek zysków</h1><p>przychody 100</p></body></html>",
            "aktywa ".repeat(600)
        );
        let id = register_doc(&state, "report.pdf", body.as_bytes());
        state
            .report_documents()
            .set_report_document_detected_container(&id, "html")
            .expect("stamp container");

        let result = run_report_extraction(&state, &id).expect("extract");
        assert_eq!(result.state, ExtractionState::Extracted);
        assert_eq!(result.format, SourceFormat::Xhtml);
        assert!(result.section_count >= 2);
    }

    /// A ZIP report package has no text layer at all. That is the same *unreadable*
    /// class as a missing file — a recoverable error naming the container, with the
    /// document left un-extracted, never a `report_document_extractions` row
    /// claiming a `pdf` source format the bytes do not have.
    #[test]
    fn a_zip_container_is_an_honest_unreadable_error_not_a_pdf_parse() {
        let state = app_with_dir("zip");
        let id = register_doc(&state, "package.pdf", b"PK\x03\x04\x14\x00stuff");
        state
            .report_documents()
            .set_report_document_detected_container(&id, "zip")
            .expect("stamp container");

        let error = run_report_extraction(&state, &id).expect_err("a package is not readable text");
        assert!(
            error.contains("zip") && error.contains("unreadable_container"),
            "the error must name the detected container: {error}"
        );
        assert!(
            state
                .report_sections()
                .extraction(&id)
                .expect("extraction lookup")
                .is_none(),
            "no extraction row may be stored for a container we cannot read"
        );
    }

    #[test]
    fn flags_no_text_layer_for_tiny_document() {
        let state = app_with_dir("tiny");
        let id = register_doc(
            &state,
            "tiny.xhtml",
            b"<html><body><p>short</p></body></html>",
        );
        let result = run_report_extraction(&state, &id).expect("extract");
        assert_eq!(result.state, ExtractionState::NoTextLayer);
        assert_eq!(result.section_count, 0);
    }
}
