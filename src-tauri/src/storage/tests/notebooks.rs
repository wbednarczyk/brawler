use super::*;

#[test]
fn creates_and_lists_notebook_entries_for_company() {
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
        .expect("company should be created");

    let entry = state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Management claim about release schedule".to_owned(),
            body: "Management said the next milestone should happen in two quarters.".to_owned(),
            body_format: None,
            tags: vec!["Product".to_owned(), " management-guidance ".to_owned()],
            kind: "claim".to_owned(),
            claim_status: Some("open".to_owned()),
            event_date: Some("2026-05-29".to_owned()),
            follow_up_after: Some("2026-Q4".to_owned()),
            follow_up_date: Some("2026-11-30".to_owned()),
            origins: vec![NewNotebookOrigin {
                source_type: "feed_item".to_owned(),
                source_id: Some("feed_sample_cdr_report".to_owned()),
                source_url: Some("https://www.gpw.pl/komunikaty".to_owned()),
                label: Some("GPW report".to_owned()),
            }],
        })
        .expect("notebook entry should be created");

    let entries = state
        .list_notebook_entries(&company.id)
        .expect("notebook entries should list");

    assert_eq!(entry.body_format, "markdown");
    assert_eq!(entry.kind, "claim");
    assert_eq!(entry.claim_status.as_deref(), Some("open"));
    assert_eq!(entry.tags, vec!["management-guidance", "product"]);
    assert_eq!(entry.origins.len(), 1);
    assert_eq!(entry.origins[0].source_type, "feed_item");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, entry.id);

    let updated = state
        .update_notebook_entry(NotebookEntryUpdate {
            id: entry.id.clone(),
            title: "Updated release schedule claim".to_owned(),
            body: "Management clarified the next milestone date.".to_owned(),
            tags: vec!["product".to_owned(), "clarified".to_owned()],
            kind: "claim".to_owned(),
            claim_status: Some("unknown".to_owned()),
            event_date: Some("2026-05-29".to_owned()),
            follow_up_after: Some("2026-Q3".to_owned()),
            follow_up_date: None,
        })
        .expect("notebook entry should update");

    assert_eq!(updated.title, "Updated release schedule claim");
    assert_eq!(
        updated.body,
        "Management clarified the next milestone date."
    );
    assert_eq!(updated.claim_status.as_deref(), Some("unknown"));
    assert_eq!(updated.follow_up_after.as_deref(), Some("2026-Q3"));
    assert_eq!(updated.tags, vec!["clarified", "product"]);
    assert_eq!(updated.origins.len(), 1);
    assert_eq!(updated.origins[0].source_type, "feed_item");
    assert_eq!(
        updated.origins[0].source_id.as_deref(),
        Some("feed_sample_cdr_report")
    );
    assert_eq!(
        updated.origins[0].source_url.as_deref(),
        Some("https://www.gpw.pl/komunikaty")
    );
    assert_eq!(updated.origins[0].label.as_deref(), Some("GPW report"));

    state
        .delete_notebook_entry(&updated.id)
        .expect("notebook entry should delete");

    let entries_after_delete = state
        .list_notebook_entries(&company.id)
        .expect("notebook entries should list after delete");

    assert!(entries_after_delete.is_empty());
}

#[test]
fn creates_and_lists_notebook_entries_for_future_exchange_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = state
        .create_company(NewCompany {
            exchange: "XETRA".to_owned(),
            ticker: "SAP".to_owned(),
            display_name: "SAP SE".to_owned(),
            isin: Some("DE0007164600".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("future exchange company should be created");

    let entry = state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Future exchange note".to_owned(),
            body: "This note should stay attached by company ID.".to_owned(),
            body_format: None,
            tags: vec!["europe".to_owned()],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: vec![],
        })
        .expect("notebook entry should be created");

    let entries = state
        .list_notebook_entries(&company.id)
        .expect("notebook entries should list");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, entry.id);
}
