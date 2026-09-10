//! Real-data probe for #463 (ADR 0111): migrating a throwaway copy of the
//! maintainer's database removes the `youtube_transcription_*` settings rows
//! and any transcript search-index rows, keeps the legacy transcript tables,
//! and the research read model still answers for the audit companies.
//!
//! **Inert in CI** — skips unless `BRAWLER_REAL_DB_SCRATCH` points at a
//! throwaway copy that MAY be migrated (`open_database` migrates on open).

use crate::storage::{open_database, AppState, ResearchEvidenceInput};

#[test]
fn migration_0156_retires_transcription_residue_on_the_owner_copy() {
    let probe = "migration_0156_retires_transcription_residue_on_the_owner_copy";
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB_SCRATCH") else {
        eprintln!(
            "SKIP {probe}: set BRAWLER_REAL_DB_SCRATCH to a THROWAWAY copy of the owner's database"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP {probe}: no database at {db_path}");
        return;
    }

    let connection = open_database(&db_path).expect("open + migrate the scratch copy");
    let settings_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM settings WHERE key LIKE 'youtube_transcription_%'",
            [],
            |row| row.get(0),
        )
        .expect("settings count");
    assert_eq!(settings_rows, 0, "transcription settings rows must be gone");
    let index_rows: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM search_index WHERE content_type = 'transcript_segment'",
            [],
            |row| row.get(0),
        )
        .expect("search index count");
    assert_eq!(index_rows, 0, "transcript search-index rows must be gone");
    for table in ["transcript_jobs", "transcript_segments"] {
        let exists: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [table],
                |row| row.get(0),
            )
            .expect("table check");
        assert!(exists, "{table} stays (append-only migrations)");
    }
    eprintln!("transcription probe: settings rows {settings_rows}, index rows {index_rows}, legacy tables present");

    // The research read model no longer unions the transcript tables — it must
    // still answer for the audit companies without error.
    let state = AppState::new(connection);
    for company_id in ["company_gpw_dvl", "company_gpw_xtb", "company_gpw_kgh"] {
        let timeline = state
            .list_research_evidence(ResearchEvidenceInput {
                company_id: Some(company_id.to_owned()),
                watchlist_id: None,
                evidence_types: None,
                changed_since_review_only: None,
                limit: Some(50),
            })
            .expect("research evidence lists");
        assert!(
            timeline
                .items
                .iter()
                .all(|item| item.evidence_type != "transcript_segment"),
            "{company_id}: no transcript evidence may surface"
        );
        eprintln!(
            "transcription probe: {company_id} evidence rows = {}",
            timeline.items.len()
        );
    }
}
