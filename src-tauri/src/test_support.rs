//! Shared, test-only helpers. Compiled only under `cfg(test)`, never into the
//! shipped binary (mirrors [`crate::transform_invariants`]).

use std::sync::{Mutex, Once, OnceLock};

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
