//! Insider-substrate storage tests (ADR 0083 D6, plan v0.57 T4): the corrected
//! classification patterns over the **real** MAR art. 19 title forms (guardrail:
//! seeds validated against reality), and the deterministic cover-note parse
//! sweep (rows written, idempotent re-ingest, no-unit parking).
//!
//! Titles are short public disclosure headings (the real corpus). Bodies are
//! synthetic, structurally equivalent to the real cover notes — never copied
//! verbatim from `private/`.

use super::*;
use proptest::prelude::*;

use crate::fundamentals::insider::attachment::AttachmentTxUnit;
use crate::fundamentals::insider::{Direction, InsiderRole, Instrument};

#[derive(Debug, Clone, PartialEq, Eq)]
struct AttachmentRowSnapshot {
    id: String,
    company_id: String,
    feed_item_id: String,
    unit_index: i64,
    person_name_raw: String,
    person_normalized: String,
    role: Option<String>,
    related_pdmr_raw: Option<String>,
    related_pdmr_normalized: Option<String>,
    related_pdmr_role: Option<String>,
    direction: Option<String>,
    instrument: Option<String>,
    volume: Option<String>,
    price: Option<String>,
    currency: Option<String>,
    tx_date: Option<String>,
    source_document_id: Option<String>,
    source_unit_ord: Option<i64>,
}

fn attachment_rows(state: &AppState, feed_item_id: &str) -> Vec<AttachmentRowSnapshot> {
    let connection = state.checkout_for_tests().expect("connection");
    let mut statement = connection
        .prepare(
            "SELECT id, company_id, feed_item_id, unit_index, person_name_raw, \
             person_normalized, role, related_pdmr_raw, related_pdmr_normalized, \
             related_pdmr_role, direction, instrument, volume, price, currency, tx_date, \
             source_document_id, source_unit_ord FROM insider_transactions \
             WHERE feed_item_id = ?1 ORDER BY unit_index ASC",
        )
        .expect("prepare insider snapshot");
    statement
        .query_map([feed_item_id], |row| {
            Ok(AttachmentRowSnapshot {
                id: row.get(0)?,
                company_id: row.get(1)?,
                feed_item_id: row.get(2)?,
                unit_index: row.get(3)?,
                person_name_raw: row.get(4)?,
                person_normalized: row.get(5)?,
                role: row.get(6)?,
                related_pdmr_raw: row.get(7)?,
                related_pdmr_normalized: row.get(8)?,
                related_pdmr_role: row.get(9)?,
                direction: row.get(10)?,
                instrument: row.get(11)?,
                volume: row.get(12)?,
                price: row.get(13)?,
                currency: row.get(14)?,
                tx_date: row.get(15)?,
                source_document_id: row.get(16)?,
                source_unit_ord: row.get(17)?,
            })
        })
        .expect("query insider snapshot")
        .collect::<Result<Vec<_>, _>>()
        .expect("read insider snapshot")
}

fn attachment_unit(
    person_raw: &str,
    person_normalized: &str,
    direction: Option<Direction>,
    tx_date: Option<&str>,
    volume: Option<&str>,
) -> AttachmentTxUnit {
    AttachmentTxUnit {
        person_raw: person_raw.to_owned(),
        person_normalized: person_normalized.to_owned(),
        role: None,
        related_pdmr_raw: None,
        related_pdmr_normalized: None,
        direction,
        instrument: None,
        volume: volume.map(str::to_owned),
        price: None,
        currency: None,
        tx_date: tx_date.map(str::to_owned),
    }
}

fn sourced(
    source_document_id: &str,
    source_unit_ord: usize,
    unit: AttachmentTxUnit,
) -> SourcedAttachmentUnit {
    SourcedAttachmentUnit {
        source_document_id: source_document_id.to_owned(),
        source_unit_ord,
        unit,
    }
}

fn attachment_state(ticker: &str, article_id: &str) -> (AppState, Company, String) {
    let state = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&state, ticker);
    state
        .ingest_bankier_company_items(&[item(
            &c,
            article_id,
            "Informacja o transakcjach uzyskana w trybie art. 19 MAR",
            SINGLE_PDMR_BODY,
        )])
        .expect("ingest cover note");
    (
        state,
        c,
        format!("feed_bankier_company_komunikatyarticle{article_id}"),
    )
}

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
    let units = vec![
        sourced("doc-cmp", 0, mk("275000", "2026-07-03")),
        sourced("doc-cmp", 1, mk("40000", "2026-07-07")),
    ];

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

/// Two separately parsed documents are two transactions even when the first
/// document filled the cover-note row's NULLs before the second merge call.
#[test]
fn attachment_merge_is_batching_independent() {
    let (split_state, split_company, feed_item_id) = attachment_state("BIN", "9300200");
    let a = sourced(
        "doc-1",
        0,
        attachment_unit("Jan Testowy", "JAN TESTOWY", None, Some("2026-07-03"), None),
    );
    let b = sourced(
        "doc-2",
        0,
        attachment_unit(
            "Jan Testowy",
            "JAN TESTOWY",
            Some(Direction::Buy),
            None,
            None,
        ),
    );

    split_state
        .insider()
        .merge_attachment_units(&split_company.id, &feed_item_id, std::slice::from_ref(&a))
        .expect("merge A");
    split_state
        .insider()
        .merge_attachment_units(&split_company.id, &feed_item_id, std::slice::from_ref(&b))
        .expect("merge B");
    let split_rows = attachment_rows(&split_state, &feed_item_id);

    let (single_state, single_company, single_feed_item_id) = attachment_state("BIN", "9300200");
    single_state
        .insider()
        .merge_attachment_units(&single_company.id, &single_feed_item_id, &[a, b])
        .expect("merge A+B");
    let single_rows = attachment_rows(&single_state, &single_feed_item_id);

    assert_eq!(split_rows, single_rows);
    assert_eq!(split_rows.len(), 2, "the split path must retain both units");
    assert_eq!(split_rows[0].unit_index, 0);
    assert_eq!(split_rows[0].direction.as_deref(), Some("buy"));
    assert_eq!(split_rows[0].tx_date.as_deref(), Some("2026-07-03"));
    assert_eq!(split_rows[1].unit_index, 1);
    assert_eq!(split_rows[1].direction.as_deref(), Some("buy"));
    assert!(split_rows[1].tx_date.is_none());
}

// The merge invariant is checked against real in-memory SQLite state rather
// than only against a pure model: arbitrary units and arbitrary contiguous
// batch partitions converge to the same rows, and a full re-merge is stable.
proptest! {
    #![proptest_config(ProptestConfig { cases: 24, .. ProptestConfig::default() })]

    #[test]
    fn attachment_merge_associativity_proptest(
        specs in prop::collection::vec(
            (
                0usize..3,
                prop::option::of(prop::sample::select(vec!["buy", "sell"])),
                prop::option::of(prop::sample::select(vec![
                    InsiderRole::Management,
                    InsiderRole::Supervisory,
                ])),
                prop::option::of(prop::sample::select(vec![
                    "2026-07-01", "2026-07-02", "2026-07-03",
                ])),
                prop::option::of(prop::sample::select(vec!["10", "20", "30"])),
                0usize..2,
            ),
            0..7,
        ),
        partition_count in 1usize..=3,
    ) {
        let names = [
            ("Jan Testowy", "JAN TESTOWY"),
            ("Anna Nowak", "ANNA NOWAK"),
            ("Piotr Kowalski", "PIOTR KOWALSKI"),
        ];
        let mut next_ord = [0usize; 2];
        let units: Vec<SourcedAttachmentUnit> = specs
            .into_iter()
            .map(|(person, direction, role, tx_date, volume, document)| {
                let (person_raw, person_normalized) = names[person];
                let direction = direction.map(|value| match value {
                    "buy" => Direction::Buy,
                    "sell" => Direction::Sell,
                    _ => unreachable!("strategy only yields buy/sell"),
                });
                let ordinal = next_ord[document];
                next_ord[document] += 1;
                let mut unit = attachment_unit(
                    person_raw,
                    person_normalized,
                    direction,
                    tx_date,
                    volume,
                );
                unit.role = role;
                sourced(
                    &format!("doc-{document}"),
                    ordinal,
                    unit,
                )
            })
            .collect();

        let batch_count = partition_count.min(units.len().max(1));
        let mut batches = Vec::new();
        if units.is_empty() {
            batches.push(Vec::new());
        } else {
            for batch_index in 0..batch_count {
                let start = units.len() * batch_index / batch_count;
                let end = units.len() * (batch_index + 1) / batch_count;
                batches.push(units[start..end].to_vec());
            }
        }

        let (sequential_state, sequential_company, sequential_feed_item_id) =
            attachment_state("PRO", "9300300");
        for batch in &batches {
            sequential_state
                .insider()
                .merge_attachment_units(
                    &sequential_company.id,
                    &sequential_feed_item_id,
                    batch,
                )
                .expect("sequential merge");
        }
        let before_remerge = attachment_rows(&sequential_state, &sequential_feed_item_id);
        sequential_state
            .insider()
            .merge_attachment_units(
                &sequential_company.id,
                &sequential_feed_item_id,
                &units,
            )
            .expect("full re-merge");
        prop_assert_eq!(
            before_remerge,
            attachment_rows(&sequential_state, &sequential_feed_item_id),
            "re-merging the full batch must be idempotent",
        );

        let (single_state, single_company, single_feed_item_id) =
            attachment_state("PRO", "9300300");
        single_state
            .insider()
            .merge_attachment_units(
                &single_company.id,
                &single_feed_item_id,
                &units,
            )
            .expect("single merge");
        prop_assert_eq!(
            attachment_rows(&sequential_state, &sequential_feed_item_id),
            attachment_rows(&single_state, &single_feed_item_id),
            "partitioned and single-batch merges must converge",
        );
    }
}

#[test]
fn attachment_merge_identical_fully_filled_units_stay_two_rows() {
    let mut unit = attachment_unit(
        "Anna Nowak",
        "ANNA NOWAK",
        Some(Direction::Buy),
        Some("2026-07-03"),
        Some("100"),
    );
    unit.role = Some(InsiderRole::Management);
    unit.instrument = Some(Instrument::Shares);
    unit.price = Some("10".to_owned());
    unit.currency = Some("PLN".to_owned());
    unit.related_pdmr_raw = Some("Jan Testowy".to_owned());
    unit.related_pdmr_normalized = Some("JAN TESTOWY".to_owned());
    let units = vec![sourced("doc-1", 0, unit.clone()), sourced("doc-1", 1, unit)];

    let (single_state, single_company, single_feed_item_id) = attachment_state("FUL", "9300400");
    single_state
        .insider()
        .merge_attachment_units(&single_company.id, &single_feed_item_id, &units)
        .expect("single merge");

    let (split_state, split_company, split_feed_item_id) = attachment_state("FUL", "9300400");
    for unit in &units {
        split_state
            .insider()
            .merge_attachment_units(
                &split_company.id,
                &split_feed_item_id,
                std::slice::from_ref(unit),
            )
            .expect("singleton merge");
    }

    let single_rows: Vec<_> = attachment_rows(&single_state, &single_feed_item_id)
        .into_iter()
        .filter(|row| row.source_document_id.as_deref() == Some("doc-1"))
        .collect();
    let split_rows: Vec<_> = attachment_rows(&split_state, &split_feed_item_id)
        .into_iter()
        .filter(|row| row.source_document_id.as_deref() == Some("doc-1"))
        .collect();
    assert_eq!(single_rows.len(), 2);
    assert_eq!(split_rows.len(), 2);
    assert_eq!(single_rows, split_rows);
}

#[test]
fn attachment_merge_provenance_person_mismatch_never_fills() {
    let (state, company, feed_item_id) = attachment_state("PMM", "9300450");
    {
        let connection = state.checkout_for_tests().expect("connection");
        connection
            .execute(
                "DELETE FROM insider_transactions WHERE feed_item_id = ?1",
                [&feed_item_id],
            )
            .expect("clear cover-note row");
    }

    let anna = attachment_unit(
        "Anna Nowak",
        "ANNA NOWAK",
        Some(Direction::Buy),
        Some("2026-07-03"),
        Some("100"),
    );
    state
        .insider()
        .merge_attachment_units(&company.id, &feed_item_id, &[sourced("doc-1", 0, anna)])
        .expect("claim Anna row");

    let jan = attachment_unit(
        "Jan Testowy",
        "JAN TESTOWY",
        Some(Direction::Buy),
        Some("2026-07-03"),
        Some("200"),
    );
    state
        .insider()
        .merge_attachment_units(&company.id, &feed_item_id, &[sourced("doc-1", 0, jan)])
        .expect("append person-mismatched row");

    let rows = attachment_rows(&state, &feed_item_id);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].person_normalized, "ANNA NOWAK");
    assert_eq!(rows[0].volume.as_deref(), Some("100"));
    assert!(rows[0].source_document_id.is_none());
    assert!(rows[0].source_unit_ord.is_none());
    assert_eq!(rows[1].person_normalized, "JAN TESTOWY");
    assert_eq!(rows[1].volume.as_deref(), Some("200"));
    assert_eq!(rows[1].source_document_id.as_deref(), Some("doc-1"));
    assert_eq!(rows[1].source_unit_ord, Some(0));
}

/// Adversarial-review regression (2026-08-04): a claim released by a person
/// mismatch must return to the unclaimed pool *within the same batch*, so a
/// later compatible unit can claim it — otherwise the released-row outcome
/// depends on batching (3 rows in one call vs 2 rows split).
#[test]
fn attachment_merge_released_claim_is_reclaimable_same_batch() {
    let build_state = || {
        let (state, company, feed_item_id) = attachment_state("RCL", "9300460");
        {
            let connection = state.checkout_for_tests().expect("connection");
            connection
                .execute(
                    "DELETE FROM insider_transactions WHERE feed_item_id = ?1",
                    [&feed_item_id],
                )
                .expect("clear cover-note row");
        }
        let anna = attachment_unit(
            "Anna Nowak",
            "ANNA NOWAK",
            Some(Direction::Buy),
            Some("2026-07-03"),
            Some("100"),
        );
        state
            .insider()
            .merge_attachment_units(&company.id, &feed_item_id, &[sourced("doc-1", 0, anna)])
            .expect("seed Anna claim");
        (state, company, feed_item_id)
    };
    // U1: parser change put Jan at (doc-1, 0) — releases Anna's stale claim.
    // U2: Anna re-appears from doc-2, compatible with her released row.
    let u1 = sourced(
        "doc-1",
        0,
        attachment_unit(
            "Jan Testowy",
            "JAN TESTOWY",
            Some(Direction::Buy),
            Some("2026-07-03"),
            Some("200"),
        ),
    );
    let u2 = sourced(
        "doc-2",
        0,
        attachment_unit("Anna Nowak", "ANNA NOWAK", Some(Direction::Buy), None, None),
    );

    let (single_state, single_company, single_feed_item_id) = build_state();
    single_state
        .insider()
        .merge_attachment_units(
            &single_company.id,
            &single_feed_item_id,
            &[u1.clone(), u2.clone()],
        )
        .expect("single-batch merge");

    let (split_state, split_company, split_feed_item_id) = build_state();
    split_state
        .insider()
        .merge_attachment_units(
            &split_company.id,
            &split_feed_item_id,
            std::slice::from_ref(&u1),
        )
        .expect("merge U1");
    split_state
        .insider()
        .merge_attachment_units(
            &split_company.id,
            &split_feed_item_id,
            std::slice::from_ref(&u2),
        )
        .expect("merge U2");

    let single_rows = attachment_rows(&single_state, &single_feed_item_id);
    let split_rows = attachment_rows(&split_state, &split_feed_item_id);
    assert_eq!(
        single_rows.len(),
        2,
        "released row must be reclaimed, not tripled"
    );
    assert_eq!(single_rows, split_rows);
    let anna_row = single_rows
        .iter()
        .find(|row| row.person_normalized == "ANNA NOWAK")
        .expect("Anna row");
    assert_eq!(anna_row.source_document_id.as_deref(), Some("doc-2"));
    assert_eq!(anna_row.source_unit_ord, Some(0));
}

/// Defensive guard: a duplicated (document, ordinal) tag inside one batch is
/// the same parse artifact twice — the first occurrence wins and the merge
/// must not trip the partial unique provenance index.
#[test]
fn attachment_merge_duplicate_tags_in_batch_keep_first() {
    let (state, company, feed_item_id) = attachment_state("DUP", "9300470");
    {
        let connection = state.checkout_for_tests().expect("connection");
        connection
            .execute(
                "DELETE FROM insider_transactions WHERE feed_item_id = ?1",
                [&feed_item_id],
            )
            .expect("clear cover-note row");
    }
    let first = sourced(
        "doc-1",
        0,
        attachment_unit(
            "Anna Nowak",
            "ANNA NOWAK",
            Some(Direction::Buy),
            Some("2026-07-03"),
            Some("100"),
        ),
    );
    let duplicate = sourced(
        "doc-1",
        0,
        attachment_unit(
            "Anna Nowak",
            "ANNA NOWAK",
            Some(Direction::Buy),
            Some("2026-07-03"),
            Some("999"),
        ),
    );
    state
        .insider()
        .merge_attachment_units(&company.id, &feed_item_id, &[first, duplicate])
        .expect("duplicate tag must not fail the merge");

    let rows = attachment_rows(&state, &feed_item_id);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].volume.as_deref(),
        Some("100"),
        "first occurrence wins"
    );
    assert_eq!(rows[0].source_document_id.as_deref(), Some("doc-1"));
    assert_eq!(rows[0].source_unit_ord, Some(0));
}

#[test]
fn attachment_merge_legacy_rows_self_heal() {
    let (legacy_state, legacy_company, legacy_feed_item_id) = attachment_state("LEG", "9300500");
    {
        let connection = legacy_state.checkout_for_tests().expect("connection");
        connection
            .execute(
                "UPDATE insider_transactions SET volume = '100', price = '10', \
                 currency = 'PLN', tx_date = '2026-07-03', source_document_id = NULL, \
                 source_unit_ord = NULL WHERE feed_item_id = ?1",
                [&legacy_feed_item_id],
            )
            .expect("seed legacy filled row");
    }
    let mut unit = attachment_unit(
        "Jan Testowy",
        "JAN TESTOWY",
        Some(Direction::Buy),
        Some("2026-07-03"),
        Some("100"),
    );
    unit.role = Some(InsiderRole::Management);
    unit.instrument = Some(Instrument::Shares);
    unit.price = Some("10".to_owned());
    unit.currency = Some("PLN".to_owned());
    let sourced_unit = sourced("doc-legacy", 0, unit.clone());
    legacy_state
        .insider()
        .merge_attachment_units(
            &legacy_company.id,
            &legacy_feed_item_id,
            std::slice::from_ref(&sourced_unit),
        )
        .expect("self-heal legacy row");

    let (clean_state, clean_company, clean_feed_item_id) = attachment_state("LEG", "9300500");
    clean_state
        .insider()
        .merge_attachment_units(&clean_company.id, &clean_feed_item_id, &[sourced_unit])
        .expect("clean merge");

    assert_eq!(
        attachment_rows(&legacy_state, &legacy_feed_item_id),
        attachment_rows(&clean_state, &clean_feed_item_id),
    );
    let healed = attachment_rows(&legacy_state, &legacy_feed_item_id);
    assert_eq!(healed.len(), 1);
    assert_eq!(healed[0].source_document_id.as_deref(), Some("doc-legacy"));
    assert_eq!(healed[0].source_unit_ord, Some(0));
}

#[test]
fn attachment_merge_three_partial_units_stay_distinct() {
    let units = vec![
        sourced(
            "doc-three",
            0,
            attachment_unit(
                "Jan Testowy",
                "JAN TESTOWY",
                Some(Direction::Buy),
                Some("2026-07-01"),
                None,
            ),
        ),
        sourced(
            "doc-three",
            1,
            attachment_unit(
                "Jan Testowy",
                "JAN TESTOWY",
                Some(Direction::Buy),
                None,
                None,
            ),
        ),
        sourced(
            "doc-three",
            2,
            attachment_unit(
                "Jan Testowy",
                "JAN TESTOWY",
                Some(Direction::Buy),
                Some("2026-07-02"),
                None,
            ),
        ),
    ];

    let (single_state, single_company, single_feed_item_id) = attachment_state("THR", "9300600");
    single_state
        .insider()
        .merge_attachment_units(&single_company.id, &single_feed_item_id, &units)
        .expect("single merge");

    let (split_state, split_company, split_feed_item_id) = attachment_state("THR", "9300600");
    for unit in &units {
        split_state
            .insider()
            .merge_attachment_units(
                &split_company.id,
                &split_feed_item_id,
                std::slice::from_ref(unit),
            )
            .expect("singleton merge");
    }

    assert_eq!(
        attachment_rows(&single_state, &single_feed_item_id),
        attachment_rows(&split_state, &split_feed_item_id),
    );
    assert_eq!(
        attachment_rows(&single_state, &single_feed_item_id).len(),
        3,
        "the three source units stay distinct",
    );
}

#[test]
fn attachment_merge_progressive_fill_same_identity() {
    let (state, company, feed_item_id) = attachment_state("PRG", "9300700");
    let first = attachment_unit("Anna Nowak", "ANNA NOWAK", Some(Direction::Buy), None, None);
    let second = attachment_unit(
        "Anna Nowak",
        "ANNA NOWAK",
        Some(Direction::Buy),
        Some("2026-07-03"),
        None,
    );
    let third = attachment_unit(
        "Anna Nowak",
        "ANNA NOWAK",
        Some(Direction::Buy),
        Some("2026-07-03"),
        Some("100"),
    );

    state
        .insider()
        .merge_attachment_units(
            &company.id,
            &feed_item_id,
            &[sourced("doc-progressive", 0, first)],
        )
        .expect("initial partial unit");
    state
        .insider()
        .merge_attachment_units(
            &company.id,
            &feed_item_id,
            &[sourced("doc-progressive", 0, second)],
        )
        .expect("date fill");
    state
        .insider()
        .merge_attachment_units(
            &company.id,
            &feed_item_id,
            &[sourced("doc-progressive", 0, third)],
        )
        .expect("volume fill");

    let rows: Vec<_> = attachment_rows(&state, &feed_item_id)
        .into_iter()
        .filter(|row| row.source_document_id.as_deref() == Some("doc-progressive"))
        .collect();
    assert_eq!(rows.len(), 1, "progressive fills keep one source row");
    assert_eq!(rows[0].tx_date.as_deref(), Some("2026-07-03"));
    assert_eq!(rows[0].volume.as_deref(), Some("100"));
}
