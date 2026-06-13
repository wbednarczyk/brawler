use super::*;

fn test_company(state: &AppState) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: Some("PLOPTTC00011".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create")
}

#[test]
fn creates_pending_report_document_on_first_call() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/report.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Example Report".to_owned()),
            attribution: Some("Example Inc".to_owned()),
        })
        .expect("report document should create");

    assert_eq!(doc.company_id, company.id);
    assert_eq!(doc.source_type, "user_url");
    assert_eq!(doc.url, "https://example.com/report.pdf");
    assert_eq!(doc.fetch_status, "pending");
    assert!(doc.local_path.is_none());
    assert!(doc.fetched_at.is_none());
    assert_eq!(doc.title, Some("Example Report".to_owned()));
    assert_eq!(doc.attribution, Some("Example Inc".to_owned()));
}

#[test]
fn idempotent_create_on_duplicate_company_url() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let url = "https://example.com/report.pdf";

    let doc1 = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Example Report".to_owned()),
            attribution: None,
        })
        .expect("first create should succeed");

    let doc2 = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Example Report".to_owned()),
            attribution: None,
        })
        .expect("second create should succeed");

    assert_eq!(doc1.id, doc2.id);
    assert_eq!(doc1.created_at, doc2.created_at);
}

#[test]
fn marks_document_as_fetched() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/report.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("document should create");

    let updated = state
        .mark_report_document_fetched(
            &doc.id,
            Some("report_documents/doc_abc.pdf"),
            Some("application/pdf"),
            None,
            Some(1024),
        )
        .expect("document should mark as fetched");

    assert_eq!(updated.fetch_status, "fetched");
    assert_eq!(updated.local_path, Some("report_documents/doc_abc.pdf".to_owned()));
    assert_eq!(updated.content_type, Some("application/pdf".to_owned()));
    assert_eq!(updated.byte_size, Some(1024));
    assert!(updated.fetched_at.is_some());
}

#[test]
fn marks_document_as_failed_with_error() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/missing.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("document should create");

    let updated = state
        .mark_report_document_failed(&doc.id, "HTTP 404: Not Found")
        .expect("document should mark as failed");

    assert_eq!(updated.fetch_status, "failed");
    assert_eq!(updated.fetch_error, Some("HTTP 404: Not Found".to_owned()));
    assert!(updated.local_path.is_none());
}

#[test]
fn lists_documents_by_company() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company1 = test_company(&state);
    let company2 = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "PZU".to_owned(),
            display_name: "PZU".to_owned(),
            isin: Some("PLPZU0000016".to_owned()),
            cik: None,
            lei: None,
        })
        .expect("company should create");

    // Create documents for both companies
    for i in 0..3 {
        state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company1.id.clone(),
                source_type: "user_url".to_owned(),
                url: format!("https://example.com/report{}.pdf", i),
                period_id: None,
                origin_ref: None,
                title: None,
                attribution: None,
            })
            .expect("document should create");
    }

    state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company2.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/report_pzu.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("document should create");

    let company1_docs = state
        .list_report_documents_by_company(&company1.id)
        .expect("documents should list");
    let company2_docs = state
        .list_report_documents_by_company(&company2.id)
        .expect("documents should list");

    assert_eq!(company1_docs.len(), 3);
    assert_eq!(company2_docs.len(), 1);
    assert!(company1_docs.iter().all(|d| d.company_id == company1.id));
    assert!(company2_docs.iter().all(|d| d.company_id == company2.id));
}

#[test]
fn get_returns_document_by_id() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let created = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "article".to_owned(),
            url: "https://example.com/article".to_owned(),
            period_id: None,
            origin_ref: Some("feed_item_123".to_owned()),
            title: Some("Article Title".to_owned()),
            attribution: None,
        })
        .expect("document should create");

    let retrieved = state
        .get_report_document(&created.id)
        .expect("document should retrieve");

    assert_eq!(retrieved.id, created.id);
    assert_eq!(retrieved.source_type, "article");
    assert_eq!(retrieved.origin_ref, Some("feed_item_123".to_owned()));
    assert_eq!(retrieved.title, Some("Article Title".to_owned()));
}
