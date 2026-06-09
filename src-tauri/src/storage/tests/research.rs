use super::common::*;
use super::*;

#[test]
fn lists_company_evidence_from_canonical_domains() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("test listing should ingest");
    let feed_item = state
        .list_feed_items()
        .expect("feed items should list")
        .pop()
        .expect("feed item should exist");
    let note = state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Management claim about schedule".to_owned(),
            body: "Management said the next milestone should happen in two quarters.".to_owned(),
            body_format: None,
            tags: vec!["management".to_owned()],
            kind: "claim".to_owned(),
            claim_status: Some("open".to_owned()),
            event_date: Some("2026-05-30".to_owned()),
            follow_up_after: Some("2026-Q4".to_owned()),
            follow_up_date: Some("2026-11-30".to_owned()),
            origins: vec![NewNotebookOrigin {
                source_type: "feed_item".to_owned(),
                source_id: Some(feed_item.id.clone()),
                source_url: Some(feed_item.source_url.clone()),
                label: Some("Source report".to_owned()),
            }],
        })
        .expect("claim note should create");

    let evidence = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            limit: None,
        })
        .expect("research evidence should list");

    assert!(evidence.iter().any(|item| item.evidence_type == "feed_item"
        && item.source_id == feed_item.id
        && item.trust_category == "official_report"));
    assert!(evidence.iter().any(|item| item.evidence_type == "claim"
        && item.source_id == note.id
        && item.trust_category == "user_note"));
    assert!(evidence.iter().all(|item| item.company_id == company.id));
}

#[test]
fn review_checkpoints_drive_changed_since_review_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("test listing should ingest");

    let before_review = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            limit: None,
        })
        .expect("research evidence should list before review");

    assert!(before_review
        .iter()
        .all(|item| item.review_state.changed_since_company_review));

    let checkpoint = state
        .mark_research_scope_reviewed(ResearchReviewCheckpointInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            reviewed_at: Some("2026-06-01T00:00:00Z".to_owned()),
        })
        .expect("review checkpoint should save");

    let after_review = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            limit: None,
        })
        .expect("research evidence should list after review");
    let stored_checkpoint = state
        .list_research_review_state(ResearchReviewCheckpointInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            reviewed_at: None,
        })
        .expect("review checkpoint should read")
        .expect("review checkpoint should exist");

    assert_eq!(checkpoint.reviewed_at, "2026-06-01T00:00:00Z");
    assert_eq!(stored_checkpoint.id, checkpoint.id);
    assert!(after_review
        .iter()
        .all(|item| !item.review_state.changed_since_company_review));
}

#[test]
fn lists_watchlist_timeline_and_watchlist_review_state() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Core".to_owned(),
            description: None,
        })
        .expect("watchlist should create");

    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("watchlist membership should create");
    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("test listing should ingest");
    state
        .mark_research_scope_reviewed(ResearchReviewCheckpointInput {
            scope_type: "watchlist".to_owned(),
            scope_id: watchlist.id.clone(),
            reviewed_at: Some("2026-06-01T00:00:00Z".to_owned()),
        })
        .expect("watchlist review checkpoint should save");

    let evidence = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: None,
            watchlist_id: Some(watchlist.id.clone()),
            limit: None,
        })
        .expect("watchlist evidence should list");

    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].company_id, company.id);
    assert!(!evidence[0].review_state.changed_since_watchlist_review);
}

#[test]
fn creates_idempotent_typed_evidence_links() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("test listing should ingest");
    let feed_item = state
        .list_feed_items()
        .expect("feed items should list")
        .pop()
        .expect("feed item should exist");
    let note = state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Observation".to_owned(),
            body: "Source-backed observation.".to_owned(),
            body_format: None,
            tags: Vec::new(),
            kind: "observation".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: Vec::new(),
        })
        .expect("notebook entry should create");

    let input = NewEvidenceLink {
        from_type: "notebook_entry".to_owned(),
        from_id: note.id.clone(),
        to_type: "feed_item".to_owned(),
        to_id: feed_item.id.clone(),
        relation_type: "cites".to_owned(),
    };
    let first = state
        .create_evidence_link(input)
        .expect("evidence link should create");
    let second = state
        .create_evidence_link(NewEvidenceLink {
            from_type: "notebook_entry".to_owned(),
            from_id: note.id,
            to_type: "feed_item".to_owned(),
            to_id: feed_item.id,
            relation_type: "cites".to_owned(),
        })
        .expect("duplicate evidence link should return existing link");

    assert_eq!(first.id, second.id);

    state
        .delete_evidence_link(&first.id)
        .expect("evidence link should delete");
}

#[test]
fn rejects_links_to_missing_evidence() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let note = state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id,
            title: "Observation".to_owned(),
            body: "Source-backed observation.".to_owned(),
            body_format: None,
            tags: Vec::new(),
            kind: "observation".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: Vec::new(),
        })
        .expect("notebook entry should create");

    let error = state
        .create_evidence_link(NewEvidenceLink {
            from_type: "notebook_entry".to_owned(),
            from_id: note.id,
            to_type: "feed_item".to_owned(),
            to_id: "missing_feed".to_owned(),
            relation_type: "cites".to_owned(),
        })
        .expect_err("missing evidence should be rejected");

    assert!(error.to_string().contains("missing research reference"));
}

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
        .expect("tracked company should create")
}
