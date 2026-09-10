//! Startup self-heal (#460): rows whose URL carries a bare filename glued into
//! the host are downgraded to `metadata_only` with the typed reason; the URL is
//! never rewritten and no row is deleted (data-model.md § Report Document
//! Model). Same shape as `repair_misassociated_report_documents`.

use super::status::PROTECTION_EXISTS_CLAUSES;
use super::FETCH_ERROR_LINK_INCOMPLETE;
use super::*;

/// One `IMMEDIATE` transaction. Candidates: rows whose URL still carries the
/// pre-#460 glued shape (`https://www.bankier.pl_...`, an escaped `_` so the
/// SQL wildcard cannot over-match), currently `failed`, `metadata_only`, or
/// `pending` (a glued `pending` row can never fetch either), not already
/// carrying this reason, and not protected by the retention
/// contract ([`PROTECTION_EXISTS_CLAUSES`] — correlated against the row
/// being updated, since this is a bulk statement, not the single-id form
/// `mark_metadata_only` uses). Returns the number of rows repaired.
pub(crate) fn repair_incomplete_attachment_links(
    connection: &mut Connection,
) -> StorageResult<usize> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let protection = PROTECTION_EXISTS_CLAUSES.replace("?1", "report_documents.id");
    let sql = format!(
        r"
        UPDATE report_documents
        SET fetch_status = 'metadata_only',
            fetch_error = ?1,
            local_path = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE url LIKE 'https://www.bankier.pl\_%' ESCAPE '\'
          AND fetch_status IN ('failed', 'metadata_only', 'pending')
          AND (fetch_error IS NULL OR fetch_error <> ?1)
          AND NOT ({protection})
        "
    );
    let repaired = transaction.execute(&sql, params![FETCH_ERROR_LINK_INCOMPLETE])?;

    transaction.commit()?;

    if repaired > 0 {
        log::warn!("module=report_documents stage=repair_incomplete_links repaired={repaired}");
    } else {
        log::debug!("module=report_documents stage=repair_incomplete_links repaired=0");
    }

    Ok(repaired)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_company(state: &AppState) -> Company {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "PAS".to_owned(),
                display_name: "Passus S.A.".to_owned(),
                isin: Some("PLPASSU00016".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should create")
    }

    fn seed_document(
        state: &AppState,
        company_id: &str,
        url: &str,
        fetch_status: &str,
        fetch_error: Option<&str>,
    ) -> String {
        let raw = state.checkout_for_tests().expect("raw connection");
        let id = url.to_owned(); // unique per call in these tests; simplest valid id
        raw.execute(
            "INSERT INTO report_documents
                (id, company_id, source_type, url, fetch_status, fetch_error)
             VALUES (?1, ?2, 'espi_attachment', ?3, ?4, ?5)",
            params![id, company_id, url, fetch_status, fetch_error],
        )
        .expect("seed report document");
        id
    }

    #[test]
    fn repairs_glued_failed_and_metadata_only_rows_and_is_idempotent() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = test_company(&state);

        let failed_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_Passus_2023_PSSF.pdf",
            "failed",
            Some("some prior error"),
        );
        let metadata_only_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_Passus_2023_PSF.pdf",
            "metadata_only",
            None,
        );
        let already_repaired_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_Already_Repaired.pdf",
            "metadata_only",
            Some(FETCH_ERROR_LINK_INCOMPLETE),
        );
        let unrelated_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl/static/att/emitent/2026-05/x.pdf",
            "failed",
            Some("network error"),
        );

        let repaired = state
            .report_documents()
            .repair_incomplete_attachment_links()
            .expect("repair should succeed");
        assert_eq!(repaired, 2);

        let doc = |id: &str| state.get_report_document(id).expect("document exists");
        assert_eq!(doc(&failed_id).fetch_status, "metadata_only");
        assert_eq!(
            doc(&failed_id).fetch_error,
            Some(FETCH_ERROR_LINK_INCOMPLETE.to_owned())
        );
        assert_eq!(doc(&metadata_only_id).fetch_status, "metadata_only");
        assert_eq!(
            doc(&metadata_only_id).fetch_error,
            Some(FETCH_ERROR_LINK_INCOMPLETE.to_owned())
        );
        let already_updated_at = doc(&already_repaired_id).updated_at;
        assert_eq!(doc(&unrelated_id).fetch_status, "failed");

        // Second run: no candidates left, `updated_at` on the pre-repaired
        // row is untouched.
        let second = state
            .report_documents()
            .repair_incomplete_attachment_links()
            .expect("second run should succeed");
        assert_eq!(second, 0);
        assert_eq!(doc(&already_repaired_id).updated_at, already_updated_at);
    }

    /// Regression for #460: a glued `pending` row can never fetch (the URL
    /// is bogus), so it must be caught by the same repair as `failed` and
    /// `metadata_only` rows rather than lingering forever as "in progress".
    #[test]
    fn repairs_a_glued_pending_row() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = test_company(&state);

        let pending_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_Pending.pdf",
            "pending",
            None,
        );

        let repaired = state
            .report_documents()
            .repair_incomplete_attachment_links()
            .expect("repair should succeed");
        assert_eq!(repaired, 1);

        let doc = state
            .get_report_document(&pending_id)
            .expect("document exists");
        assert_eq!(doc.fetch_status, "metadata_only");
        assert_eq!(
            doc.fetch_error,
            Some(FETCH_ERROR_LINK_INCOMPLETE.to_owned())
        );
    }

    #[test]
    fn never_downgrades_a_protected_document() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = test_company(&state);

        let protected_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_Protected.pdf",
            "failed",
            None,
        );
        let raw = state.checkout_for_tests().expect("raw connection");
        raw.execute(
            "INSERT INTO notebook_entries (id, company_id, title, body)
             VALUES ('note_incomplete', ?1, 'Note', 'Body')",
            params![company.id],
        )
        .expect("seed notebook entry");
        raw.execute(
            "INSERT INTO notebook_entry_origins (id, notebook_entry_id, source_type, source_id)
             VALUES ('orig_incomplete', 'note_incomplete', 'report_document', ?1)",
            params![protected_id],
        )
        .expect("seed notebook origin");
        drop(raw);

        let repaired = state
            .report_documents()
            .repair_incomplete_attachment_links()
            .expect("repair should succeed");
        assert_eq!(repaired, 0);
        let doc = state
            .get_report_document(&protected_id)
            .expect("document exists");
        assert_eq!(
            doc.fetch_status, "failed",
            "protected document must not be downgraded"
        );
    }

    #[test]
    fn a_poisoned_second_row_rolls_back_the_whole_repair() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = test_company(&state);

        let first_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_First.pdf",
            "failed",
            None,
        );
        let poisoned_id = seed_document(
            &state,
            &company.id,
            "https://www.bankier.pl_2410_Poisoned.pdf",
            "failed",
            None,
        );

        let guard = state.checkout_for_tests().expect("checkout");
        guard
            .execute_batch(&format!(
                "CREATE TRIGGER poison_repair_incomplete_links BEFORE UPDATE ON report_documents
                 WHEN OLD.id = '{poisoned_id}'
                 BEGIN SELECT RAISE(ABORT, 'repair_incomplete_links poisoned for test'); END;"
            ))
            .expect("install poison trigger");
        drop(guard); // never hold the checkout across the store call — pool deadlock (#360/#376)

        let result = state
            .report_documents()
            .repair_incomplete_attachment_links();
        assert!(
            result.is_err(),
            "the poisoned update must surface as an error"
        );

        let first = state
            .get_report_document(&first_id)
            .expect("document exists");
        assert_eq!(
            first.fetch_status, "failed",
            "no partial state: the first row's write must roll back with the transaction"
        );
    }

    /// Real-data probe (#460): before/after counts against a throwaway copy
    /// of the owner's database. Env-gated; SKIPs loudly when unset — never
    /// fails CI.
    ///
    /// ```text
    /// BRAWLER_REPAIR_PROBE_DB=$SCRATCH/owner-db-copy.sqlite3 \
    ///   cargo nextest run -p brawler repair_incomplete_attachment_links_real_data_probe \
    ///   --run-ignored ignored-only --no-capture
    /// ```
    #[test]
    #[ignore = "real-data probe for #460; needs BRAWLER_REPAIR_PROBE_DB (a throwaway copy)"]
    fn repair_incomplete_attachment_links_real_data_probe() {
        let Ok(db_path) = std::env::var("BRAWLER_REPAIR_PROBE_DB") else {
            eprintln!(
                "SKIP repair_incomplete_attachment_links_real_data_probe: \
                 BRAWLER_REPAIR_PROBE_DB not set"
            );
            return;
        };
        if !std::path::Path::new(&db_path).is_file() {
            eprintln!(
                "SKIP repair_incomplete_attachment_links_real_data_probe: no database at {db_path}"
            );
            return;
        }
        let file_name = std::path::Path::new(&db_path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_owned();
        assert!(
            file_name != "brawler.sqlite3" && !db_path.starts_with("/mnt/d/"),
            "refusing to run: {db_path} is the master snapshot or the live application database. \
             This harness migrates — copy it first."
        );

        // Opening a database APPLIES MIGRATIONS — never the master snapshot,
        // never the live application database (checked above).
        let mut connection = open_database(&db_path).expect("open throwaway real db");

        let candidates_before: i64 = connection
            .query_row(
                r"SELECT COUNT(*) FROM report_documents
                  WHERE url LIKE 'https://www.bankier.pl\_%' ESCAPE '\'
                    AND fetch_status IN ('failed', 'metadata_only', 'pending')",
                [],
                |row| row.get(0),
            )
            .expect("count candidates");

        let repaired = repair_incomplete_attachment_links(&mut connection)
            .expect("repair should succeed against real data");

        let candidates_remaining: i64 = connection
            .query_row(
                r"SELECT COUNT(*) FROM report_documents
                  WHERE url LIKE 'https://www.bankier.pl\_%' ESCAPE '\'
                    AND fetch_status IN ('failed', 'metadata_only', 'pending')
                    AND (fetch_error IS NULL OR fetch_error <> ?1)",
                params![FETCH_ERROR_LINK_INCOMPLETE],
                |row| row.get(0),
            )
            .expect("count remaining candidates");

        let mut by_status: std::collections::BTreeMap<String, i64> =
            std::collections::BTreeMap::new();
        {
            let mut statement = connection
                .prepare(
                    r"SELECT fetch_status, COUNT(*) FROM report_documents
                      WHERE url LIKE 'https://www.bankier.pl\_%' ESCAPE '\'
                      GROUP BY fetch_status",
                )
                .expect("prepare breakdown");
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })
                .expect("query breakdown");
            for row in rows {
                let (status, count) = row.expect("breakdown row");
                by_status.insert(status, count);
            }
        }

        let pas_remaining_periodic_with_file: i64 = connection
            .query_row(
                r"SELECT COUNT(*) FROM report_documents rd
                  JOIN companies c ON c.id = rd.company_id
                  WHERE c.ticker = 'PAS'
                    AND rd.source_type = 'espi_attachment'
                    AND rd.fetch_status IN ('fetched', 'pending')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);

        eprintln!("== #460 repair_incomplete_attachment_links real-data probe ==");
        eprintln!("db={db_path}");
        eprintln!("candidates_before={candidates_before}");
        eprintln!("repaired={repaired}");
        eprintln!("candidates_remaining={candidates_remaining}");
        eprintln!("by_status={by_status:?}");
        eprintln!("pas_remaining_periodic_with_file={pas_remaining_periodic_with_file}");
    }
}
