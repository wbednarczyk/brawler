//! Restore the report documents migration `0107` wrongly deleted (epic #229 T3,
//! owner decision 2026-07-30).
//!
//! `0107_repair_misassociation_and_note_ref_facts.sql` deleted every
//! `espi_attachment` row matching `company_id = 'company_gpw_cbf' AND url LIKE
//! '%energa%'`, on the belief that a `Grupy-Energa` filename meant an Energa
//! filing OCR'd onto cyber_Folks. **The bytes say otherwise.** All four files
//! carry `/Author: cyber_Folks`, two carry `/Title: cyber_Folks_2024 Q3_SSF` and
//! `cyber_Folks 2024 Q3_JSF`, and the other two extract as "Raport kwartalny
//! Grupy cyber_Folks_ … za III kwartał 2024" (PL) and its English twin. The
//! attachment host simply reuses one issuer's filename across unrelated same-day
//! filings — the same `Grupy-Energa` slugs sit on Vercom's and Orlen's own
//! Q3-2024 statements, whose bytes likewise name their owners.
//!
//! The damage: cyber_Folks lost its **entire** 2024-Q3 periodic filing set, so
//! the period has no canonical report and nothing to extract. The files were
//! never touched — only the rows were deleted — so the repair is a re-insert.
//!
//! # Why a Rust startup pass and not a migration
//!
//! Only content can settle an association, and **SQL cannot read bytes**: a
//! migration could neither verify these files are cyber_Folks' nor check that
//! they are still on this machine, and would happily insert four rows pointing at
//! nothing. That asymmetry is exactly what
//! [`crate::storage::migrations`]'s `no_migration_deletes_report_documents_by_url_pattern`
//! guardrail encodes. This pass restores a row **only when its bytes are present**,
//! and mirrors the container self-heal
//! ([`crate::report_documents_container::sniff_missing_containers`]): idempotent,
//! best-effort, wired into `lib.rs` setup, and never fatal.
//!
//! Every restored field is evidence-backed, not invented:
//! - **url** — reconstructed from the surviving Vercom/Orlen rows carrying the
//!   same reused slug, then *verified* by round-tripping it through the
//!   `report_documents` id derivation: each URL below hashes to exactly the
//!   filename still sitting in `report_documents/` (see
//!   [`tests::restored_urls_round_trip_to_the_files_on_disk`]).
//! - **title** — the PDF's own `/Title` where it is accurate (the two
//!   statements). The two report decks carry a **stale template** `/Title`
//!   ("cyber_Folks - raport Q3 2023") naming the wrong year, so they are titled
//!   from their own cover page instead, in cyber_Folks' surviving naming
//!   convention (`CBF_Q1_2025_Raport_kwartalny.pdf`, `CBF_Report_H1_2024_ENG.pdf`).
//! - **doc_kind** — never hardcoded: classified through the normal write seam
//!   (`classify_for_storage`), so a restored row is indistinguishable from an
//!   ingested one. Note the slug-distrust added by this task does **not** fire
//!   here: `Energa` is not a tracked issuer, and foreign detection only ranges
//!   over tracked companies — the structural hole this epic diagnosed. These four
//!   land right because the slug happens to agree with the title about the kind
//!   (`skonsolidowane SF` / `jednostkowe SF`), not because it was distrusted.
//! - **detected_container** — sniffed from the restored bytes (migration 0121).

use crate::fundamentals::extraction::container::detect_container;
use crate::storage::{AppState, CaptureReportDocumentInput, StorageResult};

/// One document `0107` deleted, with the evidence for each reconstructed field.
struct DeletedDocument {
    company_id: &'static str,
    /// The original CDN URL. Verified: it derives the id of a file still on disk.
    url: &'static str,
    /// Filename under the data dir's `report_documents/`.
    local_path: &'static str,
    /// Reconstructed title — see the module header for each one's provenance.
    title: &'static str,
}

/// The four rows, all from cyber_Folks' 2024-Q3 filing (Bankier article suffix
/// `202411130833851271`, published 2024-11-13). Deliberately an explicit table
/// and not a pattern: this is a point-in-time repair of one known deletion, and a
/// heuristic here would be the same mistake `0107` made.
const DELETED_BY_0107: &[DeletedDocument] = &[
    DeletedDocument {
        company_id: "company_gpw_cbf",
        url: "https://www.bankier.pl/static/att/emitent/2024-11/\
              1.-Skrocone-srodroczne-skonsolidowane-SF-Grupy-Energa-za-okres-9-miesiecy-\
              zakonczony-30.09.2024_202411130833851271.pdf",
        local_path: "report_documents/doc_company_gpw_cbf_httpswwwbankierplstaticattemitent\
                     2024_111_skrocone_srodroczne_skonsolidowane_sf_grupy_energa_za_okres_9_\
                     miesiecy_zakonczony_30092024_202411130833851271pdf.pdf",
        // PDF /Title and /Subject, /Author: cyber_Folks.
        title: "cyber_Folks_2024 Q3_SSF.pdf",
    },
    DeletedDocument {
        company_id: "company_gpw_cbf",
        url: "https://www.bankier.pl/static/att/emitent/2024-11/\
              2.-Skrocone-srodroczne-jednostkowe-SF-Energa-SA-za-okres-9-miesiecy-\
              zakonczony-30.09.2024_202411130833851271.pdf",
        local_path: "report_documents/doc_company_gpw_cbf_httpswwwbankierplstaticattemitent\
                     2024_112_skrocone_srodroczne_jednostkowe_sf_energa_sa_za_okres_9_\
                     miesiecy_zakonczony_30092024_202411130833851271pdf.pdf",
        // PDF /Title and /Subject, /Author: cyber_Folks.
        title: "cyber_Folks 2024 Q3_JSF.pdf",
    },
    DeletedDocument {
        company_id: "company_gpw_cbf",
        url: "https://www.bankier.pl/static/att/emitent/2024-11/\
              3.-Informacja-do-skroconego-skonsolidowanego-raportu-Grupy-Energa-za-III-kw.\
              -2024_202411130833851271.pdf",
        local_path: "report_documents/doc_company_gpw_cbf_httpswwwbankierplstaticattemitent\
                     2024_113_informacja_do_skroconego_skonsolidowanego_raportu_grupy_energa_\
                     za_iii_kw_2024_202411130833851271pdf.pdf",
        // Cover page: "Raport kwartalny Grupy cyber_Folks_ / za III kwartał 2024 r.
        // / Poznań, 13 listopada 2024 r." The PDF /Title is a stale template
        // ("cyber_Folks - raport Q3 2023") and is deliberately NOT used.
        title: "CBF_Q3_2024_Raport_kwartalny.pdf",
    },
    DeletedDocument {
        company_id: "company_gpw_cbf",
        url: "https://www.bankier.pl/static/att/emitent/2024-11/\
              4.-Wybrane-skonsolidowane-dane-finansowe-Grupy-Energa-9M-2024\
              _202411130833851271.pdf",
        local_path: "report_documents/doc_company_gpw_cbf_httpswwwbankierplstaticattemitent\
                     2024_114_wybrane_skonsolidowane_dane_finansowe_grupy_energa_9m_2024_\
                     202411130833851271pdf.pdf",
        // Cover page: "cyber_Folks Group Quarterly Report_ / for Q3 2024 ended
        // 30 September 2024 / Poznań, 13 November 2024" — the English twin of the
        // row above; same stale /Title, same treatment.
        title: "CBF_Report_Q3_2024_ENG.pdf",
    },
];

/// Aggregate outcome of the startup restore pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RestoreSummary {
    /// Rows re-inserted by this pass (their bytes were on disk).
    pub restored: usize,
    /// Rows already present — the idempotent no-op path.
    pub already_present: usize,
    /// Rows whose file is not on this machine; left absent and retried next start.
    pub file_missing: usize,
    /// Rows whose company is not tracked in this database — nothing to attach to.
    pub company_absent: usize,
}

impl RestoreSummary {
    /// Whether the pass did anything worth logging.
    pub fn is_noop(&self) -> bool {
        self.restored == 0 && self.file_missing == 0 && self.company_absent == 0
    }
}

/// Re-insert the documents `0107` deleted whose bytes are still on disk.
///
/// Idempotent and best-effort. Per row, exactly one of four things happens:
///
/// - the row already exists → **no-op** (a second start restores nothing);
/// - the company is not tracked here → skipped (nothing to attach the row to);
/// - the file is not on this machine → skipped and **retried next start**,
///   because a row claiming `fetch_status = 'fetched'` with no bytes behind it is
///   worse than a missing row;
/// - otherwise → inserted, marked fetched against the real byte length, and
///   stamped with the container read from those bytes.
///
/// `doc_kind` is classified by the normal write seam, never hardcoded.
pub fn restore_migration_0107_deletions(state: &AppState) -> StorageResult<RestoreSummary> {
    let mut summary = RestoreSummary::default();

    let tracked: std::collections::BTreeSet<String> = state
        .list_companies()?
        .into_iter()
        .map(|company| company.id)
        .collect();

    for document in DELETED_BY_0107 {
        if !tracked.contains(document.company_id) {
            summary.company_absent += 1;
            continue;
        }
        if state
            .list_report_documents_by_company(document.company_id)?
            .iter()
            .any(|existing| existing.url == document.url)
        {
            summary.already_present += 1;
            continue;
        }

        let path = state.data_dir().join(document.local_path);
        let Ok(bytes) = std::fs::read(&path) else {
            summary.file_missing += 1;
            continue;
        };

        let row = state.create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: document.company_id.to_owned(),
            source_type: "espi_attachment".to_owned(),
            url: document.url.to_owned(),
            period_id: None,
            // The originating feed item was pruned long ago; a dangling
            // `origin_ref` would be worse than none.
            origin_ref: None,
            title: Some(document.title.to_owned()),
            attribution: Some("Bankier.pl".to_owned()),
        })?;
        state.mark_report_document_fetched(
            &row.id,
            Some(document.local_path),
            Some("application/pdf"),
            None,
            Some(bytes.len() as i64),
        )?;
        state.set_report_document_detected_container(&row.id, detect_container(&bytes).as_str())?;

        summary.restored += 1;
        log::warn!(
            "module=report_documents_restore stage=migration_0107 company={} url={} \
             (re-inserted a document 0107 deleted on URL-slug evidence; content verified)",
            document.company_id,
            document.url
        );
    }

    Ok(summary)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, NewCompany};
    use std::path::PathBuf;

    fn app(tag: &str) -> AppState {
        let dir: PathBuf =
            std::env::temp_dir().join(format!("brawler-restore-0107-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("report_documents")).expect("data dir");
        let state = AppState::with_data_dir(open_in_memory_database().expect("db"), dir);
        // `company_id(exchange, ticker)` derives `company_gpw_cbf` — the very id
        // the table names, so the fixture and production agree by construction.
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CBF".to_owned(),
                display_name: "cyber_Folks S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");
        assert_eq!(company.id, "company_gpw_cbf");
        state
    }

    /// A minimal real PDF — the restore reads the bytes for their length and
    /// container, so they must actually sniff as a PDF.
    const PDF_BYTES: &[u8] = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\ntrailer\n<<>>\n%%EOF\n";

    fn place_all_files(state: &AppState) {
        for document in DELETED_BY_0107 {
            std::fs::write(state.data_dir().join(document.local_path), PDF_BYTES)
                .expect("write restored file");
        }
    }

    /// The reconstruction check that makes every other field trustworthy: each
    /// URL in the table must derive the id of the file the table points at. A
    /// typo in either column breaks this — and would otherwise ship a row whose
    /// `local_path` and `url` describe different documents.
    #[test]
    fn restored_urls_round_trip_to_the_files_on_disk() {
        let state = app("roundtrip");
        place_all_files(&state);
        restore_migration_0107_deletions(&state).expect("restore");

        for document in DELETED_BY_0107 {
            let expected = format!("report_documents/{}.pdf", {
                let row = state
                    .list_report_documents_by_company(document.company_id)
                    .expect("documents")
                    .into_iter()
                    .find(|row| row.url == document.url)
                    .expect("restored row");
                row.id
            });
            assert_eq!(
                expected, document.local_path,
                "the URL must derive exactly the stored filename it is paired with"
            );
        }
    }

    /// The repair itself: bytes present → the rows come back, fetched, with the
    /// container read from those bytes.
    #[test]
    fn restores_the_deleted_documents_when_their_bytes_are_present() {
        let state = app("restore");
        place_all_files(&state);

        let summary = restore_migration_0107_deletions(&state).expect("restore");
        assert_eq!(summary.restored, 4);
        assert_eq!(summary.file_missing, 0);
        assert_eq!(summary.already_present, 0);

        let rows = state
            .list_report_documents_by_company("company_gpw_cbf")
            .expect("documents");
        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert_eq!(row.source_type, "espi_attachment");
            assert_eq!(row.fetch_status, "fetched");
            assert_eq!(row.byte_size, Some(PDF_BYTES.len() as i64));
            assert_eq!(row.detected_container.as_deref(), Some("pdf"));
            assert!(row.local_path.is_some());
        }
    }

    /// `doc_kind` comes from the write seam, not the table: the restored
    /// statements must land `periodic_ssf`/`periodic_jsf` so they can compete for
    /// a canonical slot. Both carry an *Energa* URL slug, which the classifier
    /// still reads — the slug-distrust shipped by this task does not fire, because
    /// Energa is untracked and foreign detection only ranges over tracked issuers.
    /// They classify correctly because the slug happens to agree with the title
    /// (`skonsolidowane SF` / `jednostkowe SF`); this test is what would catch a
    /// future marker or seam change that stops making them periodic.
    #[test]
    fn restored_statements_classify_as_periodic_despite_the_energa_slugs() {
        let state = app("classify");
        place_all_files(&state);
        restore_migration_0107_deletions(&state).expect("restore");

        let kind = |title: &str| {
            state
                .list_report_documents_by_company("company_gpw_cbf")
                .expect("documents")
                .into_iter()
                .find(|row| row.title.as_deref() == Some(title))
                .unwrap_or_else(|| panic!("no restored row titled {title}"))
                .doc_kind
        };
        assert_eq!(
            kind("cyber_Folks_2024 Q3_SSF.pdf").as_deref(),
            Some("periodic_ssf")
        );
        assert_eq!(
            kind("cyber_Folks 2024 Q3_JSF.pdf").as_deref(),
            Some("periodic_jsf")
        );
    }

    /// cyber_Folks' 2024-Q3 slot was empty because 0107 deleted everything that
    /// could fill it. The restored consolidated statement must re-enter canonical
    /// selection — the user-visible point of the whole repair.
    #[test]
    fn restored_report_re_enters_canonical_selection_for_2024_q3() {
        let state = app("canonical");
        assert!(
            crate::commands::report_documents_view::compute_report_documents_view(
                &state,
                "company_gpw_cbf"
            )
            .expect("view")
            .rows
            .is_empty(),
            "precondition: 0107 left the company with no documents at all"
        );

        place_all_files(&state);
        restore_migration_0107_deletions(&state).expect("restore");

        let view = crate::commands::report_documents_view::compute_report_documents_view(
            &state,
            "company_gpw_cbf",
        )
        .expect("view");
        assert!(
            view.totals.has_periodic_coverage,
            "restoring the statements restores the company's periodic coverage"
        );
        let canonical: Vec<_> = view
            .rows
            .iter()
            .filter(|row| row.canonical)
            .map(|row| {
                (
                    row.fiscal_year,
                    row.period_type.clone(),
                    row.document.title.clone(),
                )
            })
            .collect();
        assert!(
            canonical.contains(&(
                Some(2024),
                Some("Q3".to_owned()),
                Some("cyber_Folks_2024 Q3_SSF.pdf".to_owned())
            )),
            "the restored consolidated statement must hold the 2024 Q3 slot: {canonical:?}"
        );
    }

    /// Missing bytes are not a reason to invent a row — and not a reason to give
    /// up either: the next start retries and succeeds once the file is there.
    #[test]
    fn a_missing_file_restores_nothing_and_is_retried_next_start() {
        let state = app("missing");

        let first = restore_migration_0107_deletions(&state).expect("first pass");
        assert_eq!(first.restored, 0);
        assert_eq!(first.file_missing, 4);
        assert!(state
            .list_report_documents_by_company("company_gpw_cbf")
            .expect("documents")
            .is_empty());

        place_all_files(&state);
        let second = restore_migration_0107_deletions(&state).expect("retry");
        assert_eq!(second.restored, 4);
        assert_eq!(second.file_missing, 0);
    }

    /// Idempotence, the startup contract: a second start finds the rows present
    /// and writes nothing.
    #[test]
    fn a_second_start_restores_nothing() {
        let state = app("idempotent");
        place_all_files(&state);
        restore_migration_0107_deletions(&state).expect("first pass");

        let second = restore_migration_0107_deletions(&state).expect("second pass");
        assert_eq!(second.restored, 0);
        assert_eq!(second.already_present, 4);
        assert!(second.is_noop());
        assert_eq!(
            state
                .list_report_documents_by_company("company_gpw_cbf")
                .expect("documents")
                .len(),
            4
        );
    }

    /// A database that does not track cyber_Folks (every other user) is untouched.
    #[test]
    fn an_untracked_company_is_skipped() {
        let dir: PathBuf = std::env::temp_dir().join(format!(
            "brawler-restore-0107-{}-untracked",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("report_documents")).expect("data dir");
        let state = AppState::with_data_dir(open_in_memory_database().expect("db"), dir);
        place_all_files(&state);

        let summary = restore_migration_0107_deletions(&state).expect("restore");
        assert_eq!(summary.restored, 0);
        assert_eq!(summary.company_absent, 4);
    }
}
