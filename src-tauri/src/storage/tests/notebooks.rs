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

/// Fault-injection tests (#461, docs/testing.md § Failure-path tests: fault
/// injection): `create_notebook_entry`/`update_notebook_entry` now own a
/// transaction around the entry row, its tags and its origins, so a failure
/// mid-write must leave no partial state — never an entry with some but not
/// all of its tags/origins, never an update that changed the row but dropped
/// its tags.
mod fault_injection {
    use super::*;

    fn sample_company(state: &AppState) -> Company {
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

    #[test]
    fn create_notebook_entry_rolls_back_when_the_tags_insert_fails() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = sample_company(&state);

        // Targeted poison (docs/testing.md convention): only the specific tag
        // this test inserts trips it, never an unrelated write.
        let guard = state.checkout_for_tests().expect("checkout");
        guard
            .execute_batch(
                "CREATE TRIGGER poison_notebook_tags_insert BEFORE INSERT ON notebook_entry_tags
                 WHEN NEW.tag = 'poison-tag'
                 BEGIN SELECT RAISE(ABORT, 'notebook_entry_tags poisoned for test'); END;",
            )
            .expect("install poison trigger");
        drop(guard); // never hold the checkout across a store call — pool deadlock (#360/#376)

        let result = state.create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Rolled back entry".to_owned(),
            body: "Should never be visible.".to_owned(),
            body_format: None,
            tags: vec!["poison-tag".to_owned()],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: vec![NewNotebookOrigin {
                source_type: "manual".to_owned(),
                source_id: None,
                source_url: None,
                label: Some("should also roll back".to_owned()),
            }],
        });

        assert!(
            result.is_err(),
            "the poisoned tags insert must surface as an error"
        );

        let entries = state
            .list_notebook_entries(&company.id)
            .expect("notebook entries should list");
        assert!(
            entries.is_empty(),
            "the entry row must not survive a failed tags insert — no partial state"
        );
    }

    #[test]
    fn create_notebook_entry_rolls_back_when_the_origins_insert_fails() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = sample_company(&state);

        let guard = state.checkout_for_tests().expect("checkout");
        guard
            .execute_batch(
                "CREATE TRIGGER poison_notebook_origins_insert BEFORE INSERT ON notebook_entry_origins
                 WHEN NEW.label = 'poison-origin'
                 BEGIN SELECT RAISE(ABORT, 'notebook_entry_origins poisoned for test'); END;",
            )
            .expect("install poison trigger");
        drop(guard);

        let result = state.create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Rolled back entry with tags".to_owned(),
            body: "Should never be visible.".to_owned(),
            body_format: None,
            tags: vec!["some-tag".to_owned()],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: vec![NewNotebookOrigin {
                source_type: "manual".to_owned(),
                source_id: None,
                source_url: None,
                label: Some("poison-origin".to_owned()),
            }],
        });

        assert!(
            result.is_err(),
            "the poisoned origins insert must surface as an error"
        );

        let entries = state
            .list_notebook_entries(&company.id)
            .expect("notebook entries should list");
        assert!(
            entries.is_empty(),
            "the entry row (and its already-inserted tags) must not survive a failed origins insert"
        );
    }

    #[test]
    fn update_notebook_entry_keeps_the_old_tags_when_the_tag_reinsert_fails() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = sample_company(&state);

        let entry = state
            .create_notebook_entry(NewNotebookEntry {
                company_id: company.id.clone(),
                title: "Original title".to_owned(),
                body: "Original body.".to_owned(),
                body_format: None,
                tags: vec!["old-tag".to_owned()],
                kind: "manual".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
                origins: vec![NewNotebookOrigin {
                    source_type: "manual".to_owned(),
                    source_id: None,
                    source_url: None,
                    label: Some("kept origin".to_owned()),
                }],
            })
            .expect("notebook entry should be created");

        // Poison only the re-INSERT of the NEW tag — the DELETE of `old-tag`
        // (which does not touch this table's INSERT path) is unaffected, so
        // this targets the reinsert step specifically, not the whole update.
        let guard = state.checkout_for_tests().expect("checkout");
        guard
            .execute_batch(
                "CREATE TRIGGER poison_notebook_tag_reinsert BEFORE INSERT ON notebook_entry_tags
                 WHEN NEW.tag = 'new-tag'
                 BEGIN SELECT RAISE(ABORT, 'notebook_entry_tags reinsert poisoned for test'); END;",
            )
            .expect("install poison trigger");
        drop(guard);

        let result = state.update_notebook_entry(NotebookEntryUpdate {
            id: entry.id.clone(),
            title: "Changed title".to_owned(),
            body: "Changed body.".to_owned(),
            tags: vec!["new-tag".to_owned()],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
        });

        assert!(
            result.is_err(),
            "the poisoned tag reinsert must surface as an error"
        );

        let entries = state
            .list_notebook_entries(&company.id)
            .expect("notebook entries should list");
        let after = entries
            .into_iter()
            .find(|candidate| candidate.id == entry.id)
            .expect("the entry row must still exist — only the reinsert was poisoned");

        // The WHOLE entry stays exactly as it was before the failed update —
        // title/body (the same transaction as the tag reinsert) and tags
        // (the old tag was never actually removed once the transaction rolled
        // back), not just the tags in isolation.
        assert_eq!(after.title, "Original title");
        assert_eq!(after.body, "Original body.");
        assert_eq!(after.tags, vec!["old-tag".to_owned()]);
        assert_eq!(after.origins.len(), 1);
        assert_eq!(after.origins[0].label.as_deref(), Some("kept origin"));
    }
}
