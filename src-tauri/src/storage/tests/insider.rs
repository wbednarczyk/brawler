//! Insider-substrate storage tests (ADR 0083 D6, plan v0.57 T4): the corrected
//! classification patterns over the **real** MAR art. 19 title forms (guardrail:
//! seeds validated against reality), and the deterministic cover-note parse
//! sweep (rows written, idempotent re-ingest, no-unit parking).
//!
//! Titles are short public disclosure headings (the real corpus). Bodies are
//! synthetic, structurally equivalent to the real cover notes — never copied
//! verbatim from `private/`.

use super::*;

fn company(state: &AppState, ticker: &str) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company created")
}

fn item(company: &Company, article_id: &str, title: &str, body: &str) -> BankierCompanyItem {
    BankierCompanyItem {
        company_id: company.id.clone(),
        qualified_ticker: company.qualified_ticker.clone(),
        title: title.to_owned(),
        link: format!("https://www.bankier.pl/wiadomosc/X-{article_id}.html"),
        summary: "Komunikat ESPI".to_owned(),
        published_at: Some("2026-07-01T09:00:00".to_owned()),
        fetched_at: "2026-07-01T10:00:00Z".to_owned(),
        article_id: article_id.to_owned(),
        pub_id: 3,
        dedupe_key: format!("bankier-company-komunikaty:article:{article_id}"),
        duplicate_signature: format!("official-secondary:GPW:{}:{article_id}", company.ticker),
        body_text: Some(body.to_owned()),
        attachments: Vec::new(),
        detail_fetch_attempted: true,
    }
}

fn categories(state: &AppState, company_id: &str) -> Vec<String> {
    state
        .list_company_signals(CompanySignalListInput {
            company_id: Some(company_id.to_owned()),
            ..Default::default()
        })
        .expect("signals list")
        .into_iter()
        .map(|s| s.category)
        .collect()
}

/// The five distinct real title forms all classify as `insider_transaction`, and
/// none of the non-insider disclosure titles do (precision + recall).
#[test]
fn insider_classification_corpus_real_titles() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);

    // Recall: real MAR art. 19 title forms (the distinct shapes from the 22-filing
    // ground truth) must classify as insider_transaction.
    let insider_titles = [
        ("INS1", "Informacja o transakcjach uzyskana w trybie art. 19 MAR"),
        ("INS2", "Powiadomienie o transakcji osoby pełniącej obowiązki zarządcze"),
        (
            "INS3",
            "Informacja o otrzymaniu powiadomienia o transakcji osoby zobowiązanej",
        ),
        (
            "INS4",
            "Zawiadomienie Członka Rady Nadzorczej i osoby blisko związanej o nabyciu akcji Emitenta",
        ),
        ("INS5", "Informacja o transakcji na akcjach ASBISc Enterprises Plc"),
        (
            "INS6",
            "Informacja o transakcji nabycia przez osobę pełniącą obowiązki zarządcze warrantów subskrypcyjnych serii A Emitenta",
        ),
    ];
    for (idx, (article, title)) in insider_titles.iter().enumerate() {
        let c = company(&state, &format!("IN{idx}"));
        state
            .ingest_bankier_company_items(&[item(&c, article, title, "Treść raportu: brak.")])
            .expect("ingest");
        assert!(
            categories(&state, &c.id).contains(&"insider_transaction".to_owned()),
            "recall miss: {title:?} should classify as insider_transaction"
        );
    }

    // Precision: non-insider disclosure titles must NOT classify as insider.
    let non_insider = [
        (
            "DIV",
            "Rekomendacja Zarządu w sprawie wypłaty dywidendy za rok 2025",
        ),
        (
            "GM",
            "Ogłoszenie o zwołaniu Zwyczajnego Walnego Zgromadzenia",
        ),
        ("CTR", "Zawarcie znaczącej umowy z kontrahentem"),
        (
            "MH",
            "Zawiadomienie w trybie art. 69 o zmianie udziału w ogólnej liczbie głosów",
        ),
    ];
    for (idx, (article, title)) in non_insider.iter().enumerate() {
        let c = company(&state, &format!("NI{idx}"));
        state
            .ingest_bankier_company_items(&[item(&c, article, title, "Treść raportu: brak.")])
            .expect("ingest");
        assert!(
            !categories(&state, &c.id).contains(&"insider_transaction".to_owned()),
            "precision breach: {title:?} must NOT classify as insider_transaction"
        );
    }
}

const SINGLE_PDMR_BODY: &str = "Treść raportu:Zarząd Przykład S.A. informuje o otrzymaniu w dniu \
    dzisiejszym od Jana Testowego, Prezesa Zarządu Spółki, powiadomienia w trybie art. 19 MAR \
    o transakcji nabycia akcji Emitenta.ZałącznikiPlikOpisPowiadomienie.pdf\
    MESSAGE (ENGLISH VERSION)the board informs";

/// A confirmed insider filing's cover note is parsed into a transaction row, and
/// re-ingestion creates zero new rows (idempotent end-to-end).
#[test]
fn insider_cover_note_parsed_and_idempotent() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let c = company(&state, "PRZ");

    let items = vec![item(
        &c,
        "9300001",
        "Informacja o transakcjach uzyskana w trybie art. 19 MAR",
        SINGLE_PDMR_BODY,
    )];
    state
        .ingest_bankier_company_items(&items)
        .expect("ingest 1");

    let txs = state.insider().list_by_company(&c.id).expect("list");
    assert_eq!(txs.len(), 1, "one parsed transaction");
    assert_eq!(txs[0].person_normalized, "JAN TESTOWY");
    assert_eq!(txs[0].role.as_deref(), Some("management"));
    assert_eq!(txs[0].direction.as_deref(), Some("buy"));
    assert_eq!(txs[0].instrument.as_deref(), Some("shares"));
    assert!(
        txs[0].volume.is_none(),
        "figures are PDF-only (T4b): NULL, not guessed"
    );

    // Re-ingest → deterministic id upserts in place, no duplicate row.
    state
        .ingest_bankier_company_items(&items)
        .expect("ingest 2");
    let txs2 = state.insider().list_by_company(&c.id).expect("list 2");
    assert_eq!(txs2.len(), 1, "re-ingest creates zero new rows");
    assert_eq!(txs2[0].id, txs[0].id);
}

/// Startup catch-up (F-B): a confirmed `insider_transaction` filing that a prior
/// app version ingested but never parsed (no `insider_transactions` rows, no
/// unparsed marker) is parsed by the exact `state.insider().parse_pending()` seam
/// the lib.rs startup catch-up calls — populating the timeline with zero source
/// refresh — and re-running the catch-up writes zero new rows (idempotent). This
/// locks the contract the tauri `.setup(...)` closure relies on for a cold update.
#[test]
fn startup_catch_up_parses_pending_insider_filings() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let c = company(&state, "CUP");

    // Ingest a real MAR art. 19 cover note. Current ingest parses inline, so drop
    // the parsed rows afterwards to reproduce the pre-v0.57 cold-DB state: a
    // confirmed `insider_transaction` signal whose cover note was never parsed.
    state
        .ingest_bankier_company_items(&[item(
            &c,
            "9300500",
            "Informacja o transakcjach uzyskana w trybie art. 19 MAR",
            SINGLE_PDMR_BODY,
        )])
        .expect("ingest");
    {
        let connection = state.checkout().expect("connection");
        connection
            .execute("DELETE FROM insider_transactions", [])
            .expect("clear inline-parsed rows to simulate the cold-DB backlog");
    }

    // Cold-update precondition: the timeline is empty because no refresh (hence no
    // parse) has run since the update.
    assert!(
        state
            .insider()
            .list_by_company(&c.id)
            .expect("list")
            .is_empty(),
        "timeline empty before the startup catch-up runs"
    );

    // The exact seam the startup catch-up invokes.
    let written = state.insider().parse_pending().expect("parse_pending");
    assert!(written > 0, "catch-up parses the pending filing");
    assert!(
        !state
            .insider()
            .list_by_company(&c.id)
            .expect("list")
            .is_empty(),
        "timeline populated by the catch-up with zero refresh"
    );

    // Idempotent: the filing is now parsed, so a second catch-up writes nothing.
    let again = state
        .insider()
        .parse_pending()
        .expect("parse_pending again");
    assert_eq!(again, 0, "second catch-up writes zero new rows");
}

/// The overview query enriches each parsed transaction with the feed item's
/// `source_url` (the timeline's link to the filing) via the LEFT joins.
#[test]
fn insider_overview_source_carries_provenance() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let c = company(&state, "OVR");

    state
        .ingest_bankier_company_items(&[item(
            &c,
            "9300010",
            "Informacja o transakcjach uzyskana w trybie art. 19 MAR",
            SINGLE_PDMR_BODY,
        )])
        .expect("ingest");

    let sources = state
        .insider()
        .list_for_overview(&c.id)
        .expect("list_for_overview");
    assert_eq!(sources.len(), 1, "one enriched transaction");
    assert_eq!(sources[0].tx.person_normalized, "JAN TESTOWY");
    assert_eq!(
        sources[0].source_url.as_deref(),
        Some("https://www.bankier.pl/wiadomosc/X-9300010.html"),
        "the feed item's source_url is joined for the timeline link"
    );
}

/// A classified insider filing whose cover note yields no writable unit parks
/// once as unparsed (never guessed), and stays parked on re-run.
#[test]
fn insider_no_unit_parks_once() {
    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let c = company(&state, "PRK");

    // Title classifies as insider (art. 19 MAR) but the body names no party.
    let items = vec![item(
        &c,
        "9300002",
        "Informacja o transakcjach uzyskana w trybie art. 19 MAR",
        "Treść raportu:Pełna treść powiadomienia znajduje się w załączniku do raportu.",
    )];
    state.ingest_bankier_company_items(&items).expect("ingest");

    assert!(
        state
            .insider()
            .list_by_company(&c.id)
            .expect("list")
            .is_empty(),
        "no transaction row written for an unresolvable cover note"
    );
    assert!(
        state
            .insider()
            .is_parked("feed_bankier_company_komunikatyarticle9300002")
            .expect("parked"),
        "filing parked once as unparsed"
    );

    // Re-ingest does not un-park or write rows.
    state
        .ingest_bankier_company_items(&items)
        .expect("reingest");
    assert!(state
        .insider()
        .list_by_company(&c.id)
        .expect("list")
        .is_empty());
}

/// Attachment-tier merge (T4b): a parsed notification document with MORE
/// transaction rows than the cover note enumerated fills the matched unit's NULLs
/// and **appends** the extra row as a new unit (the CMP second-disposal class), with
/// a deterministic index extending `(feed_item_id, unit_index)`. Re-running the same
/// merge is idempotent (fill-NULLs; the append matches on the second pass).
#[test]
fn attachment_merge_fills_then_appends_new_unit() {
    use crate::fundamentals::insider::attachment::AttachmentTxUnit;
    use crate::fundamentals::insider::{Direction, InsiderRole, Instrument};
    use std::collections::BTreeSet;

    let connection = open_in_memory_database().expect("db");
    let state = AppState::new(connection);
    let c = company(&state, "CMP");

    state
        .ingest_bankier_company_items(&[item(
            &c,
            "9300100",
            "Informacja o transakcjach uzyskana w trybie art. 19 MAR",
            SINGLE_PDMR_BODY,
        )])
        .expect("ingest");
    let feed_item_id = "feed_bankier_company_komunikatyarticle9300100";
    let before = state.insider().list_by_company(&c.id).expect("list");
    assert_eq!(before.len(), 1, "one cover-note unit, figures NULL");
    assert!(before[0].volume.is_none());

    // The cover note is a `buy` (nabycie); the PDF restates the same direction and
    // adds a second dated transaction the cover note never enumerated.
    let mk = |vol: &str, date: &str| AttachmentTxUnit {
        person_raw: "Jan Testowy".to_owned(),
        person_normalized: "JAN TESTOWY".to_owned(),
        role: Some(InsiderRole::Management),
        related_pdmr_raw: None,
        related_pdmr_normalized: None,
        direction: Some(Direction::Buy),
        instrument: Some(Instrument::Shares),
        volume: Some(vol.to_owned()),
        price: Some("12.50".to_owned()),
        currency: Some("PLN".to_owned()),
        tx_date: Some(date.to_owned()),
    };
    let units = vec![mk("275000", "2026-07-03"), mk("40000", "2026-07-07")];

    let outcome = state
        .insider()
        .merge_attachment_units(&c.id, feed_item_id, &units)
        .expect("merge");
    assert_eq!(outcome.appended, 1, "the 2nd disposal is a new unit");
    assert!(
        outcome.filled >= 4,
        "1st unit's volume/price/currency/tx_date filled, got {}",
        outcome.filled
    );
    assert!(outcome.conflicts.is_empty());

    let rows = state.insider().list_by_company(&c.id).expect("list");
    assert_eq!(rows.len(), 2, "one filled + one appended");
    // The appended unit extends the index space (0 → 1).
    let mut indices: Vec<i64> = rows.iter().map(|r| r.unit_index).collect();
    indices.sort_unstable();
    assert_eq!(indices, vec![0, 1]);
    let volumes: BTreeSet<String> = rows.iter().filter_map(|r| r.volume.clone()).collect();
    assert_eq!(
        volumes,
        BTreeSet::from(["275000".to_owned(), "40000".to_owned()])
    );

    // Idempotent: a second identical merge fills nothing new and appends nothing
    // (each PDF unit now matches an existing unit on person+direction+tx_date).
    let again = state
        .insider()
        .merge_attachment_units(&c.id, feed_item_id, &units)
        .expect("merge 2");
    assert_eq!(again.appended, 0);
    assert_eq!(again.filled, 0);
    assert_eq!(
        state.insider().list_by_company(&c.id).expect("list").len(),
        2,
        "re-merge creates zero new rows"
    );
}
