//! ESEF report-package (ZIP) handling — locate the inline-XBRL instance (ADR 0061
//! decision 1; the T7-C follow-up the ADR flagged as out of scope for the initial
//! slice: "unpacking a ZIP package is a separate follow-up").
//!
//! A GPW ESEF annual filing is delivered as an **ESEF report package**: a ZIP
//! whose extension is `.xbri` (per the xbrl.org report-package spec) or `.zip`,
//! bundling a taxonomy plus the inline-XBRL **instance document** under a
//! top-level `reports/` folder. The rest of the pipeline (`esef::parse_esef`)
//! parses the bare instance xHTML; this module is the thin seam that pulls that
//! instance out of the container so the deterministic ESEF tier can see it. It is
//! pure over `&[u8]` (an in-memory ZIP read) — no filesystem IO, fully testable.

use std::io::Read;

/// PKZIP local-file-header magic — the first bytes of every ZIP container.
const ZIP_MAGIC: &[u8] = b"PK\x03\x04";

/// Upper bound on an unpacked instance we will read into memory. Real ESEF
/// instances run to a few MB (CBF's FY2025 is ~10 MB); this cap keeps a hostile
/// or corrupt package from ballooning memory while staying far above any real
/// filing.
const MAX_INSTANCE_BYTES: u64 = 64 * 1024 * 1024;

/// True when the stored bytes/extension denote an ESEF report package (a ZIP
/// container). The extension is the primary signal (`.xbri`/`.zip`); the ZIP
/// magic is a fallback for a package delivered with a misleading extension or a
/// generic `application/octet-stream` content type (exactly how the maintainer's
/// real `.xbri` was stored).
pub fn is_report_package(path: &str, bytes: &[u8]) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.ends_with(".xbri") || lower.ends_with(".zip") || bytes.starts_with(ZIP_MAGIC)
}

/// Extract the inline-XBRL instance document bytes from an ESEF report package.
///
/// Per the xbrl.org report-package spec the instance document(s) live in the
/// top-level `reports/` folder, so an entry under a `reports/` path wins; among
/// candidates the **largest** `.xhtml`/`.html` entry is chosen (the primary
/// statement document dwarfs any auxiliary page). Falls back to the largest
/// xhtml anywhere when nothing sits under `reports/`. `None` when the bytes are
/// not a readable ZIP, hold no xhtml entry, or the instance exceeds
/// [`MAX_INSTANCE_BYTES`].
pub fn extract_instance(bytes: &[u8]) -> Option<Vec<u8>> {
    let reader = std::io::Cursor::new(bytes);
    let mut archive = zip::ZipArchive::new(reader).ok()?;

    // Pick the best candidate index first (an immutable scan), then read it —
    // `ZipArchive::by_index` borrows the archive mutably, so we cannot hold a
    // file handle open across the scan.
    let mut best: Option<(bool, u64, usize)> = None; // (under reports/, size, index)
    for i in 0..archive.len() {
        let file = archive.by_index(i).ok()?;
        if !file.is_file() {
            continue;
        }
        let name = file.name().to_ascii_lowercase();
        if !(name.ends_with(".xhtml") || name.ends_with(".html")) {
            continue;
        }
        let in_reports = name.starts_with("reports/") || name.contains("/reports/");
        let size = file.size();
        // Order candidates by (under reports/, size); `false < true` so a
        // reports/ entry outranks a sibling, larger outranks smaller within.
        let candidate = (in_reports, size, i);
        if best.is_none_or(|b| (b.0, b.1) < (candidate.0, candidate.1)) {
            best = Some(candidate);
        }
    }

    let (_, size, idx) = best?;
    if size > MAX_INSTANCE_BYTES {
        return None;
    }
    let mut file = archive.by_index(idx).ok()?;
    let mut out = Vec::with_capacity(size as usize);
    file.read_to_end(&mut out).ok()?;
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;

    /// Builds a minimal ESEF report package: a ZIP with the named entries, each
    /// holding the given bytes. Mirrors the real container layout (a `reports/`
    /// instance plus taxonomy siblings) without shipping a real filing.
    fn build_package(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts =
                SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            for (name, body) in entries {
                zip.start_file(*name, opts).expect("start entry");
                zip.write_all(body).expect("write entry");
            }
            zip.finish().expect("finish zip");
        }
        buf
    }

    const INSTANCE: &[u8] = b"<html><ix:nonFraction/></html>";

    #[test]
    fn recognizes_a_package_by_extension_and_magic() {
        let pkg = build_package(&[("CBF/reports/inst.xhtml", INSTANCE)]);
        assert!(is_report_package("something.xbri", &[]));
        assert!(is_report_package("something.zip", &[]));
        // Extension lies (octet-stream, no hint) → the ZIP magic still catches it.
        assert!(is_report_package("something.bin", &pkg));
        // A bare xhtml / pdf is not a package.
        assert!(!is_report_package("report.xhtml", b"<html></html>"));
        assert!(!is_report_package("report.pdf", b"%PDF-1.4"));
    }

    #[test]
    fn extracts_the_reports_instance_over_siblings() {
        // The taxonomy xsd is larger junk, and there is a stray xhtml outside
        // reports/ — the reports/ instance must still win.
        let pkg = build_package(&[
            ("CBF-2025/META-INF/reportPackage.json", b"{}"),
            ("CBF-2025/www/CBF.xsd", &vec![b'x'; 5000]),
            ("CBF-2025/decoy.xhtml", b"<html>decoy</html>"),
            ("CBF-2025/reports/CBF-2025-12-31-1-pl.xhtml", INSTANCE),
        ]);
        let instance = extract_instance(&pkg).expect("instance found");
        assert_eq!(instance, INSTANCE);
    }

    #[test]
    fn falls_back_to_largest_xhtml_without_reports_folder() {
        let pkg = build_package(&[
            ("bundle/small.xhtml", b"<html>s</html>"),
            ("bundle/big.xhtml", INSTANCE),
        ]);
        let instance = extract_instance(&pkg).expect("instance found");
        assert_eq!(instance, INSTANCE);
    }

    #[test]
    fn none_for_a_package_without_xhtml() {
        let pkg = build_package(&[("bundle/data.xml", b"<x/>"), ("bundle/notes.txt", b"hi")]);
        assert_eq!(extract_instance(&pkg), None);
    }

    #[test]
    fn none_for_non_zip_bytes() {
        assert_eq!(extract_instance(b"%PDF-1.4 not a zip"), None);
    }
}
