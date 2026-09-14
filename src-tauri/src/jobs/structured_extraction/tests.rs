use super::*;
use crate::app_state::AppState;
// MODE_AUTOPILOT is test-only — production does not branch on mode for
// confirmation state (ADR 0086 dec. 5).
use crate::storage::{
    open_in_memory_database, CaptureReportDocumentInput, ListKpiDefinitionsInput, NewCompany,
    NewFinancialFact, NewFinancialPeriod, MODE_ASSIST, MODE_AUTOPILOT,
};
// Shared test-only fixtures (moved out of this module, #331 PR-A): both this
// module's tests and the ESEF measurement-v2 harness build synthetic ZIP
// packages/documents the same way.
use crate::test_support::{
    esef_presentation_linkbase_xml as presentation_linkbase_xml, minimal_zip,
    seed_document_with_bytes, unique_temp_dir,
};

/// The outcome row records the facts AT the slot, so a re-run that
/// re-observed everything cannot overwrite a healthy count with `0` while
/// still claiming `emitted`.
#[test]
fn slot_fact_count_counts_reobservations_not_only_new_facts() {
    let produced = vec!["fact_1".to_owned(), "fact_2".to_owned()];
    let reobserved = vec![
        "fact_3".to_owned(),
        "fact_4".to_owned(),
        "fact_5".to_owned(),
    ];

    assert_eq!(slot_fact_count(&produced, &[]), 2);
    // The re-run of a landed period: nothing new, five facts at the slot.
    assert_eq!(slot_fact_count(&[], &reobserved), 3);
    assert_eq!(slot_fact_count(&produced, &reobserved), 5);
    // A genuinely empty slot still records zero — the honest zero.
    assert_eq!(slot_fact_count(&[], &[]), 0);
}

/// The balance-sheet trio every fixture below tags — classified `balance`
/// (role family `-210000`, ADR 0100 decision 3) so `Assets`/`Liabilities`/
/// `Equity` survive Layer 2 projection's primary-statement role filter.
const BALANCE_SHEET_ROLES: &[(&str, &str)] = &[
    ("Assets", "ias_1_role-210000"),
    ("Liabilities", "ias_1_role-210000"),
    ("Equity", "ias_1_role-210000"),
];

// --- route_document: magic-byte container routing (card eb71488) --------
// The routing decision is a pure function of the bytes, so it is provable
// without an AppState — these pin every arm the router relies on.

#[test]
fn route_pdf_bytes_keep_the_pdf_tier() {
    // A real `%PDF` document routes exactly as before this card — no regression.
    let pdf = minimal_text_pdf(&["Przychody 100"]);
    assert_eq!(route_document(&pdf), DocumentRoute::Pdf);
}

#[test]
fn route_non_ixbrl_markup_under_pdf_name_goes_positional() {
    // The maintainer's pdf2htmlEX render: XML preamble, an `<html>` root, no
    // `ix:` tags. It must reach the positional parser, not the PDF reader.
    let render = b"\xEF\xBB\xBF<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
            <!-- Created by pdf2htmlEX -->\n<html xmlns=\"http://www.w3.org/1999/xhtml\">\
            <body><div>Przychody 100</div></body></html>";
    assert_eq!(route_document(render), DocumentRoute::Positional);
}

#[test]
fn route_ixbrl_markup_goes_to_the_esef_instance() {
    let ixbrl = br#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"><body>
            <ix:nonFraction name="ifrs-full:Revenue">100</ix:nonFraction></body></html>"#;
    assert_eq!(route_document(ixbrl), DocumentRoute::IxbrlInstance);
}

#[test]
fn route_zip_under_pdf_name_goes_to_the_package_path() {
    let zip = minimal_zip(&[("reports/instance.xhtml", b"<html></html>")]);
    assert_eq!(route_document(&zip), DocumentRoute::ZipPackage);
}

#[test]
fn route_garbage_bytes_are_unsupported() {
    assert_eq!(
        route_document(b"\x00\x01\x02 definitely not a document"),
        DocumentRoute::Unsupported(Container::Unknown)
    );
}

#[test]
fn xml_content_under_pdf_name_routes_on_bytes_not_the_pdf_reader() {
    // The maintainer's real failure (card eb71488): a pdf2htmlEX render
    // stored as `*.pdf`. Routing must key on the MAGIC BYTES, not the
    // filename — this document must never be handed to the PDF reader
    // (which would either fail outright or silently produce nothing).
    // ADR 0095: the positional parser is retired, so the correctly-routed
    // outcome is the same benign-empty result `DocumentRoute::Pdf`
    // returns — this test proves routing is byte-driven.
    let bytes = POSITIONAL_XHTML.as_bytes();
    assert_eq!(
        route_document(bytes),
        DocumentRoute::Positional,
        "magic bytes must route this as bare markup, never DocumentRoute::Pdf"
    );

    let (state, company_id, document_id) = seed_document_with_bytes(
        "xml-under-pdf",
        "CDR",
        "Interim condensed consolidated statement Q3 2024",
        "raport_q3_2024_signed.pdf",
        bytes,
    );
    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2024,
        "Q3",
        "2024-09-30",
        MODE_AUTOPILOT,
    )
    .expect("routing on bytes must never error, whatever the filename says");
    assert!(
        !result.emitted && result.produced_fact_ids.is_empty(),
        "the retired positional route must emit nothing — never a PDF-reader failure either"
    );
}

#[test]
fn garbage_under_pdf_name_records_document_unreadable_with_container() {
    // Genuine garbage under a `.pdf` name lands an explicit, typed outcome
    // naming the detected container — never a mute PDF-reader failure, and
    // never an error that aborts the sweep (the caller catches the Err).
    let (state, company_id, document_id) = seed_document_with_bytes(
        "garbage-under-pdf",
        "ATR",
        "Zawiadomienie",
        "zawiadomienie.pdf",
        b"\x00\x01\x02\x03 not a document at all",
    );
    let err = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "Q1",
        "2025-03-31",
        MODE_AUTOPILOT,
    )
    .expect_err("an unsupported container is a recorded gap, surfaced as Err");
    assert!(
        err.contains("unsupported container"),
        "err names the gap: {err}"
    );

    let outcome = all_outcomes(&state, &company_id)
        .into_iter()
        .find(|o| o.report_document_id == document_id)
        .expect("an outcome row is recorded, not silence");
    assert_eq!(outcome.reason_code, reason::DOCUMENT_UNREADABLE);
    let detail = outcome.detail_json.expect("detail names the container");
    assert!(
        detail.contains("\"detectedContainer\":\"unknown\""),
        "detailJson must name the detected container: {detail}"
    );
}

/// Builds a minimal, valid single-page PDF whose extracted text reproduces
/// `lines` (one label+value statement line per output line) — just enough
/// PDF structure for `pdf-extract` to recover plain ASCII content, without a
/// PDF-writing dependency. Byte offsets in the xref table are computed as
/// the buffer is assembled, so it stays valid however `lines` changes.
fn minimal_text_pdf(lines: &[&str]) -> Vec<u8> {
    // `extract_pdf` treats anything under 200 chars/page as a scanned
    // no-text-layer document (`MIN_CHARS_PER_PAGE`); pad with boilerplate
    // filler lines so a short statement excerpt still clears that density
    // floor, the way a real report page (full of surrounding prose) would.
    let filler = "Nota objasniajaca do sprawozdania finansowego za okres sprawozdawczy.";
    let mut all_lines: Vec<&str> = lines.to_vec();
    while all_lines.iter().map(|l| l.len() + 1).sum::<usize>() < 220 {
        all_lines.push(filler);
    }

    let mut content = String::from("BT /F1 12 Tf 40 750 Td 16 TL\n");
    for (i, line) in all_lines.iter().enumerate() {
        if i > 0 {
            content.push_str("T*\n");
        }
        let escaped = line
            .replace('\\', "\\\\")
            .replace('(', "\\(")
            .replace(')', "\\)");
        content.push_str(&format!("({escaped}) Tj\n"));
    }
    content.push_str("ET");

    let objects = [
            "<</Type/Catalog/Pages 2 0 R>>".to_owned(),
            "<</Type/Pages/Kids[3 0 R]/Count 1>>".to_owned(),
            "<</Type/Page/Parent 2 0 R/Resources<</Font<</F1 4 0 R>>>>/MediaBox[0 0 612 792]/Contents 5 0 R>>"
                .to_owned(),
            "<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>".to_owned(),
            format!(
                "<</Length {}>>\nstream\n{}\nendstream",
                content.len(),
                content
            ),
        ];

    let mut buf = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(buf.len());
        buf.extend_from_slice(format!("{} 0 obj\n{obj}\nendobj\n", i + 1).as_bytes());
    }
    let xref_offset = buf.len();
    buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    buf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in &offsets {
        buf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    buf.extend_from_slice(
        format!(
            "trailer\n<</Size {}/Root 1 0 R>>\nstartxref\n{}\n%%EOF",
            objects.len() + 1,
            xref_offset
        )
        .as_bytes(),
    );
    buf
}

const ESEF: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
    </html>"#;

/// A package (not a bare instance): a bare, non-package iXBRL instance has
/// no presentation linkbase to read, so its facts get no role rows and
/// never survive Layer 2 projection's role filter (ADR 0100 decision 3) —
/// which every real GPW filing sidesteps by shipping as a package anyway.
fn seed_esef() -> (AppState, String, String) {
    let dir = unique_temp_dir("esef");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/annual-2026.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Annual 2026 ESEF".to_owned()),
            attribution: None,
        })
        .expect("document");
    let pre_xml = presentation_linkbase_xml(BALANCE_SHEET_ROLES);
    let bytes = minimal_zip(&[
        ("reports/annual-2026.xhtml", ESEF.as_bytes()),
        ("www/annual-2026_pre.xml", pre_xml.as_bytes()),
    ]);
    std::fs::write(dir.join("report.xhtml"), &bytes).expect("write esef");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.xhtml"),
            // Extension lies on purpose (card eb71488): routing reads the
            // ZIP magic bytes, never the content-type/filename.
            Some("application/octet-stream"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

/// A GENUINELY bare, non-package iXBRL instance — the exact `ESEF` markup
/// above, stored on disk with no ZIP wrapper and no `*_pre.xml` (`seed_esef`
/// above is misleadingly named: it wraps the same markup in a ZIP alongside
/// a linkbase, so it is really a package). `route_document` reads magic
/// bytes, not the filename, so this routes `DocumentRoute::IxbrlInstance`
/// (regression fix, ADR 0100 decision 3, epic #398): no linkbase evidence
/// exists for this document at all.
fn seed_bare_esef() -> (AppState, String, String) {
    let dir = unique_temp_dir("bare-esef");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "BAR".to_owned(),
            display_name: "Bare Instance S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/bare-2026.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Bare Instance Annual 2026".to_owned()),
            attribution: None,
        })
        .expect("document");
    let bytes = ESEF.as_bytes();
    std::fs::write(dir.join("report.xhtml"), bytes).expect("write bare esef");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.xhtml"),
            Some("application/xhtml+xml"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

/// A minimal ESEF report package (ZIP) whose inner `reports/` instance is a
/// balanced iXBRL statement at `2025-12-31` — the shape of a real GPW `.xbri`
/// annual filing, without shipping a real one. Includes a dimensional
/// (`explicitMember`) Equity component that must be filtered out.
fn esef_package_bytes() -> Vec<u8> {
    let instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:xbrldi="http://xbrl.org/2006/xbrldi"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="i"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:context id="nci"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period>
        <xbrli:scenario><xbrldi:explicitMember dimension="ifrs-full:ComponentsOfEquityAxis">ifrs-full:NoncontrollingInterestsMember</xbrldi:explicitMember></xbrli:scenario>
      </xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="i" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="i" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="i" unitRef="pln" scale="3">25 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="nci" unitRef="pln" scale="3">3 000</ix:nonFraction>
    </html>"#;
    let pre_xml = presentation_linkbase_xml(BALANCE_SHEET_ROLES);
    minimal_zip(&[
        (
            "CBF-2025-12-31-1-pl/reports/CBF-2025-12-31-1-pl.xhtml",
            instance.as_bytes(),
        ),
        (
            "CBF-2025-12-31-1-pl/www/CBF-2025-12-31-1-pl_pre.xml",
            pre_xml.as_bytes(),
        ),
    ])
}

fn seed_esef_package() -> (AppState, String, String) {
    let dir = unique_temp_dir("esef-pkg");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "Cyber_Folks S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://example.com/CBF-2025-12-31-1-pl.xbri".to_owned(),
            period_id: None,
            origin_ref: None,
            // Title carries no parseable period on purpose — the period MUST
            // come from the iXBRL contexts, not the filename.
            title: Some("CBF-2025-12-31-1-pl.xbri".to_owned()),
            attribution: None,
        })
        .expect("document");
    let bytes = esef_package_bytes();
    std::fs::write(dir.join("report.xbri"), &bytes).expect("write package");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.xbri"),
            // A real `.xbri` is stored with a generic content type.
            Some("application/octet-stream"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

/// A stored PDF whose title/URL name no period at all — the real
/// `SSF.pdf` / `Benefit_Systems_SSF_Raport_signed.pdf` attachment shape —
/// with `cover` as its first page text.
fn seed_untitled_pdf(cover: &[&str]) -> (AppState, String) {
    let dir = unique_temp_dir("cover-period");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "ABC".to_owned(),
            display_name: "ABC S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://example.com/SSF.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("SSF.pdf".to_owned()),
            attribution: None,
        })
        .expect("document");
    let bytes = minimal_text_pdf(cover);
    std::fs::write(dir.join("ssf.pdf"), &bytes).expect("write pdf");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("ssf.pdf"),
            Some("application/pdf"),
            // The real capture path always records the content hash
            // (report_documents_capture.rs) — the provenance-aware cache
            // predicate (migration 0140) needs the realistic shape.
            Some(&format!("{:064x}", bytes.len())),
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, document.id)
}

#[test]
fn period_falls_back_to_the_documents_own_cover_page() {
    // Card fc692da: on the maintainer's database a run of periodic statements
    // is stored as a bare `SSF.pdf` — nothing in the title or URL names a
    // period, so the document never reached extraction. Its cover page states
    // the period, and the SAME grammar reads it.
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "za okres 6 miesiecy zakonczony 30.06.2025",
    ]);
    let document = state.get_report_document(&document_id).expect("document");
    assert_eq!(
        derive_report_period(&state, &document),
        Some((2025, "H1", "2025-06-30".to_owned()))
    );
}

/// `is_esef_route` decides — without reading the file — whether a document
/// goes down the ESEF/iXBRL path.
#[test]
fn esef_route_follows_the_sniffed_container_not_the_pdf_name() {
    let (state, _company_id, document_id) = seed_document_with_bytes(
        "esef-route-liar",
        "PKN",
        "Skonsolidowane sprawozdanie finansowe 2024",
        "ssf_2024_signed.pdf",
        POSITIONAL_XHTML.as_bytes(),
    );
    // Never sniffed: the `.pdf` name decides when no container is stamped.
    let unsniffed = state.get_report_document(&document_id).expect("document");
    assert!(!is_esef_route(&unsniffed));

    for (container, expected) in [
        ("xml", true),
        ("html", true),
        ("zip", true),
        ("pdf", false),
        ("unknown", false),
    ] {
        state
            .set_report_document_detected_container(&document_id, container)
            .expect("stamp container");
        let document = state.get_report_document(&document_id).expect("document");
        assert_eq!(
            is_esef_route(&document),
            expected,
            "a document sniffed as `{container}` under a .pdf name"
        );
    }
}

/// Epic #229 T2: the residual→PDF-sibling fallback exists because a pdf2htmlEX
/// container has no usable text layer and its real content sits in the
/// companion PDF. Both ends must be container truth: a sibling that is a ZIP
/// package wearing a `.pdf` name has no text layer either, so choosing it
/// swaps one unreadable document for another.
#[test]
fn pdf_sibling_selection_uses_container_truth_on_both_ends() {
    let dir = unique_temp_dir("sibling-container");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "SIB".to_owned(),
            display_name: "Sibling S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");

    let seed = |file: &str, title: &str, bytes: &[u8], container: &str| -> String {
        let document = state
            .create_or_find_pending_report_document(CaptureReportDocumentInput {
                company_id: company.id.clone(),
                source_type: "espi_attachment".to_owned(),
                url: format!("https://example.com/{file}"),
                period_id: None,
                origin_ref: None,
                title: Some(title.to_owned()),
                attribution: None,
            })
            .expect("document");
        assert!(
            matches!(
                document.doc_kind.as_deref(),
                Some("periodic_ssf") | Some("periodic_jsf")
            ),
            "sample title must classify as periodic, got {:?}",
            document.doc_kind
        );
        std::fs::write(dir.join(file), bytes).expect("write bytes");
        state
            .mark_report_document_fetched(
                &document.id,
                Some(file),
                Some("application/octet-stream"),
                None,
                Some(bytes.len() as i64),
            )
            .expect("mark fetched");
        state
            .set_report_document_detected_container(&document.id, container)
            .expect("stamp container");
        document.id
    };

    // The residual: markup, stored under a `.pdf` name.
    let residual_id = seed(
        "raport_q3_2024_render.pdf",
        "Skonsolidowany raport okresowy Q3 2024 SSF",
        POSITIONAL_XHTML.as_bytes(),
        "html",
    );
    // The only same-period candidate is an ESEF package wearing `.pdf`.
    let package_id = seed(
        "raport_q3_2024_pakiet.pdf",
        "Skonsolidowany raport okresowy Q3 2024 SSF pakiet",
        &minimal_zip(&[("reports/instance.xhtml", b"<html></html>")]),
        "zip",
    );
    let residual = state.get_report_document(&residual_id).expect("residual");
    assert_eq!(
        find_pdf_sibling(&state, &residual).map(|d| d.id),
        None,
        "a ZIP package is not a readable PDF sibling, whatever its name says"
    );

    // Add a genuine PDF for the same period: now the fallback has a real target.
    let pdf_id = seed(
        "raport_q3_2024_ssf.pdf",
        "Skonsolidowany raport okresowy Q3 2024 SSF podpisany",
        &minimal_text_pdf(&["Przychody 100"]),
        "pdf",
    );
    assert_eq!(
        find_pdf_sibling(&state, &residual).map(|d| d.id),
        Some(pdf_id),
        "the genuine PDF is the sibling — selected over the same-period package"
    );
    assert_ne!(residual_id, package_id);
}

/// Epic #229 T2: the cover-page tier is the last resort for a bare `SSF.pdf`
/// whose title and URL name no period. Reading that cover with the PDF reader
/// because the name ends `.pdf` returns nothing when the bytes are markup — the
/// document then has no period, so it never reaches extraction at all. Container
/// truth reads the same cover as markup and the period lands.
#[test]
fn cover_page_period_reads_markup_stored_under_a_pdf_name() {
    let dir = unique_temp_dir("cover-container");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CVR".to_owned(),
            display_name: "Cover S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: "https://example.com/SSF.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("SSF.pdf".to_owned()),
            attribution: None,
        })
        .expect("document");
    // A pdf2htmlEX render: the cover text is markup, the name says PDF.
    let body = format!(
        "<html><body><h1>SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ CVR</h1>\
             <p>za okres 6 miesiecy zakonczony 30.06.2025</p><p>{}</p></body></html>",
        "dane porownawcze oraz komentarz zarzadu. ".repeat(120)
    );
    std::fs::write(dir.join("ssf.pdf"), body.as_bytes()).expect("write render");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("ssf.pdf"),
            Some("application/pdf"),
            None,
            Some(body.len() as i64),
        )
        .expect("mark fetched");
    state
        .set_report_document_detected_container(&document.id, "html")
        .expect("stamp container");

    let stored = state.get_report_document(&document.id).expect("document");
    assert_eq!(
        derive_report_period(&state, &stored),
        Some((2025, "H1", "2025-06-30".to_owned())),
        "the cover page must be read with the reader the BYTES call for"
    );
}

#[test]
fn period_abstains_when_neither_title_nor_cover_page_names_one() {
    // The abstention contract survives the new fallback: a cover page that
    // states no period persists nothing and records `no_period_derived`
    // (ADR 0061 decision 1) — widening the parse must never turn "I don't
    // know" into a guess.
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "Nota informacyjna do sprawozdania.",
    ]);
    let document = state.get_report_document(&document_id).expect("document");
    assert_eq!(derive_report_period(&state, &document), None);
}

#[test]
fn cover_page_period_is_derived_once_then_served_from_cache() {
    // E2/C4: the bare-SSF cover-page tier costs a full PDF text extraction.
    // The first derivation persists the period (migration 0109); a second one
    // reads the cache — proven by DELETING the file between the two calls, so a
    // second call that still returns the period cannot have re-read/extracted.
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "za okres 6 miesiecy zakonczony 30.06.2025",
    ]);
    let document = state.get_report_document(&document_id).expect("document");

    let first = derive_report_period(&state, &document);
    assert_eq!(first, Some((2025, "H1", "2025-06-30".to_owned())));

    let cached = state
        .financials()
        .cached_derived_period(&document_id)
        .expect("cache read")
        .expect("the first derivation persisted a row");
    assert!(cached.has_period);

    // Any re-extraction now fails, so an identical result proves the cache.
    let local_path = document.local_path.clone().expect("local path");
    std::fs::remove_file(state.data_dir().join(&local_path)).expect("remove pdf");

    assert_eq!(
        derive_report_period(&state, &document),
        first,
        "second derivation must be served from the cache, not re-extracted"
    );
}

#[test]
fn cover_page_abstention_is_cached_as_a_none_marker() {
    // An abstention (cover page names no period) is recorded too — has_period
    // = 0 — so the next load does not re-extract a document that once again
    // yields nothing.
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "Nota informacyjna do sprawozdania.",
    ]);
    let document = state.get_report_document(&document_id).expect("document");

    assert_eq!(derive_report_period(&state, &document), None);
    let cached = state
        .financials()
        .cached_derived_period(&document_id)
        .expect("cache read")
        .expect("the abstention is persisted");
    assert!(
        !cached.has_period,
        "an abstention is an explicit none-marker"
    );

    // Delete the file: the second call must still return None from the marker
    // without touching the (now absent) file.
    let local_path = document.local_path.clone().expect("local path");
    std::fs::remove_file(state.data_dir().join(&local_path)).expect("remove pdf");
    assert_eq!(derive_report_period(&state, &document), None);
}

#[test]
fn a_stale_derivation_version_is_re_derived_not_served() {
    // Self-healing invalidation: a row stamped with an older DERIVATION_VERSION
    // is ignored and re-derived. Proven by planting a stale row with a BOGUS
    // period, deleting the file, and asserting the derive returns None (a
    // re-derivation of a now-fileless document) rather than the stale period.
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "za okres 6 miesiecy zakonczony 30.06.2025",
    ]);
    let document = state.get_report_document(&document_id).expect("document");

    state
        .financials()
        .store_derived_period(
            &document_id,
            Some((1999, "FY", "1999-12-31")),
            DERIVATION_VERSION - 1,
            // Matching provenance on purpose: this test isolates VERSION
            // staleness (the provenance axis has its own tests).
            document.content_hash.as_deref(),
        )
        .expect("plant a stale-version row");

    let local_path = document.local_path.clone().expect("local path");
    std::fs::remove_file(state.data_dir().join(&local_path)).expect("remove pdf");

    assert_eq!(
        derive_report_period(&state, &document),
        None,
        "a stale-version row must be re-derived, never served"
    );
}

#[test]
fn esef_package_derives_fy_period_from_ixbrl_not_the_filename() {
    // T7-C: a `.xbri` ZIP package resolves to the ESEF tier; the period is
    // self-derived from the unpacked instance's contexts (FY 2025-12-31),
    // even though the filename carries no parseable period.
    let (state, _company_id, document_id) = seed_esef_package();
    let document = state.get_report_document(&document_id).expect("document");
    assert_eq!(
        derive_report_period(&state, &document),
        Some((2025, "FY", "2025-12-31".to_owned()))
    );
}

#[test]
fn esef_package_extraction_emits_dimensionless_totals() {
    // T7-C end to end: the button path (derive + run) over a `.xbri` package
    // emits the three balance-sheet totals from the unpacked instance, with
    // the dimensional NCI-component Equity filtered out (total_equity = 25m,
    // not 25m+3m), so the identity validates and the set is Accepted.
    let (state, company_id, document_id) = seed_esef_package();
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");
    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    assert_eq!(result.tier, Some(SourceTier::Esef));
    assert_eq!(result.acceptance, Acceptance::Accepted);
    assert!(result.emitted, "the ESEF package should emit facts");
    assert_eq!(result.produced_fact_ids.len(), 3);
}

/// Epic #398 Item B blocker 1 regression: a re-extraction of an
/// ALREADY-LANDED period re-observes every fact (nothing new to create,
/// everything lands in `skipped_fact_ids`) — that is still a genuine
/// success, not a gap. Before this fix `emitted` was `!produced_fact_ids.
/// is_empty()` alone, so this exact shape (the one a version-aware
/// re-extraction produces) was misrecorded as `extractionAvailable:false`.
#[test]
fn a_rerun_that_only_reobserves_every_fact_is_still_emitted() {
    let (state, company_id, document_id) = seed_esef_package();
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");
    let first = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("first extraction runs");
    assert_eq!(first.produced_fact_ids.len(), 3);

    let second = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("second extraction runs");

    assert!(
        second.produced_fact_ids.is_empty(),
        "nothing new — every fact is already stored at this slot"
    );
    assert_eq!(
        second.skipped_fact_ids.len(),
        3,
        "all 3 facts re-observed identically"
    );
    assert!(
        second.emitted,
        "an all-reobserved rerun is a success, not a gap"
    );
    assert_eq!(second.acceptance, Acceptance::Accepted);
}

/// ADR 0100 decisions 1/8/9 (epic #398 slice): Layer 1 raw tagged-fact
/// capture runs alongside Layer 2 without changing its outcome — the same
/// assertions as `esef_package_extraction_emits_dimensionless_totals`
/// above, PLUS every one of the 4 real occurrences (including the
/// dimensional NCI-component Equity Layer 2 filters out) lands in
/// `report_tagged_facts`.
#[test]
fn esef_package_extraction_also_captures_layer1_tagged_facts_without_changing_layer2() {
    let (state, company_id, document_id) = seed_esef_package();
    let document = state.get_report_document(&document_id).expect("document");
    let (fiscal_year, period_type, period_end) =
        derive_report_period(&state, &document).expect("period derives");
    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        fiscal_year,
        period_type,
        &period_end,
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    // Layer 2: byte-identical to the sibling test above.
    assert_eq!(result.tier, Some(SourceTier::Esef));
    assert_eq!(result.acceptance, Acceptance::Accepted);
    assert_eq!(result.produced_fact_ids.len(), 3);

    // Layer 1: all 4 occurrences captured (3 default-member + 1
    // dimensional) — never filtered down to only what Layer 2 keeps.
    let facts = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("layer1 facts");
    assert_eq!(facts.len(), 4);
    assert_eq!(facts.iter().filter(|f| f.is_dimensional).count(), 1);
    let extraction = state
        .report_tagged_facts()
        .extraction(&document_id)
        .expect("extraction")
        .expect("extraction row exists");
    assert_eq!(extraction.encountered_count, 4);
    assert_eq!(
        extraction.stored_count, 4,
        "decision 9: encountered == stored"
    );
    assert_eq!(extraction.dimensional_count, 1);
}

/// ADR 0100 decision 8: freshness is `(source_content_hash,
/// extractor_version)`. Re-running Layer 1 capture with unchanged bytes
/// must be a no-op (same rows, same ids); different bytes must rebuild.
#[test]
fn layer1_capture_skips_unchanged_bytes_and_rebuilds_on_change() {
    let (state, company_id, document_id) = seed_esef_package();
    let bytes = esef_package_bytes();
    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &bytes,
        DocumentRoute::ZipPackage,
    );
    let first_ids: Vec<String> = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts")
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(first_ids.len(), 4);

    // Same bytes again: must skip — same generation, same ids.
    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &bytes,
        DocumentRoute::ZipPackage,
    );
    let second_ids: Vec<String> = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts")
        .into_iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(
        first_ids, second_ids,
        "unchanged bytes must skip, not mint a new generation"
    );

    // Different bytes (a genuinely different package): must rebuild.
    use std::io::Write;
    let changed_instance = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="i"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="i" unitRef="pln" scale="3">46 000</ix:nonFraction>
    </html>"#;
    let mut changed_bytes = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut changed_bytes));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(
            "CBF-2025-12-31-1-pl/reports/CBF-2025-12-31-1-pl.xhtml",
            opts,
        )
        .unwrap();
        zip.write_all(changed_instance.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &changed_bytes,
        DocumentRoute::ZipPackage,
    );
    let third_facts = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts");
    assert_eq!(
        third_facts.len(),
        1,
        "changed bytes must rebuild with the new generation's own fact set"
    );
}

/// ADR 0100 decision 1's "the package reader loses whole filings" fix: a
/// package with two real instances (a standalone filing alongside a
/// consolidated one, the real GPW shape) yields Layer 1 rows for BOTH,
/// distinguished by `package_entry_path`.
#[test]
fn a_multi_instance_package_yields_rows_for_every_instance() {
    let (state, company_id, document_id) = seed_esef_package();
    use std::io::Write;
    let instance_a = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full" xmlns:xbrli="http://www.xbrl.org/2003/instance">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" id="a1">100</ix:nonFraction>
    </html>"#;
    let instance_b = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full" xmlns:xbrli="http://www.xbrl.org/2003/instance">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2025-12-31</xbrli:instant></xbrli:period></xbrli:context>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" id="b1">200</ix:nonFraction>
    </html>"#;
    let mut buf = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("pkg/reports/standalone.xhtml", opts)
            .unwrap();
        zip.write_all(instance_a.as_bytes()).unwrap();
        zip.start_file("pkg/reports/consolidated.xhtml", opts)
            .unwrap();
        zip.write_all(instance_b.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        &buf,
        DocumentRoute::ZipPackage,
    );
    let facts = state
        .report_tagged_facts()
        .facts(&document_id)
        .expect("facts");
    assert_eq!(facts.len(), 2);
    let paths: std::collections::HashSet<&str> = facts
        .iter()
        .map(|f| f.package_entry_path.as_str())
        .collect();
    assert!(paths.contains("pkg/reports/standalone.xhtml"));
    assert!(paths.contains("pkg/reports/consolidated.xhtml"));
}

/// Regression fix (ADR 0100 decision 3, epic #398): a bare (non-package)
/// iXBRL instance carries no `*_pre.xml`, so `compute_layer1_generation`
/// must report `has_presentation_linkbase = false` and count every
/// dimensionless/valued fact as a fallback candidate — "the fallback
/// explicitly recorded ... never silent", visible on the extraction
/// record itself once captured. A package that DOES ship a linkbase
/// (`esef_package_bytes`) must report the opposite: `true` and `0`.
#[test]
fn a_bare_instance_generation_reports_no_linkbase_and_the_fallback_counter() {
    let generation = compute_layer1_generation(ESEF.as_bytes(), DocumentRoute::IxbrlInstance);
    assert!(!generation.has_presentation_linkbase);
    assert_eq!(
        generation.no_linkbase_fallback_count, 3,
        "all 3 dimensionless, valued facts (Assets/Liabilities/Equity) are \
             fallback candidates"
    );

    let package_generation =
        compute_layer1_generation(&esef_package_bytes(), DocumentRoute::ZipPackage);
    assert!(package_generation.has_presentation_linkbase);
    assert_eq!(
        package_generation.no_linkbase_fallback_count, 0,
        "a linkbase-bearing package never uses the fallback"
    );
}

/// The same evidence, visible on the STORED extraction record — the
/// counter is not just an in-memory computation, `capture_layer1_tagged_
/// facts` must persist it.
#[test]
fn a_bare_instance_extraction_record_shows_the_fallback_count() {
    let (state, company_id, document_id) = seed_esef_package();
    capture_layer1_tagged_facts(
        &state,
        &company_id,
        &document_id,
        ESEF.as_bytes(),
        DocumentRoute::IxbrlInstance,
    );
    let extraction = state
        .report_tagged_facts()
        .extraction(&document_id)
        .expect("extraction")
        .expect("extraction row exists");
    assert_eq!(extraction.no_linkbase_fallback_count, 3);
}

/// End-to-end regression proof through the REAL job entry point (not just
/// the pure pipeline): before this fix, a bare instance's facts all failed
/// the strict role filter (no linkbase to attach roles from) and the
/// document silently projected zero facts — `Acceptance::Empty`, nothing
/// emitted. It must now emit its balance-sheet trio via the no-linkbase
/// fallback.
#[test]
fn a_bare_instance_document_emits_facts_through_run_structured_extraction() {
    let (state, company_id, document_id) = seed_bare_esef();
    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    assert_eq!(result.tier, Some(SourceTier::Esef));
    assert!(
        result.emitted,
        "the no-linkbase fallback must still emit — a realistic count > 0"
    );
    assert_eq!(result.produced_fact_ids.len(), 3);
}

/// A non-iXBRL pdf2htmlEX render (an XHTML with no `ix:` tags): the visual
/// geometry with CSS coordinate maps and shredded numbers no ESEF/PDF tier can
/// read. `derive_report_period` falls through to the title (T-A1).
const POSITIONAL_XHTML: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<html xmlns="http://www.w3.org/1999/xhtml"><head>
<style type="text/css">
.x0{left:56.000000px;}
.y0{bottom:700.000000px;}
.y1{bottom:680.000000px;}
.y2{bottom:660.000000px;}
.y3{bottom:640.000000px;}
</style></head><body>
<div id="pf1" class="pf w0 h0">
<div class="t m0 x0 h4 y0 ff2">(all amounts in PLN thousand, unless stated otherwise) </div>
<div class="t m0 x0 hb y1 ff2">Sales revenue  227 555  442 682  652 375  767 692</div>
<div class="t m0 x0 hb y2 ff2">Total assets  2 755 416  2 613 500</div>
<div class="t m0 x0 hb y3 ff2">Equity  2 570 916  2 403 223</div>
</div></body></html>"#;

fn seed_positional() -> (AppState, String, String) {
    let dir = unique_temp_dir("positional");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/interim-q3-2024.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Interim condensed consolidated statement Q3 2024".to_owned()),
            attribution: None,
        })
        .expect("document");
    std::fs::write(dir.join("report.xhtml"), POSITIONAL_XHTML.as_bytes()).expect("write");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.xhtml"),
            Some("application/xhtml+xml"),
            None,
            Some(POSITIONAL_XHTML.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

/// A non-iXBRL document yields the same benign-empty result the retired
/// PDF arm returns: no tier, no facts, no outcome row (ADR 0095) — the
/// route still classifies the document for period-grouping purposes.
#[test]
fn non_ixbrl_xhtml_no_longer_extracts_via_the_retired_positional_tier() {
    let (state, company_id, document_id) = seed_positional();
    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2024,
        "Q3",
        "2024-09-30",
        MODE_AUTOPILOT,
    )
    .expect("routing a positional-shaped document must never error");

    assert_eq!(
        result.tier, None,
        "no tier reads a positional document anymore"
    );
    assert!(!result.emitted, "the retired route must never emit");
    assert!(result.produced_fact_ids.is_empty());
    assert!(result.skipped_fact_ids.is_empty());
    assert!(result.divergences.is_empty());
    assert_eq!(
        result.reason_code, None,
        "mirrors the DocumentRoute::Pdf idiom — no outcome row either"
    );

    let facts = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts");
    assert!(
        facts.is_empty(),
        "the retired positional route must write no facts at all: {facts:?}"
    );
}

#[test]
fn re_extracting_the_same_document_is_idempotent_not_a_unique_violation() {
    // Owner T7 bug: clicking "Wyciągnij dane" a second time on a document
    // whose facts already landed must NOT surface a UNIQUE constraint error.
    // Each incoming fact whose full uniqueness slot matches an existing row
    // is a RE-OBSERVATION: same value ⇒ skipped (counted, never produced),
    // the run succeeds cleanly and the DB keeps exactly one row per slot.
    let (state, company_id, document_id) = seed_esef_package();

    let first = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("first extraction runs");
    assert_eq!(first.produced_fact_ids.len(), 3);
    assert!(first.skipped_fact_ids.is_empty());
    assert!(first.divergences.is_empty());

    // Second click over the identical document + period.
    let second = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("re-extraction must not error with a UNIQUE violation");

    assert!(
        second.produced_fact_ids.is_empty(),
        "a re-observation produces no new facts"
    );
    assert_eq!(
        second.skipped_fact_ids.len(),
        3,
        "all three facts already exist at their slot → skipped"
    );
    // Epic #398 Item B blocker 1: an all-reobserved rerun is a SUCCESS
    // (produced_fact_ids OR skipped_fact_ids nonempty), never a gap — the
    // exact shape a version-aware re-extraction produces. Before that fix
    // `emitted` was `!produced_fact_ids.is_empty()` alone, so this legitimate
    // re-observation was misrecorded as `extractionAvailable:false`.
    assert!(
        second.emitted,
        "an all-reobserved rerun is a success, not a gap"
    );
    assert!(
        second.divergences.is_empty(),
        "identical values → no divergence"
    );

    // The DB still holds exactly one fact per slot — no duplication.
    let facts = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts");
    assert_eq!(facts.len(), 3, "re-extraction must not duplicate rows");
}

#[test]
fn re_extraction_with_a_diverging_value_is_skipped_and_reported_not_overwritten() {
    // A re-observation whose slot matches but whose value differs from the
    // already-committed (confirmed) fact must NOT silently overwrite it: the
    // safe minimal behavior (spec is silent on value conflicts) is skip +
    // record the divergence for ratification. The stored value is unchanged.
    let (state, company_id, document_id) = seed_esef_package();
    let first = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("first extraction runs");
    assert_eq!(first.produced_fact_ids.len(), 3);

    // Mutate one stored fact so the next identical extraction diverges.
    let assets = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .find(|f| f.value_numeric.trim_start_matches('-').starts_with("45"))
        .expect("the 45m total-assets fact should exist");
    state
        .update_financial_fact(crate::storage::UpdateFinancialFact {
            id: assets.id.clone(),
            value_numeric: Some("999000000".to_owned()),
            currency: None,
            data_quality: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("mutate stored fact");

    let second = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("re-extraction must not error");

    assert!(second.produced_fact_ids.is_empty());
    assert_eq!(
        second.skipped_fact_ids.len(),
        3,
        "every matching slot is skipped, diverging or not"
    );
    assert_eq!(second.divergences.len(), 1, "the one mutated slot diverges");
    let divergence = &second.divergences[0];
    assert_eq!(divergence.existing.trim(), "999000000");

    // The confirmed fact is untouched — never silently overwritten.
    let after = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .find(|f| f.id == assets.id)
        .expect("fact still present");
    assert_eq!(after.value_numeric.trim(), "999000000");
}

#[test]
fn a_value_divergence_leaves_a_durable_flagged_outcome_that_upserts() {
    // A divergence records a durable `value_divergence` outcome, keyed per
    // (document, metric), so a re-extraction refreshes the row instead of
    // duplicating it.
    let (state, company_id, document_id) = seed_esef_package();
    run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("first extraction runs");
    let assets = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .find(|f| f.value_numeric.trim_start_matches('-').starts_with("45"))
        .expect("the 45m total-assets fact should exist");
    state
        .update_financial_fact(crate::storage::UpdateFinancialFact {
            id: assets.id.clone(),
            value_numeric: Some("999000000".to_owned()),
            currency: None,
            data_quality: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("mutate stored fact");

    for _ in 0..2 {
        run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2025,
            "FY",
            "2025-12-31",
            MODE_AUTOPILOT,
        )
        .expect("re-extraction must not error");
    }

    let flagged = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes");
    let divergences: Vec<_> = flagged
        .iter()
        .filter(|outcome| outcome.reason_code == "value_divergence")
        .collect();
    assert_eq!(
        divergences.len(),
        1,
        "one row per (document, metric), refreshed by the re-run: {flagged:?}"
    );
    let outcome = divergences[0];
    assert_eq!(outcome.acceptance, "flagged");
    assert_eq!(outcome.fact_count, 0);
    assert_eq!(
        outcome.tier.as_deref(),
        Some("esef"),
        "the outcome names the tier that re-read the value: {outcome:?}"
    );
    assert!(
        outcome.report_document_id.starts_with(&document_id)
            && outcome.report_document_id.contains('#'),
        "the slot ref is per-metric so two diverging metrics cannot overwrite \
             each other: {outcome:?}"
    );
    assert!(
        outcome.attempt_count >= 2,
        "a repeated divergence is visibly repeated: {outcome:?}"
    );

    // The detail renders through the Coverage panel's gate shape (the
    // `witnessDisagreements` precedent) — stored vs freshly read.
    let detail: serde_json::Value =
        serde_json::from_str(outcome.detail_json.as_deref().expect("detail_json"))
            .expect("detail parses");
    let entry = &detail["valueDivergences"][0];
    assert_eq!(entry["metricKey"], "total_assets");
    assert_eq!(
        entry["detail"]["actual"], "999000000",
        "actual is the STORED value: {detail}"
    );
    assert_eq!(
        entry["detail"]["storedValue"], "999000000",
        "the raw stored/incoming pair travels alongside: {detail}"
    );
    assert_eq!(entry["detail"]["factId"], assets.id);
    assert!(
        entry["detail"]["incomingValue"]
            .as_str()
            .is_some_and(|v| v != "999000000"),
        "incoming is the freshly re-read value: {detail}"
    );
}

/// Guardrail-harvest (epic #229 T5): a per-metric slot ref (`docId#metricKey`)
/// names a REAL stored document — the re-run must re-extract it, not hand the
/// synthetic ref to the pipeline and fail with "no such document". The
/// "Try again" action on a `value_divergence` row is only honest if this holds.
#[test]
fn rerunning_a_value_divergence_reextracts_the_real_document() {
    let (state, company_id, document_id) = seed_esef_package();
    run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("first extraction runs");
    let assets = state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.clone()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .find(|f| f.value_numeric.trim_start_matches('-').starts_with("45"))
        .expect("the 45m total-assets fact should exist");
    state
        .update_financial_fact(crate::storage::UpdateFinancialFact {
            id: assets.id.clone(),
            value_numeric: Some("999000000".to_owned()),
            currency: None,
            data_quality: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("mutate stored fact");
    run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2025,
        "FY",
        "2025-12-31",
        MODE_AUTOPILOT,
    )
    .expect("re-extraction records the divergence");

    let divergence = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("flagged outcomes")
        .into_iter()
        .find(|outcome| outcome.reason_code == "value_divergence")
        .expect("the divergence outcome exists");
    assert!(divergence.report_document_id.contains('#'));

    let result = rerun_extraction_outcome(&state, &divergence.id, MODE_AUTOPILOT)
        .expect("the re-run must reach the real document, not the synthetic slot ref");
    assert!(
        !result.skipped_fact_ids.is_empty(),
        "the re-run re-read the document's slots: {result:?}"
    );
}

/// The reversed-witnessing rows key their slot by an AGGREGATOR PAGE URL, so
/// there is no document to re-read. The UI hides the action, but the backend
/// is the boundary that must hold: an MCP/API caller gets a TYPED refusal,
/// never a confusing "no report document 'https://…#metric'".
#[test]
fn rerunning_a_witness_disagreement_is_refused_with_a_typed_code() {
    let (state, company_id, _document_id) = seed_esef_package();
    let outcome_id = state
        .fundamentals_provenance()
        .record_extraction_outcome(crate::storage::NewExtractionOutcome {
            company_id: &company_id,
            report_document_id:
                "https://www.biznesradar.pl/raporty-finansowe-bilans/CDR#current_assets",
            fiscal_year: 2025,
            period_type: "FY",
            period_end: "2025-12-31",
            tier: Some("esef"),
            acceptance: "flagged",
            reason_code: "witness_disagreement",
            detail_json: None,
            drift_json: None,
            structure_changed: false,
            fact_count: 0,
        })
        .expect("record a reversed-witnessing outcome");

    let error = rerun_extraction_outcome(&state, &outcome_id, MODE_AUTOPILOT)
        .expect_err("a witness disagreement has no document to re-read");
    assert!(
        error.starts_with(RERUN_NOT_APPLICABLE),
        "the refusal must be a typed code the caller can branch on: {error}"
    );
}

#[test]
fn derive_report_period_reads_the_esef_period_from_the_stored_file() {
    // The shared derivation the on-demand "Extract data" command relies on:
    // an ESEF filing self-derives its `FY` period from the iXBRL contexts.
    let (state, _company_id, document_id) = seed_esef();
    let document = state.get_report_document(&document_id).expect("document");
    assert_eq!(
        derive_report_period(&state, &document),
        Some((2026, "FY", "2026-03-31".to_owned()))
    );
}

#[test]
fn derive_report_period_is_none_for_a_document_with_no_stored_file() {
    // A metadata-only (unfetched) document has no local file to parse, so no
    // period can be derived — the "Extract data" command surfaces this as a
    // clear error instead of inventing a period.
    let dir = unique_temp_dir("no-file");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir);
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/pending.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Some report".to_owned()),
            attribution: None,
        })
        .expect("document");
    // Never marked fetched → `local_path` is None.
    assert_eq!(derive_report_period(&state, &document), None);
}

/// A plain XHTML with NO inline-XBRL (`ix:`) tags — a pdf2htmlEX render of an
/// interim report. It is on the ESEF route by extension but cannot self-derive
/// a period from contexts.
const NON_IXBRL_XHTML: &str = "<html><head><title>Raport</title></head><body>\
<h1>Skonsolidowany raport za III kwartał 2024</h1><p>Treść raportu.</p></body></html>";

fn seed_non_ixbrl_xhtml(title: &str, url: &str) -> (AppState, String, String) {
    let dir = unique_temp_dir("non-ixbrl");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CBF".to_owned(),
            display_name: "Cyber_Folks S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some(title.to_owned()),
            attribution: None,
        })
        .expect("document");
    std::fs::write(dir.join("interim.xhtml"), NON_IXBRL_XHTML.as_bytes()).expect("write xhtml");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("interim.xhtml"),
            Some("application/xhtml+xml"),
            None,
            Some(NON_IXBRL_XHTML.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

#[test]
fn derive_report_period_falls_through_to_title_for_non_ixbrl_xhtml() {
    // T-A1: a stored `.xhtml` that is NOT valid iXBRL (a pdf2htmlEX render of
    // an interim report) cannot self-derive a period from contexts. Before the
    // fallthrough `derive_report_period` returned None ("no derivable period");
    // now the title's "Q3_2024" is parsed the same way a PDF's title would be.
    let (state, _company_id, document_id) = seed_non_ixbrl_xhtml(
        "cyber_Folks_SSF_Q3_2024.xhtml",
        "https://example.com/ssf_q3_2024.xhtml",
    );
    let document = state.get_report_document(&document_id).expect("document");
    assert_eq!(
        derive_report_period(&state, &document),
        Some((2024, "Q3", "2024-09-30".to_owned()))
    );
}

// A minimal balanced ESEF instance carrying a second, prior-period context
// (40m = 18m + 22m at 2025-03-31) alongside the current one (45m = 20m +
// 25m at 2026-03-31) — ESEF tags comparatives natively, so this exercises
// the comparative cross-check (ADR 0061 dec. 4b) through tier 1.
const ESEF_WITH_PRIOR: &str = r#"<html xmlns:ix="http://www.xbrl.org/2013/inlineXBRL"
      xmlns:ifrs-full="https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full"
      xmlns:xbrli="http://www.xbrl.org/2003/instance"
      xmlns:iso4217="http://www.xbrl.org/2003/iso4217">
      <xbrli:context id="c"><xbrli:period><xbrli:instant>2026-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:context id="p"><xbrli:period><xbrli:instant>2025-03-31</xbrli:instant></xbrli:period></xbrli:context>
      <xbrli:unit id="pln"><xbrli:measure>iso4217:PLN</xbrli:measure></xbrli:unit>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="c" unitRef="pln" scale="3">45 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="c" unitRef="pln" scale="3">20 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="c" unitRef="pln" scale="3">25 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Assets" contextRef="p" unitRef="pln" scale="3">40 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Liabilities" contextRef="p" unitRef="pln" scale="3">18 000</ix:nonFraction>
      <ix:nonFraction name="ifrs-full:Equity" contextRef="p" unitRef="pln" scale="3">22 000</ix:nonFraction>
    </html>"#;

fn seed_esef_with_prior() -> (AppState, String, String) {
    let dir = unique_temp_dir("esef-prior");
    std::fs::create_dir_all(&dir).expect("temp dir");
    let connection = open_in_memory_database().expect("db");
    let state = AppState::with_data_dir(connection, dir.clone());
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/annual-2026.xhtml".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Annual 2026 ESEF".to_owned()),
            attribution: None,
        })
        .expect("document");
    let pre_xml = presentation_linkbase_xml(BALANCE_SHEET_ROLES);
    let bytes = minimal_zip(&[
        ("reports/annual-2026.xhtml", ESEF_WITH_PRIOR.as_bytes()),
        ("www/annual-2026_pre.xml", pre_xml.as_bytes()),
    ]);
    std::fs::write(dir.join("report.xhtml"), &bytes).expect("write esef");
    state
        .mark_report_document_fetched(
            &document.id,
            Some("report.xhtml"),
            Some("application/octet-stream"),
            None,
            Some(bytes.len() as i64),
        )
        .expect("mark fetched");
    (state, company.id, document.id)
}

/// Seeds a prior-period `financial_period` + facts for `company_id`,
/// bridging each `(metric_key, value)` through the canonical KPI
/// definition catalog — the "already known" prior period the comparative
/// cross-check reads back via `stored_fact_set`.
pub(super) fn seed_prior_period(
    state: &AppState,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    facts: &[(&str, &str)],
) {
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company_id.to_owned(),
            fiscal_year,
            period_type: period_type.to_owned(),
            period_end_date: None,
            report_evidence_ref: None,
        })
        .expect("prior financial period should create");
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    for (metric_key, value) in facts {
        let definition = definitions
            .iter()
            .find(|d| d.metric_key == *metric_key)
            .unwrap_or_else(|| panic!("{metric_key} should exist in the canonical catalog"));
        state
            .create_financial_fact(NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id: period.id.clone(),
                definition_id: definition.id.clone(),
                value_numeric: (*value).to_owned(),
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
                source_document_ref: None,
                annotation: None,
            })
            .expect("prior financial fact should create");
    }
}

/// Fetches the `confirmation_state` of a produced fact via the public
/// list-facts read model (the only way to read that field from outside
/// `storage::financials`).
fn confirmation_states(state: &AppState, company_id: &str, fact_ids: &[String]) -> Vec<String> {
    state
        .list_financial_facts(crate::storage::ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("list facts")
        .into_iter()
        .filter(|f| fact_ids.contains(&f.id))
        .map(|f| f.confirmation_state)
        .collect()
}

#[test]
fn esef_extraction_confirms_in_both_modes() {
    for mode in [MODE_ASSIST, MODE_AUTOPILOT] {
        let (state, company_id, document_id) = seed_esef();
        let result = run_structured_extraction(
            &state,
            &company_id,
            &document_id,
            2026,
            "FY",
            "2026-03-31",
            mode,
        )
        .expect("structured extraction runs");

        assert!(result.emitted, "ESEF facts should be emitted");
        assert_eq!(result.tier, Some(SourceTier::Esef));
        assert_eq!(result.acceptance, Acceptance::Accepted);
        assert_eq!(result.produced_fact_ids.len(), 3);

        // Every produced fact carries structured provenance: tier + passed status.
        // (The retired drift plumbing is asserted absent at the DB level below.)
        let provenance = state
            .fundamentals_provenance()
            .get_many(&result.produced_fact_ids)
            .expect("provenance");
        assert_eq!(provenance.len(), 3);
        assert!(provenance.iter().all(|p| p.source_tier == "esef"));
        assert!(provenance.iter().all(|p| p.validation_status == "passed"));
        assert!(provenance.iter().all(|p| p.drift_json.is_none()));

        // ADR 0061 dec. 3/8/9: a validation-clean structured set auto-confirms
        // in BOTH modes — no unreviewed grace period for a proven fact.
        let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
        assert!(
            states.iter().all(|s| s == "confirmed"),
            "mode={mode} states={states:?}"
        );
    }
}

// -------------------------------------------------------------------
// Comparative cross-check + completeness, live in the pipeline (ADR
// 0061 dec. 4b/4d): the DB-level wiring — stored_fact_set lookup,
// prior_period_end derivation, kpi_relevance → expected_keys bridge.
// -------------------------------------------------------------------

#[test]
fn esef_with_matching_stored_prior_cross_check_stays_confirmed() {
    let (state, company_id, document_id) = seed_esef_with_prior();
    seed_prior_period(
        &state,
        &company_id,
        2025,
        "FY",
        &[
            ("total_assets", "40000000"),
            ("total_liabilities", "18000000"),
            ("total_equity", "22000000"),
        ],
    );

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    assert_eq!(result.acceptance, Acceptance::Accepted);
    assert!(result.emitted);
    assert_eq!(result.tier, Some(SourceTier::Esef));
    assert_eq!(result.produced_fact_ids.len(), 3);
    let states = confirmation_states(&state, &company_id, &result.produced_fact_ids);
    assert!(states.iter().all(|s| s == "confirmed"), "states={states:?}");
}

#[test]
fn esef_with_mismatching_stored_prior_cross_check_is_not_silently_accepted() {
    // The filing's comparative column (40m) disagrees with what is
    // already stored for that period (999m) — the strongest signal of a
    // misread/wrong-context tagging. No PDF/witness fallback exists for
    // this report, so the tier-1 failure yields no accepted tier — the
    // key guardrail is that it is never silently emitted as if proven.
    let (state, company_id, document_id) = seed_esef_with_prior();
    seed_prior_period(
        &state,
        &company_id,
        2025,
        "FY",
        &[("total_assets", "999000000")],
    );

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    assert_ne!(
        result.acceptance,
        Acceptance::Accepted,
        "a contradicted comparative must never be silently accepted"
    );
    assert!(!result.emitted);
    assert!(result.produced_fact_ids.is_empty());
}

/// ADR 0086 decisions 3/4: a stored prior sourced by a LOWER tier (the daily
/// BiznesRadar pull) must never veto an ISSUER tier's emission — the issuer
/// witnesses the aggregator, not the other way around. (Live regression,
/// 2026-07-22: CBF's whole FY2025 ESEF set was discarded because BR's stored
/// FY2024 equity disagreed with the filing's own comparative by 68%.)
#[test]
fn esef_emits_despite_a_mismatching_aggregator_sourced_prior() {
    let (state, company_id, document_id) = seed_esef_with_prior();
    // Seed the prior period the way the BR-primary pull does: value + an
    // html_aggregator provenance row.
    state
        .kpi_extraction()
        .record_structured_fact(crate::storage::StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2025,
            period_type: "FY",
            period_end: Some("2025-03-31"),
            report_document_id: &document_id,
            metric_key: "total_assets",
            value_numeric: "999000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example | Aktywa razem"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        })
        .expect("aggregator prior");

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    assert_eq!(
        result.acceptance,
        Acceptance::Accepted,
        "an aggregator-sourced prior must not veto the issuer's filing"
    );
    assert!(result.emitted);
    assert_eq!(result.tier, Some(SourceTier::Esef));
}

/// The honesty half (ADR 0061 dec. 2): an ESEF set a SAME-or-higher-tier
/// prior contradicts is a FLAGGED outcome carrying the failing checks —
/// never a silent `empty` that reads like "nothing to extract".
#[test]
fn esef_failed_validation_is_flagged_with_detail_not_silent_empty() {
    let (state, company_id, document_id) = seed_esef_with_prior();
    seed_prior_period(
        &state,
        &company_id,
        2025,
        "FY",
        &[("total_assets", "999000000")],
    );

    let result = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    )
    .expect("structured extraction runs");

    assert_eq!(
        result.acceptance,
        Acceptance::Flagged,
        "a contradicted filing is flagged, never a silent empty"
    );
    assert!(!result.emitted);
    let outcome = state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(&company_id)
        .expect("outcomes")
        .into_iter()
        .find(|o| o.report_document_id == document_id)
        .expect("the flagged run must leave a reviewable outcome row");
    assert_eq!(outcome.reason_code, "validation_failed");
    assert!(
        outcome.detail_json.is_some(),
        "the failing checks must be persisted for review"
    );
}

#[test]
fn missing_file_errors_cleanly() {
    let (state, company_id, document_id) = seed_esef();
    // Remove the file to simulate a broken fetch.
    std::fs::remove_file(state.data_dir().join("report.xhtml")).ok();
    let err = run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    );
    assert!(err.is_err());
}

#[test]
fn minimal_pdf_fixture_extracts_expected_text() {
    // Guards the hand-built PDF test fixture itself: if this regresses, every
    // PDF-tier test above would fail for the wrong reason (a broken fixture,
    // not a pipeline bug).
    let bytes = minimal_text_pdf(&["Zysk netto 12 000", "Aktywa razem 45 000"]);
    let outcome = crate::report_diff::extraction::extract_report(
        &bytes,
        crate::report_diff::extraction::SourceFormat::Pdf,
    );
    let text = outcome
        .sections
        .iter()
        .map(|s| s.body.clone())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        text.contains("Zysk netto") && text.contains("45 000"),
        "state={:?} char_count={} text={text:?}",
        outcome.state,
        outcome.char_count
    );
}

// -----------------------------------------------------------------------
// A2 — the persistence half of "never silently wrong" (ADR 0061 dec. 2,
// ADR 0084 dec. 4). A run that emits nothing must still leave a durable,
// queryable record of what the pipeline tried and what objected.
// -----------------------------------------------------------------------

/// Every outcome the store holds for a company, flagged or not — the
/// "was this period ever attempted?" question the review read model
/// deliberately narrows.
fn all_outcomes(state: &AppState, company_id: &str) -> Vec<crate::storage::ExtractionOutcome> {
    // `list_flagged_extraction_outcomes` is the review surface (non-emitting
    // only); for the never-attempted assertions the tests need the raw set,
    // so read both and merge with the by-id lookup of the emitting slots.
    state
        .fundamentals_provenance()
        .list_flagged_extraction_outcomes(company_id)
        .expect("list outcomes")
}

/// F6 (ADR 0086 code-review): when an issuer tier OVERWRITES a lower-tier
/// slot's VALUE (a real disagreement, not a label-only takeover), the ESEF /
/// positional emit path records a `tier_upgrade` diagnostic carrying the
/// previous value + tier — mirroring the WDF cover-note seam. A label-only
/// upgrade (values agreed) records nothing.
#[test]
fn a_value_overwriting_upgrade_records_a_tier_upgrade_diagnostic() {
    let (state, company_id, document_id) = seed_esef();
    state
        .set_developer_mode_enabled(true)
        .expect("developer mode enables the diagnostic sink");

    // Seed a LOWER-tier (aggregator) fact holding a DIFFERENT value in the
    // very slot the ESEF extraction will land total_assets into.
    state
        .kpi_extraction()
        .record_aggregator_fact(crate::storage::StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2026,
            period_type: "FY",
            period_end: Some("2026-03-31"),
            report_document_id: &document_id,
            metric_key: "total_assets",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Aktywa"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        })
        .expect("seed aggregator slot");

    // ESEF (issuer) re-extracts total_assets = 45,000,000 — a value overwrite
    // of the aggregator-held slot (Upgraded { previous_value: Some }).
    run_structured_extraction(
        &state,
        &company_id,
        &document_id,
        2026,
        "FY",
        "2026-03-31",
        MODE_AUTOPILOT,
    )
    .expect("structured extraction");

    let events = state.list_diagnostic_events(50).expect("list diagnostics");
    let upgrade = events
        .iter()
        .find(|e| e.module == "structured_extraction" && e.stage == "tier_upgrade")
        .expect("a value-overwriting upgrade must leave a tier_upgrade diagnostic");
    assert_eq!(
        upgrade.metadata.get("metricKey").and_then(|v| v.as_str()),
        Some("total_assets")
    );
    assert_eq!(
        upgrade
            .metadata
            .get("previousValue")
            .and_then(|v| v.as_str()),
        Some("1000000")
    );
    assert_eq!(
        upgrade
            .metadata
            .get("previousTier")
            .and_then(|v| v.as_str()),
        Some("html_aggregator")
    );
}

// -----------------------------------------------------------------------
// Provenance-aware derived-period cache (#385, migration 0140).
// -----------------------------------------------------------------------

/// Simulate a recapture: new bytes on disk, new content hash on the row —
/// exactly what `mark_report_document_fetched` does on a re-fetch.
fn recapture_with(state: &AppState, document_id: &str, cover: &[&str], hash: &str) {
    let document = state.get_report_document(document_id).expect("document");
    let local_path = document.local_path.expect("local path");
    let bytes = minimal_text_pdf(cover);
    std::fs::write(state.data_dir().join(&local_path), &bytes).expect("rewrite pdf");
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE report_documents SET content_hash = ?1, byte_size = ?2 WHERE id = ?3",
            rusqlite::params![hash, bytes.len() as i64, document_id],
        )
        .expect("update hash");
}

#[test]
fn a_recaptured_document_re_derives_instead_of_serving_the_stale_cache() {
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "za okres 6 miesiecy zakonczony 30.06.2025",
    ]);
    let document = state.get_report_document(&document_id).expect("document");
    assert_eq!(
        derive_report_period(&state, &document),
        Some((2025, "H1", "2025-06-30".to_owned()))
    );

    // Recapture with DIFFERENT content: a version-only cache hit would keep
    // serving H1 2025; the provenance predicate must re-derive.
    recapture_with(
        &state,
        &document_id,
        &[
            "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
            "za okres 3 miesiecy zakonczony 31.03.2026",
        ],
        "b000000000000000000000000000000000000000000000000000000000000002",
    );
    let recaptured = state.get_report_document(&document_id).expect("document");
    assert_eq!(
        derive_report_period(&state, &recaptured),
        Some((2026, "Q1", "2026-03-31".to_owned())),
        "stale provenance must re-derive from the new bytes"
    );
    let cached = state
        .financials()
        .cached_derived_period(&document_id)
        .expect("cache read")
        .expect("row");
    assert_eq!(
        cached.content_hash.as_deref(),
        Some("b000000000000000000000000000000000000000000000000000000000000002"),
        "the overwrite stamped the new provenance"
    );
}

#[test]
fn a_legacy_null_hash_cache_row_backfills_on_the_next_read() {
    let (state, document_id) = seed_untitled_pdf(&[
        "SKONSOLIDOWANE SPRAWOZDANIE FINANSOWE GRUPY KAPITALOWEJ ABC",
        "za okres 6 miesiecy zakonczony 30.06.2025",
    ]);
    let document = state.get_report_document(&document_id).expect("document");

    // Plant a CURRENT-version row with a bogus period and NO provenance —
    // the pre-0140 legacy shape. `None == None` must not count as a hit.
    state
        .financials()
        .store_derived_period(
            &document_id,
            Some((1999, "FY", "1999-12-31")),
            DERIVATION_VERSION,
            None,
        )
        .expect("plant legacy row");

    assert_eq!(
        derive_report_period(&state, &document),
        Some((2025, "H1", "2025-06-30".to_owned())),
        "a provenance-less row is a miss, not a hit"
    );
    let cached = state
        .financials()
        .cached_derived_period(&document_id)
        .expect("cache read")
        .expect("row");
    assert_eq!(
        cached.content_hash, document.content_hash,
        "the re-derivation backfilled the provenance"
    );
    assert_eq!(cached.fiscal_year, Some(2025));
}
