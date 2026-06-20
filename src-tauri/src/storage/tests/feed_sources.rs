use super::common::*;
use super::feed_matching::find_company_for_exchange_listing;
use super::*;

#[test]
fn starts_without_seeded_feed_or_registry_rows() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let feed_items = state.list_feed_items().expect("feed items should list");
    let registry_entries = state
        .list_company_registry_entries()
        .expect("registry entries should list");

    assert!(feed_items.is_empty());
    assert!(registry_entries.is_empty());
}

#[test]
fn persists_feed_item_read_and_saved_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");

    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("test listing should ingest");
    let feed_item_id = state
        .list_feed_items()
        .expect("feed items should list")
        .first()
        .expect("test feed item should exist")
        .id
        .clone();

    let updated = state
        .update_feed_item_state(FeedItemStateInput {
            id: feed_item_id.clone(),
            read: Some(true),
            saved: Some(true),
        })
        .expect("feed item state should update");

    assert!(!updated.unread);
    assert!(updated.saved);

    let feed_items = state.list_feed_items().expect("feed items should list");
    let cdr = feed_items
        .iter()
        .find(|item| item.id == feed_item_id)
        .expect("CDR test item should remain present");

    assert!(!cdr.unread);
    assert!(cdr.saved);
}

#[test]
fn ingests_gpw_listings_and_matches_tracked_company_by_isin() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "NTC".to_owned(),
            display_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
            isin: Some("PLECMNG00019".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");

    let result = state
        .ingest_gpw_report_listings(&[
            GpwReportListing {
                report_type: "Bieżący".to_owned(),
                system: "ESPI".to_owned(),
                report_number: "7/2026".to_owned(),
                company_ticker: "NTC".to_owned(),
                company_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
                isin: "PLECMNG00019".to_owned(),
                title: "Oświadczenie w sprawie formy przekazywania raportów kwartalnych."
                    .to_owned(),
                detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=123456".to_owned(),
                published_at: "2026-05-30T17:13:31+02:00".to_owned(),
                fetched_at: "2026-05-30T17:30:00Z".to_owned(),
                dedupe_key: "gpw-espi-ebi:espi:PLECMNG00019:7/2026:2026-05-30T17:13:31+02:00"
                    .to_owned(),
                body_text: Some("Official report body from GPW detail page.".to_owned()),
                attachments: vec![GpwReportAttachment {
                    label: "7_2026_oswiadczenie.pdf".to_owned(),
                    url: "https://www.gpw.pl/pub/GPW/ESPI/2026/7_2026_oswiadczenie.pdf".to_owned(),
                }],
            },
            GpwReportListing {
                report_type: "Bieżący".to_owned(),
                system: "ESPI".to_owned(),
                report_number: "9/2026".to_owned(),
                company_ticker: "UNK".to_owned(),
                company_name: "UNTRACKED S.A.".to_owned(),
                isin: "PLUNTRK00001".to_owned(),
                title: "Untracked company report".to_owned(),
                detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=999999".to_owned(),
                published_at: "2026-05-30T18:13:31+02:00".to_owned(),
                fetched_at: "2026-05-30T18:30:00Z".to_owned(),
                dedupe_key: "gpw-espi-ebi:espi:PLUNTRK00001:9/2026:2026-05-30T18:13:31+02:00"
                    .to_owned(),
                body_text: None,
                attachments: Vec::new(),
            },
        ])
        .expect("listings should ingest");

    assert_eq!(result.items_fetched, 2);
    assert_eq!(result.items_created, 2);
    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 1);

    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("source adapters should list");
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("GPW adapter should exist");

    assert_eq!(adapter.last_items_fetched, Some(2));
    assert_eq!(adapter.last_items_created, Some(2));
    assert_eq!(adapter.last_items_matched, Some(1));
    assert_eq!(adapter.last_items_unmatched, Some(1));

    let visible_items = state.list_feed_items().expect("feed items should list");
    let ntc = visible_items
        .iter()
        .find(|item| item.company == "GPW:NTC")
        .expect("matched GPW listing should be visible");

    assert_eq!(ntc.source, "GPW ESPI/EBI");
    assert_eq!(ntc.item_type, "Official report");
    assert_eq!(ntc.attribution, "GPW");
    assert_eq!(ntc.language, "pl");
    assert_eq!(ntc.body_text, "Official report body from GPW detail page.");
    assert_eq!(ntc.attachments.len(), 1);
    assert_eq!(ntc.attachments[0].label, "7_2026_oswiadczenie.pdf");

    assert!(visible_items
        .iter()
        .all(|item| item.title != "Untracked company report"));

    let unmatched_items = state
        .list_unmatched_source_items(ADAPTER_ID)
        .expect("unmatched source diagnostics should list");
    let untracked = unmatched_items
        .iter()
        .find(|item| item.title == "Untracked company report")
        .expect("unmatched ingested listing should be diagnosable");

    assert_eq!(untracked.company_name, "UNTRACKED S.A.");
}

#[test]
fn ingests_gpw_listing_by_registry_ticker_when_local_isin_is_missing() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "11B".to_owned(),
            display_name: "11 BIT STUDIOS S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");
    state
        .refresh_gpw_company_registry(
            &[registry_entry(
                "11B",
                "11 BIT STUDIOS SPÓŁKA AKCYJNA",
                "PL11BTS00015",
            )],
            "2026-05-31T12:00:00Z",
        )
        .expect("test registry should refresh");

    let result = state
        .ingest_gpw_report_listings(&[GpwReportListing {
            report_type: "Bieżący".to_owned(),
            system: "ESPI".to_owned(),
            report_number: "20/2026".to_owned(),
            company_ticker: String::new(),
            company_name: "11 BIT STUDIOS SPÓŁKA AKCYJNA".to_owned(),
            isin: "PL11BTS00015".to_owned(),
            title: "Informacja o zawarciu znaczącej umowy".to_owned(),
            detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=777777".to_owned(),
            published_at: "2026-05-30T17:13:31+02:00".to_owned(),
            fetched_at: "2026-05-30T17:30:00Z".to_owned(),
            dedupe_key: "gpw-espi-ebi:espi:PL11BTS00015:20/2026:2026-05-30T17:13:31+02:00"
                .to_owned(),
            body_text: None,
            attachments: Vec::new(),
        }])
        .expect("listing should ingest");

    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 0);

    let visible_items = state.list_feed_items().expect("feed items should list");
    let item = visible_items
        .iter()
        .find(|item| item.company == "GPW:11B")
        .expect("ticker-registry matched listing should be visible");

    assert_eq!(item.title, "Informacja o zawarciu znaczącej umowy");
}

#[test]
fn source_listing_match_can_use_future_exchange_registry_ticker() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .create_company(NewCompany {
            exchange: "XETRA".to_owned(),
            ticker: "SAP".to_owned(),
            display_name: "SAP SE".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("future exchange company should create");
    {
        let connection = state.checkout().expect("database connection");
        connection
            .execute(
                "
                    INSERT INTO source_adapters (
                        id,
                        display_name,
                        source_type,
                        fetch_mode,
                        enabled,
                        default_poll_interval_seconds
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                    ",
                (
                    "future-company-directory",
                    "Future Company Directory",
                    "company_registry",
                    "public_page",
                    1,
                    86_400,
                ),
            )
            .expect("future source adapter should insert");
        connection
            .execute(
                "
                    INSERT INTO company_registry_entries (
                        id,
                        exchange,
                        ticker,
                        qualified_ticker,
                        display_name,
                        isin,
                        source_adapter_id,
                        source_url,
                        fetched_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                    ",
                (
                    "registry_future_xetra_sap",
                    "XETRA",
                    "SAP",
                    "XETRA:SAP",
                    "SAP SE",
                    "DE0007164600",
                    "future-company-directory",
                    "https://example.test/xetra/sap",
                    "2026-05-31T12:00:00Z",
                ),
            )
            .expect("future registry row should insert");
    }

    let matched = {
        let connection = state.checkout().expect("database connection");
        find_company_for_exchange_listing(&connection, "XETRA", "", "DE0007164600")
            .expect("source listing should match")
            .expect("future registry should resolve source identifier")
    };

    assert_eq!(matched.qualified_ticker, "XETRA:SAP");
    assert_eq!(matched.match_type, "ticker");
}

#[test]
fn ingests_bankier_rss_items_and_matches_tracked_company_by_strong_signal() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    let result = state
        .ingest_bankier_rss_items(&sample_bankier_items())
        .expect("RSS items should ingest");

    assert_eq!(result.adapter_id, BANKIER_RSS_ADAPTER_ID);
    assert_eq!(result.items_fetched, 2);
    assert_eq!(result.items_created, 2);
    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 1);

    let visible_items = state.list_feed_items().expect("feed items should list");

    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].company, "GPW:CDR");
    assert_eq!(visible_items[0].item_type, "Public media");
    assert_eq!(visible_items[0].source, "Bankier Giełda RSS");
    assert_eq!(visible_items[0].attribution, "Bankier.pl");
    assert_eq!(
        visible_items[0].summary,
        "Inwestorzy obserwują CD Projekt po nowych informacjach."
    );

    let unmatched = state
        .list_unmatched_source_items(BANKIER_RSS_ADAPTER_ID)
        .expect("unmatched RSS item should be diagnosable");

    assert_eq!(unmatched.len(), 1);
    assert_eq!(
        unmatched[0].title,
        "Rynek czeka na decyzje banków centralnych"
    );
}

#[test]
fn ingestion_persists_cross_source_story_key_for_matched_items() {
    // The ingestion pipeline (ADR 0050) derives a canonical story key from the
    // matched company + day + title and persists it, so items from different
    // sources about the same event cluster together. Driven through the free
    // ingest functions so the persisted column can be read back directly.
    let mut connection = open_in_memory_database().expect("database should initialize");
    crate::storage::companies::create_company(
        &connection,
        NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        },
    )
    .expect("company should create");

    crate::storage::sources::ingest_bankier_rss_items(&mut connection, &sample_bankier_items())
        .expect("RSS items should ingest");

    // The matched item is keyed by company + publication day + title slug.
    let key: Option<String> = connection
        .query_row(
            "SELECT story_key FROM feed_items WHERE display_company = 'GPW:CDR'",
            [],
            |row| row.get(0),
        )
        .expect("matched feed item should exist");
    let key = key.expect("a matched item gets a story key");
    assert!(key.starts_with("story:GPW:CDR:2026-05-31:"), "{key}");

    // The unmatched item (no company) is not clustered.
    let null_keys: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM feed_items WHERE story_key IS NULL",
            [],
            |row| row.get(0),
        )
        .expect("count should run");
    assert_eq!(null_keys, 1);
}

#[test]
fn media_ingestion_matches_tracked_companies_from_future_exchanges() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .create_company(NewCompany {
            exchange: "XETRA".to_owned(),
            ticker: "SAP".to_owned(),
            display_name: "SAP SE".to_owned(),
            isin: Some("DE0007164600".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("future exchange company should create");

    let result = state
        .ingest_bankier_rss_items(&[BankierRssItem {
            title: "SAP SE zwiększa prognozy po mocnym kwartale".to_owned(),
            link: "https://www.bankier.pl/wiadomosc/sap-se-prognozy-900002.html".to_owned(),
            summary: "Inwestorzy obserwują SAP po publikacji wyników.".to_owned(),
            published_at: Some("2026-05-31T09:15:00+02:00".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            dedupe_key: "bankier-market-rss:bankier-900002".to_owned(),
        }])
        .expect("RSS item should ingest");

    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 0);

    let visible_items = state.list_feed_items().expect("feed items should list");

    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].company, "XETRA:SAP");
}

#[test]
fn end_to_end_pipeline_parses_ingests_dedupes_and_unifies_across_sources() {
    // ADR 0049 T7: the full ingestion seam in one go — a REAL adapter parse of a
    // checked-in sample, into real storage, out to the unified read model — proving
    // cross-source unification and dedup as a *pipeline*. This is distinct from the
    // isolated parse tests (T2 golden) and the per-source ingest tests above; it is
    // the only layer that proves the "many sources into one set" thesis composes.
    let state = AppState::new(open_in_memory_database().expect("database should initialize"));

    // Track two companies the real samples reference.
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "NTC".to_owned(),
            display_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
            isin: Some("PLECMNG00019".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("NTC company should create");
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("CDR company should create");

    // SOURCE 1: parse the real GPW ESPI/EBI listing sample, then ingest it. The
    // NEW TECH CAPITAL report matches the tracked company by ISIN.
    let gpw_listings = crate::source_adapters::gpw_espi_ebi::parse_report_listings(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gpw_espi_ebi_listing.html"
        )),
        "2026-05-30T17:30:00Z",
    )
    .expect("GPW ESPI/EBI sample should parse");
    state
        .ingest_gpw_report_listings(&gpw_listings)
        .expect("parsed GPW listings should ingest");

    // SOURCE 2: the canonical Bankier RSS market sample, matched to CD PROJEKT by
    // strong name signal — a different adapter feeding the same unified read model.
    state
        .ingest_bankier_rss_items(&sample_bankier_items())
        .expect("Bankier RSS items should ingest");

    // Unified read model: one feed carries the ISIN-matched GPW report AND the
    // signal-matched Bankier item — two different adapters, one set.
    let feed = state.list_feed_items().expect("feed should list");
    assert!(
        feed.iter().any(|item| item.company == "GPW:NTC"),
        "the GPW-sourced NTC report should appear in the unified feed: {feed:#?}"
    );
    assert!(
        feed.iter().any(|item| item.company == "GPW:CDR"),
        "the Bankier-sourced CD Projekt item should appear in the unified feed: {feed:#?}"
    );
    let sources: std::collections::BTreeSet<&str> =
        feed.iter().map(|item| item.source.as_str()).collect();
    assert!(
        sources.len() >= 2,
        "the feed should unify items from multiple distinct sources: {sources:?}"
    );
    let count_after_first = feed.len();

    // DEDUP: re-parse and re-ingest BOTH sources. Ingestion is idempotent on the
    // dedupe key, so the unified feed must not grow — the property that keeps
    // re-fetching many overlapping sources from multiplying the same story.
    let gpw_again = crate::source_adapters::gpw_espi_ebi::parse_report_listings(
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/samples/gpw_espi_ebi_listing.html"
        )),
        "2026-05-31T09:00:00Z",
    )
    .expect("GPW sample should re-parse");
    state
        .ingest_gpw_report_listings(&gpw_again)
        .expect("re-ingest should be idempotent");
    state
        .ingest_bankier_rss_items(&sample_bankier_items())
        .expect("re-ingest should be idempotent");
    let feed_after = state.list_feed_items().expect("feed should list");
    assert_eq!(
        feed_after.len(),
        count_after_first,
        "re-ingesting identical items across sources must not duplicate the unified feed"
    );
}

#[test]
fn bankier_rss_ingestion_does_not_match_company_name_inside_unrelated_words() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CMP".to_owned(),
            display_name: "COMP S.A.".to_owned(),
            isin: Some("PLCMP0000017".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    let result = state
        .ingest_bankier_rss_items(&[BankierRssItem {
            title: "Feerum wybrane do realizacji projektu Dandara w Egipcie za 15,4 mln USD"
                .to_owned(),
            link: "https://www.bankier.pl/wiadomosc/Feerum-wybrane-do-realizacji-projektu-Dandara-w-Egipcie-za-15-4-mln-USD-9149454.html"
                .to_owned(),
            summary: "Lokalnym partnerem jest The Egyptian Holding Company for Silos and Storage."
                .to_owned(),
            published_at: Some("2026-06-11T08:34:00+02:00".to_owned()),
            fetched_at: "2026-06-11T08:40:00Z".to_owned(),
            dedupe_key: "bankier-market-rss:feerum-dandara-9149454".to_owned(),
        }])
        .expect("RSS item should ingest");

    assert_eq!(result.items_matched, 0);
    assert_eq!(result.items_unmatched, 1);
    assert!(state
        .list_feed_items()
        .expect("feed items should list")
        .is_empty());

    let unmatched = state
        .list_unmatched_source_items(BANKIER_RSS_ADAPTER_ID)
        .expect("unmatched RSS item should be diagnosable");

    assert_eq!(unmatched.len(), 1);
    assert_eq!(
        unmatched[0].title,
        "Feerum wybrane do realizacji projektu Dandara w Egipcie za 15,4 mln USD"
    );
}

#[test]
fn bankier_rss_ingestion_updates_existing_item_by_source_url() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    {
        let connection = state.checkout().expect("database connection");
        connection
                .execute(
                    "
                    INSERT INTO feed_items (
                        id,
                        type,
                        source_adapter_id,
                        source_name,
                        source_url,
                        title,
                        summary,
                        language,
                        published_at,
                        fetched_at,
                        dedupe_key,
                        attribution,
                        display_company
                    ) VALUES (?1, 'Public media', ?2, ?3, ?4, ?5, '', 'pl', ?6, ?7, ?8, ?9, 'GPW:CDR')
                    ",
                    params![
                        "feed_bankier_old_title",
                        BANKIER_RSS_ADAPTER_ID,
                        BANKIER_RSS_DISPLAY_NAME,
                        "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html",
                        "&quot;Maluchy&quot; z nowym rekordem. CD Projekt rośnie po komentarzu zarządu",
                        "2026-05-31T09:15:00+02:00",
                        "2026-05-31T09:30:00Z",
                        "bankier-market-rss:old-title-derived-key",
                        BANKIER_RSS_ATTRIBUTION,
                    ],
                )
                .expect("old row should insert");
    }

    state
        .ingest_bankier_rss_items(&[BankierRssItem {
            title: "\"Maluchy\" z nowym rekordem. CD Projekt rośnie po komentarzu zarządu"
                .to_owned(),
            link: "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html".to_owned(),
            summary: "Zdekodowany opis.".to_owned(),
            published_at: Some("2026-05-31T09:15:00+02:00".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            dedupe_key: "bankier-market-rss:bankier-900001".to_owned(),
        }])
        .expect("RSS item should update existing row");

    let visible_items = state.list_feed_items().expect("feed items should list");

    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].id, "feed_bankier_old_title");
    assert_eq!(
        visible_items[0].title,
        "\"Maluchy\" z nowym rekordem. CD Projekt rośnie po komentarzu zarządu"
    );
    assert_eq!(visible_items[0].summary, "Zdekodowany opis.");
}

#[test]
fn bankier_rss_ingestion_skips_cross_source_media_duplicate() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");
    let bankier_item = sample_bankier_items()
        .into_iter()
        .next()
        .expect("sample Bankier item should exist");
    let duplicate_signature = media_duplicate_signature(
        &bankier_item,
        &[MediaMatchCompany {
            id: company.id.clone(),
            ticker: "CDR".to_owned(),
            qualified_ticker: "GPW:CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
        }],
    )
    .expect("matched media item should have duplicate signature");

    {
        let connection = state.checkout().expect("database connection");
        connection
            .execute(
                "
                    INSERT INTO source_adapters (
                        id,
                        display_name,
                        source_type,
                        fetch_mode,
                        enabled,
                        default_poll_interval_seconds
                    ) VALUES ('other-media-rss', 'Other Media RSS', 'public_media', 'rss', 1, 900)
                    ",
                [],
            )
            .expect("other media adapter should insert");
        connection
                .execute(
                    "
                    INSERT INTO feed_items (
                        id,
                        type,
                        source_adapter_id,
                        source_name,
                        source_url,
                        title,
                        summary,
                        language,
                        published_at,
                        fetched_at,
                        dedupe_key,
                        attribution,
                        display_company,
                        duplicate_signature
                    ) VALUES (?1, 'Public media', 'other-media-rss', 'Other Media RSS', ?2, ?3, ?4, 'pl', ?5, ?6, ?7, 'Other Media', 'GPW:CDR', ?8)
                    ",
                    params![
                        "feed_other_media_cdr",
                        "https://example.test/cd-projekt-komentarz",
                        &bankier_item.title,
                        &bankier_item.summary,
                        bankier_item.published_at.as_deref(),
                        &bankier_item.fetched_at,
                        "other-media-rss:cd-projekt-komentarz",
                        &duplicate_signature,
                    ],
                )
                .expect("other media item should insert");
        connection
            .execute(
                "
                    INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                    VALUES ('feed_other_media_cdr', ?1, 'media_signal')
                    ",
                [&company.id],
            )
            .expect("other media company match should insert");
    }

    let result = state
        .ingest_bankier_rss_items(&[bankier_item])
        .expect("Bankier duplicate should ingest");

    assert_eq!(result.items_fetched, 1);
    assert_eq!(result.items_created, 0);
    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 0);

    let visible_items = state.list_feed_items().expect("feed items should list");
    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].id, "feed_other_media_cdr");
    assert_eq!(visible_items[0].source, "Other Media RSS");
}

#[test]
fn stores_bankier_company_identifiers_for_tracked_companies() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    let targets_before = state
        .list_bankier_company_targets()
        .expect("targets should list");
    assert_eq!(targets_before.len(), 1);
    assert_eq!(targets_before[0].bankier_slug, None);
    assert_eq!(targets_before[0].bankier_tag_id, None);

    state
        .upsert_bankier_company_identifiers(
            &company.id,
            &BankierCompanyIdentifiers {
                slug: "CDPROJEKT".to_owned(),
                tag_id: "722".to_owned(),
            },
        )
        .expect("identifiers should store");

    let targets_after = state
        .list_bankier_company_targets()
        .expect("targets should list");
    assert_eq!(targets_after[0].bankier_slug.as_deref(), Some("CDPROJEKT"));
    assert_eq!(targets_after[0].bankier_tag_id.as_deref(), Some("722"));

    state
        .upsert_bankier_company_identifiers(
            &company.id,
            &BankierCompanyIdentifiers {
                slug: "CDPROJEKT".to_owned(),
                tag_id: "999".to_owned(),
            },
        )
        .expect("changed identifiers should update");

    let changed_targets = state
        .list_bankier_company_targets()
        .expect("targets should list");
    assert_eq!(changed_targets[0].bankier_tag_id.as_deref(), Some("999"));
}

#[test]
fn ingests_bankier_company_items_for_tracked_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    let result = state
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("Bankier company items should ingest");

    assert_eq!(result.adapter_id, BANKIER_COMPANY_ADAPTER_ID);
    assert_eq!(result.items_fetched, 1);
    assert_eq!(result.items_created, 1);
    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 0);
    assert_eq!(result.detail_items_attempted, 1);
    assert_eq!(result.detail_items_stored, 1);
    assert_eq!(result.detail_items_failed, 0);

    let visible_items = state.list_feed_items().expect("feed items should list");
    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].company, "GPW:CDR");
    assert_eq!(visible_items[0].item_type, "Official report");
    assert_eq!(visible_items[0].source, BANKIER_COMPANY_DISPLAY_NAME);
    assert_eq!(visible_items[0].attribution, "Bankier.pl");
    assert_eq!(visible_items[0].summary, "ESPI");
    assert_eq!(
        visible_items[0].body_text,
        "Official Bankier report body from the article page."
    );
    assert_eq!(visible_items[0].attachments.len(), 1);
    assert_eq!(visible_items[0].attachments[0].label, "report.xhtml");
}

#[test]
fn lists_bankier_company_detail_cached_urls() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    state
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("Bankier company item should ingest");

    let cached_urls = state
        .list_bankier_company_detail_cached_urls()
        .expect("cached URLs should list");

    assert_eq!(
            cached_urls,
            vec![
                "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                    .to_owned()
            ]
        );
}

#[test]
fn does_not_prune_existing_bankier_company_items_during_ingestion() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");
    {
        let connection = state.checkout().expect("database connection");
        connection
                .execute(
                    "
                    INSERT INTO feed_items (
                        id,
                        type,
                        source_adapter_id,
                        source_name,
                        source_url,
                        title,
                        language,
                        published_at,
                        fetched_at,
                        dedupe_key,
                        display_company
                    ) VALUES (?1, 'Official report', ?2, 'Bankier Company Komunikaty', ?3, ?4, 'pl', ?5, ?6, ?7, ?8)
                    ",
                    params![
                        "feed_bankier_company_komunikaty_legacy",
                        BANKIER_COMPANY_ADAPTER_ID,
                        "https://www.bankier.pl/wiadomosc/legacy.html",
                        "Legacy Bankier report",
                        "2026-05-20T10:00:00",
                        "2026-05-21T10:00:00Z",
                        "bankier-company-komunikaty:article:legacy",
                        company.qualified_ticker,
                    ],
                )
                .expect("legacy Bankier item should insert");
    }
    {
        let connection = state.checkout().expect("database connection");
        let legacy_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                [BANKIER_COMPANY_ADAPTER_ID],
                |row| row.get(0),
            )
            .expect("legacy item count should query");
        assert_eq!(legacy_count, 1);
    }

    state
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("Bankier company refresh should not prune existing rows");

    let visible_items = state.list_feed_items().expect("feed items should list");
    assert_eq!(visible_items.len(), 2);
    {
        let connection = state.checkout().expect("database connection");
        let legacy_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                [BANKIER_COMPANY_ADAPTER_ID],
                |row| row.get(0),
            )
            .expect("legacy item count should query");
        assert_eq!(legacy_count, 2);
    }
}

#[test]
fn bankier_company_items_do_not_duplicate_existing_gpw_report() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    state
        .ingest_gpw_report_listings(&[GpwReportListing {
            report_type: "Okresowy".to_owned(),
            system: "ESPI".to_owned(),
            report_number: "QSr 1/2026".to_owned(),
            company_ticker: "CDR".to_owned(),
            company_name: "CD PROJEKT S.A.".to_owned(),
            isin: "PLOPTTC00011".to_owned(),
            title: "Wyniki finansowe QSr 1/2026".to_owned(),
            detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=9141553".to_owned(),
            published_at: "2026-05-28T17:33:09+02:00".to_owned(),
            fetched_at: "2026-05-28T17:40:00Z".to_owned(),
            dedupe_key: "gpw-espi-ebi:espi:PLOPTTC00011:QSr 1/2026:2026-05-28T17:33:09+02:00"
                .to_owned(),
            body_text: None,
            attachments: Vec::new(),
        }])
        .expect("GPW report should ingest");

    let result = state
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("Bankier company duplicate should ingest");

    assert_eq!(result.items_fetched, 1);
    assert_eq!(result.items_created, 0);
    assert_eq!(result.items_matched, 1);
    assert_eq!(result.items_unmatched, 0);

    let visible_items = state.list_feed_items().expect("feed items should list");
    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].source, "GPW ESPI/EBI");
    assert_eq!(visible_items[0].title, "Wyniki finansowe QSr 1/2026");
}

#[test]
fn hides_bankier_company_item_after_matching_gpw_report_arrives() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    state
        .ingest_bankier_company_items(&sample_bankier_company_items(&company))
        .expect("Bankier company item should ingest first");
    assert_eq!(
        state
            .list_feed_items()
            .expect("feed items should list")
            .len(),
        1
    );

    state
        .ingest_gpw_report_listings(&[GpwReportListing {
            report_type: "Okresowy".to_owned(),
            system: "ESPI".to_owned(),
            report_number: "QSr 1/2026".to_owned(),
            company_ticker: "CDR".to_owned(),
            company_name: "CD PROJEKT S.A.".to_owned(),
            isin: "PLOPTTC00011".to_owned(),
            title: "Wyniki finansowe QSr 1/2026".to_owned(),
            detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=9141553".to_owned(),
            published_at: "2026-05-28T17:33:09+02:00".to_owned(),
            fetched_at: "2026-05-28T17:40:00Z".to_owned(),
            dedupe_key: "gpw-espi-ebi:espi:PLOPTTC00011:QSr 1/2026:2026-05-28T17:33:09+02:00"
                .to_owned(),
            body_text: Some("Official GPW body.".to_owned()),
            attachments: Vec::new(),
        }])
        .expect("GPW report should ingest");

    let visible_items = state.list_feed_items().expect("feed items should list");
    assert_eq!(visible_items.len(), 1);
    assert_eq!(visible_items[0].source, "GPW ESPI/EBI");
    assert_eq!(visible_items[0].body_text, "Official GPW body.");

    let stored_bankier_count: i64 = {
        let connection = state.checkout().expect("database connection");
        connection
            .query_row(
                "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                [BANKIER_COMPANY_ADAPTER_ID],
                |row| row.get(0),
            )
            .expect("stored Bankier count should query")
    };
    assert_eq!(stored_bankier_count, 1);
}

#[test]
fn prunes_old_unsaved_feed_items_only_when_maintenance_runs() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    {
        let connection = state.checkout().expect("database connection");
        for (id, published_at, saved) in [
            ("feed_old_unsaved", "2000-01-01T10:00:00Z", false),
            ("feed_old_saved", "2000-01-01T11:00:00Z", true),
            ("feed_recent_unsaved", "2999-05-31T10:00:00Z", false),
        ] {
            connection
                    .execute(
                        "
                        INSERT INTO feed_items (
                            id,
                            type,
                            source_adapter_id,
                            source_name,
                            source_url,
                            title,
                            language,
                            published_at,
                            fetched_at,
                            dedupe_key,
                            saved,
                            display_company
                        ) VALUES (?1, 'Public media', ?2, 'Bankier Giełda RSS', ?3, ?4, 'pl', ?5, ?5, ?6, ?7, ?8)
                        ",
                        params![
                            id,
                            BANKIER_RSS_ADAPTER_ID,
                            format!("https://www.bankier.pl/wiadomosc/{id}.html"),
                            id,
                            published_at,
                            id,
                            saved,
                            company.qualified_ticker,
                        ],
                    )
                    .expect("feed item should insert");
            connection
                .execute(
                    "
                        INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                        VALUES (?1, ?2, 'test')
                        ",
                    params![id, company.id],
                )
                .expect("feed item company should insert");
        }
    }

    let result = state
        .prune_old_feed_items(30)
        .expect("old feed items should prune");

    assert_eq!(result.retention_days, 30);
    assert_eq!(result.items_deleted, 1);

    let remaining_ids = {
        let connection = state.checkout().expect("database connection");
        let mut statement = connection
            .prepare("SELECT id FROM feed_items ORDER BY id")
            .expect("remaining feed query should prepare");
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("remaining feed query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("remaining feed ids should collect")
    };

    assert_eq!(
        remaining_ids,
        vec![
            "feed_old_saved".to_owned(),
            "feed_recent_unsaved".to_owned()
        ]
    );
}

#[test]
fn deletes_all_unsaved_feed_items_when_requested() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    {
        let connection = state.checkout().expect("database connection");
        for (id, saved) in [
            ("feed_old_unsaved", false),
            ("feed_recent_unsaved", false),
            ("feed_saved", true),
        ] {
            connection
                    .execute(
                        "
                        INSERT INTO feed_items (
                            id,
                            type,
                            source_adapter_id,
                            source_name,
                            source_url,
                            title,
                            language,
                            published_at,
                            fetched_at,
                            dedupe_key,
                            saved,
                            display_company
                        ) VALUES (?1, 'Public media', ?2, 'Bankier Giełda RSS', ?3, ?4, 'pl', '2026-05-31T10:00:00Z', '2026-05-31T10:00:00Z', ?5, ?6, ?7)
                        ",
                        params![
                            id,
                            BANKIER_RSS_ADAPTER_ID,
                            format!("https://www.bankier.pl/wiadomosc/{id}.html"),
                            id,
                            id,
                            saved,
                            company.qualified_ticker,
                        ],
                    )
                    .expect("feed item should insert");
            connection
                .execute(
                    "
                        INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                        VALUES (?1, ?2, 'test')
                        ",
                    params![id, company.id],
                )
                .expect("feed item company should insert");
            connection
                .execute(
                    "
                        INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
                        VALUES (?1, ?2, 'report.pdf', ?3, 0)
                        ",
                    params![
                        format!("attachment_{id}"),
                        id,
                        format!("https://www.bankier.pl/{id}.pdf")
                    ],
                )
                .expect("feed item attachment should insert");
        }

        connection
                .execute(
                    "
                    INSERT INTO ai_analysis_results (
                        id,
                        feed_item_id,
                        provider_id,
                        model,
                        summary,
                        significance,
                        reasoning,
                        language
                    ) VALUES ('analysis_unsaved', 'feed_old_unsaved', 'local', 'test', 'summary', 'medium', 'reasoning', 'en')
                    ",
                    [],
                )
                .expect("analysis result should insert");
        connection
            .execute(
                "
                    INSERT INTO ai_analysis_tags (ai_analysis_result_id, tag)
                    VALUES ('analysis_unsaved', 'important')
                    ",
                [],
            )
            .expect("analysis tag should insert");
        connection
                .execute(
                    "
                    INSERT INTO ai_analysis_source_references (id, ai_analysis_result_id, source_url, label)
                    VALUES ('analysis_reference_unsaved', 'analysis_unsaved', 'https://example.local/report', 'Report')
                    ",
                    [],
                )
                .expect("analysis source reference should insert");
    }

    let result = state
        .delete_unsaved_feed_items()
        .expect("unsaved feed items should delete");

    assert_eq!(result.items_deleted, 2);

    let (remaining_ids, attachment_count, company_link_count, analysis_count) = {
        let connection = state.checkout().expect("database connection");
        let mut statement = connection
            .prepare("SELECT id FROM feed_items ORDER BY id")
            .expect("remaining feed query should prepare");
        let remaining_ids = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("remaining feed query should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("remaining feed ids should collect");
        let attachment_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM feed_item_attachments", [], |row| {
                row.get(0)
            })
            .expect("attachment count should query");
        let company_link_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM feed_item_companies", [], |row| {
                row.get(0)
            })
            .expect("company link count should query");
        let analysis_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_analysis_results", [], |row| {
                row.get(0)
            })
            .expect("analysis count should query");

        (
            remaining_ids,
            attachment_count,
            company_link_count,
            analysis_count,
        )
    };

    assert_eq!(remaining_ids, vec!["feed_saved".to_owned()]);
    assert_eq!(attachment_count, 1);
    assert_eq!(company_link_count, 1);
    assert_eq!(analysis_count, 0);
}

#[test]
fn replaces_gpw_detail_attachments_when_accepted_detail_has_none() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "NTC".to_owned(),
            display_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
            isin: Some("PLECMNG00019".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("tracked company should create");

    let listing = GpwReportListing {
        report_type: "Bieżący".to_owned(),
        system: "ESPI".to_owned(),
        report_number: "7/2026".to_owned(),
        company_ticker: "NTC".to_owned(),
        company_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
        isin: "PLECMNG00019".to_owned(),
        title: "Oświadczenie w sprawie formy przekazywania raportów kwartalnych.".to_owned(),
        detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=123456".to_owned(),
        published_at: "2026-05-30T17:13:31+02:00".to_owned(),
        fetched_at: "2026-05-30T17:30:00Z".to_owned(),
        dedupe_key: "gpw-espi-ebi:espi:PLECMNG00019:7/2026:2026-05-30T17:13:31+02:00".to_owned(),
        body_text: Some("Official report body from GPW detail page.".to_owned()),
        attachments: vec![GpwReportAttachment {
            label: "7_2026_oswiadczenie.pdf".to_owned(),
            url: "https://www.gpw.pl/pub/GPW/ESPI/2026/7_2026_oswiadczenie.pdf".to_owned(),
        }],
    };

    state
        .ingest_gpw_report_listings(std::slice::from_ref(&listing))
        .expect("listing should ingest");

    let mut replacement = listing;
    replacement.body_text = Some("Updated official report body from GPW detail page.".to_owned());
    replacement.attachments = Vec::new();
    state
        .ingest_gpw_report_listings(&[replacement])
        .expect("replacement listing should ingest");

    let feed_items = state.list_feed_items().expect("feed items should list");
    let ntc = feed_items
        .iter()
        .find(|item| item.company == "GPW:NTC")
        .expect("matched GPW listing should be visible");

    assert!(ntc.attachments.is_empty());
}

#[test]
fn records_successful_zero_item_gpw_refresh() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let result = state
        .ingest_gpw_report_listings(&[])
        .expect("zero-item source refresh should record success");

    assert_eq!(result.items_fetched, 0);
    assert_eq!(result.items_created, 0);
    assert_eq!(result.items_matched, 0);
    assert_eq!(result.items_unmatched, 0);
    assert!(result.fetched_at.is_some());

    let adapters = state
        .list_source_adapters_with_developer(true)
        .expect("source adapters should list");
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("GPW adapter should exist");

    assert_eq!(adapter.last_success_at, result.fetched_at);
    assert!(adapter.last_error_at.is_none());
    assert!(adapter.last_error.is_none());
    assert_eq!(adapter.last_items_fetched, Some(0));
    assert_eq!(adapter.last_items_created, Some(0));
    assert_eq!(adapter.last_items_matched, Some(0));
    assert_eq!(adapter.last_items_unmatched, Some(0));
}
