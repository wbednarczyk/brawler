use super::*;

#[test]
fn determine_extension_from_content_type() {
    assert_eq!(
        determine_extension(&Some("application/pdf".to_owned()), "http://example.com"),
        "pdf"
    );
    assert_eq!(
        determine_extension(&Some("text/html".to_owned()), "http://example.com"),
        "html"
    );
    // Structured ESEF/iXBRL statements (ADR 0061 decision 1b).
    assert_eq!(
        determine_extension(
            &Some("application/xhtml+xml".to_owned()),
            "http://example.com"
        ),
        "xhtml"
    );
}

#[test]
fn determine_extension_from_url() {
    assert_eq!(
        determine_extension(&None, "http://example.com/document.pdf"),
        "pdf"
    );
    assert_eq!(
        determine_extension(&None, "http://example.com/doc?v=1"),
        "doc"
    );
}

#[test]
fn determine_extension_default() {
    assert_eq!(
        determine_extension(&None, "http://example.com/document"),
        "bin"
    );
}

/// Epic #229 T2: the fetch path records what the bytes REALLY are. The
/// maintainer's corpus stores 38 XML documents under a `.pdf` name served as
/// `application/pdf` — name and header agree with each other and both lie.
/// The stored `content_type` stays verbatim (it is the audit value); the new
/// `detected_container` carries the truth.
#[test]
fn store_time_sniff_records_the_real_container_not_the_lying_name() {
    use crate::document_fetcher::FakeDocumentFetcher;
    use crate::storage::{AppState, CaptureReportDocumentInput};

    let dir = std::env::temp_dir().join(format!(
        "brawler-capture-sniff-{}-{}",
        std::process::id(),
        "xhtml-under-pdf"
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
    let state = AppState::with_data_dir(connection, dir);

    // The real pdf2htmlEX shape from the corpus: XHTML bytes, `.pdf` URL,
    // `application/pdf` header.
    let fetcher = FakeDocumentFetcher::new_success(
        b"<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<!-- Created by pdf2htmlEX -->\n<html></html>"
            .to_vec(),
        Some("application/pdf".to_owned()),
    );
    let captured = capture_report_document(
        &state,
        &fetcher,
        CaptureReportDocumentInput {
            company_id: "c1".to_owned(),
            source_type: "espi_attachment".to_owned(),
            url: "https://x/raport-okresowy.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Raport okresowy".to_owned()),
            attribution: None,
        },
    )
    .expect("capture");
    assert!(captured.success);

    let document = state
        .get_report_document(&captured.document_id)
        .expect("document");
    assert_eq!(
        document.detected_container,
        Some("xml".to_owned()),
        "the fetch path must record the magic-byte container, not the .pdf name"
    );
    assert_eq!(
        document.content_type,
        Some("application/pdf".to_owned()),
        "the server's content type stays verbatim — it is the audit value"
    );
    assert_eq!(
        crate::report_documents_container::resolved_source_format(&document),
        Some(crate::report_diff::extraction::SourceFormat::Xhtml),
        "container truth must beat the extension for every downstream consumer"
    );
}

// -----------------------------------------------------------------
// #455: capture never downgrades a fetched document; verified identity
// decides heal-vs-refetch, never "bytes on disk win" unchecked.
// -----------------------------------------------------------------

fn fresh_capture_state(suffix: &str) -> (crate::storage::AppState, String) {
    use crate::storage::AppState;

    let dir = std::env::temp_dir().join(format!(
        "brawler-capture-455-{}-{suffix}",
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
    (AppState::with_data_dir(connection, dir), "c1".to_owned())
}

fn capture_input(company_id: &str, url: &str) -> crate::storage::CaptureReportDocumentInput {
    crate::storage::CaptureReportDocumentInput {
        company_id: company_id.to_owned(),
        source_type: "user_url".to_owned(),
        url: url.to_owned(),
        period_id: None,
        origin_ref: None,
        title: None,
        attribution: None,
    }
}

#[test]
fn capture_returns_the_fetched_row_without_refetching() {
    use crate::document_fetcher::FakeDocumentFetcher;

    let (state, company_id) = fresh_capture_state("no-refetch");
    let first_fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 real bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let first = capture_report_document(
        &state,
        &first_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("first capture");
    assert!(first.success);

    let second_fetcher = FakeDocumentFetcher::new_error(
        crate::document_fetcher::DocumentFetcherError::InvalidUrl("must not be called".into()),
    );
    let second = capture_report_document(
        &state,
        &second_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("second capture");
    assert!(
        second.success,
        "an already-fetched matching file must report success"
    );
    assert_eq!(second.document_id, first.document_id);
    assert_eq!(second.local_path, first.local_path);
    assert_eq!(
        second_fetcher.calls.get(),
        0,
        "a matching identity must never trigger a network fetch"
    );
}

#[test]
fn capture_heals_a_failed_row_whose_file_matches_its_hash() {
    use crate::document_fetcher::FakeDocumentFetcher;

    let (state, company_id) = fresh_capture_state("heal-failed");
    let first_fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 real bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let first = capture_report_document(
        &state,
        &first_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("first capture");
    assert!(first.success);

    // Simulate the pre-#455 bug: an unconditional mark_failed downgraded
    // an already-fetched row while its file/hash stayed on disk.
    {
        let raw = state.checkout_for_tests().expect("raw connection");
        raw.execute(
            "UPDATE report_documents SET fetch_status = 'failed', fetch_error = 'stale error' WHERE id = ?1",
            rusqlite::params![first.document_id],
        )
        .expect("seed the downgraded state");
    }

    let second_fetcher = FakeDocumentFetcher::new_error(
        crate::document_fetcher::DocumentFetcherError::InvalidUrl("must not be called".into()),
    );
    let second = capture_report_document(
        &state,
        &second_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("healing capture");
    assert!(
        second.success,
        "a matching-hash failed row must heal to fetched"
    );
    assert_eq!(
        second_fetcher.calls.get(),
        0,
        "healing a verified-identity row must never trigger a network fetch"
    );

    let healed = state
        .get_report_document(&first.document_id)
        .expect("document");
    assert_eq!(healed.fetch_status, "fetched");
    assert!(
        healed.fetch_error.is_none(),
        "the stale error must be cleared on heal"
    );
}

#[test]
fn capture_refetches_when_the_file_is_missing() {
    use crate::document_fetcher::FakeDocumentFetcher;

    let (state, company_id) = fresh_capture_state("missing-file");
    let first_fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 real bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let first = capture_report_document(
        &state,
        &first_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("first capture");
    assert!(first.success);
    let full_path = state.data_dir().join(first.local_path.as_ref().unwrap());
    std::fs::remove_file(&full_path).expect("delete the stored file");

    let second_fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 refetched bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let second = capture_report_document(
        &state,
        &second_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("refetch capture");
    assert!(second.success);
    assert_eq!(
        second_fetcher.calls.get(),
        1,
        "a missing file must trigger exactly one refetch"
    );

    let refetched = state
        .get_report_document(&first.document_id)
        .expect("document");
    assert_eq!(refetched.fetch_status, "fetched");
    assert!(std::fs::exists(state.data_dir().join(refetched.local_path.unwrap())).unwrap_or(false));
}

#[test]
fn capture_refetches_when_the_file_hash_mismatches() {
    use crate::document_fetcher::FakeDocumentFetcher;

    let (state, company_id) = fresh_capture_state("hash-mismatch");
    let first_fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 real bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let first = capture_report_document(
        &state,
        &first_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("first capture");
    assert!(first.success);
    let full_path = state.data_dir().join(first.local_path.as_ref().unwrap());
    std::fs::write(&full_path, b"truncated garbage").expect("corrupt the stored file");

    let second_fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 refetched bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let second = capture_report_document(
        &state,
        &second_fetcher,
        capture_input(&company_id, "https://x/report.pdf"),
    )
    .expect("refetch capture");
    assert!(second.success);
    assert_eq!(
        second_fetcher.calls.get(),
        1,
        "a hash mismatch must trigger exactly one refetch"
    );

    let refetched_bytes =
        std::fs::read(state.data_dir().join(second.local_path.unwrap())).expect("bytes");
    assert_eq!(refetched_bytes, b"%PDF-1.7 refetched bytes");
}

#[test]
fn an_interrupted_publish_leaves_no_partial_final_file() {
    use crate::document_fetcher::FakeDocumentFetcher;
    use crate::storage::CaptureReportDocumentInput;

    let (state, company_id) = fresh_capture_state("interrupted-publish");
    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://x/report.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("pending document");

    // Pre-create the FINAL path as a directory so the no-clobber publish
    // step fails after the `.part` write succeeds — proving the final
    // path is only ever touched by a successful publish, never a direct
    // partial write, and that a genuine obstruction (not a concurrent
    // capture's file) surfaces as an error rather than being swallowed.
    let final_path = state
        .data_dir()
        .join(format!("report_documents/{}.pdf", doc.id));
    std::fs::create_dir_all(&final_path).expect("pre-create final path as a directory");

    let fetcher = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 real bytes".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let fetched = fetcher.fetch(&doc.url).expect("fake fetch");
    let result = store_fetched_document(&state, &doc.id, &doc.url, &fetched);
    assert!(result.is_err(), "publishing onto a directory must fail");

    let doc_dir = state.data_dir().join("report_documents");
    let part_files: Vec<_> = std::fs::read_dir(&doc_dir)
        .expect("read the report_documents dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&format!("{}.pdf.", doc.id)) && name.ends_with(".part")
        })
        .collect();
    assert_eq!(
        part_files.len(),
        1,
        "the .part file must remain after a failed publish"
    );
    assert!(
        final_path.is_dir(),
        "the final path must never become a partial file — it stays whatever it was before the failed publish"
    );
}

/// #487 P1 (adversarial review on the atomicity-wave PR): two concurrent
/// captures of the SAME document used to share one `.part` name, so the
/// slower writer could corrupt the faster one's bytes before either
/// published, and the unconditional row update let the last DB write win
/// even when its bytes never made it to disk. The fix: a unique temp
/// name per attempt, a no-clobber (`hard_link`) publish so only the
/// first attempt's bytes ever land at the final path, and a guarded row
/// update that never overwrites an already-published different identity.
#[test]
fn two_captures_of_one_document_publish_one_consistent_file_and_row() {
    use crate::document_fetcher::FakeDocumentFetcher;
    use crate::storage::CaptureReportDocumentInput;

    let (state, company_id) = fresh_capture_state("two-captures-race");
    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://x/report.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("pending document");

    let fetcher_a = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 bytes from capture A, longer than B".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let fetcher_b = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 bytes from capture B".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let fetched_a = fetcher_a.fetch(&doc.url).expect("fake fetch A");

    // Run capture A up to its publish step; while paused there, run
    // capture B to full completion (write, fsync, publish, DB write) —
    // the exact interleaving that used to let A's slower write clobber
    // B's already-committed bytes.
    let result = store_fetched_document_with(
        &state,
        &doc.id,
        &doc.url,
        &fetched_a,
        || {
            let fetched_b = fetcher_b.fetch(&doc.url).expect("fake fetch B");
            store_fetched_document(&state, &doc.id, &doc.url, &fetched_b)
                .expect("capture B must publish successfully");
        },
        || {},
    );
    assert!(
        result.is_ok(),
        "the losing attempt must not error, only report the winning identity: {result:?}"
    );

    let document = state.get_report_document(&doc.id).expect("document");
    assert_eq!(document.fetch_status, "fetched");

    let full_path = state.data_dir().join(document.local_path.as_ref().unwrap());
    let on_disk_bytes = std::fs::read(&full_path).expect("exactly one final file must exist");
    assert_eq!(
        content_hash_hex(&on_disk_bytes),
        document
            .content_hash
            .clone()
            .expect("row must carry a hash"),
        "the row's hash must equal the sha256 of the file actually on disk"
    );

    let doc_dir = state.data_dir().join("report_documents");
    let entries: Vec<_> = std::fs::read_dir(&doc_dir)
        .expect("read the report_documents dir")
        .filter_map(|entry| entry.ok())
        .collect();
    let leftover_parts = entries
        .iter()
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
        .count();
    assert_eq!(leftover_parts, 0, "no .part file must be left behind");
    let final_files = entries
        .iter()
        .filter(|entry| entry.path().is_file())
        .count();
    assert_eq!(final_files, 1, "exactly one final file must exist");
}

/// #487 P1 r3 (second adversarial review): the file+row race in
/// `two_captures_of_one_document_publish_one_consistent_file_and_row` isn't
/// the only hole. A can hard-link its bytes then pause before its row
/// write; a second caller starting in that window used to see the file
/// already published but the row still `pending`, treat the file as stale
/// residue of a crashed attempt, delete it, and publish its own — leaving
/// A to then record ITS hash against B's file (or lose the guarded row
/// write and report success with the wrong pairing). Compare-and-set on the
/// row cannot see this: it only arbitrates a write race, not a caller that
/// starts after the file exists but before the row says so. The
/// per-document lock closes it by serializing the whole capture: a second
/// caller for the same document must wait for the first to finish (file AND
/// row), not observe it mid-flight.
#[test]
fn a_second_capture_waits_for_the_first_and_returns_its_published_row() {
    use crate::document_fetcher::FakeDocumentFetcher;
    use std::thread;
    use std::time::Duration;

    let (state, company_id) = fresh_capture_state("lock-waits");
    let url = "https://x/report.pdf";
    let doc = state
        .create_or_find_pending_report_document(capture_input(&company_id, url))
        .expect("pending document");

    let fetcher_a = FakeDocumentFetcher::new_success(
        b"%PDF-1.7 bytes from capture A".to_vec(),
        Some("application/pdf".to_owned()),
    );
    let fetched_a = fetcher_a.fetch(&doc.url).expect("fake fetch A");

    // Hold this document's lock exactly as `capture_report_document` would
    // for its entire body, so capture B below — using the real public API —
    // has to wait on it instead of racing ahead while the row is still
    // `pending`.
    let doc_lock = doc_lock::lock_document(&doc.id);
    let _guard = doc_lock.lock().unwrap_or_else(|p| p.into_inner());

    let mut b_handle: Option<thread::JoinHandle<(StorageResult<DocumentCaptureResult>, usize)>> =
        None;
    let store_result = store_fetched_document_with(
        &state,
        &doc.id,
        &doc.url,
        &fetched_a,
        || {},
        || {
            // A's bytes are the ones at the final path now; A's row write
            // has NOT happened yet — the row is still `pending`. This is
            // exactly the window #487 P1 r3 named.
            let state_b = state.clone();
            let company_id_b = company_id.clone();
            let fetcher_b = FakeDocumentFetcher::new_success(
                b"must never be reached: A already holds the document lock".to_vec(),
                Some("application/pdf".to_owned()),
            );
            let handle = thread::spawn(move || {
                let result = capture_report_document(
                    &state_b,
                    &fetcher_b,
                    capture_input(&company_id_b, url),
                );
                (result, fetcher_b.calls.get())
            });
            thread::sleep(Duration::from_millis(200));
            assert!(
                !handle.is_finished(),
                "capture B must block while capture A still holds the document lock"
            );
            b_handle = Some(handle);
        },
    );
    let local_path = store_result.expect("capture A's store must succeed");

    // Release A's lock — only now may B make progress.
    drop(_guard);

    let (b_result, b_fetches) = b_handle
        .expect("capture B must have been spawned")
        .join()
        .expect("capture B thread must not panic");
    let b_result = b_result.expect("capture B must succeed");
    assert_eq!(
        b_fetches, 0,
        "B re-reads the row under the lock and never fetches after A published (astra r4)"
    );

    assert!(b_result.success);
    assert_eq!(
        b_result.local_path.as_deref(),
        Some(local_path.as_str()),
        "B must return A's published row, not race ahead of it"
    );

    let document = state.get_report_document(&doc.id).expect("document");
    assert_eq!(document.fetch_status, "fetched");
    let full_path = state.data_dir().join(document.local_path.as_ref().unwrap());
    let on_disk_bytes = std::fs::read(&full_path).expect("exactly one final file must exist");
    assert_eq!(
        content_hash_hex(&on_disk_bytes),
        document
            .content_hash
            .clone()
            .expect("row must carry a hash"),
        "the row's hash must equal the sha256 of the single final file"
    );

    let doc_dir = state.data_dir().join("report_documents");
    let leftover_parts = std::fs::read_dir(&doc_dir)
        .expect("read the report_documents dir")
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
        .count();
    assert_eq!(leftover_parts, 0, "no .part file must remain");
}

#[cfg(test)]
mod properties {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        /// content_hash_hex: totality + shape (64 lowercase hex chars) +
        /// determinism + collision sensitivity (a single flipped byte must
        /// change the hash). A mutant that truncates the hex output, or
        /// drops the update() call, fails the shape/collision checks.
        #[test]
        fn content_hash_hex_is_a_deterministic_64_char_lowercase_hex_digest(
            bytes in prop::collection::vec(any::<u8>(), 0..256),
            flip_idx in 0usize..256,
        ) {
            let hash = content_hash_hex(&bytes);
            prop_assert_eq!(hash.len(), 64);
            prop_assert!(hash.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
            prop_assert_eq!(content_hash_hex(&bytes), hash.clone());

            if !bytes.is_empty() {
                let mut flipped = bytes.clone();
                let idx = flip_idx % flipped.len();
                flipped[idx] ^= 0xFF;
                prop_assert_ne!(content_hash_hex(&flipped), hash);
            }
        }

        /// determine_extension: totality over arbitrary content-type/url
        /// text, lowercase output, bounded length.
        #[test]
        fn determine_extension_never_panics_lowercase_and_bounded(
            content_type in prop::option::of(".{0,40}"),
            url in ".{0,60}",
        ) {
            let ext = determine_extension(&content_type, &url);
            // The contract an on-disk filename relies on: "bin", or 1..=9
            // ASCII lowercase alphanumerics — never a NUL, a symbol, or a
            // non-ASCII char whose lowercase form is longer than its input.
            prop_assert!(
                ext == "bin" || (ext.len() <= 9 && ext.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
                "extension outside the filename contract: {ext:?}"
            );
        }
    }

    /// Counter-example the 2000-case property run found (#194): a URL whose
    /// "extension" holds a NUL and a non-ASCII char that grows under
    /// lowercasing passed the byte bound and reached the filename.
    #[test]
    fn determine_extension_rejects_non_ascii_or_control_extensions() {
        assert_eq!(determine_extension(&None, ".a0\0\u{10000}\u{023A}"), "bin");
        assert_eq!(determine_extension(&None, "https://x/report.PDF"), "pdf");
        assert_eq!(determine_extension(&None, "https://x/report.p-d"), "bin");
    }

    /// Meaning check (table-driven, not random): every documented
    /// content-type -> extension mapping, a case-varied URL filename
    /// extension, and the "nothing matched" default. A mutant that always
    /// returns "bin", or drops the URL-lowercasing, fails one of these.
    #[test]
    fn determine_extension_matches_the_documented_table() {
        let cases: &[(&str, &str)] = &[
            ("application/pdf", "pdf"),
            ("text/html", "html"),
            ("application/xhtml+xml", "xhtml"),
            ("text/plain", "txt"),
            ("application/msword", "doc"),
            (
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                "docx",
            ),
            ("application/vnd.ms-excel", "xls"),
            (
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "xlsx",
            ),
        ];
        for (content_type, expected) in cases {
            assert_eq!(
                determine_extension(&Some(content_type.to_string()), "http://example.com/x"),
                *expected,
                "content type {content_type} must map to {expected}"
            );
        }

        // Uppercase URL filename extension, no content type: case-folded.
        assert_eq!(
            determine_extension(&None, "https://example.com/report.PDF"),
            "pdf"
        );

        // No content type, no usable URL segment: the documented default.
        assert_eq!(determine_extension(&None, "https://example.com/"), "bin");
    }
}
