//! Magic-byte container detection for stored report documents (card `eb71488`,
//! v0.59 "reliable fundamentals").
//!
//! The extraction router must **not** trust the stored filename. A measured 4.4%
//! of the maintainer's stored `.pdf` documents (19 of 430 periodic filings, 7
//! companies) are actually XML/ZIP/HTML under a `.pdf` name — a pdf2htmlEX
//! visual render, a signed eSprawozdanie/ESEF report package, or an HTML-Tidy
//! export — and they fail 100% today because routing hands a non-PDF to the PDF
//! reader. Several of those XMLs carry real financial data, so correct routing
//! does not merely stop a wasted parse: it can *add* coverage via the
//! ESEF/iXBRL/positional tiers that already exist.
//!
//! This module answers one narrow question from the bytes alone — "what
//! container is this, really?" — leaving *how to parse each container* to the
//! router in [`crate::jobs::structured_extraction`]. Detection is deliberately
//! conservative: it recognises the four containers the pipeline can act on and
//! reports everything else as [`Container::Unknown`], which the router turns
//! into an honest `document_unreadable` outcome (never a crash, never a silent
//! PDF-reader failure).

/// PKZIP local-file-header magic — the first bytes of every ZIP container
/// (ESEF/eSprawozdanie report packages arrive as ZIPs). Shared conceptually
/// with [`crate::fundamentals::extraction::esef_package`], which sniffs the same
/// four bytes for the package path.
const ZIP_MAGIC: &[u8] = b"PK\x03\x04";

/// The PDF header token. The PDF spec (§7.5.2) permits leading bytes before
/// `%PDF-`, so real-world readers scan a window rather than requiring it at
/// offset 0; we do the same over the first [`PDF_SCAN_WINDOW`] bytes so a valid
/// PDF with a byte-order junk prefix is never misclassified as Unknown.
const PDF_MAGIC: &[u8] = b"%PDF";

/// How far into the file to look for the PDF header. Comfortably covers the
/// spec's tolerance for pre-header bytes while staying cheap.
const PDF_SCAN_WINDOW: usize = 1024;

/// What a stored document's leading bytes reveal it to be — independent of the
/// filename or the recorded content-type, both of which the maintainer's corpus
/// proves unreliable (every mislabeled file was `application/octet-stream`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// A PDF (`%PDF` within the first [`PDF_SCAN_WINDOW`] bytes).
    Pdf,
    /// A ZIP archive — an ESEF/eSprawozdanie report package.
    Zip,
    /// An XML document (`<?xml …`), including pdf2htmlEX xhtml renders.
    Xml,
    /// An HTML document (`<!doctype html>` / `<html …>` / a bare start tag).
    Html,
    /// None of the containers the pipeline can act on — an honest, explicit gap.
    Unknown,
}

impl Container {
    /// Stable marker for outcome detail / logs (ADR 0084 decision 6 — typed
    /// markers, not prose).
    pub fn as_str(self) -> &'static str {
        match self {
            Container::Pdf => "pdf",
            Container::Zip => "zip",
            Container::Xml => "xml",
            Container::Html => "html",
            Container::Unknown => "unknown",
        }
    }
}

/// Detect the container from a document's leading bytes.
///
/// Order matters: the two binary signatures (`%PDF`, ZIP magic) are checked
/// first and exactly, then — after stripping a UTF-8/UTF-16 BOM and leading
/// whitespace — the textual markup preambles. A ZIP is recognised only at
/// offset 0 (its magic is the local-file header), whereas `%PDF` is scanned
/// across a window because the spec tolerates pre-header bytes.
pub fn detect_container(bytes: &[u8]) -> Container {
    // ZIP magic is a fixed 4-byte prefix; check it before the PDF window so a
    // ZIP whose payload happens to contain the ASCII "%PDF" (an embedded PDF
    // entry) is still routed as the package it is.
    if bytes.starts_with(ZIP_MAGIC) {
        return Container::Zip;
    }
    let pdf_window = &bytes[..bytes.len().min(PDF_SCAN_WINDOW)];
    if contains(pdf_window, PDF_MAGIC) {
        return Container::Pdf;
    }

    // Textual containers: strip a BOM and leading whitespace, then read the
    // first tag. `<?xml` is XML (pdf2htmlEX renders open this way); an html
    // doctype or `<html` root is HTML; any other leading tag is treated as
    // generic HTML markup so a fragment still reaches the positional/aggregator
    // parsers rather than the PDF reader.
    let text = strip_leading(bytes);
    let lower: Vec<u8> = text
        .iter()
        .take(64)
        .map(|b| b.to_ascii_lowercase())
        .collect();
    if lower.starts_with(b"<?xml") {
        return Container::Xml;
    }
    if lower.starts_with(b"<!doctype html") || lower.starts_with(b"<html") {
        return Container::Html;
    }
    if lower.first() == Some(&b'<') {
        return Container::Html;
    }

    Container::Unknown
}

/// Strip a leading UTF-8/UTF-16 byte-order mark and ASCII whitespace, so a file
/// that opens with `\xEF\xBB\xBF<?xml` or a blank line is still recognised.
fn strip_leading(bytes: &[u8]) -> &[u8] {
    const BOMS: [&[u8]; 3] = [b"\xEF\xBB\xBF", b"\xFF\xFE", b"\xFE\xFF"];
    let mut rest = bytes;
    for bom in BOMS {
        if let Some(stripped) = rest.strip_prefix(bom) {
            rest = stripped;
            break;
        }
    }
    let start = rest
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(rest.len());
    &rest[start..]
}

/// Whether `needle` appears anywhere in `haystack`. Small linear scan — the
/// window is at most [`PDF_SCAN_WINDOW`] bytes.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_pdf_at_offset_zero() {
        assert_eq!(detect_container(b"%PDF-1.7\n..."), Container::Pdf);
    }

    #[test]
    fn detects_pdf_with_leading_junk_within_window() {
        // The PDF spec tolerates pre-header bytes; a lenient reader accepts them,
        // so detection must too rather than falling to Unknown.
        let mut bytes = vec![0u8; 200];
        bytes.extend_from_slice(b"%PDF-1.4 rest");
        assert_eq!(detect_container(&bytes), Container::Pdf);
    }

    #[test]
    fn detects_zip_by_magic_even_without_extension() {
        assert_eq!(detect_container(b"PK\x03\x04\x14\x00"), Container::Zip);
    }

    #[test]
    fn zip_wins_over_embedded_pdf_bytes() {
        // A ZIP whose payload contains "%PDF" (a signed package with an embedded
        // PDF entry, exactly the maintainer's eSprawozdanie case) is the package,
        // not a PDF.
        let mut bytes = b"PK\x03\x04".to_vec();
        bytes.extend_from_slice(b"....%PDF-1.4....");
        assert_eq!(detect_container(&bytes), Container::Zip);
    }

    #[test]
    fn detects_xml_including_bom_and_pdf2htmlex() {
        assert_eq!(detect_container(b"<?xml version=\"1.0\"?>"), Container::Xml);
        // The real pdf2htmlEX render shape from the maintainer's corpus.
        let real = b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- Created by pdf2htmlEX -->\n<html>";
        assert_eq!(detect_container(real), Container::Xml);
    }

    #[test]
    fn detects_html_by_doctype_and_root_and_bare_tag() {
        assert_eq!(
            detect_container(b"<!DOCTYPE html><html></html>"),
            Container::Html
        );
        assert_eq!(
            detect_container(b"<html xmlns=\"http://www.w3.org/1999/xhtml\">"),
            Container::Html
        );
        assert_eq!(
            detect_container(b"   \n<table><tr></tr></table>"),
            Container::Html
        );
    }

    #[test]
    fn garbage_and_empty_are_unknown() {
        assert_eq!(
            detect_container(b"\x00\x01\x02not a document"),
            Container::Unknown
        );
        assert_eq!(detect_container(b""), Container::Unknown);
        assert_eq!(
            detect_container(b"plain text with no tag"),
            Container::Unknown
        );
    }
}
