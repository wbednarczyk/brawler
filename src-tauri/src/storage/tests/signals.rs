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

fn espi_item(company: &Company, article_id: &str, title: &str) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: title.to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-{article_id}.html"),
        summary: "Komunikat ESPI/EBI".to_owned(),
        published_at: Some("2026-05-28T17:33:09".to_owned()),
        fetched_at: "2026-05-31T10:00:00Z".to_owned(),
        article_id: article_id.to_owned(),
        pub_id: 3,
        dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
        duplicate_signature: format!("official-secondary:GPW:CDR:{article_id}"),
        body_text: Some("Official Bankier report body.".to_owned()),
        attachments: Vec::new(),
        detail_fetch_attempted: true,
    }
}

fn espi_item_with_body(
    company: &Company,
    article_id: &str,
    title: &str,
    body: &str,
) -> BankierCompanyItem {
    let mut item = espi_item(company, article_id, title);
    item.body_text = Some(body.to_owned());
    item
}

fn list_proposed_events(state: &AppState, company_id: &str) -> Vec<CompanyEvent> {
    state
        .list_company_events(CompanyEventListInput {
            mode: Some("all".to_owned()),
            company_id: Some(company_id.to_owned()),
            ..Default::default()
        })
        .expect("events should list")
        .into_iter()
        .filter(|event| event.status == "proposed")
        .collect()
}

#[test]
fn confirmed_dividend_signal_derives_proposed_event_and_confirms() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![espi_item_with_body(
        &company,
        "9200001",
        "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
        "Zarząd rekomenduje wypłatę dywidendy. Dzień dywidendy: 10.07.2030 r. \
         Termin wypłaty dywidendy: 24.07.2030 r.",
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should classify and derive");

    let proposed = list_proposed_events(&state, &company.id);
    assert_eq!(proposed.len(), 1, "one proposed dividend event expected");
    assert_eq!(proposed[0].event_type, "dividend");
    assert_eq!(proposed[0].event_date, "2030-07-24");
    assert_eq!(proposed[0].source_type, "derived_signal");

    // The signal now links to its derived event.
    let signals = state
        .list_company_signals(CompanySignalListInput {
            company_id: Some(company.id.clone()),
            ..Default::default()
        })
        .expect("signals should list");
    assert!(signals
        .iter()
        .any(|s| s.category == "dividend" && s.derived_event_id.is_some()));

    // Confirming places it on the calendar.
    state
        .confirm_derived_event(&proposed[0].id, true)
        .expect("confirm should succeed");
    assert!(list_proposed_events(&state, &company.id).is_empty());

    // Re-ingesting does not duplicate the derived event.
    state
        .ingest_bankier_company_items(&items)
        .expect("re-ingestion");
    let all = state
        .list_company_events(CompanyEventListInput {
            mode: Some("all".to_owned()),
            company_id: Some(company.id.clone()),
            event_type: Some("dividend".to_owned()),
            ..Default::default()
        })
        .expect("events should list");
    assert_eq!(all.len(), 1, "derivation must be idempotent");
}

#[test]
fn general_meeting_derivation_can_be_rejected() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![espi_item_with_body(
        &company,
        "9200002",
        "Zwołanie Zwyczajnego Walnego Zgromadzenia CD PROJEKT S.A.",
        "Zarząd zwołuje Zwyczajne Walne Zgromadzenie na dzień 28 czerwca 2030 r. o godzinie 11:00.",
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should classify and derive");

    let proposed = list_proposed_events(&state, &company.id);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].event_type, "shareholder_meeting");
    assert_eq!(proposed[0].event_date, "2030-06-28");

    // Rejecting discards the proposed event and clears the link.
    state
        .confirm_derived_event(&proposed[0].id, false)
        .expect("reject should succeed");
    assert!(list_proposed_events(&state, &company.id).is_empty());

    let signals = state
        .list_company_signals(CompanySignalListInput {
            company_id: Some(company.id.clone()),
            ..Default::default()
        })
        .expect("signals should list");
    assert!(signals.iter().all(|s| s.derived_event_id.is_none()));
}

#[test]
fn dividend_without_a_date_derives_no_event() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![espi_item_with_body(
        &company,
        "9200003",
        "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
        "Zarząd rekomenduje przeznaczenie zysku na wypłatę dywidendy w wysokości 1,50 zł na akcję.",
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should classify");

    assert!(
        list_proposed_events(&state, &company.id).is_empty(),
        "no event without a parseable date (never guess)"
    );
}

#[test]
fn rule_classifier_produces_confirmed_typed_signals_for_real_filings() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![
        espi_item(
            &company,
            "9000001",
            "Powiadomienie o transakcjach na akcjach CD PROJEKT S.A. - art. 19 ust. 1 MAR",
        ),
        espi_item(
            &company,
            "9000002",
            "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
        ),
        espi_item(
            &company,
            "9000003",
            "Zwołanie Zwyczajnego Walnego Zgromadzenia CD PROJEKT S.A.",
        ),
        espi_item(
            &company,
            "9000004",
            "Skup akcji własnych - informacja o nabyciu",
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

    let categories: std::collections::HashSet<_> = signals
        .iter()
        .map(|signal| signal.category.as_str())
        .collect();
    assert!(categories.contains("insider_transaction"));
    assert!(categories.contains("dividend"));
    assert!(categories.contains("general_meeting"));
    assert!(categories.contains("own_shares"));
    assert!(signals
        .iter()
        .all(|signal| signal.classified_by == "rule" && signal.status == "confirmed"));
    assert!(signals.iter().all(|signal| signal.confidence > 0.0));
}

#[test]
fn auditor_opinion_signal_classifies_qualified_and_going_concern_filings() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![
        espi_item(
            &company,
            "9100001",
            "Raport niezależnego biegłego rewidenta z zastrzeżeniem",
        ),
        espi_item(
            &company,
            "9100002",
            "Odmowa wyrażenia opinii przez biegłego rewidenta",
        ),
        espi_item(
            &company,
            "9100003",
            "Istotna niepewność co do kontynuacji działalności Spółki",
        ),
        // A plain dividend recommendation must NOT be typed as an auditor opinion.
        espi_item(
            &company,
            "9100004",
            "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
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

    let auditor: Vec<_> = signals
        .iter()
        .filter(|s| s.category == "auditor_opinion")
        .collect();
    assert_eq!(
        auditor.len(),
        3,
        "the three auditor red-flag filings classify to auditor_opinion"
    );
    assert!(auditor
        .iter()
        .all(|s| s.classified_by == "rule" && s.status == "confirmed" && s.confidence > 0.0));

    // The dividend filing keeps its own category, never auditor_opinion.
    assert!(
        signals.iter().any(|s| s.category == "dividend"),
        "the dividend filing still classifies to dividend"
    );
    assert!(
        !signals
            .iter()
            .any(|s| s.category == "auditor_opinion" && s.title.contains("dywidend")),
        "a dividend filing must not be typed as an auditor opinion"
    );
}

#[test]
fn unmatched_filing_produces_no_signal_under_conservative_posture() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![espi_item(
        &company,
        "9000010",
        "Zmiana adresu siedziby spółki oraz danych rejestrowych",
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should run");

    let signals = state
        .list_company_signals(CompanySignalListInput::default())
        .expect("signals should list");
    assert!(
        signals.is_empty(),
        "an unrecognized filing must not be assigned a guessed category"
    );
}

#[test]
fn classification_is_idempotent_across_reingestion() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![espi_item(
        &company,
        "9000020",
        "Rekomendacja Zarządu w sprawie wypłaty dywidendy",
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("first ingestion should classify");
    state
        .ingest_bankier_company_items(&items)
        .expect("second ingestion should be idempotent");

    let signals = state
        .list_company_signals(CompanySignalListInput::default())
        .expect("signals should list");
    assert_eq!(signals.len(), 1, "re-ingestion must not duplicate signals");
    assert_eq!(signals[0].category, "dividend");
    assert_eq!(signals[0].signal_date.as_deref(), Some("2026-05-28"));
}

#[test]
fn ai_proposed_signal_confirm_and_reject_flow() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    // An ambiguous filing the rules leave unknown; the AI fallback proposes types.
    let items = vec![
        espi_item(
            &company,
            "9000040",
            "Pozostałe informacje korporacyjne spółki",
        ),
        espi_item(&company, "9000041", "Inne zdarzenia wymagające ujawnienia"),
    ];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should run");
    assert!(
        state
            .list_company_signals(CompanySignalListInput::default())
            .expect("list")
            .is_empty(),
        "ambiguous filings produce no rule signal"
    );

    let feed_items = state.list_feed_items().expect("feed items list");
    let first = feed_items
        .iter()
        .find(|item| item.title.contains("Pozostałe"))
        .expect("first ambiguous feed item");
    let second = feed_items
        .iter()
        .find(|item| item.title.contains("Inne zdarzenia"))
        .expect("second ambiguous feed item");

    let created = state
        .propose_company_signal(ProposedSignalInput {
            feed_item_id: first.id.clone(),
            company_id: company.id.clone(),
            category: "guidance_change".to_owned(),
            confidence: 0.72,
            signal_date: Some("2026-05-28".to_owned()),
            provider_id: "gemini".to_owned(),
            model_id: "gemini-2.5-pro".to_owned(),
        })
        .expect("proposal should be created");
    assert!(created);
    state
        .propose_company_signal(ProposedSignalInput {
            feed_item_id: second.id.clone(),
            company_id: company.id.clone(),
            category: "significant_contract".to_owned(),
            confidence: 0.66,
            signal_date: Some("2026-05-28".to_owned()),
            provider_id: "gemini".to_owned(),
            model_id: "gemini-2.5-pro".to_owned(),
        })
        .expect("second proposal should be created");

    let proposed = state
        .list_company_signals(CompanySignalListInput {
            status: Some("proposed".to_owned()),
            ..Default::default()
        })
        .expect("list proposals");
    assert_eq!(proposed.len(), 2);
    assert!(proposed
        .iter()
        .all(|signal| signal.classified_by == "ai"
            && signal.provider_id.as_deref() == Some("gemini")));

    // Confirm the first; it transitions to confirmed with no derived event (v0.40).
    let confirmed = state
        .confirm_company_signal(&proposed[0].id)
        .expect("confirm should succeed");
    assert_eq!(confirmed.status, "confirmed");
    assert!(confirmed.derived_event_id.is_none());

    // Reject the second; it is discarded entirely.
    state
        .reject_company_signal(&proposed[1].id)
        .expect("reject should succeed");

    let remaining = state
        .list_company_signals(CompanySignalListInput::default())
        .expect("list");
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].status, "confirmed");
}

#[test]
fn confirmed_signals_surface_in_research_timeline_and_high_signal_creates_reminder() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![
        espi_item(
            &company,
            "9000050",
            "Powiadomienie o transakcjach, o których mowa w art. 19 ust. 1 MAR",
        ),
        espi_item(
            &company,
            "9000051",
            "Rekomendacja Zarządu w sprawie wypłaty dywidendy",
        ),
    ];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should classify");

    // Confirmed signals surface as company_signal evidence in the research timeline.
    let timeline = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            evidence_types: Some(vec!["company_signal".to_owned()]),
            changed_since_review_only: None,
            limit: Some(50),
        })
        .expect("timeline should list");
    assert!(timeline.items.len() >= 2);
    assert!(timeline
        .items
        .iter()
        .all(|item| item.evidence_type == "company_signal"));

    // The high-signal insider transaction drives a signal_review reminder; the
    // dividend (not high-signal) does not.
    let reminders = state
        .list_research_reminders(ResearchReminderListInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            status: Some("open".to_owned()),
        })
        .expect("reminders should list");
    let signal_reminders: Vec<_> = reminders
        .iter()
        .filter(|reminder| reminder.reminder_kind == "signal_review")
        .collect();
    assert_eq!(signal_reminders.len(), 1);
    assert_eq!(
        signal_reminders[0].source_type.as_deref(),
        Some("company_signal")
    );
}

#[test]
fn list_filters_by_category_and_status() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let items = vec![
        espi_item(&company, "9000031", "Skup akcji własnych"),
        espi_item(
            &company,
            "9000032",
            "Rekomendacja Zarządu w sprawie wypłaty dywidendy",
        ),
    ];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should classify");

    let own_shares = state
        .list_company_signals(CompanySignalListInput {
            category: Some("own_shares".to_owned()),
            ..Default::default()
        })
        .expect("signals should list");
    assert_eq!(own_shares.len(), 1);
    assert_eq!(own_shares[0].category, "own_shares");

    let proposed = state
        .list_company_signals(CompanySignalListInput {
            status: Some("proposed".to_owned()),
            ..Default::default()
        })
        .expect("signals should list");
    assert!(
        proposed.is_empty(),
        "rule signals are confirmed, not proposed"
    );
}

// ---- Unclassified-filings triage (v0.60 M4, ADR 0088 dec. 4) ----------------

/// Seed a matched 'Public media' feed item that must NEVER surface as an
/// unclassified filing (the bucket is official filings only).
fn seed_media_item(state: &AppState, company: &Company, id: &str) {
    let connection = state.checkout().expect("connection");
    connection
        .execute(
            "
            INSERT INTO feed_items (
                id, type, source_adapter_id, source_name, source_url, title,
                summary, language, published_at, fetched_at, dedupe_key,
                attribution, display_company
            ) VALUES (?1, 'Public media', 'bankier-market-rss', 'Bankier RSS',
                      ?2, ?3, '', 'pl', '2026-05-30T09:00:00Z',
                      '2026-05-30T09:30:00Z', ?4, 'Bankier', ?5)
            ",
            params![
                id,
                format!("https://example.test/{id}"),
                "CD Projekt w mediach — komentarz analityka",
                format!("media:{id}"),
                company.qualified_ticker,
            ],
        )
        .expect("media feed item inserts");
    connection
        .execute(
            "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
             VALUES (?1, ?2, 'ticker')",
            params![id, company.id],
        )
        .expect("media company match inserts");
}

#[test]
fn unclassified_filings_lists_official_reports_without_a_signal() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    // One official filing the rule classifier cannot place (matches no pattern),
    // one it can (dividend), and one 'Public media' item.
    state
        .ingest_bankier_company_items(&[
            espi_item(
                &company,
                "9300001",
                "Zmiana adresu strony internetowej Spółki",
            ),
            espi_item(
                &company,
                "9300002",
                "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
            ),
        ])
        .expect("ingestion should classify");
    seed_media_item(&state, &company, "feed_media_1");

    let unclassified = state
        .list_unclassified_filings(None, 50)
        .expect("unclassified filings should list");

    // Exactly the unmatched official filing appears — never the classified
    // dividend filing, never the media item.
    assert_eq!(
        unclassified.len(),
        1,
        "only the unclassifiable official filing surfaces: {unclassified:?}"
    );
    let filing = &unclassified[0];
    assert_eq!(filing.title, "Zmiana adresu strony internetowej Spółki");
    assert_eq!(filing.company_id, company.id);
    assert!(
        !unclassified
            .iter()
            .any(|f| f.title.contains("dywidend") || f.title.contains("mediach")),
        "classified + media items are excluded"
    );

    // The company filter narrows to the same company; an unknown company is empty.
    assert_eq!(
        state
            .list_unclassified_filings(Some(&company.id), 50)
            .expect("scoped list")
            .len(),
        1
    );
    assert!(state
        .list_unclassified_filings(Some("company_absent"), 50)
        .expect("scoped list")
        .is_empty());
}

#[test]
fn classify_filing_creates_a_confirmed_signal_and_clears_the_bucket() {
    use crate::storage::ClassifyFilingOutcome;

    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .ingest_bankier_company_items(&[espi_item(
            &company,
            "9400001",
            "Zawiadomienie o zmianie adresu Spółki",
        )])
        .expect("ingestion should classify");

    let feed_item_id = state
        .list_unclassified_filings(None, 50)
        .expect("bucket")
        .first()
        .expect("one unclassified filing")
        .feed_item_id
        .clone();

    let signal = match state
        .classify_filing_outcome(&feed_item_id, "significant_contract")
        .expect("classification should create a signal")
    {
        ClassifyFilingOutcome::Created(signal) => signal,
        other => panic!("expected a created signal, got {other:?}"),
    };
    assert_eq!(signal.category, "significant_contract");
    assert_eq!(signal.status, "confirmed");
    // Honest provenance: an agent-authored classification over the MCP triage
    // tool is `agent`, never `rule` (the deterministic classifier) — ADR 0088.
    assert_eq!(signal.classified_by, "agent");
    assert_eq!(signal.feed_item_id, feed_item_id);
    assert_eq!(signal.company_id, company.id);

    // The filing has left the unclassified bucket.
    assert!(state
        .list_unclassified_filings(None, 50)
        .expect("bucket")
        .is_empty());
    // The signal is readable through the normal signals list.
    assert!(state
        .list_company_signals(CompanySignalListInput {
            company_id: Some(company.id.clone()),
            ..Default::default()
        })
        .expect("signals")
        .iter()
        .any(|s| s.feed_item_id == feed_item_id && s.category == "significant_contract"));
}

#[test]
fn classify_filing_rejects_unknown_category_non_official_and_already_classified() {
    use crate::storage::ClassifyFilingOutcome;

    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .ingest_bankier_company_items(&[espi_item(
            &company,
            "9500001",
            "Zawiadomienie o zmianie adresu Spółki",
        )])
        .expect("ingestion");
    seed_media_item(&state, &company, "feed_media_reject");

    let feed_item_id = state
        .list_unclassified_filings(None, 50)
        .expect("bucket")
        .first()
        .expect("one filing")
        .feed_item_id
        .clone();

    // Unknown category is rejected before any write.
    assert!(matches!(
        state
            .classify_filing_outcome(&feed_item_id, "not_a_real_category")
            .expect("query"),
        ClassifyFilingOutcome::UnknownCategory
    ));
    // A 'Public media' item is not an official filing.
    assert!(matches!(
        state
            .classify_filing_outcome("feed_media_reject", "dividend")
            .expect("query"),
        ClassifyFilingOutcome::NotAnOfficialFiling
    ));
    // A missing feed item is not found.
    assert!(matches!(
        state
            .classify_filing_outcome("feed_absent", "dividend")
            .expect("query"),
        ClassifyFilingOutcome::FeedItemNotFound
    ));

    // First classification succeeds; a second is a conflict.
    assert!(matches!(
        state
            .classify_filing_outcome(&feed_item_id, "dividend")
            .expect("first classification"),
        ClassifyFilingOutcome::Created(_)
    ));
    // A second classification of the same filing — any valid category — is a
    // conflict: the filing already has a signal (it left the bucket).
    assert!(matches!(
        state
            .classify_filing_outcome(&feed_item_id, "own_shares")
            .expect("query"),
        ClassifyFilingOutcome::AlreadyClassified
    ));
}
