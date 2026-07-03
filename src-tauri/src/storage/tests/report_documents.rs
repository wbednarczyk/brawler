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
    assert_eq!(
        updated.local_path,
        Some("report_documents/doc_abc.pdf".to_owned())
    );
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

fn espi_item_with_attachments(
    company: &Company,
    article_id: &str,
    title: &str,
    attachments: Vec<BankierCompanyAttachment>,
) -> BankierCompanyItem {
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
        body_text: Some("Treść raportu okresowego.".to_owned()),
        attachments,
        detail_fetch_attempted: true,
    }
}

#[test]
fn periodic_report_attachments_register_pending_for_full_fetch() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let items = vec![espi_item_with_attachments(
        &company,
        "9100001",
        "Skonsolidowany raport kwartalny QSr 1/2026",
        vec![BankierCompanyAttachment {
            label: "Raport XHTML".to_owned(),
            url: "https://bonnier.pl/report-q1.xhtml".to_owned(),
        }],
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should register attachments");

    let docs = state
        .list_report_documents_by_company(&company.id)
        .expect("documents should list");

    assert_eq!(docs.len(), 1);
    let doc = &docs[0];
    assert_eq!(doc.source_type, "espi_attachment");
    assert_eq!(doc.url, "https://bonnier.pl/report-q1.xhtml");
    assert_eq!(doc.fetch_status, "pending");
    assert!(doc.local_path.is_none());
    assert_eq!(doc.attribution, Some("Bankier.pl".to_owned()));
    assert!(doc.origin_ref.is_some());

    let pending = state
        .list_pending_attachment_documents()
        .expect("pending attachments should list");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, doc.id);
}

#[test]
fn non_periodic_attachments_register_metadata_only() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let mut items = vec![espi_item_with_attachments(
        &company,
        "9100002",
        "Powiadomienie o transakcjach na akcjach - art. 19 ust. 1 MAR",
        vec![BankierCompanyAttachment {
            label: "Załącznik".to_owned(),
            url: "https://bonnier.pl/insider-notice.pdf".to_owned(),
        }],
    )];
    items[0].body_text =
        Some("Powiadomienie o transakcji osoby pełniącej obowiązki zarządcze.".to_owned());
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should register attachments");

    let docs = state
        .list_report_documents_by_company(&company.id)
        .expect("documents should list");

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].fetch_status, "metadata_only");
    assert!(docs[0].local_path.is_none());

    let pending = state
        .list_pending_attachment_documents()
        .expect("pending attachments should list");
    assert!(pending.is_empty());
}

/// ADR 0061 decision 1b: a structured ESEF/iXBRL (`.xhtml`) attachment is
/// always a fetch candidate, even on a non-periodic-classified filing — the
/// periodic-report text classifier can miss an xhtml-only ESEF filing. Its
/// `.xades` digital-signature sibling stays `metadata_only` (no data to
/// fetch); a plain PDF sibling on the same non-periodic item also stays
/// `metadata_only` as before.
#[test]
fn structured_xhtml_attachment_registers_pending_even_on_non_periodic_item() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let mut items = vec![espi_item_with_attachments(
        &company,
        "9100004",
        "Powiadomienie o transakcjach na akcjach - art. 19 ust. 1 MAR",
        vec![
            BankierCompanyAttachment {
                label: "Raport XHTML".to_owned(),
                url: "https://bonnier.pl/static/att/emitent/2026-05/report.xhtml".to_owned(),
            },
            BankierCompanyAttachment {
                label: "Raport PDF".to_owned(),
                url: "https://bonnier.pl/static/att/emitent/2026-05/report.pdf".to_owned(),
            },
            BankierCompanyAttachment {
                label: "Podpis".to_owned(),
                url: "https://bonnier.pl/static/att/emitent/2026-05/report.xades".to_owned(),
            },
        ],
    )];
    items[0].body_text =
        Some("Powiadomienie o transakcji osoby pełniącej obowiązki zarządcze.".to_owned());
    state
        .ingest_bankier_company_items(&items)
        .expect("ingestion should register attachments");

    let docs = state
        .list_report_documents_by_company(&company.id)
        .expect("documents should list");
    assert_eq!(docs.len(), 3);

    let xhtml = docs
        .iter()
        .find(|d| d.url.ends_with(".xhtml"))
        .expect("xhtml doc");
    assert_eq!(xhtml.fetch_status, "pending");

    let pdf = docs
        .iter()
        .find(|d| d.url.ends_with(".pdf"))
        .expect("pdf doc");
    assert_eq!(pdf.fetch_status, "metadata_only");

    let xades = docs
        .iter()
        .find(|d| d.url.ends_with(".xades"))
        .expect("xades doc");
    assert_eq!(xades.fetch_status, "metadata_only");

    let pending = state
        .list_pending_attachment_documents()
        .expect("pending attachments should list");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, xhtml.id);
}

#[test]
fn attachment_registration_is_idempotent_across_reruns() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let items = vec![espi_item_with_attachments(
        &company,
        "9100003",
        "Raport roczny za 2025 rok",
        vec![BankierCompanyAttachment {
            label: "Raport roczny".to_owned(),
            url: "https://bonnier.pl/annual-2025.xhtml".to_owned(),
        }],
    )];

    state
        .ingest_bankier_company_items(&items)
        .expect("first ingestion");
    state
        .ingest_bankier_company_items(&items)
        .expect("second ingestion");

    let docs = state
        .list_report_documents_by_company(&company.id)
        .expect("documents should list");
    assert_eq!(
        docs.len(),
        1,
        "re-running ingestion must not duplicate documents"
    );
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

/// Migration 0058 (ADR 0061 decision 1b): a structured `.xhtml` attachment
/// stuck `metadata_only` from before the per-attachment gate existed is
/// flipped back to `pending`. An already-fetched xhtml doc and any non-xhtml
/// `metadata_only` doc (a PDF) are left untouched. Idempotent: re-applying
/// must not re-touch a document a second time.
#[test]
fn migration_0058_flips_legacy_metadata_only_structured_attachments_to_pending() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    {
        let raw = state.checkout().expect("connection should check out");
        raw.execute_batch(&format!(
            "
            INSERT INTO report_documents (id, company_id, source_type, url, fetch_status, local_path)
            VALUES
                ('doc_legacy_xhtml', '{company_id}', 'espi_attachment',
                 'https://bonnier.pl/static/att/emitent/2025-11/legacy.xhtml', 'metadata_only', NULL),
                ('doc_legacy_pdf', '{company_id}', 'espi_attachment',
                 'https://bonnier.pl/static/att/emitent/2025-11/legacy.pdf', 'metadata_only', NULL),
                ('doc_fetched_xhtml', '{company_id}', 'espi_attachment',
                 'https://bonnier.pl/static/att/emitent/2025-11/fetched.xhtml', 'fetched',
                 'report_documents/doc_fetched_xhtml.xhtml');
            ",
            company_id = company.id
        ))
        .expect("legacy rows should seed");
    }

    // Re-run the forward migration twice: it must converge legacy structured
    // attachments and be idempotent.
    {
        let raw = state.checkout().expect("connection should check out");
        let sql = include_str!("../../../migrations/0058_structured_attachment_fetch_gate.sql");
        raw.execute_batch(sql).expect("migration should apply");
        raw.execute_batch(sql)
            .expect("migration should be idempotent");
    }

    let legacy_xhtml = state
        .get_report_document("doc_legacy_xhtml")
        .expect("legacy xhtml doc should retrieve");
    assert_eq!(legacy_xhtml.fetch_status, "pending");
    assert!(legacy_xhtml.local_path.is_none());

    let legacy_pdf = state
        .get_report_document("doc_legacy_pdf")
        .expect("legacy pdf doc should retrieve");
    assert_eq!(
        legacy_pdf.fetch_status, "metadata_only",
        "a non-xhtml metadata_only sibling must not be touched"
    );

    let fetched_xhtml = state
        .get_report_document("doc_fetched_xhtml")
        .expect("already-fetched xhtml doc should retrieve");
    assert_eq!(
        fetched_xhtml.fetch_status, "fetched",
        "an already-fetched document must not be reset to pending"
    );

    let pending = state
        .list_pending_attachment_documents()
        .expect("pending attachments should list");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "doc_legacy_xhtml");
}
