use super::common::*;
use super::*;

#[test]
fn creates_and_completes_research_brief_snapshots() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("listing should ingest");
    let feed_item = state
        .list_feed_items()
        .expect("feed items should list")
        .pop()
        .expect("feed item should exist");

    let job = state
        .create_research_brief_job(NewResearchBriefJob {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            provider_id: "test_sample".to_owned(),
            model: "test-sample-analysis-v1".to_owned(),
        })
        .expect("brief job should create");

    assert_eq!(job.status, "queued");
    assert_eq!(job.scope_type, "company");
    assert!(job.brief.is_none());

    let running = state
        .mark_research_brief_job_running(&job.id)
        .expect("brief job should run");
    assert_eq!(running.status, "running");

    let completed = state
        .complete_research_brief_job(CompletedResearchBrief {
            job_id: job.id.clone(),
            title: "CDR brief".to_owned(),
            summary: "Source-grounded summary.".to_owned(),
            content_markdown: "## What changed\n\nReport changed. [E1]".to_owned(),
            language: Some("en".to_owned()),
            citations: vec![NewResearchBriefCitation {
                citation_key: "E1".to_owned(),
                evidence_type: "feed_item".to_owned(),
                evidence_id: feed_item.id,
                label: "Current report".to_owned(),
                snippet: Some("Report snippet".to_owned()),
            }],
        })
        .expect("brief job should complete");

    let brief = completed.brief.expect("completed job should include brief");
    assert_eq!(completed.status, "succeeded");
    assert_eq!(brief.provider_id, "test_sample");
    assert_eq!(brief.prompt_version, RESEARCH_BRIEF_PROMPT_VERSION);
    assert_eq!(brief.citations.len(), 1);

    let second_job = state
        .create_research_brief_job(NewResearchBriefJob {
            scope_type: "company".to_owned(),
            scope_id: company.id.clone(),
            provider_id: "test_sample".to_owned(),
            model: "test-sample-analysis-v1".to_owned(),
        })
        .expect("second brief job should create");

    assert_ne!(second_job.id, job.id);
    let jobs = state
        .list_research_brief_jobs(ResearchBriefScopeInput {
            scope_type: "company".to_owned(),
            scope_id: company.id,
        })
        .expect("brief jobs should list");
    assert_eq!(jobs.len(), 2);
}

#[test]
fn collects_company_and_watchlist_research_brief_evidence() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = tracked_company(&state);
    state
        .ingest_gpw_report_listings(&[sample_cdr_listing()])
        .expect("listing should ingest");
    let watchlist = state
        .create_watchlist(NewWatchlist {
            name: "Main".to_owned(),
            description: None,
        })
        .expect("watchlist should create");
    state
        .add_company_to_watchlist(WatchlistCompanyInput {
            watchlist_id: watchlist.id.clone(),
            company_id: company.id.clone(),
        })
        .expect("company should join watchlist");

    let company_job = state
        .create_research_brief_job(NewResearchBriefJob {
            scope_type: "company".to_owned(),
            scope_id: company.id,
            provider_id: "test_sample".to_owned(),
            model: "test-sample-analysis-v1".to_owned(),
        })
        .expect("company job should create");
    let watchlist_job = state
        .create_research_brief_job(NewResearchBriefJob {
            scope_type: "watchlist".to_owned(),
            scope_id: watchlist.id,
            provider_id: "test_sample".to_owned(),
            model: "test-sample-analysis-v1".to_owned(),
        })
        .expect("watchlist job should create");

    let company_context = state
        .collect_research_brief_evidence(&company_job.id)
        .expect("company evidence should collect");
    let watchlist_context = state
        .collect_research_brief_evidence(&watchlist_job.id)
        .expect("watchlist evidence should collect");

    assert!(company_context
        .evidence_items
        .iter()
        .any(|item| item.evidence_type == "feed_item"));
    assert!(watchlist_context
        .evidence_items
        .iter()
        .any(|item| item.evidence_type == "feed_item"));
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
