//! Container truth for stored report documents (epic #229 T2, closes #193).
//!
//! The stored filename and the server-sent content-type both lie. The T1 trust
//! audit measured **87 of 3807** stored files whose magic-byte container differs
//! from the extension/content_type (`pdf→xml` 38, `xhtml→pdf` 24, `xhtml→zip`
//! 18, `pdf→zip` 5, `pdf→html` 2). Until now only the structured-extraction
//! router sniffed the bytes ([`crate::jobs::structured_extraction::route_document`]);
//! every other consumer resolved format from the name via
//! [`SourceFormat::resolve`] and mis-routed those documents.
//!
//! This module is the single seam that turns the persisted
//! `report_documents.detected_container` column (migration 0121) into a routing
//! answer, so no consumer has to re-derive the precedence:
//!
//! 1. **`detected_container`** when the row has been sniffed — ground truth,
//!    read from the bytes themselves.
//! 2. **Name/content-type** ([`SourceFormat::resolve`] plus the `.zip`/`.xbri`
//!    package extensions) only when the column is still `NULL` — a legacy row
//!    whose file is missing, or one not yet reached by the startup self-heal.
//!    This fallback reproduces the pre-T2 behaviour exactly, so an un-sniffed
//!    row routes today as it did yesterday.
//!
//! Deliberately **no** global `zip → SourceFormat` answer: `SourceFormat` has
//! only `Pdf | Xhtml`, and a ZIP is neither — it is an ESEF/eSprawozdanie report
//! *package* whose inner iXBRL instance the structured path unpacks.
//! [`resolved_source_format`] therefore returns `None` for a ZIP (and for an
//! unrecognised container), forcing every call site to give it an honest,
//! site-specific treatment instead of pretending the bytes are a PDF.

use std::collections::BTreeMap;

use crate::fundamentals::extraction::container::{detect_container, Container};
use crate::report_diff::extraction::SourceFormat;
use crate::storage::{AppState, ReportDocument, StorageResult};

/// Parse a persisted `detected_container` value back into a [`Container`].
/// An unrecognised string (a value written by a future/older build) is treated
/// as *not sniffed* rather than silently coerced, so the name fallback applies.
fn container_from_stored(value: &str) -> Option<Container> {
    match value.trim() {
        "pdf" => Some(Container::Pdf),
        "zip" => Some(Container::Zip),
        "xml" => Some(Container::Xml),
        "html" => Some(Container::Html),
        "unknown" => Some(Container::Unknown),
        _ => None,
    }
}

/// The pre-T2 name/content-type guess, expressed as a [`Container`] so the
/// fallback and the sniffed answer share one vocabulary. Markup collapses onto
/// [`Container::Html`] (the `Xml`/`Html` split only matters to the byte sniffer);
/// a `.zip`/`.xbri` name is the ESEF package extension the structured router
/// already recognised; everything else was — and stays — a presumed PDF.
fn container_from_name(content_type: Option<&str>, path_or_url: &str) -> Container {
    if SourceFormat::resolve(content_type, path_or_url) == SourceFormat::Xhtml {
        return Container::Html;
    }
    let lower = path_or_url.to_ascii_lowercase();
    if lower.ends_with(".zip") || lower.ends_with(".xbri") {
        return Container::Zip;
    }
    Container::Pdf
}

/// What a stored document really is: its sniffed container when known, else the
/// name/content-type guess. Resolution uses the **local path** when a file is
/// stored and falls back to the URL, matching what each rerouted call site
/// previously passed.
pub fn resolved_container(document: &ReportDocument) -> Container {
    let name = document
        .local_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
        .unwrap_or(&document.url);
    resolved_container_named(document, name)
}

/// As [`resolved_container`], but with an explicit **name carrier** for the
/// un-sniffed fallback. The sniffed column always wins; `name` only decides what
/// a `NULL` row looks like.
///
/// This exists because the pre-T2 call sites did not agree on the carrier: the
/// extraction tiers resolved from `local_path` (the file they are about to read),
/// while the canonical tie-break resolved from `url` — it ranks candidate
/// documents that may not be fetched yet. Keeping that split explicit means an
/// un-sniffed row routes exactly as it did before T2 at every site.
pub fn resolved_container_named(document: &ReportDocument, name: &str) -> Container {
    if let Some(container) = document
        .detected_container
        .as_deref()
        .and_then(container_from_stored)
    {
        return container;
    }
    container_from_name(document.content_type.as_deref(), name)
}

/// The document's [`SourceFormat`] when its bytes are readable as one — a real
/// PDF, or markup (XML/HTML, including an iXBRL instance and a pdf2htmlEX
/// render).
///
/// `None` means the document is **not** text-extractable as either: a ZIP report
/// package (only the structured/package path can open it) or a container the
/// pipeline does not recognise. Callers must handle `None` honestly — recording
/// an unreadable outcome — rather than defaulting to `Pdf`, which is exactly the
/// mis-routing this epic removes.
pub fn resolved_source_format(document: &ReportDocument) -> Option<SourceFormat> {
    match resolved_container(document) {
        Container::Pdf => Some(SourceFormat::Pdf),
        Container::Xml | Container::Html => Some(SourceFormat::Xhtml),
        Container::Zip | Container::Unknown => None,
    }
}

/// Whether the document's real bytes are markup (an iXBRL instance, an ESEF
/// xhtml statement, or a pdf2htmlEX/HTML render).
pub fn is_markup(document: &ReportDocument) -> bool {
    matches!(
        resolved_container(document),
        Container::Xml | Container::Html
    )
}

/// Whether the document's real bytes are an ESEF/eSprawozdanie report *package*
/// (a ZIP). The structured path unpacks its inner iXBRL instance; no text tier
/// can read it directly.
pub fn is_package(document: &ReportDocument) -> bool {
    resolved_container(document) == Container::Zip
}

/// Whether the document's real bytes are a genuine PDF — the by-design
/// "core KPIs arrive from the aggregator" case (ADR 0086 dec. 1). A ZIP or XML
/// stored under a `.pdf` name is **not** one.
pub fn is_real_pdf(document: &ReportDocument) -> bool {
    resolved_container(document) == Container::Pdf
}

/// Aggregate outcome of the startup container self-heal.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContainerSniffSummary {
    /// Rows examined (fetched, with a stored file, `detected_container IS NULL`).
    pub scanned: usize,
    /// Rows stamped with a container read from their bytes.
    pub stamped: usize,
    /// Rows whose file could not be read — left `NULL` so the next start retries.
    pub file_missing: usize,
    /// Final per-container counts over the rows stamped by this pass.
    pub by_container: BTreeMap<String, usize>,
}

/// Back-fill `detected_container` for fetched documents stored before the column
/// existed (migration 0121 cannot read file bytes from SQL).
///
/// Idempotent and best-effort, mirroring `repair_misassociated_report_documents`
/// (wired in `lib.rs` setup): a second run finds no candidates and writes
/// nothing. The two failure modes are kept apart on purpose:
///
/// - **File gone / unreadable** → the row is left `NULL`, counted as
///   `file_missing`, and retried on the next start. Stamping it would freeze a
///   guess made from an absent file.
/// - **Readable but unrecognised bytes** → stamped `unknown`, an honest terminal
///   answer that stops the PDF reader from being handed garbage forever.
pub fn sniff_missing_containers(state: &AppState) -> StorageResult<ContainerSniffSummary> {
    let candidates = state.report_documents_needing_container_sniff()?;
    let mut summary = ContainerSniffSummary {
        scanned: candidates.len(),
        ..Default::default()
    };

    for (id, local_path) in candidates {
        let path = state.data_dir().join(&local_path);
        let Ok(bytes) = std::fs::read(&path) else {
            summary.file_missing += 1;
            continue;
        };
        let container = detect_container(&bytes);
        state.set_report_document_detected_container(&id, container.as_str())?;
        summary.stamped += 1;
        *summary
            .by_container
            .entry(container.as_str().to_owned())
            .or_insert(0) += 1;
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::CaptureReportDocumentInput;
    use std::path::PathBuf;

    fn app(tag: &str) -> AppState {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "brawler-container-truth-{}-{tag}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("data dir");
        let connection = crate::storage::open_in_memory_database().expect("db");
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
                [],
            )
            .expect("company");
        AppState::with_data_dir(connection, dir)
    }

    /// Register a fetched document with a file on disk, leaving
    /// `detected_container` NULL (the legacy shape the self-heal repairs).
    fn fetched_doc(state: &AppState, file_name: &str, bytes: &[u8]) -> String {
        std::fs::write(state.data_dir().join(file_name), bytes).expect("write file");
        register_row(state, file_name, Some(bytes.len() as i64))
    }

    fn register_row(state: &AppState, file_name: &str, byte_size: Option<i64>) -> String {
        let doc = state
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
        state
            .mark_report_document_fetched(
                &doc.id,
                Some(file_name),
                Some("application/octet-stream"),
                None,
                byte_size,
            )
            .expect("mark fetched");
        doc.id
    }

    #[test]
    fn stored_container_beats_the_lying_extension() {
        let state = app("resolver");
        let id = fetched_doc(&state, "report.pdf", b"<?xml version=\"1.0\"?><x/>");
        // Before the sniff: the name wins and the row looks like a PDF.
        let before = state.get_report_document(&id).expect("doc");
        assert_eq!(resolved_container(&before), Container::Pdf);

        state
            .set_report_document_detected_container(&id, "xml")
            .expect("stamp");
        let after = state.get_report_document(&id).expect("doc");
        assert_eq!(resolved_container(&after), Container::Xml);
        assert_eq!(resolved_source_format(&after), Some(SourceFormat::Xhtml));
        assert!(is_markup(&after));
        assert!(!is_real_pdf(&after));
    }

    #[test]
    fn a_zip_and_an_unknown_container_have_no_source_format() {
        let state = app("nonformat");
        let zip_id = fetched_doc(&state, "package.pdf", b"PK\x03\x04\x14\x00");
        let junk_id = fetched_doc(&state, "junk.pdf", b"\x00\x01\x02not a document");
        sniff_missing_containers(&state).expect("sniff");

        let zip = state.get_report_document(&zip_id).expect("doc");
        assert_eq!(resolved_source_format(&zip), None);
        assert!(is_package(&zip));

        let junk = state.get_report_document(&junk_id).expect("doc");
        assert_eq!(resolved_source_format(&junk), None);
        assert!(!is_package(&junk));
        assert!(!is_markup(&junk));
    }

    #[test]
    fn startup_pass_stamps_every_readable_row_and_is_idempotent() {
        let state = app("selfheal");
        let xml_id = fetched_doc(&state, "a.pdf", b"<?xml version=\"1.0\"?><x/>");
        let pdf_id = fetched_doc(&state, "b.xhtml", b"%PDF-1.7\nbody");
        let zip_id = fetched_doc(&state, "c.xhtml", b"PK\x03\x04\x14\x00");
        let junk_id = fetched_doc(&state, "d.pdf", b"\x00\x01\x02junk");

        let first = sniff_missing_containers(&state).expect("first pass");
        assert_eq!(first.scanned, 4);
        assert_eq!(first.stamped, 4);
        assert_eq!(first.file_missing, 0);
        assert_eq!(first.by_container.get("xml"), Some(&1));
        assert_eq!(first.by_container.get("pdf"), Some(&1));
        assert_eq!(first.by_container.get("zip"), Some(&1));
        assert_eq!(first.by_container.get("unknown"), Some(&1));

        let stored = |id: &str| {
            state
                .get_report_document(id)
                .expect("doc")
                .detected_container
        };
        assert_eq!(stored(&xml_id), Some("xml".to_owned()));
        assert_eq!(stored(&pdf_id), Some("pdf".to_owned()));
        assert_eq!(stored(&zip_id), Some("zip".to_owned()));
        assert_eq!(stored(&junk_id), Some("unknown".to_owned()));

        // Idempotence: a second start finds nothing left to do.
        let second = sniff_missing_containers(&state).expect("second pass");
        assert_eq!(second, ContainerSniffSummary::default());
    }

    #[test]
    fn a_missing_file_stays_null_and_is_retried_next_start() {
        let state = app("missingfile");
        // A fetched row pointing at a file that is not on disk (pruned bytes, a
        // half-restored data dir) must NOT be frozen as `unknown` — `unknown`
        // means "we read the bytes and did not recognise them".
        let id = register_row(&state, "gone.pdf", Some(10));

        let first = sniff_missing_containers(&state).expect("first pass");
        assert_eq!(first.scanned, 1);
        assert_eq!(first.stamped, 0);
        assert_eq!(first.file_missing, 1);
        assert_eq!(
            state
                .get_report_document(&id)
                .expect("doc")
                .detected_container,
            None,
            "a file-gone row stays NULL so the next start retries it"
        );

        // The file reappears: the retry now stamps the real container.
        std::fs::write(state.data_dir().join("gone.pdf"), b"%PDF-1.4 x").expect("write");
        let second = sniff_missing_containers(&state).expect("second pass");
        assert_eq!(second.stamped, 1);
        assert_eq!(
            state
                .get_report_document(&id)
                .expect("doc")
                .detected_container,
            Some("pdf".to_owned())
        );
    }
}
