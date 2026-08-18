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

/// Reads the raw `doc_kind` column for one document (the DTO does not carry it
/// until T1.4, so tests query it directly).
fn doc_kind_of(state: &AppState, id: &str) -> Option<String> {
    let raw = state.checkout().expect("connection should check out");
    raw.query_row(
        "SELECT doc_kind FROM report_documents WHERE id = ?1",
        rusqlite::params![id],
        |row| row.get(0),
    )
    .expect("doc_kind column should read")
}

/// T1.2: inserting a report document classifies it immediately (no lingering
/// NULL) — a consolidated statement title lands as `periodic_ssf`.
#[test]
fn insert_sets_doc_kind_immediately() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let doc = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/raport/SSF.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Skonsolidowane sprawozdanie finansowe Grupy 2025".to_owned()),
            attribution: None,
        })
        .expect("document should create");

    assert_eq!(
        doc_kind_of(&state, &doc.id).as_deref(),
        Some("periodic_ssf"),
        "insert must classify the document, not leave doc_kind NULL"
    );
}

/// T1.2: an upsert that changes the stored title reclassifies in place — a row
/// first stored as governance flips to periodic_jsf when re-ingested with a
/// standalone-statement title.
#[test]
fn upsert_with_changed_title_reclassifies() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let url = "https://example.com/doc.xhtml";
    let first = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Projekty uchwał ZWZ".to_owned()),
            attribution: None,
        })
        .expect("first create should succeed");
    assert_eq!(
        doc_kind_of(&state, &first.id).as_deref(),
        Some("governance")
    );

    let second = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Jednostkowe sprawozdanie finansowe za 2025".to_owned()),
            attribution: None,
        })
        .expect("upsert should succeed");

    assert_eq!(first.id, second.id, "same (company, url) must be one row");
    assert_eq!(
        second.title.as_deref(),
        Some("Jednostkowe sprawozdanie finansowe za 2025"),
        "upsert must refresh the stored title"
    );
    assert_eq!(
        doc_kind_of(&state, &first.id).as_deref(),
        Some("periodic_jsf"),
        "changed title must reclassify the document"
    );
}

/// T1.2: `reclassify_report_documents` classifies every row and is idempotent —
/// a second run reports `updated == 0` and the same per-kind counts.
#[test]
fn reclassify_report_documents_is_idempotent() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    // Seed rows directly with a NULL doc_kind to mimic pre-0061 documents.
    let seeds = [
        (
            "doc_ssf",
            "Skonsolidowane sprawozdanie finansowe Grupy 2025",
            "periodic_ssf",
        ),
        (
            "doc_jsf",
            "Jednostkowe sprawozdanie finansowe za 2025",
            "periodic_jsf",
        ),
        (
            "doc_auditor",
            "Opinia i raport biegłego rewidenta",
            "auditor_opinion",
        ),
        ("doc_gov", "Projekty uchwał ZWZ", "governance"),
        ("doc_other", "Wybrane dane finansowe", "other"),
    ];
    {
        let raw = state.checkout().expect("connection should check out");
        for (id, title, _) in &seeds {
            raw.execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, title, fetch_status, doc_kind)
                 VALUES (?1, ?2, 'user_url', ?3, ?4, 'pending', NULL)",
                rusqlite::params![id, company.id, format!("https://example.com/{id}"), title],
            )
            .expect("seed row should insert");
        }
    }

    let summary = state
        .reclassify_report_documents()
        .expect("reclassify should run");
    assert_eq!(summary.total, seeds.len());
    assert_eq!(summary.updated, seeds.len(), "every NULL row is classified");
    for (id, _, expected) in &seeds {
        assert_eq!(
            doc_kind_of(&state, id).as_deref(),
            Some(*expected),
            "{id} should classify as {expected}"
        );
    }
    for kind in [
        "periodic_ssf",
        "periodic_jsf",
        "auditor_opinion",
        "governance",
        "other",
    ] {
        assert_eq!(summary.by_kind.get(kind).copied(), Some(1), "one {kind}");
    }

    let second = state
        .reclassify_report_documents()
        .expect("reclassify should re-run");
    assert_eq!(second.total, seeds.len());
    assert_eq!(second.updated, 0, "a second run must be a no-op");
    assert_eq!(second.by_kind, summary.by_kind, "final counts are stable");
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

// ---------------------------------------------------------------------------
// Report-bytes protection (#359, ADR 0098 dec. 3): document_bytes_are_protected
// / mark_metadata_only's atomic guard. Five physical legs, tested separately,
// plus negatives that must NOT protect, plus the unprotected happy path.
// ---------------------------------------------------------------------------

fn fresh_doc(state: &AppState, company_id: &str, suffix: &str) -> ReportDocument {
    state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "user_url".to_owned(),
            url: format!("https://example.com/protect-{suffix}.pdf"),
            period_id: None,
            origin_ref: None,
            title: None,
            attribution: None,
        })
        .expect("doc should create")
}

fn new_fact(
    company_id: &str,
    period_id: &str,
    definition_id: &str,
    doc_id: &str,
) -> NewFinancialFact {
    NewFinancialFact {
        company_id: company_id.to_owned(),
        period_id: period_id.to_owned(),
        definition_id: definition_id.to_owned(),
        value_numeric: "1000".to_owned(),
        currency: Some("PLN".to_owned()),
        statement_basis: None,
        attribution: None,
        variant: None,
        measure_window: None,
        data_quality: None,
        as_reported_value: None,
        as_reported_scale: None,
        reporting_standard: None,
        extraction_method: None,
        confidence: None,
        confirmation_state: Some("confirmed".to_owned()),
        supersedes_id: None,
        source_document_ref: Some(doc_id.to_owned()),
        annotation: None,
    }
}

fn total_assets_definition_id(state: &AppState) -> String {
    state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("definitions should list")
        .iter()
        .find(|d| d.metric_key == "total_assets")
        .expect("total_assets should exist canonically")
        .id
        .clone()
}

#[test]
fn mark_metadata_only_refuses_when_a_confirmed_fact_cites_the_document() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);
    let doc = fresh_doc(&state, &company.id, "fact");
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: Some("2025-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("period should create");
    let definition_id = total_assets_definition_id(&state);
    state
        .create_financial_fact(new_fact(&company.id, &period.id, &definition_id, &doc.id))
        .expect("fact should create");

    let error = state
        .mark_report_document_metadata_only(&doc.id)
        .expect_err("a confirmed fact citing the doc must protect its bytes");
    assert!(matches!(
        error,
        StorageError::ReportDocumentBytesProtected { .. }
    ));
}

#[test]
fn mark_metadata_only_refuses_when_notebook_evidence_cites_the_document() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);
    let doc = fresh_doc(&state, &company.id, "notebook");

    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO notebook_entries (id, company_id, title, body)
         VALUES ('note1', ?1, 'Note', 'Body')",
        params![company.id],
    )
    .expect("seed notebook entry");
    raw.execute(
        "INSERT INTO notebook_entry_origins (id, notebook_entry_id, source_type, source_id)
         VALUES ('orig1', 'note1', 'report_document', ?1)",
        params![doc.id],
    )
    .expect("seed notebook origin");
    drop(raw);

    let error = state
        .mark_report_document_metadata_only(&doc.id)
        .expect_err("research evidence citing the doc must protect its bytes");
    assert!(matches!(
        error,
        StorageError::ReportDocumentBytesProtected { .. }
    ));
}

#[test]
fn mark_metadata_only_refuses_when_a_management_claim_cites_the_document() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);
    let doc = fresh_doc(&state, &company.id, "claim");

    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO management_claims (id, company_id, statement, source_evidence_type, source_evidence_id)
         VALUES ('claim1', ?1, 'We will grow', 'report_document', ?2)",
        params![company.id, doc.id],
    )
    .expect("seed management claim");
    drop(raw);

    let error = state
        .mark_report_document_metadata_only(&doc.id)
        .expect_err("a management claim's evidence must protect the doc's bytes");
    assert!(matches!(
        error,
        StorageError::ReportDocumentBytesProtected { .. }
    ));
}

/// Seeds a `feed_items` row plus a `company_signals` row for it, and points
/// `doc`'s `origin_ref` at that feed item — the join `document_bytes_are_protected`
/// leg (c) requires.
fn seed_signal_for_doc(
    state: &AppState,
    company_id: &str,
    doc_id: &str,
    feed_item_id: &str,
    status: &str,
) {
    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO feed_items
            (id, type, source_adapter_id, source_name, source_url, title, fetched_at, dedupe_key)
         VALUES (?1, 'official_report', 'gpw-espi-ebi', 'GPW ESPI/EBI', 'https://x/item',
                 'Item', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?1)",
        params![feed_item_id],
    )
    .expect("seed feed item");
    raw.execute(
        "INSERT INTO company_signals (id, company_id, feed_item_id, category, classified_by, status)
         VALUES (?1, ?2, ?3, 'other', 'rule', ?4)",
        params![format!("sig_{feed_item_id}"), company_id, feed_item_id, status],
    )
    .expect("seed signal");
    raw.execute(
        "UPDATE report_documents SET origin_ref = ?1 WHERE id = ?2",
        params![feed_item_id, doc_id],
    )
    .expect("point origin_ref at the feed item");
}

#[test]
fn mark_metadata_only_refuses_when_a_confirmed_signal_derives_from_the_document() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);
    let doc = fresh_doc(&state, &company.id, "signal");
    seed_signal_for_doc(&state, &company.id, &doc.id, "feed1", "confirmed");

    let error = state
        .mark_report_document_metadata_only(&doc.id)
        .expect_err("a confirmed signal derived from the doc must protect its bytes");
    assert!(matches!(
        error,
        StorageError::ReportDocumentBytesProtected { .. }
    ));
}

#[test]
fn mark_metadata_only_refuses_when_any_kpi_ingest_run_references_the_document_even_terminal() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);
    let doc = fresh_doc(&state, &company.id, "run");

    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id, profile_version, status)
         VALUES ('run1', ?1, ?2, 'p1', 'failed')",
        params![doc.id, company.id],
    )
    .expect("seed a TERMINAL run");
    drop(raw);

    let error = state
        .mark_report_document_metadata_only(&doc.id)
        .expect_err("ANY durable run, even a terminal/failed one, must protect the doc's bytes");
    assert!(matches!(
        error,
        StorageError::ReportDocumentBytesProtected { .. }
    ));
}

#[test]
fn mark_metadata_only_negatives_do_not_protect() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    // An UNCONFIRMED fact does not protect.
    let doc_fact = fresh_doc(&state, &company.id, "unconfirmed-fact");
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2025,
            period_type: "FY".to_owned(),
            period_end_date: Some("2025-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("period should create");
    let definition_id = total_assets_definition_id(&state);
    let raw = state.checkout_for_tests().expect("raw connection");
    raw.execute(
        "INSERT INTO financial_facts
            (id, company_id, period_id, definition_id, value_numeric, confirmation_state, source_document_ref)
         VALUES ('fact_unconfirmed', ?1, ?2, ?3, '1000', 'auto_unreviewed', ?4)",
        params![company.id, period.id, definition_id, doc_fact.id],
    )
    .expect("seed unconfirmed fact");
    drop(raw);
    state
        .mark_report_document_metadata_only(&doc_fact.id)
        .expect("an unconfirmed fact must not protect the doc");

    // A `proposed` signal does not protect.
    let doc_signal = fresh_doc(&state, &company.id, "proposed-signal");
    seed_signal_for_doc(
        &state,
        &company.id,
        &doc_signal.id,
        "feed_proposed",
        "proposed",
    );
    state
        .mark_report_document_metadata_only(&doc_signal.id)
        .expect("a proposed (unconfirmed) signal must not protect the doc");

    // A CONFIRMED signal exists in the table, but for a DIFFERENT document's
    // origin_ref — the join must scope by THIS document's origin_ref, never
    // "any confirmed signal exists anywhere".
    let doc_other = fresh_doc(&state, &company.id, "unrelated-origin-owner");
    seed_signal_for_doc(
        &state,
        &company.id,
        &doc_other.id,
        "feed_other",
        "confirmed",
    );
    let doc_wrong_origin = fresh_doc(&state, &company.id, "wrong-origin");
    {
        let raw = state.checkout_for_tests().expect("raw connection");
        raw.execute(
            "UPDATE report_documents SET origin_ref = 'feed_unrelated' WHERE id = ?1",
            params![doc_wrong_origin.id],
        )
        .expect("point origin_ref at an unrelated feed item with no signal");
    }
    state
        .mark_report_document_metadata_only(&doc_wrong_origin.id)
        .expect("a confirmed signal owned by a DIFFERENT document's origin_ref must not protect this one");
}

#[test]
fn document_bytes_are_protected_reports_true_and_false() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let protected_doc = fresh_doc(&state, &company.id, "predicate-protected");
    {
        let raw = state.checkout_for_tests().expect("raw connection");
        raw.execute(
            "INSERT INTO kpi_ingest_runs (id, report_document_id, company_id, profile_version, status)
             VALUES ('run_pred', ?1, ?2, 'p1', 'extracting')",
            params![protected_doc.id, company.id],
        )
        .expect("seed run");
    }
    let unprotected_doc = fresh_doc(&state, &company.id, "predicate-unprotected");

    let raw = state.checkout_for_tests().expect("raw connection");
    assert!(
        crate::storage::report_documents::document_bytes_are_protected(&raw, &protected_doc.id)
            .expect("predicate should run"),
        "a document referenced by a durable run must report protected"
    );
    assert!(
        !crate::storage::report_documents::document_bytes_are_protected(&raw, &unprotected_doc.id)
            .expect("predicate should run"),
        "an unreferenced document must report unprotected"
    );
}

#[test]
fn document_bytes_are_protected_by_a_tagged_fact_extraction_in_any_state() {
    // ADR 0100 decision 8: Layer 1 is derived data rebuilt from the
    // document's stored bytes, so a pruned document could never be
    // rebuilt — a report_tagged_fact_extractions row (any state, mirroring
    // the kpi_ingest_runs leg) must protect the document's bytes.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);

    let protected_doc = fresh_doc(&state, &company.id, "tagged-fact-protected");
    {
        let raw = state.checkout_for_tests().expect("raw connection");
        raw.execute(
            "INSERT INTO report_tagged_fact_extractions
                (report_document_id, extractor_version, state)
             VALUES (?1, 1, 'extraction_failed')",
            params![protected_doc.id],
        )
        .expect("seed extraction record");
    }
    let unprotected_doc = fresh_doc(&state, &company.id, "tagged-fact-unprotected");

    let raw = state.checkout_for_tests().expect("raw connection");
    assert!(
        crate::storage::report_documents::document_bytes_are_protected(&raw, &protected_doc.id)
            .expect("predicate should run"),
        "a document with a tagged-fact extraction record (any state) must report protected"
    );
    assert!(
        !crate::storage::report_documents::document_bytes_are_protected(&raw, &unprotected_doc.id)
            .expect("predicate should run"),
        "a document with no extraction record must report unprotected"
    );
}

#[test]
fn mark_metadata_only_succeeds_on_an_unprotected_document() {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let company = test_company(&state);
    let doc = fresh_doc(&state, &company.id, "unprotected");

    let updated = state
        .mark_report_document_metadata_only(&doc.id)
        .expect("an unprotected document must downgrade");
    assert_eq!(updated.fetch_status, "metadata_only");
    assert!(updated.local_path.is_none());
}
