use super::*;

fn sample_company(state: &AppState, ticker: &str, display_name: &str) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: display_name.to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

fn sample_note(state: &AppState, company_id: &str, title: &str, body: &str) -> NotebookEntry {
    state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company_id.to_owned(),
            title: title.to_owned(),
            body: body.to_owned(),
            body_format: None,
            tags: Vec::new(),
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: Vec::new(),
        })
        .expect("notebook entry should be created")
}

#[test]
fn indexes_companies_and_notes_on_insert() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state, "CDR", "CD PROJEKT S.A.");
    sample_note(
        &state,
        &company.id,
        "Profit warning follow-up",
        "Management flagged a possible profit warning next quarter.",
    );

    let company_hits = state
        .search("projekt", &[], None, 50)
        .expect("search should run");
    assert!(company_hits
        .iter()
        .any(|hit| hit.content_type == "company" && hit.source_id == company.id));

    let note_hits = state
        .search("profit warning", &[], None, 50)
        .expect("search should run");
    assert!(note_hits
        .iter()
        .any(|hit| hit.content_type == "notebook_entry"));
}

#[test]
fn folds_diacritics_and_case() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    sample_company(&state, "ZAB", "Żabka Group");

    let hits = state
        .search("zabka", &[], None, 50)
        .expect("search should run");
    assert!(hits
        .iter()
        .any(|hit| hit.content_type == "company" && hit.title == "Żabka Group"));
}

#[test]
fn reflects_updates_and_deletes() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state, "CDR", "CD PROJEKT S.A.");
    let note = sample_note(
        &state,
        &company.id,
        "Original heading",
        "The original distinctive token is dragonfruit.",
    );

    assert!(!state
        .search("dragonfruit", &[], None, 50)
        .expect("search should run")
        .is_empty());

    state
        .update_notebook_entry(NotebookEntryUpdate {
            id: note.id.clone(),
            title: "Updated heading".to_owned(),
            body: "The replacement distinctive token is starfruit.".to_owned(),
            tags: Vec::new(),
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
        })
        .expect("notebook entry should update");

    assert!(
        state
            .search("dragonfruit", &[], None, 50)
            .expect("search should run")
            .is_empty(),
        "stale term should be gone after update"
    );
    assert!(!state
        .search("starfruit", &[], None, 50)
        .expect("search should run")
        .is_empty());

    state
        .delete_notebook_entry(&note.id)
        .expect("notebook entry should delete");
    assert!(
        state
            .search("starfruit", &[], None, 50)
            .expect("search should run")
            .is_empty(),
        "deleted note should leave no index rows"
    );
}

#[test]
fn scopes_by_content_type_and_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let one = sample_company(&state, "AAA", "Alpha synergy holding");
    let two = sample_company(&state, "BBB", "Beta synergy holding");
    sample_note(
        &state,
        &one.id,
        "Alpha note",
        "shared synergy keyword alpha",
    );
    sample_note(&state, &two.id, "Beta note", "shared synergy keyword beta");

    let only_notes = state
        .search("synergy", &["notebook_entry".to_owned()], None, 50)
        .expect("search should run");
    assert!(!only_notes.is_empty());
    assert!(only_notes
        .iter()
        .all(|hit| hit.content_type == "notebook_entry"));

    let only_company_one = state
        .search("synergy", &["notebook_entry".to_owned()], Some(&one.id), 50)
        .expect("search should run");
    assert!(only_company_one
        .iter()
        .all(|hit| hit.company_id.as_deref() == Some(one.id.as_str())));
    assert!(!only_company_one.is_empty());
}

#[test]
fn indexes_watchlists_and_company_events() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = sample_company(&state, "CDR", "CD PROJEKT S.A.");

    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "High-conviction longs".to_owned(),
            description: Some("Distinctive zucchini thesis".to_owned()),
        })
        .expect("watchlist should be created");
    let event = state
        .create_company_event(NewCompanyEvent {
            company_id: company.id.clone(),
            event_type: "conference_call".to_owned(),
            title: "Distinctive papaya earnings call".to_owned(),
            event_date: "2026-08-01".to_owned(),
            event_time: None,
            status: None,
            source_type: None,
            source_adapter_id: None,
            source_event_key: None,
            source_url: None,
            attribution: None,
            fetched_at: None,
        })
        .expect("company event should be created");

    let watchlist_hits = state
        .search("zucchini", &[], None, 50)
        .expect("search should run");
    assert!(watchlist_hits
        .iter()
        .any(|hit| hit.content_type == "watchlist" && hit.source_id == watchlist.id));

    let event_hits = state
        .search("papaya", &[], None, 50)
        .expect("search should run");
    let event_hit = event_hits
        .iter()
        .find(|hit| hit.content_type == "event")
        .expect("event should be indexed");
    assert_eq!(event_hit.source_id, event.id);
    assert_eq!(event_hit.company_id.as_deref(), Some(company.id.as_str()));
}

#[test]
fn transcript_segment_carries_parent_job_id() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    {
        let connection = state.checkout().expect("database connection");
        connection
            .execute(
                "INSERT INTO transcript_jobs (id, provider_id, source_type, source_url, status) \
                 VALUES ('job_1', 'provider_gemini', 'youtube', 'https://example.test/v', 'succeeded')",
                [],
            )
            .expect("transcript job should insert");
        connection
            .execute(
                "INSERT INTO transcript_segments (id, transcript_job_id, text) \
                 VALUES ('seg_1', 'job_1', 'distinctive dragonfruit transcript segment')",
                [],
            )
            .expect("transcript segment should insert");
    }

    let hits = state
        .search("dragonfruit", &["transcript_segment".to_owned()], None, 50)
        .expect("search should run");
    let hit = hits
        .iter()
        .find(|hit| hit.source_id == "seg_1")
        .expect("transcript segment should be indexed");
    assert_eq!(
        hit.parent_id.as_deref(),
        Some("job_1"),
        "transcript segment should carry its owning job id for navigation"
    );
}

#[test]
fn blank_and_operator_only_queries_are_safe() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    sample_company(&state, "CDR", "CD PROJEKT S.A.");

    assert!(state
        .search("   ", &[], None, 50)
        .expect("blank query should be safe")
        .is_empty());

    // Raw FTS5 operators must be treated as literal text, never as syntax.
    let hits = state
        .search("\"OR AND NOT*", &[], None, 50)
        .expect("operator-like query should not error");
    assert!(hits.is_empty());
}
