use super::*;

fn tracked_company(ticker: &str, name: &str) -> NewCompany {
    NewCompany {
        exchange: "GPW".to_owned(),
        ticker: ticker.to_owned(),
        display_name: name.to_owned(),
        isin: None,
        cik: None,
        lei: None,
    }
}

fn tracked_company_on_exchange(
    exchange: &str,
    ticker: &str,
    name: &str,
    isin: Option<&str>,
) -> NewCompany {
    NewCompany {
        exchange: exchange.to_owned(),
        ticker: ticker.to_owned(),
        display_name: name.to_owned(),
        isin: isin.map(str::to_owned),
        cik: None,
        lei: None,
    }
}

#[test]
fn research_data_round_trips_companies_watchlists_and_notebooks() {
    let source = AppState::new(open_in_memory_database().expect("database should open"));
    let cdr = source
        .create_company(tracked_company("CDR", "CD PROJEKT S.A."))
        .expect("company should create");
    let watchlist = source
        .create_watchlist(NewWatchlist {
            name: "Main GPW".to_owned(),
            description: Some("Core list".to_owned()),
        })
        .expect("watchlist should create");
    source
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: cdr.id.clone(),
        })
        .expect("membership should create");
    let note = source
        .create_notebook_entry(NewNotebookEntry {
            company_id: cdr.id.clone(),
            title: "Management claim".to_owned(),
            body: "Company expects release progress.".to_owned(),
            body_format: Some("markdown".to_owned()),
            tags: vec!["claim".to_owned(), "release".to_owned()],
            kind: "claim".to_owned(),
            claim_status: Some("open".to_owned()),
            event_date: Some("2026-06-01".to_owned()),
            follow_up_after: Some("2026-Q4".to_owned()),
            follow_up_date: None,
            origins: vec![NewNotebookOrigin {
                source_type: "external_url".to_owned(),
                source_id: None,
                source_url: Some("https://example.com/report".to_owned()),
                label: Some("Report".to_owned()),
            }],
        })
        .expect("note should create");
    let question = source
        .create_research_question(NewResearchQuestion {
            scope_type: "company".to_owned(),
            scope_id: cdr.id.clone(),
            title: "Will management deliver the release progress?".to_owned(),
            body: Some("Use future reports to answer this.".to_owned()),
        })
        .expect("research question should create");
    let link_target_note_id = note.id.clone();
    source
        .create_evidence_link(NewEvidenceLink {
            from_type: "research_question".to_owned(),
            from_id: question.id.clone(),
            to_type: "notebook_entry".to_owned(),
            to_id: note.id,
            relation_type: "related".to_owned(),
        })
        .expect("question link should create");
    let brief_job = source
        .create_research_brief_job(NewResearchBriefJob {
            scope_type: "company".to_owned(),
            scope_id: cdr.id.clone(),
            provider_id: "test_sample".to_owned(),
            model: "test-sample-analysis-v1".to_owned(),
        })
        .expect("brief job should create");
    source
        .complete_research_brief_job(CompletedResearchBrief {
            job_id: brief_job.id,
            title: "CDR research brief".to_owned(),
            summary: "Brief summary.".to_owned(),
            content_markdown: "## What changed\n\nNotebook evidence changed. [E1]".to_owned(),
            language: Some("en".to_owned()),
            citations: vec![NewResearchBriefCitation {
                citation_key: "E1".to_owned(),
                evidence_type: "notebook_entry".to_owned(),
                evidence_id: link_target_note_id,
                label: "Management claim".to_owned(),
                snippet: Some("Company expects release progress.".to_owned()),
            }],
        })
        .expect("brief should complete");
    source
        .create_research_reminder(NewResearchReminder {
            scope_type: "company".to_owned(),
            scope_id: cdr.id.clone(),
            company_id: Some(cdr.id.clone()),
            reminder_kind: "manual_research".to_owned(),
            source_type: Some("research_question".to_owned()),
            source_id: Some(question.id.clone()),
            title: "Review research question".to_owned(),
            body: Some("Use new evidence to answer the question.".to_owned()),
            due_at: Some("2026-06-15T08:00:00Z".to_owned()),
        })
        .expect("reminder should create");
    let digest_job = source
        .create_research_digest_job(NewResearchDigestJob {
            scope_type: "company".to_owned(),
            scope_id: cdr.id.clone(),
            provider_id: "test_sample".to_owned(),
            model: "test-sample-analysis-v1".to_owned(),
        })
        .expect("digest job should create");
    source
        .complete_research_digest_job(CompletedResearchDigest {
            job_id: digest_job.id,
            title: "CDR research digest".to_owned(),
            summary: "Digest summary.".to_owned(),
            content_markdown: "## Review\n\nQuestion needs follow-up. [E1]".to_owned(),
            language: Some("en".to_owned()),
            citations: vec![NewResearchBriefCitation {
                citation_key: "E1".to_owned(),
                evidence_type: "research_question".to_owned(),
                evidence_id: question.id,
                label: "Research question".to_owned(),
                snippet: Some("Use future reports to answer this.".to_owned()),
            }],
        })
        .expect("digest should complete");

    let export = source
        .export_research_data()
        .expect("research data should export");
    assert_eq!(export.summary.companies, 1);
    assert_eq!(export.summary.watchlists, 1);
    assert_eq!(export.summary.memberships, 1);
    assert_eq!(export.summary.notebook_entries, 1);
    assert_eq!(export.summary.research_questions, 1);
    assert_eq!(export.summary.evidence_links, 1);
    assert_eq!(export.summary.ai_research_briefs, 1);
    assert_eq!(export.summary.ai_research_brief_citations, 1);
    assert_eq!(export.summary.research_reminders, 1);
    assert_eq!(export.summary.ai_research_digests, 1);
    assert_eq!(export.summary.ai_research_digest_citations, 1);

    let target = AppState::new(open_in_memory_database().expect("database should open"));
    let preview = target
        .preview_research_import(&export.contents)
        .expect("preview should parse");
    assert!(preview.valid, "{:?}", preview.errors);
    assert_eq!(preview.summary.companies_created, 1);
    assert_eq!(preview.summary.watchlists_created, 1);
    assert_eq!(preview.summary.memberships_created, 1);
    assert_eq!(preview.summary.notebook_entries_created, 1);
    assert_eq!(preview.summary.research_questions_created, 1);
    assert_eq!(preview.summary.evidence_links_created, 1);
    assert_eq!(preview.summary.ai_research_briefs_created, 1);
    assert_eq!(preview.summary.ai_research_brief_citations_created, 1);
    assert_eq!(preview.summary.research_reminders_created, 1);
    assert_eq!(preview.summary.ai_research_digests_created, 1);
    assert_eq!(preview.summary.ai_research_digest_citations_created, 1);

    target
        .apply_research_import(&export.contents)
        .expect("import should apply");
    let imported_companies = target.list_companies().expect("companies should list");
    assert_eq!(imported_companies.len(), 1);
    let imported_watchlists = target.list_watchlists().expect("watchlists should list");
    assert_eq!(imported_watchlists.len(), 1);
    let imported_notes = target
        .list_notebook_entries(&imported_companies[0].id)
        .expect("notes should list");
    assert_eq!(imported_notes.len(), 1);
    assert_eq!(imported_notes[0].tags, vec!["claim", "release"]);
    let imported_questions = target
        .list_research_questions(ResearchQuestionListInput {
            scope_type: Some("company".to_owned()),
            scope_id: Some(imported_companies[0].id.clone()),
            status: None,
        })
        .expect("research questions should list");
    assert_eq!(imported_questions.len(), 1);
    assert_eq!(
        imported_questions[0].title,
        "Will management deliver the release progress?"
    );
    let imported_links = target
        .list_evidence_links(EvidenceLinkListInput {
            endpoint_type: "research_question".to_owned(),
            endpoint_id: imported_questions[0].id.clone(),
        })
        .expect("evidence links should list");
    assert_eq!(imported_links.len(), 1);
    assert_eq!(imported_links[0].relation_type, "related");
    let imported_briefs = target
        .list_research_brief_jobs(ResearchBriefScopeInput {
            scope_type: "company".to_owned(),
            scope_id: imported_companies[0].id.clone(),
        })
        .expect("research briefs should list");
    assert_eq!(imported_briefs.len(), 1);
    assert_eq!(
        imported_briefs[0]
            .brief
            .as_ref()
            .expect("brief should restore")
            .citations
            .len(),
        1
    );
    let imported_reminders = target
        .list_research_reminders(ResearchReminderListInput {
            scope_type: "company".to_owned(),
            scope_id: imported_companies[0].id.clone(),
            status: None,
        })
        .expect("research reminders should list");
    assert!(imported_reminders
        .iter()
        .any(|reminder| reminder.title == "Review research question"));
    let imported_digests = target
        .list_research_digest_jobs(ResearchDigestScopeInput {
            scope_type: "company".to_owned(),
            scope_id: imported_companies[0].id.clone(),
        })
        .expect("research digests should list");
    assert_eq!(imported_digests.len(), 1);
    assert_eq!(
        imported_digests[0]
            .digest
            .as_ref()
            .expect("digest should restore")
            .citations
            .len(),
        1
    );
}

#[test]
fn research_data_round_trips_future_exchange_companies_watchlists_and_notebooks() {
    let source = AppState::new(open_in_memory_database().expect("database should open"));
    let sap = source
        .create_company(tracked_company_on_exchange(
            "XETRA",
            "SAP",
            "SAP SE",
            Some("DE0007164600"),
        ))
        .expect("future exchange company should create");
    let watchlist = source
        .create_watchlist(NewWatchlist {
            name: "Europe".to_owned(),
            description: Some("European companies".to_owned()),
        })
        .expect("watchlist should create");
    source
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: sap.id.clone(),
        })
        .expect("membership should create");
    source
        .create_notebook_entry(NewNotebookEntry {
            company_id: sap.id.clone(),
            title: "Cloud margin checkpoint".to_owned(),
            body: "Track margin commentary next quarter.".to_owned(),
            body_format: Some("markdown".to_owned()),
            tags: vec!["margin".to_owned()],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: Some("2026-09-30".to_owned()),
            origins: vec![],
        })
        .expect("note should create");

    let export = source
        .export_research_data()
        .expect("research data should export");
    let target = AppState::new(open_in_memory_database().expect("database should open"));

    target
        .apply_research_import(&export.contents)
        .expect("future exchange research import should apply");

    let imported_companies = target.list_companies().expect("companies should list");
    assert_eq!(imported_companies.len(), 1);
    assert_eq!(imported_companies[0].qualified_ticker, "XETRA:SAP");
    let imported_memberships = target
        .list_watchlist_memberships()
        .expect("memberships should list");
    assert_eq!(imported_memberships.len(), 1);
    let imported_notes = target
        .list_notebook_entries(&imported_companies[0].id)
        .expect("notes should list");
    assert_eq!(imported_notes.len(), 1);
    assert_eq!(imported_notes[0].title, "Cloud margin checkpoint");
}

#[test]
fn duplicate_notebook_ids_are_preview_warnings_and_are_skipped() {
    let source = AppState::new(open_in_memory_database().expect("database should open"));
    let company = source
        .create_company(tracked_company("ACP", "ASSECOPOL S.A."))
        .expect("company should create");
    source
        .create_notebook_entry(NewNotebookEntry {
            company_id: company.id.clone(),
            title: "Existing note".to_owned(),
            body: "First body".to_owned(),
            body_format: Some("markdown".to_owned()),
            tags: vec![],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: vec![],
        })
        .expect("note should create");
    let export = source
        .export_research_data()
        .expect("research data should export");

    let preview = source
        .preview_research_import(&export.contents)
        .expect("preview should parse");
    assert!(preview.valid);
    assert_eq!(preview.summary.notebook_entries_skipped, 1);
    assert_eq!(preview.warnings.len(), 1);

    let apply = source
        .apply_research_import(&export.contents)
        .expect("duplicate import should still apply");
    assert_eq!(apply.summary.notebook_entries_skipped, 1);
    let notes = source
        .list_notebook_entries(&company.id)
        .expect("notes should list");
    assert_eq!(notes.len(), 1);
}

#[test]
fn research_import_rejects_unresolved_membership_company() {
    let target = AppState::new(open_in_memory_database().expect("database should open"));
    let contents = r#"{
      "schemaVersion": 1,
      "exportedAt": "2026-06-05T00:00:00Z",
      "appVersion": "0.19.0",
      "sections": ["companies", "watchlists"],
      "companies": [],
      "watchlists": [{"id": "watchlist_main", "name": "Main", "description": null}],
      "memberships": [{"watchlistId": "watchlist_main", "companyQualifiedTicker": "GPW:CDR"}],
      "notebookEntries": []
    }"#;

    let preview = target
        .preview_research_import(contents)
        .expect("preview should parse");
    assert!(!preview.valid);
    assert!(
        preview
            .errors
            .iter()
            .any(|error| error.contains("missing company GPW:CDR")),
        "{:?}",
        preview.errors
    );
}

#[test]
fn research_import_merges_watchlists_by_existing_name_when_id_differs() {
    let target = AppState::new(open_in_memory_database().expect("database should open"));
    target
        .create_company(tracked_company("CDR", "CD PROJEKT S.A."))
        .expect("company should create");
    let existing_watchlist = target
        .create_watchlist(NewWatchlist {
            name: "Main GPW".to_owned(),
            description: None,
        })
        .expect("watchlist should create");
    let contents = r#"{
      "schemaVersion": 1,
      "exportedAt": "2026-06-05T00:00:00Z",
      "appVersion": "0.19.0",
      "sections": ["companies", "watchlists"],
      "companies": [{
        "id": "company_gpw_cdr",
        "exchange": "GPW",
        "ticker": "CDR",
        "qualifiedTicker": "GPW:CDR",
        "displayName": "CD PROJEKT S.A.",
        "isin": null,
        "cik": null,
        "lei": null
      }],
      "watchlists": [{"id": "watchlist_old_id", "name": "Main GPW", "description": null}],
      "memberships": [{"watchlistId": "watchlist_old_id", "companyQualifiedTicker": "GPW:CDR"}],
      "notebookEntries": []
    }"#;

    let preview = target
        .preview_research_import(contents)
        .expect("preview should parse");
    assert!(preview.valid, "{:?}", preview.errors);
    assert_eq!(preview.summary.watchlists_merged, 1);
    assert_eq!(preview.warnings.len(), 1);

    target
        .apply_research_import(contents)
        .expect("import should apply");
    let memberships = target
        .list_watchlist_memberships()
        .expect("memberships should list");
    assert_eq!(memberships.len(), 1);
    assert_eq!(memberships[0].watchlist_id, existing_watchlist.id);
}

#[test]
fn settings_export_is_yaml_and_import_updates_allowlisted_settings_only() {
    let source = AppState::new(open_in_memory_database().expect("database should open"));
    source
        .update_settings(SettingsUpdate {
            theme: Some("light".to_owned()),
            locale: Some("pl".to_owned()),
            poll_interval_seconds: Some(1800),
            ..SettingsUpdate::default()
        })
        .expect("settings should update");
    let export = source
        .export_settings_data()
        .expect("settings should export");
    assert_eq!(export.media_type, "application/x-yaml");
    assert!(!export.contents.contains("license"));
    assert!(!export.contents.contains("apiKey"));

    let target = AppState::new(open_in_memory_database().expect("database should open"));
    let preview = target
        .preview_settings_import(&export.contents)
        .expect("settings preview should parse");
    assert!(preview.valid, "{:?}", preview.errors);
    assert!(preview.summary.settings_updated > 0);

    target
        .apply_settings_import(&export.contents)
        .expect("settings import should apply");
    let settings = target.get_settings().expect("settings should load");
    assert_eq!(settings.theme, "light");
    assert_eq!(settings.locale, "pl");
    assert_eq!(settings.poll_interval_seconds, 1800);
}
