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
