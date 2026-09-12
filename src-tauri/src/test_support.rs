//! Shared, test-only helpers. Compiled only under `cfg(test)`, never into the
//! shipped binary (mirrors [`crate::transform_invariants`]).

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, Once, OnceLock};

use crate::app_state::AppState;
use crate::storage::{open_in_memory_database, CaptureReportDocumentInput, NewCompany};

/// A minimal process-global capturing logger. The crate installs no logger in
/// unit tests, so the first `set_logger` wins; every record's message is kept in
/// one shared buffer. `nextest` runs each test in its own process, so the buffer
/// is effectively per-test; a single-process `cargo test` shares it, so
/// assertions must filter by a value unique to the case under test (e.g. a
/// `company_id` or `report_document_id`) rather than clearing the buffer.
static CAPTURED_LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();

/// The shared capture buffer. Every installed [`CaptureLogger`] appends here.
pub fn captured_logs() -> &'static Mutex<Vec<String>> {
    CAPTURED_LOGS.get_or_init(|| Mutex::new(Vec::new()))
}

struct CaptureLogger;

impl log::Log for CaptureLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if let Ok(mut buffer) = captured_logs().lock() {
            buffer.push(record.args().to_string());
        }
    }

    fn flush(&self) {}
}

/// Install the shared capturing logger once per process. Idempotent: repeated
/// calls (and calls from different test modules) are no-ops after the first.
pub fn install_capture_logger() {
    static LOGGER: CaptureLogger = CaptureLogger;
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // `set_logger` errors only if another logger is already installed; in
        // unit tests none is, so this wins. Warn level must be enabled or the
        // `log::warn!` macro short-circuits before reaching the logger.
        let _ = log::set_logger(&LOGGER);
        log::set_max_level(log::LevelFilter::Warn);
    });
}

/// Minimal presentation-linkbase XML classifying each `(concept, role URI
/// suffix)` pair (ADR 0100 decision 3, epic #398) — matches
/// `esef_package.rs`'s own test fixture shape (`classify_role` matches on the
/// role URI's trailing segment, e.g. `-210000`). A tagged fact with no role
/// never survives Layer 2 projection, so every ESEF test package needs one of
/// these alongside its instance.
pub fn esef_presentation_linkbase_xml(mappings: &[(&str, &str)]) -> String {
    let links: String = mappings
        .iter()
        .map(|(concept, role_suffix)| {
            format!(
                r#"  <link:presentationLink xlink:type="extended" xlink:role="http://x/role/{role_suffix}">
    <link:loc xlink:type="locator" xlink:href="ifrs-full-2023.xsd#ifrs-full_{concept}" xlink:label="loc_{concept}"/>
  </link:presentationLink>"#
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<link:linkbase xmlns:link="http://www.xbrl.org/2003/linkbase" xmlns:xlink="http://www.w3.org/1999/xlink">
{links}
</link:linkbase>"#
    )
}

/// Wraps one iXBRL instance plus its presentation-linkbase XML into a minimal
/// ZIP report package — the shape `esef_package::extract_all_instances` /
/// `extract_presentation_roles` read (ADR 0100, epic #398). `role_mappings`
/// feeds [`esef_presentation_linkbase_xml`].
pub fn esef_package_zip(instance_xml: &str, role_mappings: &[(&str, &str)]) -> Vec<u8> {
    use std::io::Write;
    let pre_xml = esef_presentation_linkbase_xml(role_mappings);
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("reports/instance.xhtml", opts)
            .expect("start instance entry");
        zip.write_all(instance_xml.as_bytes())
            .expect("write instance");
        zip.start_file("www/instance_pre.xml", opts)
            .expect("start pre.xml entry");
        zip.write_all(pre_xml.as_bytes()).expect("write pre.xml");
        zip.finish().expect("finish zip");
    }
    buf
}

/// The balance-sheet role suffix (`ias_1_role-210000`, ADR 0100 decision 3)
/// every plain "instant totals" ESEF test fixture tags its concepts under.
pub const BALANCE_SHEET_ROLE_SUFFIX: &str = "ias_1_role-210000";

/// A minimal ZIP archive holding arbitrary named entries — enough for
/// [`crate::jobs::structured_extraction::detect_container`] to see the
/// `PK\x03\x04` magic and for `esef_package::extract_instance`/
/// `extract_all_instances` to unpack it. Unlike [`esef_package_zip`] (a fixed
/// one-instance-plus-linkbase shape), this is a generic multi-entry builder —
/// e.g. for a package carrying BOTH a consolidated and a standalone instance.
pub fn minimal_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        for (name, body) in entries {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(body).unwrap();
        }
        writer.finish().unwrap();
    }
    buf
}

/// A per-call-unique scratch dir: `std::process::id()` alone collides across
/// parallel `#[test]` threads (and across loop iterations within one test)
/// sharing this file's data dir. A monotonic counter makes every call's dir
/// distinct.
pub fn unique_temp_dir(label: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("brawler-test-{}-{label}-{n}", std::process::id()))
}

/// Seeds one fetched document whose stored file is `filename` holding exactly
/// `bytes` — used to prove that extraction/routing decides on the BYTES, not
/// the filename or content-type.
pub fn seed_document_with_bytes(
    label: &str,
    ticker: &str,
    title: &str,
    filename: &str,
    bytes: &[u8],
) -> (AppState, String, String) {
    let dir = unique_temp_dir(label);
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: format!("https://example.com/{filename}"),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("document");
    std::fs::write(dir.join(filename), bytes).expect("write bytes");
    state
        .mark_report_document_fetched(
            &document.id,
            Some(filename),
            // The maintainer's real mislabeled files are all octet-stream.
            Some("application/octet-stream"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}
