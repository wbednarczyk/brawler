use super::*;

#[test]
fn lists_seeded_source_adapters() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    let adapters = state
        .list_source_adapters()
        .expect("source adapters should list");

    assert_eq!(adapters.len(), 11);

    let report_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "gpw-espi-ebi")
        .expect("GPW report adapter should exist");
    assert_eq!(report_adapter.display_name, "GPW ESPI/EBI");
    assert_eq!(report_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!report_adapter.enabled);

    let registry_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "gpw-company-registry")
        .expect("GPW registry adapter should exist");
    assert_eq!(registry_adapter.display_name, "GPW Company Registry");
    assert_eq!(registry_adapter.markets, vec!["GPW".to_owned()]);
    assert!(registry_adapter.enabled);

    let bankier_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == BANKIER_RSS_ADAPTER_ID)
        .expect("Bankier RSS adapter should exist");
    assert_eq!(bankier_adapter.display_name, "Bankier Giełda RSS");
    assert_eq!(bankier_adapter.source_type, "public_media");
    assert_eq!(bankier_adapter.fetch_mode, "rss");
    assert_eq!(bankier_adapter.source_url, BANKIER_RSS_SOURCE_URL);
    assert_eq!(bankier_adapter.markets, vec!["GPW".to_owned()]);
    assert!(bankier_adapter.enabled);

    let bankier_company_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == BANKIER_COMPANY_ADAPTER_ID)
        .expect("Bankier company adapter should exist");
    assert_eq!(
        bankier_company_adapter.display_name,
        "Bankier Company Komunikaty"
    );
    assert_eq!(bankier_company_adapter.source_type, "official_report");
    assert_eq!(bankier_company_adapter.fetch_mode, "public_json");
    assert_eq!(
        bankier_company_adapter.source_url,
        BANKIER_COMPANY_SOURCE_URL
    );
    assert_eq!(bankier_company_adapter.markets, vec!["GPW".to_owned()]);
    assert!(bankier_company_adapter.enabled);
    assert!(bankier_company_adapter
        .policy_note
        .contains("active v1 official-report source"));

    let gpw_events_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == GPW_MARKET_EVENTS_ADAPTER_ID)
        .expect("GPW market events adapter should exist");
    assert_eq!(gpw_events_adapter.display_name, "GPW Market Events RSS");
    assert_eq!(gpw_events_adapter.source_type, "official_calendar");
    assert_eq!(gpw_events_adapter.fetch_mode, "rss");
    assert_eq!(gpw_events_adapter.source_url, GPW_MARKET_EVENTS_SOURCE_URL);
    assert_eq!(gpw_events_adapter.markets, vec!["GPW".to_owned()]);
    assert!(gpw_events_adapter.enabled);
    assert!(gpw_events_adapter.policy_note.contains("exact ticker"));

    let bankier_calendar_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "bankier-kalendarium-html")
        .expect("Bankier Kalendarium adapter should exist");
    assert_eq!(bankier_calendar_adapter.display_name, "Bankier Kalendarium");
    assert_eq!(bankier_calendar_adapter.source_type, "public_calendar");
    assert_eq!(bankier_calendar_adapter.fetch_mode, "public_page");
    assert_eq!(
        bankier_calendar_adapter.source_url,
        BANKIER_CALENDAR_SOURCE_URL
    );
    assert_eq!(bankier_calendar_adapter.markets, vec!["GPW".to_owned()]);
    assert!(bankier_calendar_adapter.enabled);
    assert!(bankier_calendar_adapter
        .policy_note
        .contains("Active M9 public calendar source"));

    let strefa_calendar_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "strefa-report-calendar")
        .expect("Strefa report calendar placeholder should exist");
    assert_eq!(
        strefa_calendar_adapter.display_name,
        "Strefa Report Calendar"
    );
    assert_eq!(strefa_calendar_adapter.source_type, "public_calendar");
    assert_eq!(strefa_calendar_adapter.fetch_mode, "public_page");
    assert_eq!(
        strefa_calendar_adapter.source_url,
        STREFA_REPORT_CALENDAR_SOURCE_URL
    );
    assert_eq!(strefa_calendar_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!strefa_calendar_adapter.enabled);
    assert!(strefa_calendar_adapter
        .policy_note
        .contains("periodic-report publication dates"));

    let money_calendar_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "money-calendar")
        .expect("Money calendar placeholder should exist");
    assert_eq!(money_calendar_adapter.display_name, "Money Calendar");
    assert_eq!(money_calendar_adapter.source_type, "public_calendar");
    assert_eq!(money_calendar_adapter.fetch_mode, "public_page");
    assert_eq!(money_calendar_adapter.source_url, MONEY_CALENDAR_SOURCE_URL);
    assert_eq!(money_calendar_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!money_calendar_adapter.enabled);
    assert!(money_calendar_adapter
        .policy_note
        .contains("Fallback/cross-check candidate"));

    let bankier_firma_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "bankier-firma-rss")
        .expect("Bankier Firma RSS placeholder should exist");
    assert_eq!(bankier_firma_adapter.display_name, "Bankier Firma RSS");
    assert_eq!(bankier_firma_adapter.source_type, "public_media");
    assert_eq!(bankier_firma_adapter.fetch_mode, "rss");
    assert_eq!(
        bankier_firma_adapter.source_url,
        BANKIER_FIRMA_RSS_SOURCE_URL
    );
    assert!(!bankier_firma_adapter.enabled);
    assert!(bankier_firma_adapter
        .policy_note
        .contains("matching-quality tests"));

    let bankier_wiadomosci_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == "bankier-wiadomosci-rss")
        .expect("Bankier Wiadomosci RSS placeholder should exist");
    assert_eq!(
        bankier_wiadomosci_adapter.display_name,
        "Bankier Wiadomosci RSS"
    );
    assert_eq!(bankier_wiadomosci_adapter.source_type, "public_media");
    assert_eq!(bankier_wiadomosci_adapter.fetch_mode, "rss");
    assert_eq!(
        bankier_wiadomosci_adapter.source_url,
        BANKIER_WIADOMOSCI_RSS_SOURCE_URL
    );
    assert!(!bankier_wiadomosci_adapter.enabled);
    assert!(bankier_wiadomosci_adapter
        .policy_note
        .contains("unsuitable for default v1 ingestion"));

    let portal_analiz_adapter = adapters
        .iter()
        .find(|adapter| adapter.id == PORTAL_ANALIZ_ADAPTER_ID)
        .expect("Portal Analiz placeholder should exist");
    assert_eq!(portal_analiz_adapter.display_name, "Portal Analiz");
    assert_eq!(portal_analiz_adapter.source_type, "authenticated_research");
    assert_eq!(portal_analiz_adapter.fetch_mode, "authenticated");
    assert_eq!(portal_analiz_adapter.source_url, PORTAL_ANALIZ_SOURCE_URL);
    assert_eq!(portal_analiz_adapter.markets, vec!["GPW".to_owned()]);
    assert!(!portal_analiz_adapter.enabled);
    assert!(portal_analiz_adapter
        .policy_note
        .contains("Late-v1 planned"));
}

#[test]
fn records_source_adapter_error_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .record_source_adapter_error(ADAPTER_ID, "network timeout")
        .expect("source adapter error should record");

    let adapters = state
        .list_source_adapters()
        .expect("source adapters should list");
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("GPW adapter should exist");

    assert_eq!(adapter.last_error.as_deref(), Some("network timeout"));
    assert!(adapter.last_error_at.is_some());
}

#[test]
fn records_source_adapter_attempt_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);

    state
        .record_source_adapter_attempt(ADAPTER_ID, "scheduler")
        .expect("source adapter attempt should record");

    let adapters = state
        .list_source_adapters()
        .expect("source adapters should list");
    let adapter = adapters
        .iter()
        .find(|adapter| adapter.id == ADAPTER_ID)
        .expect("GPW adapter should exist");

    assert!(adapter.last_attempt_at.is_some());
    assert_eq!(adapter.last_trigger.as_deref(), Some("scheduler"));
}
