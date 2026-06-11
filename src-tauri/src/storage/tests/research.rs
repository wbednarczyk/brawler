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

    let timeline = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            evidence_types: None,
            changed_since_review_only: None,
            limit: None,
        })
        .expect("research evidence should list");
    let evidence = timeline.items;

    assert_eq!(timeline.summary.total, evidence.len());
    assert_eq!(timeline.summary.changed_since_review, evidence.len());
    assert_eq!(timeline.summary.last_reviewed_at, None);
    assert!(evidence.iter().any(|item| item.evidence_type == "feed_item"
        && item.source_id == feed_item.id
        && item.trust_category == "official_report"));
    assert!(evidence.iter().any(|item| item.evidence_type == "claim"
        && item.source_id == note.id
        && item.trust_category == "user_note"));
    assert!(evidence.iter().all(|item| item.company_id == company.id));
}

#[test]
fn creates_company_research_question_and_lists_it_as_evidence() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    let question = state
        .create_research_question(NewResearchQuestion {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            title: "Will margins recover after the new contract?".to_owned(),
            body: Some("Track future reports and management comments.".to_owned()),
        })
        .expect("research question should create");

    assert_eq!(question.scope_type, "company");
    assert_eq!(question.scope_id, company.id);
    assert_eq!(question.status, "open");

    let questions = state
        .list_research_questions(ResearchQuestionListInput {
            scope_type: Some("company".to_owned()),
            scope_id: Some(company.id.clone()),
            status: Some("open".to_owned()),
        })
        .expect("research questions should list");

    assert_eq!(questions.len(), 1);
    assert_eq!(questions[0].id, question.id);

    let timeline = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            evidence_types: Some(vec!["research_question".to_owned()]),
            changed_since_review_only: None,
            limit: None,
        })
        .expect("research question evidence should list");

    assert_eq!(timeline.summary.total, 1);
    assert_eq!(timeline.items[0].evidence_type, "research_question");
    assert_eq!(timeline.items[0].source_id, question.id);
    assert_eq!(timeline.items[0].company_id, company.id);
    assert_eq!(timeline.items[0].trust_category, "user_note");
}

#[test]
fn updates_research_question_status() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let question = state
        .create_research_question(NewResearchQuestion {
            scope_type: "company".to_owned(),
            scope_id: company.id,
            title: "What changed?".to_owned(),
            body: None,
        })
        .expect("research question should create");

    let answered = state
        .update_research_question(ResearchQuestionUpdate {
            id: question.id.clone(),
            title: None,
            body: Some("Answer is now known.".to_owned()),
            status: Some("answered".to_owned()),
        })
        .expect("research question should update");

    assert_eq!(answered.status, "answered");
    assert_eq!(answered.body, "Answer is now known.");
    assert!(answered.closed_at.is_some());

    let reopened = state
        .update_research_question(ResearchQuestionUpdate {
            id: question.id,
            title: None,
            body: None,
            status: Some("open".to_owned()),
        })
        .expect("research question should reopen");

    assert_eq!(reopened.status, "open");
    assert_eq!(reopened.closed_at, None);
}

#[test]
fn rejects_visible_watchlist_question_scope_until_ui_support_exists() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Research list".to_owned(),
            description: None,
        })
        .expect("watchlist should create");

    let error = state
        .create_research_question(NewResearchQuestion {
            scope_type: "watchlist".to_owned(),
            scope_id: watchlist.id,
            title: "Watchlist-wide question".to_owned(),
            body: None,
        })
        .expect_err("watchlist question should be deferred");

    assert!(error.to_string().contains("invalid research value"));
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
            evidence_types: None,
            changed_since_review_only: None,
            limit: None,
        })
        .expect("research evidence should list before review");

    assert!(before_review
        .items
        .iter()
        .all(|item| item.review_state.changed_since_company_review));
    assert_eq!(
        before_review.summary.changed_since_review,
        before_review.summary.total
    );

    let checkpoint = state
        .mark_research_scope_reviewed(ResearchReviewCheckpointInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            reviewed_at: Some("2026-06-01T00:00:00Z".to_owned()),
            cascade_to_companies: None,
        })
        .expect("review checkpoint should save");

    let after_review = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            evidence_types: None,
            changed_since_review_only: None,
            limit: None,
        })
        .expect("research evidence should list after review");
    let stored_checkpoint = state
        .list_research_review_state(ResearchReviewCheckpointInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            reviewed_at: None,
            cascade_to_companies: None,
        })
        .expect("review checkpoint should read")
        .expect("review checkpoint should exist");

    assert_eq!(checkpoint.reviewed_at, "2026-06-01T00:00:00Z");
    assert_eq!(stored_checkpoint.id, checkpoint.id);
    assert_eq!(
        after_review.summary.last_reviewed_at,
        Some(checkpoint.reviewed_at)
    );
    assert_eq!(after_review.summary.changed_since_review, 0);
    assert!(after_review
        .items
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
            cascade_to_companies: None,
        })
        .expect("watchlist review checkpoint should save");

    let timeline = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: None,
            watchlist_id: Some(watchlist.id.clone()),
            evidence_types: None,
            changed_since_review_only: None,
            limit: None,
        })
        .expect("watchlist evidence should list");
    let evidence = timeline.items;

    assert_eq!(evidence.len(), 1);
    assert_eq!(timeline.summary.member_company_count, 1);
    assert_eq!(timeline.summary.companies_with_changed_evidence, 0);
    assert_eq!(timeline.summary.company_summaries.len(), 1);
    assert_eq!(timeline.summary.company_summaries[0].company_id, company.id);
    assert_eq!(timeline.summary.company_summaries[0].total, 1);
    assert_eq!(
        timeline.summary.company_summaries[0].changed_since_review,
        0
    );
    assert_eq!(timeline.summary.changed_since_review, 0);
    assert_eq!(
        timeline.summary.last_reviewed_at,
        Some("2026-06-01T00:00:00Z".to_owned())
    );
    assert_eq!(evidence[0].company_id, company.id);
    assert!(!evidence[0].review_state.changed_since_watchlist_review);
}

#[test]
fn watchlist_review_can_explicitly_cascade_to_member_companies() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    let second_company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "ACP".to_owned(),
            display_name: "ASSECOPOL S.A.".to_owned(),
            isin: Some("PLSOFTB00016".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("second tracked company should create");
    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Review list".to_owned(),
            description: None,
        })
        .expect("watchlist should create");

    for company_id in [&company.id, &second_company.id] {
        state
            .add_company_to_watchlist(WatchlistCompanyInput {
                watchlist_id: watchlist.id.clone(),
                company_id: (*company_id).to_owned(),
            })
            .expect("watchlist membership should create");
    }

    for (company_id, title) in [
        (&company.id, "First company note"),
        (&second_company.id, "Second company note"),
    ] {
        state
            .create_notebook_entry(NewNotebookEntry {
                company_id: (*company_id).to_owned(),
                title: title.to_owned(),
                body: "Watchlist evidence.".to_owned(),
                body_format: None,
                tags: Vec::new(),
                kind: "observation".to_owned(),
                claim_status: None,
                event_date: Some("2026-06-03".to_owned()),
                follow_up_after: None,
                follow_up_date: None,
                origins: Vec::new(),
            })
            .expect("note should create");
    }

    state
        .mark_research_scope_reviewed(ResearchReviewCheckpointInput {
            scope_type: "watchlist".to_owned(),
            scope_id: watchlist.id.clone(),
            reviewed_at: Some("2026-06-04T00:00:00Z".to_owned()),
            cascade_to_companies: Some(false),
        })
        .expect("watchlist review checkpoint should save");

    let company_state = state
        .list_research_review_state(ResearchReviewCheckpointInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            reviewed_at: None,
            cascade_to_companies: None,
        })
        .expect("company review state should read");

    assert!(company_state.is_none());

    state
        .mark_research_scope_reviewed(ResearchReviewCheckpointInput {
            scope_type: "watchlist".to_owned(),
            scope_id: watchlist.id.clone(),
            reviewed_at: Some("2026-06-05T00:00:00Z".to_owned()),
            cascade_to_companies: Some(true),
        })
        .expect("watchlist review checkpoint should cascade");

    for company_id in [&company.id, &second_company.id] {
        let checkpoint = state
            .list_research_review_state(ResearchReviewCheckpointInput {
                scope_type: "company".to_owned(),
                scope_id: (*company_id).to_owned(),
                reviewed_at: None,
                cascade_to_companies: None,
            })
            .expect("company review state should read")
            .expect("company review checkpoint should exist");

        assert_eq!(checkpoint.reviewed_at, "2026-06-05T00:00:00Z");
    }
}

#[test]
fn filters_evidence_types_and_changed_since_review_in_backend() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);

    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("test listing should ingest");
    state
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Manual observation".to_owned(),
            body: "Manual evidence.".to_owned(),
            body_format: None,
            tags: Vec::new(),
            kind: "observation".to_owned(),
            claim_status: None,
            event_date: Some("2026-06-03".to_owned()),
            follow_up_after: None,
            follow_up_date: None,
            origins: Vec::new(),
        })
        .expect("note should create");
    state
        .mark_research_scope_reviewed(ResearchReviewCheckpointInput {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            reviewed_at: Some("2026-06-02T00:00:00Z".to_owned()),
            cascade_to_companies: None,
        })
        .expect("review checkpoint should save");

    let notes_only = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id.clone()),
            watchlist_id: None,
            evidence_types: Some(vec!["notebook_entry".to_owned()]),
            changed_since_review_only: None,
            limit: None,
        })
        .expect("filtered timeline should list");

    assert_eq!(notes_only.summary.total, 1);
    assert_eq!(notes_only.summary.changed_since_review, 1);
    assert!(notes_only
        .items
        .iter()
        .all(|item| item.evidence_type == "notebook_entry"));

    let changed_only = state
        .list_research_evidence(ResearchEvidenceInput {
            company_id: Some(company.id),
            watchlist_id: None,
            evidence_types: None,
            changed_since_review_only: Some(true),
            limit: None,
        })
        .expect("changed-only timeline should list");

    assert_eq!(changed_only.summary.total, 1);
    assert!(changed_only
        .items
        .iter()
        .all(|item| item.review_state.changed_since_company_review));
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
fn creates_and_lists_question_to_evidence_links() {
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
    let question = state
        .create_research_question(NewResearchQuestion {
            scope_type: "company".to_owned(),
            scope_id: company.id,
            title: "Does the report answer the margin question?".to_owned(),
            body: None,
        })
        .expect("research question should create");

    let first = state
        .create_evidence_link(NewEvidenceLink {
            from_type: "research_question".to_owned(),
            from_id: question.id.clone(),
            to_type: "feed_item".to_owned(),
            to_id: feed_item.id.clone(),
            relation_type: "answers".to_owned(),
        })
        .expect("question evidence link should create");
    let second = state
        .create_evidence_link(NewEvidenceLink {
            from_type: "research_question".to_owned(),
            from_id: question.id.clone(),
            to_type: "feed_item".to_owned(),
            to_id: feed_item.id,
            relation_type: "answers".to_owned(),
        })
        .expect("duplicate question evidence link should return existing link");
    let links = state
        .list_evidence_links(EvidenceLinkListInput {
            endpoint_type: "research_question".to_owned(),
            endpoint_id: question.id,
        })
        .expect("question evidence links should list");

    assert_eq!(first.id, second.id);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].id, first.id);
    assert_eq!(links[0].relation_type, "answers");
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
