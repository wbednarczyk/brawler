//! Ownership-stakes storage behavior (ADR 0072, plan v0.56 T2).
//!
//! Append-only snapshots per (company, source, as_of, holder); current state is
//! selected by the DOMAIN date (`as_of`), never `created_at`; capital % and
//! votes % are carried separately; free float is derived, never stored.

use super::*;

fn tracked_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

fn stake(company_id: &str, holder: &str, as_of: &str) -> NewOwnershipStake {
    NewOwnershipStake {
        company_id: company_id.to_owned(),
        holder_name_raw: holder.to_owned(),
        holder_type: None,
        capital_pct: Some("10".to_owned()),
        votes_pct: Some("10".to_owned()),
        as_of: as_of.to_owned(),
        source: "report_document".to_owned(),
        report_document_id: None,
        feed_item_id: None,
    }
}

#[test]
fn append_is_idempotent_for_same_source_as_of_holder() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    let first = store
        .append_snapshot(stake(&company.id, "Marcin Iwiński", "2025-06-30"))
        .expect("first append");
    // Same tuple, changed capital — must update in place, not duplicate.
    let mut again = stake(&company.id, "Marcin Iwiński", "2025-06-30");
    again.capital_pct = Some("12".to_owned());
    let second = store.append_snapshot(again).expect("second append");
    assert_eq!(first.id, second.id, "same tuple keeps the deterministic id");

    let history = store.history(&company.id, None).expect("history");
    assert_eq!(history.len(), 1, "same tuple must stay one row");
    assert_eq!(history[0].capital_pct.as_deref(), Some("12"));
}

#[test]
fn new_as_of_appends_and_leaves_prior_row_untouched() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    let first = store
        .append_snapshot(stake(&company.id, "Marcin Iwiński", "2024-06-30"))
        .expect("first append");
    // A different as_of is a new snapshot: two rows, the first byte-identical.
    store
        .append_snapshot(stake(&company.id, "Marcin Iwiński", "2025-06-30"))
        .expect("second append");

    let history = store.history(&company.id, None).expect("history");
    assert_eq!(history.len(), 2, "a new as_of is a new snapshot row");

    let reloaded_first = history
        .iter()
        .find(|row| row.as_of == "2024-06-30")
        .expect("first snapshot still present");
    assert_eq!(reloaded_first.id, first.id);
    assert_eq!(reloaded_first.capital_pct, first.capital_pct);
    assert_eq!(
        reloaded_first.created_at, first.created_at,
        "prior row untouched"
    );
}

#[test]
fn current_state_selects_by_as_of_not_created_at() {
    // DoD §C guardrail: backfill makes `created_at` order diverge from `as_of`
    // order. current_state must pick the latest DOMAIN date, never the latest
    // insert. We insert the NEWER as_of first (so it gets the EARLIER created_at),
    // then force created_at apart to prove selection ignores it.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    let newer = store
        .append_snapshot(stake(&company.id, "OFE Test", "2025-06-30"))
        .expect("newer as_of appended first");
    let older = store
        .append_snapshot(stake(&company.id, "OFE Test", "2024-06-30"))
        .expect("older as_of appended second");

    // Force created_at so the OLDER as_of has the LATEST created_at (backfill).
    {
        let connection = state.checkout().expect("connection");
        connection
            .execute(
                "UPDATE ownership_stakes SET created_at = '2020-01-01T00:00:00.000Z' WHERE id = ?1",
                [&newer.id],
            )
            .expect("update newer created_at");
        connection
            .execute(
                "UPDATE ownership_stakes SET created_at = '2099-01-01T00:00:00.000Z' WHERE id = ?1",
                [&older.id],
            )
            .expect("update older created_at");
    }

    let current = store.current_state(&company.id).expect("current state");
    assert_eq!(current.len(), 1, "one row per holder");
    assert_eq!(
        current[0].as_of, "2025-06-30",
        "current_state must select by as_of, not created_at"
    );
    assert_eq!(current[0].id, newer.id);
}

#[test]
fn capital_and_votes_round_trip_distinctly() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // Preferred-vote shares: capital != votes.
    let mut split = stake(&company.id, "Founder Holdco", "2025-06-30");
    split.capital_pct = Some("33.33".to_owned());
    split.votes_pct = Some("49.90".to_owned());
    store.append_snapshot(split).expect("split disclosure");

    // One-sided disclosure: votes only, capital NULL.
    let mut votes_only = stake(&company.id, "Silent Fund", "2025-06-30");
    votes_only.capital_pct = None;
    votes_only.votes_pct = Some("5.01".to_owned());
    store
        .append_snapshot(votes_only)
        .expect("one-sided disclosure");

    let current = store.current_state(&company.id).expect("current");
    let founder = current
        .iter()
        .find(|row| row.holder_name_raw == "Founder Holdco")
        .expect("founder present");
    assert_eq!(founder.capital_pct.as_deref(), Some("33.33"));
    assert_eq!(founder.votes_pct.as_deref(), Some("49.90"));

    let silent = current
        .iter()
        .find(|row| row.holder_name_raw == "Silent Fund")
        .expect("silent fund present");
    assert_eq!(
        silent.capital_pct, None,
        "capital stays NULL when undisclosed"
    );
    assert_eq!(silent.votes_pct.as_deref(), Some("5.01"));
}

#[test]
fn history_is_ordered_by_as_of() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    for as_of in ["2023-06-30", "2025-06-30", "2024-06-30"] {
        store
            .append_snapshot(stake(&company.id, "OFE Test", as_of))
            .expect("append");
    }

    let history = store.history(&company.id, None).expect("history");
    let dates: Vec<&str> = history.iter().map(|row| row.as_of.as_str()).collect();
    assert_eq!(
        dates,
        vec!["2025-06-30", "2024-06-30", "2023-06-30"],
        "history is ordered by as_of (newest first)"
    );
}

#[test]
fn set_holder_type_persists_without_touching_history() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    store
        .append_snapshot(stake(&company.id, "PKO TFI", "2024-06-30"))
        .expect("first");
    store
        .append_snapshot(stake(&company.id, "PKO TFI", "2025-06-30"))
        .expect("second");

    let normalized = normalize_holder_name("PKO TFI");
    let updated = store
        .set_holder_type(&company.id, &normalized, Some("tfi"))
        .expect("re-type");
    assert_eq!(updated, 2, "re-type applies to the holder's rows");

    let history = store
        .history(&company.id, Some(&normalized))
        .expect("history");
    assert_eq!(history.len(), 2, "re-type never changes the row count");
    for row in &history {
        assert_eq!(row.holder_type.as_deref(), Some("tfi"));
        assert_eq!(row.capital_pct.as_deref(), Some("10"), "pct untouched");
    }
    let as_ofs: Vec<&str> = history.iter().map(|r| r.as_of.as_str()).collect();
    assert_eq!(as_ofs, vec!["2025-06-30", "2024-06-30"], "as_of untouched");
}

#[test]
fn free_float_is_derived_from_disclosed_capital() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    let mut a = stake(&company.id, "Founder Holdco", "2025-06-30");
    a.capital_pct = Some("40".to_owned());
    store.append_snapshot(a).expect("a");
    let mut b = stake(&company.id, "PKO TFI", "2025-06-30");
    b.capital_pct = Some("15".to_owned());
    store.append_snapshot(b).expect("b");

    let derived = store
        .current_state_with_free_float(&company.id)
        .expect("derived read");
    assert_eq!(derived.stakes.len(), 2);
    assert_eq!(derived.disclosed_capital_sum, "55");
    assert_eq!(
        derived.free_float_pct, "45",
        "free float = 100 - disclosed sum"
    );
}

#[test]
fn holder_dictionary_seed_covers_the_main_classes() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let entries = state
        .ownership()
        .load_holder_dictionary()
        .expect("dictionary loads");

    assert!(!entries.is_empty(), "dictionary is seeded");
    for expected in ["tfi", "ofe_pension", "state_treasury"] {
        assert!(
            entries.iter().any(|entry| entry.holder_type == expected),
            "dictionary seeds a {expected} entry"
        );
    }
}

// ---- Holder-type classification (T5, ADR 0072 §3) ----

#[test]
fn classify_unclassified_stamps_dictionary_and_heuristic_hits_only() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    store
        .append_snapshot(stake(
            &company.id,
            "Nationale-Nederlanden OFE",
            "2025-06-30",
        ))
        .expect("dictionary hit"); // dictionary
    store
        .append_snapshot(stake(&company.id, "Fundacja Semper Simul", "2025-06-30"))
        .expect("heuristic hit"); // heuristic
    store
        .append_snapshot(stake(&company.id, "ULTRO S.a.r.l.", "2025-06-30"))
        .expect("residual"); // stays NULL for AI

    // A holder the user already re-typed — must be preserved untouched.
    let mut manual = stake(&company.id, "Marcin Iwiński", "2025-06-30");
    manual.holder_type = Some("founder_insider".to_owned());
    store.append_snapshot(manual).expect("manual re-type");

    let stamped = store
        .classify_unclassified_for_company(&company.id)
        .expect("classify");
    assert_eq!(
        stamped, 2,
        "OFE + Fundacja stamped; residual + manual untouched"
    );

    let current = store.current_state(&company.id).expect("state");
    let holder_type = |raw: &str| {
        current
            .iter()
            .find(|s| s.holder_name_raw == raw)
            .and_then(|s| s.holder_type.clone())
    };
    assert_eq!(
        holder_type("Nationale-Nederlanden OFE").as_deref(),
        Some("ofe_pension")
    );
    assert_eq!(
        holder_type("Fundacja Semper Simul").as_deref(),
        Some("family_foundation")
    );
    assert_eq!(
        holder_type("ULTRO S.a.r.l."),
        None,
        "residual stays NULL for AI"
    );
    assert_eq!(
        holder_type("Marcin Iwiński").as_deref(),
        Some("founder_insider"),
        "a manual re-type is never overwritten"
    );
}

#[test]
fn manual_retype_overrides_dictionary_and_survives_reclassification() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    store
        .append_snapshot(stake(
            &company.id,
            "Nationale-Nederlanden OFE",
            "2025-06-30",
        ))
        .expect("append");
    // First pass stamps the dictionary type.
    store
        .classify_unclassified_for_company(&company.id)
        .expect("classify");
    let normalized = "NATIONALE-NEDERLANDEN OFE";
    // The user re-types it to something else.
    store
        .set_holder_type(&company.id, normalized, Some("other_institutional"))
        .expect("manual re-type");
    // A later re-classification must not overwrite the non-NULL rows.
    let restamped = store
        .classify_unclassified_for_company(&company.id)
        .expect("re-classify");
    assert_eq!(restamped, 0, "no NULL rows remain to stamp");

    let current = store.current_state(&company.id).expect("state");
    assert_eq!(
        current[0].holder_type.as_deref(),
        Some("other_institutional"),
        "the manual re-type survives re-classification"
    );
}

#[test]
fn seeded_dictionary_classifies_real_corpus_names() {
    // Validates the REAL seeded dictionary (migration 0082) + heuristic markers
    // over real GPW holder names, through the storage path end-to-end.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    let cases: [(&str, Option<&str>); 8] = [
        ("Nationale-Nederlanden OFE*", Some("ofe_pension")),
        (
            "Allianz Polska Otwarty Fundusz Emerytalny",
            Some("ofe_pension"),
        ),
        ("OFE PZU „Złota Jesień”", Some("ofe_pension")),
        (
            "Skarb Państwa Rzeczypospolitej Polskiej",
            Some("state_treasury"),
        ),
        ("PKO BP Bankowy OFE", Some("ofe_pension")),
        ("cyber_Folks S.A.", None),
        ("Fundacja Semper Simul", Some("family_foundation")),
        ("ULTRO S.a.r.l.", None),
    ];
    for (name, _) in cases {
        store
            .append_snapshot(stake(&company.id, name, "2025-06-30"))
            .expect("append");
    }
    store
        .classify_unclassified_for_company(&company.id)
        .expect("classify");

    let current = store.current_state(&company.id).expect("state");
    for (name, expected) in cases {
        let got = current
            .iter()
            .find(|s| s.holder_name_raw == name)
            .and_then(|s| s.holder_type.clone());
        assert_eq!(got.as_deref(), expected, "classification of {name}");
    }
}

#[test]
fn current_state_is_scoped_to_the_newest_disclosure_basis() {
    // Real-data harvest (2026-07-16): a holder who drops below the disclosure
    // threshold VANISHES from later reports (no "0%" row), so latest-per-holder
    // over ALL history resurrects stale holders and pushes the sum past 100%.
    // Current state is scoped to the newest FULL-PICTURE basis (report_document OR
    // aggregator; ADR 0072 amended), overlaid with later espi_filing/manual
    // snapshots. A STALE aggregator basis is scoped out just like a stale report.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // 2023 annual report: holders A and B.
    store
        .append_snapshot(stake(&company.id, "Holder A", "2023-12-31"))
        .expect("A 2023");
    store
        .append_snapshot(stake(&company.id, "Holder B", "2023-12-31"))
        .expect("B 2023");
    // A stale aggregator basis (older than the newest report) is scoped out.
    let mut agg = stake(&company.id, "Holder E", "2024-06-30");
    agg.source = "aggregator".to_owned();
    store
        .append_snapshot(agg)
        .expect("E aggregator (stale basis)");
    // 2024 annual report is the newest full-picture basis: A stays, B vanished
    // (below threshold), C entered.
    store
        .append_snapshot(stake(&company.id, "Holder A", "2024-12-31"))
        .expect("A 2024");
    store
        .append_snapshot(stake(&company.id, "Holder C", "2024-12-31"))
        .expect("C 2024");
    // Post-report ESPI notification: D crossed a threshold in 2025.
    let mut espi = stake(&company.id, "Holder D", "2025-03-05");
    espi.source = "espi_filing".to_owned();
    store.append_snapshot(espi).expect("D espi");

    let current = store.current_state(&company.id).expect("current state");
    let holders: Vec<&str> = current
        .iter()
        .map(|row| row.holder_name_raw.as_str())
        .collect();
    assert_eq!(
        holders,
        vec!["Holder A", "Holder C", "Holder D"],
        "baseline = newest full-picture as_of (2024 report); B (pre-baseline ghost) and E (stale aggregator basis) excluded"
    );
    let a = current
        .iter()
        .find(|row| row.holder_name_raw == "Holder A")
        .expect("A present");
    assert_eq!(
        a.as_of, "2024-12-31",
        "A comes from the 2024 baseline report"
    );

    // History keeps everything, including the vanished holder.
    let history = store.history(&company.id, None).expect("history");
    assert!(
        history.iter().any(|row| row.holder_name_raw == "Holder B"),
        "history is append-only and keeps pre-baseline holders"
    );
}

#[test]
fn current_state_dedupes_cosmetic_name_variants_by_canonical_key() {
    // Real-data harvest (2026-07-16): two documents of the same disclosure
    // basis print the same holder with cosmetic variants. Append-only identity
    // keeps both rows; the CURRENT-state read model must show one holder,
    // preferring the most specific raw name.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    store
        .append_snapshot(stake(&company.id, "PTE Allianz Polska", "2026-03-31"))
        .expect("short variant");
    store
        .append_snapshot(stake(&company.id, "PTE Allianz Polska S.A.", "2026-03-31"))
        .expect("long variant");

    let current = store.current_state(&company.id).expect("current");
    assert_eq!(current.len(), 1, "one canonical holder in current state");
    assert_eq!(
        current[0].holder_name_raw, "PTE Allianz Polska S.A.",
        "the most specific raw name wins"
    );

    let history = store.history(&company.id, None).expect("history");
    assert_eq!(history.len(), 2, "append-only history keeps both variants");
}

// ============================================================================
// T4 stream 2 — `major_holdings_change` signal + deterministic ESPI stake update
// ============================================================================

fn dec(s: &str) -> rust_decimal::Decimal {
    <rust_decimal::Decimal as std::str::FromStr>::from_str(s).unwrap()
}

fn stake_pct(
    company_id: &str,
    holder: &str,
    as_of: &str,
    capital: &str,
    votes: &str,
) -> NewOwnershipStake {
    NewOwnershipStake {
        company_id: company_id.to_owned(),
        holder_name_raw: holder.to_owned(),
        holder_type: None,
        capital_pct: Some(capital.to_owned()),
        votes_pct: Some(votes.to_owned()),
        as_of: as_of.to_owned(),
        source: "report_document".to_owned(),
        report_document_id: None,
        feed_item_id: None,
    }
}

fn count_stakes(state: &AppState, company_id: &str) -> i64 {
    let conn = state.checkout_for_tests().expect("conn");
    conn.query_row(
        "SELECT COUNT(*) FROM ownership_stakes WHERE company_id = ?1",
        [company_id],
        |row| row.get(0),
    )
    .expect("count")
}

fn major_holdings_item(
    company: &Company,
    article_id: &str,
    title: &str,
    body: &str,
) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: title.to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-{article_id}.html"),
        summary: "Komunikat ESPI".to_owned(),
        published_at: Some("2026-05-28T17:33:09".to_owned()),
        fetched_at: "2026-05-31T10:00:00Z".to_owned(),
        article_id: article_id.to_owned(),
        pub_id: 3,
        dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
        duplicate_signature: format!("official-secondary:GPW:CDR:{article_id}"),
        body_text: Some(body.to_owned()),
        attachments: Vec::new(),
        detail_fetch_attempted: true,
    }
}

#[test]
fn migration_0085_seeds_major_holdings_category() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let conn = state.checkout_for_tests().expect("conn");
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM signal_categories WHERE key = 'major_holdings_change'",
            [],
            |row| row.get(0),
        )
        .expect("category query");
    assert_eq!(count, 1, "the major_holdings_change category is seeded");
}

#[test]
fn threshold_notification_classifies_and_unrelated_does_not() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let items = vec![
        major_holdings_item(
            &company,
            "9300001",
            "Zawiadomienie o zmianie udziału w ogólnej liczbie głosów",
            "Fundusz otrzymał zawiadomienie od Nationale-Nederlanden OFE, zgodnie z którym \
             udział w ogólnej liczbie głosów zmienił się z 5,15% do 4,77%.",
        ),
        major_holdings_item(
            &company,
            "9300002",
            "Publikacja raportu okresowego za I kwartał 2026",
            "Zarząd przekazuje raport okresowy.",
        ),
    ];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should classify");

    let signals = state
        .list_company_signals(CompanySignalListInput {
            company_id: Some(company.id.clone()),
            ..Default::default()
        })
        .expect("signals should list");
    assert!(
        signals
            .iter()
            .any(|signal| signal.category == "major_holdings_change"),
        "a real threshold notification classifies as major_holdings_change"
    );
    assert!(
        !signals
            .iter()
            .any(|signal| signal.category == "major_holdings_change"
                && signal.title.contains("raportu okresowego")),
        "a routine periodic-report publication is not a holdings change"
    );
}

#[test]
fn clean_notification_appends_espi_filing_stake_with_provenance_idempotently() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let item = major_holdings_item(
        &company,
        "9300010",
        "Znaczne pakiety akcji — zawiadomienie w trybie art. 69",
        "Spółka otrzymała od Marcin Iwiński zawiadomienie, że posiada 12 650 000 akcji \
         stanowiących 12,66% kapitału zakładowego, uprawniających do 12,66% ogólnej liczby głosów.",
    );
    state
        .ingest_bankier_company_items(std::slice::from_ref(&item))
        .expect("ingest + espi stake update");

    let store = state.ownership();
    let espi: Vec<_> = store
        .history(&company.id, None)
        .expect("history")
        .into_iter()
        .filter(|row| row.source == "espi_filing")
        .collect();
    assert_eq!(espi.len(), 1, "one espi_filing stake from the clean parse");
    let stake = &espi[0];
    assert_eq!(stake.holder_name_raw, "Marcin Iwiński");
    assert_eq!(stake.capital_pct.as_deref(), Some("12.66"));
    assert_eq!(stake.votes_pct.as_deref(), Some("12.66"));
    assert_eq!(
        stake.as_of, "2026-05-28",
        "as_of is the filing disclosure date"
    );
    assert!(stake.feed_item_id.is_some(), "provenance to the filing");

    // Re-ingesting the same filing must not duplicate (deterministic id + once-gate).
    state
        .ingest_bankier_company_items(&[item])
        .expect("re-ingest is idempotent");
    let espi_after = store
        .history(&company.id, None)
        .expect("history")
        .into_iter()
        .filter(|row| row.source == "espi_filing")
        .count();
    assert_eq!(espi_after, 1, "re-processing the same filing is idempotent");
}

#[test]
fn ambiguous_notification_writes_no_stake_and_records_diagnostic() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode");
    let company = tracked_company(&state);
    let item = major_holdings_item(
        &company,
        "9300020",
        "Zawiadomienie o zmianie stanu posiadania",
        "Przed transakcją posiadał akcje stanowiące 5,15% kapitału zakładowego. \
         Po transakcji posiada akcje stanowiące 4,77% kapitału zakładowego.",
    );
    state.ingest_bankier_company_items(&[item]).expect("ingest");

    let has_espi = state
        .ownership()
        .history(&company.id, None)
        .expect("history")
        .iter()
        .any(|row| row.source == "espi_filing");
    assert!(!has_espi, "no stake written on ambiguity — never guess");

    let events = state.list_diagnostic_events(20).expect("diagnostics");
    assert!(
        events.iter().any(|event| event.stage == "espi_stake_update"
            && event
                .metadata
                .to_string()
                .contains("ownership_espi_unparsed")),
        "an ownership_espi_unparsed diagnostic was recorded"
    );
}

// ============================================================================
// T4 stream 3 — aggregator witness (compare-only)
// ============================================================================

#[test]
fn witness_agrees_when_aggregator_matches_disclosed() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();
    store
        .append_snapshot(stake_pct(
            &company.id,
            "Marcin Iwiński",
            "2026-06-30",
            "12.66",
            "12.66",
        ))
        .expect("disclosed stake");
    let disclosed = store.current_state(&company.id).expect("current state");

    let aggregator = vec![WitnessHolder {
        holder_name: "MARCIN IWIŃSKI".to_owned(),
        capital_pct: Some(dec("12.66")),
        votes_pct: Some(dec("12.66")),
    }];
    let comparison = compare_witness(&company.id, &aggregator, &disclosed);
    assert_eq!(comparison.status, "agree");
    assert!(
        comparison.divergences.is_empty(),
        "matching holder does not diverge"
    );

    // Recording an agreeing comparison emits no diagnostics.
    state.set_developer_mode_enabled(true).expect("dev mode");
    store
        .record_witness_comparisons(
            "biznesradar-akcjonariat",
            &[comparison],
            "2026-07-16T10:00:00Z",
        )
        .expect("record");
    let events = state.list_diagnostic_events(20).expect("diagnostics");
    assert!(
        !events
            .iter()
            .any(|event| event.stage == "witness_divergence"),
        "agreement records no divergence diagnostic"
    );
}

/// A page fetcher that returns the CDR akcjonariat sample for any ticker.
struct SampleAkcjonariatFetcher;

impl crate::source_adapters::biznesradar_ownership::BiznesRadarOwnershipFetcher
    for SampleAkcjonariatFetcher
{
    fn fetch_akcjonariat(
        &self,
        _ticker: &str,
    ) -> Result<String, crate::source_adapters::biznesradar_ownership::BiznesRadarOwnershipError>
    {
        Ok(include_str!("../../../samples/biznesradar_akcjonariat_cdr.html").to_owned())
    }
}

/// A fetcher whose główni-shaped table sums to an implausible >102% capital.
struct ImplausibleAkcjonariatFetcher;

impl crate::source_adapters::biznesradar_ownership::BiznesRadarOwnershipFetcher
    for ImplausibleAkcjonariatFetcher
{
    fn fetch_akcjonariat(
        &self,
        _ticker: &str,
    ) -> Result<String, crate::source_adapters::biznesradar_ownership::BiznesRadarOwnershipError>
    {
        Ok(r#"<h2 class="sub">Główni akcjonariusze</h2>
            <table class="qTableFull">
              <tr><th>Akcjonariusz</th><th>Udział</th><th>Liczba akcji</th><th>Wartość rynkowa</th><th>Udział na WZA</th><th>Liczba głosów</th><th>Data aktualizacji</th></tr>
              <tr><td>Holder One</td><td>90.00 %</td><td>1</td><td>2</td><td>90.00 %</td><td>1</td><td>01.06.2026</td></tr>
              <tr><td>Holder Two</td><td>90.00 %</td><td>1</td><td>2</td><td>90.00 %</td><td>1</td><td>01.06.2026</td></tr>
            </table>"#
            .to_owned())
    }
}

/// A `WitnessHolder` from percent literals (capital, votes).
fn wh(name: &str, capital: &str, votes: &str) -> WitnessHolder {
    WitnessHolder {
        holder_name: name.to_owned(),
        capital_pct: Some(dec(capital)),
        votes_pct: Some(dec(votes)),
    }
}

fn count_source_rows(state: &AppState, company_id: &str, source: &str) -> i64 {
    let conn = state.checkout_for_tests().expect("conn");
    conn.query_row(
        "SELECT COUNT(*) FROM ownership_stakes WHERE company_id = ?1 AND source = ?2",
        rusqlite::params![company_id, source],
        |row| row.get(0),
    )
    .expect("count")
}

// ADR 0072 §2c: the aggregator is now the automatic BREADTH source.
// `refresh_with` writes the parsed table as `aggregator` snapshots, then
// witnesses it against the DISCLOSED-only reference (reports/ESPI).
#[test]
fn aggregator_refresh_writes_basis_and_divergence_witnessed_by_disclosed() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    state.set_developer_mode_enabled(true).expect("dev mode");
    let company = tracked_company(&state); // ticker CDR
    let store = state.ownership();

    // A pre-existing disclosed report stake for Marcin Iwiński that DIVERGES from
    // the aggregator sample (sample capital 12.66; disclosed 20.00) → a pct_gap.
    store
        .append_snapshot(stake_pct(
            &company.id,
            "Marcin Iwiński",
            "2026-06-30",
            "20.00",
            "20.00",
        ))
        .expect("disclosed report stake");

    let ctx = crate::jobs::source_refresh::RefreshContext {
        trigger: "test",
        date: None,
    };
    crate::source_adapters::biznesradar_ownership::refresh_with(
        &SampleAkcjonariatFetcher,
        &state,
        &ctx,
        false,
    )
    .expect("refresh writes the aggregator basis");

    // 1. Aggregator rows written: the 3 główni holders of the sample (pozostali
    //    table is not ingested; ADR 0072 amended 2026-07-16).
    assert_eq!(
        count_source_rows(&state, &company.id, "aggregator"),
        3,
        "główni holders written as aggregator snapshots"
    );

    // 2. The pre-existing report row is untouched (append-only).
    let iwinski = store
        .history(&company.id, Some("MARCIN IWIŃSKI"))
        .expect("history");
    let report_row = iwinski
        .iter()
        .find(|row| row.source == "report_document")
        .expect("report row survives");
    assert_eq!(report_row.capital_pct.as_deref(), Some("20.00"));
    assert_eq!(report_row.as_of, "2026-06-30");

    // 3. Witness result diverged (compared against disclosed-only, not itself) and
    //    a divergence diagnostic was emitted.
    let witness = store
        .get_witness_result("biznesradar-akcjonariat", &company.id)
        .expect("get witness result")
        .expect("a witness result row");
    assert_eq!(witness.status, "diverged");
    let events = state.list_diagnostic_events(20).expect("diagnostics");
    assert!(
        events
            .iter()
            .any(|event| event.stage == "witness_divergence"),
        "the divergence is recorded as a diagnostic"
    );

    // 4. DoD §C: a successful refresh marks the adapter healthy.
    let adapter = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == "biznesradar-akcjonariat")
        .expect("adapter listed");
    assert!(adapter.last_success_at.is_some(), "last_success_at is set");

    // 5. A written holder got a deterministic holder_type (Skarb Państwa).
    let skarb = store
        .history(&company.id, Some("SKARB PAŃSTWA"))
        .expect("history");
    assert!(
        skarb
            .iter()
            .any(|row| row.holder_type.as_deref() == Some("state_treasury")),
        "deterministic classification stamped Skarb Państwa as state_treasury"
    );
}

#[test]
fn aggregator_basis_wins_current_state_when_newest() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // Report basis 2026-06-30 {A 20%}.
    store
        .append_snapshot(stake_pct(&company.id, "Holder A", "2026-06-30", "20", "20"))
        .expect("report basis");
    // Newer aggregator basis 2026-07-10 {A 18%, B 6%} wins.
    store
        .replace_aggregator_basis(
            &company.id,
            "2026-07-10",
            &[wh("Holder A", "18", "18"), wh("Holder B", "6", "6")],
        )
        .expect("aggregator basis");

    let state1 = store
        .current_state_with_free_float(&company.id)
        .expect("current state");
    let a = state1
        .stakes
        .iter()
        .find(|s| s.holder_name_normalized == "HOLDER A")
        .expect("A present");
    let b = state1
        .stakes
        .iter()
        .find(|s| s.holder_name_normalized == "HOLDER B")
        .expect("B present");
    assert_eq!(a.capital_pct.as_deref(), Some("18"));
    assert_eq!(b.capital_pct.as_deref(), Some("6"));
    assert_eq!(state1.stakes.len(), 2, "only the newest basis' holders");

    // A newer REPORT basis 2026-07-15 {A 21%} then wins (newest full picture).
    store
        .append_snapshot(stake_pct(&company.id, "Holder A", "2026-07-15", "21", "21"))
        .expect("newer report basis");
    let state2 = store.current_state(&company.id).expect("current state");
    assert_eq!(state2.len(), 1, "the newest basis holds exactly {{A}}");
    assert_eq!(state2[0].holder_name_normalized, "HOLDER A");
    assert_eq!(state2[0].capital_pct.as_deref(), Some("21"));
}

#[test]
fn espi_overlay_applies_over_aggregator_basis() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    store
        .replace_aggregator_basis(
            &company.id,
            "2026-07-10",
            &[wh("Holder A", "18", "18"), wh("Holder B", "6", "6")],
        )
        .expect("aggregator basis");
    // A later espi_filing single-holder update overlays the basis.
    let mut espi = stake_pct(&company.id, "Holder A", "2026-07-12", "12", "12");
    espi.source = "espi_filing".to_owned();
    store.append_snapshot(espi).expect("espi overlay");

    let current = store.current_state(&company.id).expect("current state");
    let a = current
        .iter()
        .find(|s| s.holder_name_normalized == "HOLDER A")
        .expect("A present");
    let b = current
        .iter()
        .find(|s| s.holder_name_normalized == "HOLDER B")
        .expect("B present");
    assert_eq!(
        a.capital_pct.as_deref(),
        Some("12"),
        "espi overlay wins for A"
    );
    assert_eq!(b.capital_pct.as_deref(), Some("6"), "B stays at the basis");
}

#[test]
fn aggregator_same_basis_reingest_reconciles_removed_holder() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // A report row and an OLDER aggregator basis must survive the reconcile.
    store
        .append_snapshot(stake_pct(&company.id, "Holder A", "2026-06-30", "20", "20"))
        .expect("report row");
    store
        .replace_aggregator_basis(&company.id, "2026-07-01", &[wh("Holder C", "9", "9")])
        .expect("older aggregator basis");
    // Basis 2026-07-10 with A and B.
    store
        .replace_aggregator_basis(
            &company.id,
            "2026-07-10",
            &[wh("Holder A", "18", "18"), wh("Holder B", "6", "6")],
        )
        .expect("basis with A+B");
    // Re-ingest the SAME basis with only A → B is reconciled away.
    store
        .replace_aggregator_basis(&company.id, "2026-07-10", &[wh("Holder A", "18", "18")])
        .expect("reingest with only A");

    let hist = store.history(&company.id, None).expect("history");
    assert!(
        !hist.iter().any(|row| row.source == "aggregator"
            && row.as_of == "2026-07-10"
            && row.holder_name_normalized == "HOLDER B"),
        "B's aggregator row at 2026-07-10 was deleted (same-basis reconcile)"
    );
    assert!(
        hist.iter().any(|row| row.source == "aggregator"
            && row.as_of == "2026-07-10"
            && row.holder_name_normalized == "HOLDER A"),
        "A stays at the reconciled basis"
    );
    assert!(
        hist.iter()
            .any(|row| row.source == "report_document" && row.holder_name_normalized == "HOLDER A"),
        "the report row is never touched"
    );
    assert!(
        hist.iter().any(|row| row.source == "aggregator"
            && row.as_of == "2026-07-01"
            && row.holder_name_normalized == "HOLDER C"),
        "the older aggregator basis is never touched"
    );
}

#[test]
fn free_float_history_includes_aggregator_bases() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // Report basis 2026-06-30 Σ40 → float 60.
    store
        .append_snapshot(stake_pct(&company.id, "Holder A", "2026-06-30", "25", "25"))
        .expect("report A");
    store
        .append_snapshot(stake_pct(&company.id, "Holder B", "2026-06-30", "15", "15"))
        .expect("report B");
    // Aggregator basis 2026-07-10 Σ45 → float 55.
    store
        .replace_aggregator_basis(
            &company.id,
            "2026-07-10",
            &[wh("Holder A", "30", "30"), wh("Holder B", "15", "15")],
        )
        .expect("aggregator basis");

    let points = store.free_float_history(&company.id).expect("history");
    assert_eq!(
        points,
        vec![
            ("2026-06-30".to_owned(), "60".to_owned()),
            ("2026-07-10".to_owned(), "55".to_owned()),
        ]
    );
}

#[test]
fn disclosed_reference_state_excludes_aggregator() {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // Only aggregator rows present → the disclosed reference is empty.
    store
        .replace_aggregator_basis(&company.id, "2026-07-10", &[wh("Holder A", "18", "18")])
        .expect("aggregator basis");

    let reference = store
        .disclosed_reference_state(&company.id)
        .expect("disclosed reference");
    assert!(
        reference.is_empty(),
        "aggregator rows are not a disclosed reference"
    );

    // The witness comparison against that empty reference yields no_reference.
    let comparison = compare_witness(&company.id, &[wh("Holder A", "18", "18")], &reference);
    assert_eq!(comparison.status, "no_reference");
}

#[test]
fn implausible_basis_writes_nothing_and_records_diagnostic() {
    // A parsed basis whose disclosed capital sums > 102% is implausible: write
    // nothing, record an `ownership_aggregator_implausible` diagnostic, and (as the
    // only page) fail the all-fail guard so the adapter is NOT marked healthy.
    let state = AppState::new(open_in_memory_database().expect("db"));
    state.set_developer_mode_enabled(true).expect("dev mode");
    let company = tracked_company(&state); // ticker CDR

    let ctx = crate::jobs::source_refresh::RefreshContext {
        trigger: "test",
        date: None,
    };
    let outcome = crate::source_adapters::biznesradar_ownership::refresh_with(
        &ImplausibleAkcjonariatFetcher,
        &state,
        &ctx,
        false,
    );
    assert!(outcome.is_err(), "an all-implausible run is a failed run");

    assert_eq!(
        count_source_rows(&state, &company.id, "aggregator"),
        0,
        "nothing is written for an implausible basis"
    );

    let events = state.list_diagnostic_events(20).expect("diagnostics");
    assert!(
        events
            .iter()
            .any(|event| event.stage == "ownership_aggregator_implausible"),
        "an implausible basis records its diagnostic"
    );

    let adapter = state
        .list_source_adapters()
        .expect("adapters")
        .into_iter()
        .find(|adapter| adapter.id == "biznesradar-akcjonariat")
        .expect("adapter listed");
    assert!(
        adapter.last_success_at.is_none(),
        "an all-implausible run must not mark the adapter healthy"
    );
}

#[test]
fn manual_holder_type_survives_aggregator_reingest() {
    // A daily aggregator refresh re-ingests the same basis with holder_type=None.
    // Manual re-types and confirmed classifications must NEVER be wiped by that
    // re-ingest (docs/data-model.md, Holder-type classification §4: manual wins).
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // "Jan Testowy" matches no dictionary alias / heuristic marker.
    store
        .replace_aggregator_basis(&company.id, "2026-07-10", &[wh("Jan Testowy", "10", "10")])
        .expect("initial aggregator basis");
    // Manual re-type — the label of record.
    store
        .set_holder_type(&company.id, "JAN TESTOWY", Some("founder_insider"))
        .expect("manual re-type");

    // Same-basis daily re-ingest (incoming holder_type is None).
    store
        .replace_aggregator_basis(&company.id, "2026-07-10", &[wh("Jan Testowy", "10", "10")])
        .expect("re-ingest same basis");

    let current = store.current_state(&company.id).expect("current state");
    let jan = current
        .iter()
        .find(|s| s.holder_name_normalized == "JAN TESTOWY")
        .expect("Jan present");
    assert_eq!(
        jan.holder_type.as_deref(),
        Some("founder_insider"),
        "the manual re-type must survive an aggregator re-ingest"
    );

    // A dictionary/heuristic-classified holder (mirrors the refresh path: write the
    // basis, then classify) must also stay classified across a later re-ingest
    // WITHOUT re-running classify.
    store
        .replace_aggregator_basis(
            &company.id,
            "2026-07-10",
            &[
                wh("Jan Testowy", "10", "10"),
                wh("Skarb Państwa", "30", "45"),
            ],
        )
        .expect("add Skarb");
    store
        .classify_unclassified_for_company(&company.id)
        .expect("classify");
    store
        .replace_aggregator_basis(
            &company.id,
            "2026-07-10",
            &[
                wh("Jan Testowy", "10", "10"),
                wh("Skarb Państwa", "30", "45"),
            ],
        )
        .expect("re-ingest with Skarb classified");

    let current2 = store.current_state(&company.id).expect("current state");
    let skarb = current2
        .iter()
        .find(|s| s.holder_name_normalized == "SKARB PAŃSTWA")
        .expect("Skarb present");
    assert_eq!(
        skarb.holder_type.as_deref(),
        Some("state_treasury"),
        "a prior classification survives re-ingest without a re-classify call"
    );
    let jan2 = current2
        .iter()
        .find(|s| s.holder_name_normalized == "JAN TESTOWY")
        .expect("Jan present");
    assert_eq!(
        jan2.holder_type.as_deref(),
        Some("founder_insider"),
        "the manual re-type still survives"
    );
}

#[test]
fn witness_end_to_end_sample_html_vs_seeded_state_records_result_without_writing_stakes() {
    // Sample aggregator HTML -> parsed holders -> compare against seeded disclosed
    // state -> per-company witness result, and NO stake is ever written.
    let state = AppState::new(open_in_memory_database().expect("db"));
    state.set_developer_mode_enabled(true).expect("dev mode");
    let company = tracked_company(&state);
    let store = state.ownership();
    // Disclosed state agrees with the sample on Iwiński but not on Skarb Państwa
    // (sample: 30% capital; disclosed: 10%) → a pct_gap divergence.
    store
        .append_snapshot(stake_pct(
            &company.id,
            "Marcin Iwiński",
            "2026-06-30",
            "12.66",
            "12.66",
        ))
        .expect("disclosed stake");
    store
        .append_snapshot(stake_pct(
            &company.id,
            "Skarb Państwa",
            "2026-06-30",
            "10.00",
            "10.00",
        ))
        .expect("disclosed stake");
    let disclosed = store.current_state(&company.id).expect("current state");
    let rows_before = count_stakes(&state, &company.id);

    let sample = include_str!("../../../samples/biznesradar_akcjonariat_cdr.html");
    let holders = crate::source_adapters::biznesradar_ownership::parse_akcjonariat(sample)
        .expect("sample parses");
    let comparison = compare_witness(&company.id, &holders, &disclosed);
    assert_eq!(comparison.status, "diverged");
    assert!(
        comparison
            .divergences
            .iter()
            .any(|d| d.kind == "pct_gap" && d.holder_normalized == "SKARB PAŃSTWA"),
        "the Skarb Państwa capital gap is flagged"
    );

    store
        .record_witness_comparisons(
            "biznesradar-akcjonariat",
            &[comparison],
            "2026-07-16T10:00:00Z",
        )
        .expect("record");

    // Compare-only: the witness wrote no stakes.
    assert_eq!(
        count_stakes(&state, &company.id),
        rows_before,
        "no stakes written"
    );
    let result = store
        .get_witness_result("biznesradar-akcjonariat", &company.id)
        .expect("get")
        .expect("row");
    assert_eq!(result.status, "diverged");
}

#[test]
fn current_state_merges_abbreviations_and_parenthetical_variants() {
    // The same holder printed as "nn pte" and "Nationale-Nederlanden PTE S.A."
    // (dictionary-alias identity, migration 0086) and as "cyber_Folks S.A." vs
    // "cyber_Folks S.A. (akcje własne)" (parenthetical qualifier) must be ONE
    // current-state row each.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    store
        .append_snapshot(stake(&company.id, "nn pte", "2026-03-31"))
        .expect("abbrev");
    store
        .append_snapshot(stake(
            &company.id,
            "Nationale-Nederlanden PTE S.A.",
            "2026-03-31",
        ))
        .expect("full name");
    store
        .append_snapshot(stake(&company.id, "cyber_Folks S.A.", "2026-03-31"))
        .expect("bare issuer");
    store
        .append_snapshot(stake(
            &company.id,
            "cyber_Folks S.A. (akcje własne)",
            "2026-03-31",
        ))
        .expect("treasury qualifier");

    let current = store.current_state(&company.id).expect("current");
    let names: Vec<&str> = current
        .iter()
        .map(|row| row.holder_name_raw.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "cyber_Folks S.A. (akcje własne)",
            "Nationale-Nederlanden PTE S.A."
        ],
        "each variant pair merges to its most specific raw name"
    );

    // Classification: PTE marker + treasury qualifier both stamp deterministically.
    store
        .classify_unclassified_for_company(&company.id)
        .expect("classify");
    let current = store
        .current_state(&company.id)
        .expect("current after classify");
    let types: Vec<Option<&str>> = current
        .iter()
        .map(|row| row.holder_type.as_deref())
        .collect();
    assert_eq!(
        types,
        vec![Some("treasury_shares"), Some("ofe_pension")],
        "akcje-własne marker + PTE dictionary stamp the merged holders"
    );

    let history = store.history(&company.id, None).expect("history");
    assert_eq!(
        history.len(),
        4,
        "append-only history keeps all four variants"
    );
}

#[test]
fn free_float_history_yields_one_deduped_point_per_report_basis() {
    // Owner dogfooding round 3: float as a time series, one point per report
    // disclosure basis; variant pairs within a basis must not double-count.
    let state = AppState::new(open_in_memory_database().expect("db"));
    let company = tracked_company(&state);
    let store = state.ownership();

    // 2024 basis: one 30% holder → float 70.
    let mut a = stake(&company.id, "Holder A", "2024-12-31");
    a.capital_pct = Some("30".to_owned());
    store.append_snapshot(a).expect("A 2024");
    // 2025 basis: A at 30% printed twice as cosmetic variants + B at 20% → float 50.
    let mut a1 = stake(&company.id, "Holder A", "2025-12-31");
    a1.capital_pct = Some("30".to_owned());
    store.append_snapshot(a1).expect("A 2025");
    let mut a2 = stake(&company.id, "Holder A S.A.", "2025-12-31");
    a2.capital_pct = Some("30.1".to_owned());
    store.append_snapshot(a2).expect("A variant 2025");
    let mut b = stake(&company.id, "Holder B", "2025-12-31");
    b.capital_pct = Some("20".to_owned());
    store.append_snapshot(b).expect("B 2025");
    // ESPI update is NOT a basis — no extra point.
    let mut espi = stake(&company.id, "Holder C", "2026-02-01");
    espi.source = "espi_filing".to_owned();
    store.append_snapshot(espi).expect("C espi");

    let points = store.free_float_history(&company.id).expect("history");
    assert_eq!(points.len(), 2, "one point per report basis, ESPI excluded");
    assert_eq!(points[0], ("2024-12-31".to_owned(), "70".to_owned()));
    assert_eq!(points[1].0, "2025-12-31", "second basis is the 2025 report");
    // Variant pair counted ONCE (first-by-identity), so float = 100 − (30 + 20).
    assert_eq!(points[1].1, "50", "variant pair must not double-count");
}
