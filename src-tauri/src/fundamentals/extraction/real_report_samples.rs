//! Real committed ESPI filing sample tests (ADR 0094, epic #40 / #139).
//!
//! Every real-format extraction gate before this slice ran only against inline
//! string literals or the owner's private, env-gated corpus — a parser
//! regression on a REAL container (a real ESEF `.xbri` package, a real PDF)
//! could never redden a PR. The three files committed under `samples/reports/`
//! (attribution: `samples/reports/MANIFEST.json`, policy: ADR 0094) close that
//! gap: they are read HERE, at runtime, via `CARGO_MANIFEST_DIR` — never
//! `include_bytes!`/`include_str!` — so the binary and every non-extraction
//! test run never pay their size.
//!
//! Covers:
//! - the manifest guard (attribution/hash/size/container/budget/completeness);
//! - the ESEF tier end to end (container -> package unzip -> iXBRL parse)
//!   against a cross-source-verified pinned expected-values file;
//! - the PDF text-extraction path on a real text-layer report — proves
//!   deterministic TEXT extraction only, never facts (ADR 0086);
//! - the honest no-text-layer path, on both the report_diff text extractor and
//!   the fundamentals routing seam (a Pdf container is a documented
//!   benign-empty route, never a fabricated fact).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::container::{detect_container, Container};
use super::esef::parse_esef;
use super::esef_package::extract_instance;
use super::{ExtractedFact, FactPeriod};

/// A deterministic, JSON-serializable projection of one [`ExtractedFact`] --
/// every field the struct carries (`metric_key`, `period_end`/`period_start`,
/// `basis`, `currency`, `value`, `tier`, `citation`), so a drift in ANY of
/// them (not just the headline value) reddens the pinned comparison. `value`
/// is the `Decimal`'s canonical string form and `tier` is `SourceTier::as_str()`
/// so the pinned file is plain, diffable JSON with no float-precision
/// ambiguity and no dependency on either type's own `Serialize`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
struct FactProjection {
    metric_key: String,
    period_end: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    period_start: Option<String>,
    basis: Option<String>,
    currency: Option<String>,
    value: String,
    tier: String,
    citation: String,
}

/// `src-tauri/samples/reports/` — real, complete, unmodified official ESPI/EBI
/// filings (README.md there; attribution in MANIFEST.json).
fn samples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/reports")
}

/// Reads one committed sample's bytes at runtime. Never `include_bytes!` — the
/// largest sample is ~4 MB and must never inflate the compiled test binary or
/// be paid by unrelated test runs.
fn read_sample(file: &str) -> Vec<u8> {
    let path = samples_dir().join(file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read committed sample {file}: {e}"))
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

// ============================================================================
// Manifest guard (ADR 0094 decision 3)
// ============================================================================

/// Hard ceiling on the committed sample budget (ADR 0094 decision 2: "≤ 5 MB
/// forever"). Independent of whatever `MANIFEST.json` claims for
/// `budget_bytes_max` — the manifest can only TIGHTEN this ceiling, never
/// raise it, so an editor cannot silently blow the budget by bumping the
/// number in the JSON.
const HARD_BUDGET_BYTES_MAX: u64 = 5 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    #[serde(rename = "$schema_note")]
    schema_note: String,
    budget_bytes_max: u64,
    files: Vec<ManifestFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManifestFile {
    file: String,
    issuer: String,
    report: String,
    // Modeled (not read) so `deny_unknown_fields` accepts it -- attribution,
    // not a value this test suite currently asserts on.
    #[allow(dead_code)]
    period_end: Option<String>,
    original_filename: String,
    source_url: String,
    distributor: String,
    retrieved: String,
    bytes: u64,
    sha256: String,
    expected_container: String,
    /// A string the sample's `report_diff`-extracted text must contain (ADR
    /// 0094 guardrail harvested after the first committed PDF candidate turned
    /// out to carry a DIFFERENT issuer's auditor letter under a matching hash
    /// but a mismatched attribution — see `manifest_text_markers_bind_samples_to_their_attribution`).
    /// Absent/`null` for samples where content binding isn't checked this way
    /// (the ESEF sample is bound by its exact fact-value pin instead; a
    /// no-text-layer sample has no extractable text to bind).
    #[serde(default)]
    expected_text_marker: Option<String>,
    purpose: String,
}

fn load_manifest() -> Manifest {
    let text = std::fs::read_to_string(samples_dir().join("MANIFEST.json"))
        .expect("read samples/reports/MANIFEST.json");
    serde_json::from_str(&text).expect("parse samples/reports/MANIFEST.json")
}

/// Maps a MANIFEST `expected_container` marker to the `Container` variant
/// `detect_container` actually returns for that shape. `zip_report_package` is
/// deliberately not a distinct `Container` variant — an ESEF/eSprawozdanie
/// report package IS a ZIP at the container-sniff layer (`esef_package` reads
/// the ZIP structure one layer up); the marker documents WHY the file is a ZIP.
fn expected_container(marker: &str) -> Container {
    match marker {
        "zip_report_package" => Container::Zip,
        "pdf" => Container::Pdf,
        other => panic!("MANIFEST.json: unknown expected_container marker {other:?}"),
    }
}

/// ADR 0094 decision 3: total <= budget, every file listed, hash/size/container
/// match, attribution fields present and sane, no unmanifested report binary
/// under `samples/reports/` (recursively, symlink-safe).
#[test]
fn manifest_matches_every_committed_sample_exactly() {
    let manifest = load_manifest();
    assert!(
        !manifest.schema_note.trim().is_empty(),
        "MANIFEST.json's $schema_note must be non-empty"
    );
    assert!(
        !manifest.files.is_empty(),
        "MANIFEST.json must list at least one sample"
    );

    // The manifest may only TIGHTEN the hard ADR 0094 decision 2 ceiling, never
    // raise it -- checked before the declared value is used for anything below.
    assert!(
        manifest.budget_bytes_max <= HARD_BUDGET_BYTES_MAX,
        "MANIFEST.json budget_bytes_max {} exceeds the hard ADR 0094 decision 2 \
         ceiling of {HARD_BUDGET_BYTES_MAX} bytes (5 MB, forever) -- the manifest \
         cannot raise this budget, only tighten it",
        manifest.budget_bytes_max
    );

    let mut total_bytes: u64 = 0;
    let mut manifested_names: BTreeSet<String> = BTreeSet::new();

    for entry in &manifest.files {
        let bytes = std::fs::read(samples_dir().join(&entry.file))
            .unwrap_or_else(|e| panic!("manifested sample {} is missing on disk: {e}", entry.file));

        assert_eq!(
            bytes.len() as u64,
            entry.bytes,
            "{}: on-disk byte size drifted from MANIFEST.json",
            entry.file
        );
        assert_eq!(
            hex_sha256(&bytes),
            entry.sha256,
            "{}: sha256 drifted from MANIFEST.json -- samples are append-only \
             (ADR 0094); a hash change means the committed file was edited",
            entry.file
        );
        assert_eq!(
            detect_container(&bytes),
            expected_container(&entry.expected_container),
            "{}: detect_container no longer matches MANIFEST.json's expected_container",
            entry.file
        );

        // Attribution fields (ADR 0094 decision 3) must be genuinely present,
        // not empty placeholders, and the source must be traceable (https).
        for (field_name, value) in [
            ("issuer", &entry.issuer),
            ("report", &entry.report),
            ("original_filename", &entry.original_filename),
            ("source_url", &entry.source_url),
            ("distributor", &entry.distributor),
            ("retrieved", &entry.retrieved),
            ("purpose", &entry.purpose),
        ] {
            assert!(
                !value.trim().is_empty(),
                "{}: attribution field {field_name:?} must be non-empty",
                entry.file
            );
        }
        assert!(
            entry.source_url.starts_with("https://"),
            "{}: source_url must start with https:// (got {:?})",
            entry.file,
            entry.source_url
        );

        total_bytes += entry.bytes;
        assert!(
            manifested_names.insert(entry.file.clone()),
            "{}: listed more than once in MANIFEST.json",
            entry.file
        );
    }

    assert!(
        total_bytes <= manifest.budget_bytes_max,
        "committed sample total {total_bytes} bytes exceeds budget_bytes_max {} (ADR 0094 decision 2)",
        manifest.budget_bytes_max
    );

    // No unmanifested report binary under samples/reports/ (and nothing
    // manifested that is missing) -- checked both directions, recursively and
    // symlink-safe: samples/reports/ is contractually flat, real files only.
    let (on_disk, violations) = scan_samples_dir_recursive(&samples_dir());
    assert!(
        violations.is_empty(),
        "samples/reports/ must be a flat directory of real files -- no symlinks, \
         no subdirectories: {violations:?}"
    );
    assert_eq!(
        on_disk, manifested_names,
        "every file under samples/reports/ (except MANIFEST.json/README.md) must be \
         listed in MANIFEST.json, and MANIFEST.json must list nothing else"
    );
}

/// Recursively walks `dir`, returning (regular files present, relative to
/// `dir`, excluding the top-level `MANIFEST.json`/`README.md`) and a list of
/// contract violations: any symlink (file or dir, at any depth) or any
/// subdirectory (`samples/reports/` is flat by contract) is a violation.
/// Subdirectories are still descended into (so nested violations are reported
/// too), never followed through a symlink -- `DirEntry::file_type` reports the
/// entry's own type without following it, so a symlink is never mistaken for
/// what it points to.
fn scan_samples_dir_recursive(dir: &Path) -> (BTreeSet<String>, Vec<String>) {
    let mut on_disk = BTreeSet::new();
    let mut violations = Vec::new();
    scan_dir(dir, dir, &mut on_disk, &mut violations);
    (on_disk, violations)
}

fn scan_dir(
    current: &Path,
    base: &Path,
    on_disk: &mut BTreeSet<String>,
    violations: &mut Vec<String>,
) {
    let read = std::fs::read_dir(current)
        .unwrap_or_else(|e| panic!("read dir {}: {e}", current.display()));
    for entry in read {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let rel = path
            .strip_prefix(base)
            .expect("entry under base")
            .to_string_lossy()
            .into_owned();
        let file_type = entry.file_type().expect("file type");

        if file_type.is_symlink() {
            violations.push(format!("symlink not allowed under samples/reports/: {rel}"));
            continue;
        }
        if file_type.is_dir() {
            violations.push(format!(
                "subdirectory not allowed under samples/reports/ (flat by contract): {rel}"
            ));
            scan_dir(&path, base, on_disk, violations);
            continue;
        }

        // A regular file. Only the top-level attribution docs are exempt.
        if current == base && (rel == "MANIFEST.json" || rel == "README.md") {
            continue;
        }
        on_disk.insert(rel);
    }
}

/// Guardrail harvested from a real defect found while implementing this slice
/// (epic #40/#139 A3): the first committed PDF candidate (`vercom_2024_q2_ssf.pdf`)
/// had a sha256 that matched MANIFEST.json exactly, yet its extracted text was
/// another issuer's auditor letter -- the manifest guard above proves the bytes
/// are UNMODIFIED, never that they are the RIGHT bytes. This test closes that
/// class: for every MANIFEST entry that declares a non-null
/// `expected_text_marker`, the sample's `report_diff`-extracted text MUST
/// contain it. Data-driven from MANIFEST.json, so a future sample that sets the
/// field is automatically enforced with no test-code change.
#[test]
fn manifest_text_markers_bind_samples_to_their_attribution() {
    use crate::report_diff::extraction::{extract_report, SourceFormat};

    let manifest = load_manifest();
    let mut checked = 0usize;
    for entry in &manifest.files {
        let Some(marker) = &entry.expected_text_marker else {
            continue;
        };
        let format = match entry.expected_container.as_str() {
            "pdf" => SourceFormat::Pdf,
            other => panic!(
                "{}: expected_text_marker is set but there is no report_diff \
                 SourceFormat mapping for expected_container {other:?} -- extend \
                 this test before relying on the marker for that container",
                entry.file
            ),
        };
        let bytes = std::fs::read(samples_dir().join(&entry.file))
            .unwrap_or_else(|e| panic!("manifested sample {} is missing on disk: {e}", entry.file));
        let outcome = extract_report(&bytes, format);
        let full_text: String = outcome
            .sections
            .iter()
            .map(|s| format!("{} {}\n", s.heading, s.body))
            .collect();
        // Case-insensitive: distributor filenames are often ALL-CAPS (the
        // source of the marker) while the document body uses normal title
        // case ("Sfinks Polska", not "SFINKS") -- folding case keeps the check
        // testing content identity, not marker-authoring casing.
        assert!(
            full_text.to_lowercase().contains(&marker.to_lowercase()),
            "{}: extracted text does not contain expected_text_marker {marker:?} \
             (case-insensitive) -- the committed bytes may not match their \
             MANIFEST.json attribution (this is exactly the class of bug that \
             motivated this guardrail)",
            entry.file
        );
        checked += 1;
    }
    assert!(
        checked > 0,
        "no MANIFEST entry declared an expected_text_marker -- this guardrail is \
         currently untested; add a marker to at least one sample, or this assert \
         itself should be revisited"
    );
}

// ============================================================================
// ESEF sample: container -> package unzip -> iXBRL parse, pinned exactly
// ============================================================================

fn project_facts(facts: &[ExtractedFact]) -> Vec<FactProjection> {
    let mut rows: Vec<FactProjection> = facts
        .iter()
        .map(|f| {
            let (period_end, period_start) = match &f.period {
                FactPeriod::Instant(end) => (end.clone(), None),
                FactPeriod::Duration { start, end } => (end.clone(), Some(start.clone())),
            };
            FactProjection {
                metric_key: f.metric_key.clone(),
                period_end,
                period_start,
                basis: f.basis.map(|b| b.as_str().to_owned()),
                currency: f.currency.clone(),
                value: f.value.to_string(),
                tier: f.tier.as_str().to_owned(),
                citation: f.citation.clone(),
            }
        })
        .collect();
    // Sort by EVERY projected field (derived `Ord`, declaration order) so the
    // pinned comparison's ordering can never mask a change in a non-leading
    // field for two otherwise-identical rows.
    rows.sort();
    rows
}

fn expected_cbf_projection() -> Vec<FactProjection> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/fundamentals/extraction/real_report_expected_cbf_2025.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("read real_report_expected_cbf_2025.json: {e}"));
    serde_json::from_str(&text).expect("parse real_report_expected_cbf_2025.json")
}

/// cyber_Folks FY2025 ESEF report package, end to end: container sniff ->
/// unzip the instance -> parse the iXBRL facts -> compare EXACTLY against the
/// cross-source-verified pinned values (ADR 0094 decision 4 — corroborated
/// against the production-validated facts for CBF FY2025, headline values
/// match to the złoty; never owner-read from the PDF). Fails with a readable
/// struct diff (assert_eq! on `Vec<FactProjection>`) if any value, key, period,
/// basis, or currency changes.
#[test]
fn cbf_2025_esef_sample_matches_pinned_expected_values() {
    let bytes = read_sample("cbf_2025_annual_esef.xbri");
    assert_eq!(
        detect_container(&bytes),
        Container::Zip,
        "the CBF sample must sniff as a ZIP report package"
    );

    let instance =
        extract_instance(&bytes).expect("the CBF package holds a readable iXBRL instance");
    let facts = parse_esef(&instance).expect("the CBF instance parses to tracked iXBRL facts");
    let actual = project_facts(&facts);
    let expected = expected_cbf_projection();

    assert_eq!(
        actual, expected,
        "extracted CBF FY2025 ESEF facts drifted from the pinned expected values in \
         real_report_expected_cbf_2025.json. If this is an intentional parser change, \
         regenerate that file from the new parse output AND re-verify the new values by \
         cross-source corroboration (the production-validated facts / aggregator witness \
         for the same issuer+period) before committing (ADR 0094 decision 4)."
    );

    // Supplemental insta snapshot alongside the strict pin, purely for human
    // reviewability of a diff in a PR (the pinned-file assert_eq! above is the
    // actual gate).
    insta::assert_debug_snapshot!("cbf_2025_esef_facts_projection", actual);
}

// ============================================================================
// PDF sample: real text-layer report -- report_diff text extraction only.
// ============================================================================

/// `vercom_2024_q2_ssf.pdf` (the original candidate here) was DROPPED: its
/// sha256 matched its MANIFEST.json entry exactly, yet its extracted text was
/// another issuer's auditor letter, not Vercom's -- a content/attribution
/// mismatch the hash-only manifest guard cannot catch (see
/// `manifest_text_markers_bind_samples_to_their_attribution` above, the
/// guardrail harvested from that finding). Replaced with
/// `sfinks_2026_q1_selected_data.pdf` (Sfinks Polska QSr 1/2026 "WYBRANE DANE"
/// pages), whose MANIFEST entry carries `expected_text_marker: "SFINKS"` --
/// checked generically by the guardrail test, not duplicated here.
#[test]
fn sfinks_2026_q1_pdf_sample_extracts_text_deterministically() {
    use crate::report_diff::extraction::{extract_report, ExtractionState, SourceFormat};

    let bytes = read_sample("sfinks_2026_q1_selected_data.pdf");
    assert_eq!(
        detect_container(&bytes),
        Container::Pdf,
        "the sample must sniff as a real PDF"
    );

    let run_1 = extract_report(&bytes, SourceFormat::Pdf);
    let run_2 = extract_report(&bytes, SourceFormat::Pdf);

    assert_eq!(run_1.state, ExtractionState::Extracted);
    assert_eq!(
        run_1.char_count, run_2.char_count,
        "pdf-extract must be deterministic across runs on the same bytes"
    );
    assert_eq!(
        run_1.sections, run_2.sections,
        "pdf-extract must recover the identical sections (heading + body), not \
         just the identical char count, across runs on the same bytes"
    );
    assert!(
        !run_1.sections.is_empty(),
        "a real text-layer PDF must yield at least one section"
    );

    let full_text: String = run_1
        .sections
        .iter()
        .map(|s| format!("{} {}\n", s.heading, s.body))
        .collect();
    assert!(
        !full_text.trim().is_empty(),
        "extracted text must be non-empty"
    );
    assert!(
        full_text.to_lowercase().contains("sfinks"),
        "the issuer's own name must appear in its extracted report text; got a prefix: {:?}",
        full_text.chars().take(500).collect::<String>()
    );

    // Pinned, deterministic properties (verified above by running extraction
    // twice in-process, and confirmed stable across separate `cargo test`
    // invocations while authoring this test): the exact char count, and a
    // sha256 content digest of the normalized full text, that pdf-extract
    // recovers from this specific, immutable committed PDF. The digest catches
    // a same-length reflow/reordering that char_count alone would miss.
    assert_eq!(
        run_1.char_count, SFINKS_EXPECTED_CHAR_COUNT,
        "char_count drifted -- either the sample changed (must not, ADR 0094) or \
         pdf-extract's output changed (a dependency bump); re-verify before re-pinning"
    );
    assert_eq!(
        hex_sha256(full_text.as_bytes()),
        SFINKS_EXPECTED_TEXT_SHA256,
        "the extracted text's content digest drifted -- either the sample changed \
         (must not, ADR 0094) or pdf-extract's output changed (a dependency bump); \
         re-verify before re-pinning"
    );
}

/// Pinned deterministic properties for `sfinks_2026_q1_selected_data.pdf` (see
/// the test above) -- measured from the real extraction output, stable across
/// repeated runs.
const SFINKS_EXPECTED_CHAR_COUNT: usize = 4834;
const SFINKS_EXPECTED_TEXT_SHA256: &str =
    "53f40aee4bce55c5227b735695d8ac89a5667cf865f35ab986eff9d5a615388f";

// ============================================================================
// No-text-layer sample: honest failure on BOTH the report_diff text extractor
// and the fundamentals routing seam.
// ============================================================================

#[test]
fn editel_no_text_layer_pdf_is_honestly_flagged_on_both_paths() {
    use crate::report_diff::extraction::{extract_report, ExtractionState, SourceFormat};

    let bytes = read_sample("editel_annual_no_text_layer.pdf");
    assert_eq!(
        detect_container(&bytes),
        Container::Pdf,
        "the EDITEL sample must sniff as a real PDF (a scan is still a PDF container)"
    );

    // report_diff side: a scanned PDF has no text layer -- never fabricated
    // sections, never crashes.
    let outcome = extract_report(&bytes, SourceFormat::Pdf);
    assert_eq!(outcome.state, ExtractionState::NoTextLayer);
    assert!(outcome.sections.is_empty());

    // fundamentals side: a Pdf container routes to the PDF fact arm, retired
    // by ADR 0086 dec. 1 -- proven at the routing seam...
    assert_eq!(
        crate::jobs::structured_extraction::route_document(&bytes),
        crate::jobs::structured_extraction::DocumentRoute::Pdf
    );

    // ...and end to end through the real entry point: benign-empty, never an
    // error, never a fabricated fact.
    let (state, company_id, document_id) = seed_pdf_document(&bytes);
    let result = crate::jobs::structured_extraction::run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2025-12-31",
        crate::storage::MODE_AUTOPILOT,
    )
    .expect("the Pdf route returns Ok(benign-empty), never an Err");

    assert_eq!(
        result.acceptance,
        crate::fundamentals::extraction::pipeline::Acceptance::Empty
    );
    assert!(result.tier.is_none());
    assert!(result.produced_fact_ids.is_empty());
    assert!(!result.emitted);
}

/// Seeds a fetched report document holding exactly `bytes` -- mirrors the
/// pattern established in `jobs::structured_extraction`'s own tests
/// (`seed_document_with_bytes`), which is private to that module's test tree.
fn seed_pdf_document(bytes: &[u8]) -> (crate::app_state::AppState, String, String) {
    use crate::storage::{open_in_memory_database, CaptureReportDocumentInput, NewCompany};

    let dir = unique_temp_dir("editel");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("in-memory db");
    let state = crate::app_state::AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "NewConnect".to_owned(),
            ticker: "EDT".to_owned(),
            display_name: "EDITEL S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/editel_annual_no_text_layer.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Annual report (scanned, no text layer)".to_owned()),
            attribution: None,
        })
        .expect("document");
    let filename = "editel_annual_no_text_layer.pdf";
    std::fs::write(dir.join(filename), bytes).expect("write sample bytes");
    state
        .mark_report_document_fetched(
            &document.id,
            Some(filename),
            Some("application/pdf"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

/// Distinct per-call temp dir (mirrors `jobs::structured_extraction`'s test
/// helper) -- avoids two test binaries/threads racing on the same data dir.
fn unique_temp_dir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "brawler-real-report-samples-{}-{label}-{n}",
        std::process::id()
    ))
}
