//! Management-holdings storage + founder-stamping join tests (ADR 0083 D6, plan
//! v0.57 T5). The stamping join is exact-canonical-identity only (never a shared
//! surname), covers both substrates (management holdings + insider transactions),
//! and never overrides an existing (manual/dictionary) label.

use super::*;
use crate::fundamentals::management_holdings::MgmtRole;

fn company(state: &AppState, ticker: &str) -> String {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: format!("{ticker} S.A."),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company")
        .id
}

fn report_doc(state: &AppState, company_id: &str, url: &str) -> String {
    state
        .create_or_find_pending_report_document(CaptureReportDocumentInput {
            company_id: company_id.to_owned(),
            source_type: "user_url".to_owned(),
            url: url.to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("Sprawozdanie Zarządu 2025".to_owned()),
            attribution: None,
        })
        .expect("report document")
        .id
}

fn stake(state: &AppState, company_id: &str, holder: &str, holder_type: Option<&str>) {
    state
        .ownership()
        .append_snapshot(NewOwnershipStake {
            company_id: company_id.to_owned(),
            holder_name_raw: holder.to_owned(),
            holder_type: holder_type.map(str::to_owned),
            capital_pct: Some("20.0".to_owned()),
            votes_pct: Some("20.0".to_owned()),
            as_of: "2025-12-31".to_owned(),
            source: "report_document".to_owned(),
            report_document_id: None,
            feed_item_id: None,
        })
        .expect("stake");
}

fn holding(
    state: &AppState,
    company_id: &str,
    doc_id: &str,
    person: &str,
    role: MgmtRole,
    shares: Option<&str>,
    indirect_via: Option<&str>,
) {
    state
        .management_holdings()
        .upsert_holding(NewManagementHolding {
            company_id: company_id.to_owned(),
            report_document_id: doc_id.to_owned(),
            person_name_raw: person.to_owned(),
            role: Some(role.as_str().to_owned()),
            shares: shares.map(str::to_owned),
            indirect_via_raw: indirect_via.map(str::to_owned),
            prior_shares: None,
            prior_as_of: None,
            as_of: "2025-12-31".to_owned(),
        })
        .expect("holding");
}

fn holder_type(state: &AppState, company_id: &str, holder_normalized: &str) -> Option<String> {
    state
        .ownership()
        .current_state(company_id)
        .expect("state")
        .into_iter()
        .find(|s| s.holder_name_normalized == holder_normalized)
        .and_then(|s| s.holder_type)
}

#[test]
fn holding_upsert_is_idempotent_by_document_and_person() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "TXT");
    let doc = report_doc(&s, &c, "https://example.com/txt.xhtml");
    holding(
        &s,
        &c,
        &doc,
        "Tadeusz Wróblewski",
        MgmtRole::Management,
        Some("3366250"),
        None,
    );
    holding(
        &s,
        &c,
        &doc,
        "Tadeusz Wróblewski",
        MgmtRole::Management,
        Some("3366250"),
        None,
    );
    let rows = s.management_holdings().list_by_company(&c).expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].shares.as_deref(), Some("3366250"));
    assert_eq!(rows[0].person_normalized, "TADEUSZ WRÓBLEWSKI");
}

#[test]
fn zero_aggregate_marker_is_kept_out_of_by_person_list() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "GPW");
    let doc = report_doc(&s, &c, "https://example.com/gpw.xhtml");
    s.management_holdings()
        .upsert_zero_aggregate(&c, &doc, None, "2023-12-31")
        .expect("zero aggregate");
    // Not surfaced as a by-person holding (it carries no real person).
    assert!(s
        .management_holdings()
        .list_by_company(&c)
        .expect("list")
        .is_empty());
}

#[test]
fn residual_records_and_clears() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "CDR");
    let doc = report_doc(&s, &c, "https://example.com/cdr.xhtml");
    s.management_holdings()
        .record_residual(ManagementHoldingsResidual {
            report_document_id: doc.clone(),
            company_id: c.clone(),
            parse_state: "glyph_encoded".to_owned(),
            detected_as_of: Some("2025-06-30".to_owned()),
            matched_heading: Some("Akcje Spółki w posiadaniu członków Zarządu".to_owned()),
            created_at: String::new(),
            updated_at: String::new(),
        })
        .expect("residual");
    assert_eq!(
        s.management_holdings()
            .list_residuals(&c)
            .expect("list")
            .len(),
        1
    );
    s.management_holdings().clear_residual(&doc).expect("clear");
    assert!(s
        .management_holdings()
        .list_residuals(&c)
        .expect("list")
        .is_empty());
}

#[test]
fn founder_stamp_matches_person_name() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "TXT");
    let doc = report_doc(&s, &c, "https://example.com/txt.xhtml");
    stake(&s, &c, "Tadeusz Wróblewski", None);
    holding(
        &s,
        &c,
        &doc,
        "Tadeusz Wróblewski",
        MgmtRole::Management,
        Some("3366250"),
        None,
    );

    let stamped = s
        .management_holdings()
        .stamp_founder_insiders(&c)
        .expect("stamp");
    assert_eq!(stamped, 1);
    assert_eq!(
        holder_type(&s, &c, "TADEUSZ WRÓBLEWSKI").as_deref(),
        Some("founder_insider")
    );
}

#[test]
fn founder_stamp_matches_indirect_vehicle() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "KRU");
    let doc = report_doc(&s, &c, "https://example.com/kru.xhtml");
    // The >5% stake is held by the family foundation; the founder holds via it.
    stake(&s, &c, "Krupczak Fundacja Rodzinna", None);
    holding(
        &s,
        &c,
        &doc,
        "Piotr Krupczak",
        MgmtRole::Management,
        Some("1716965"),
        Some("Krupczak Fundacja Rodzinna"),
    );

    let stamped = s
        .management_holdings()
        .stamp_founder_insiders(&c)
        .expect("stamp");
    assert_eq!(stamped, 1);
    assert_eq!(
        holder_type(&s, &c, "KRUPCZAK FUNDACJA RODZINNA").as_deref(),
        Some("founder_insider")
    );
}

#[test]
fn founder_stamp_rejects_shared_surname() {
    // The classic false friend: Adam Kiciński (board) vs Michał Kiciński (>5%
    // owner) are different holders — a surname must never match.
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "CDR");
    let doc = report_doc(&s, &c, "https://example.com/cdr.xhtml");
    stake(&s, &c, "Michał Kiciński", None);
    holding(
        &s,
        &c,
        &doc,
        "Adam Kiciński",
        MgmtRole::Supervisory,
        Some("4046001"),
        None,
    );

    let stamped = s
        .management_holdings()
        .stamp_founder_insiders(&c)
        .expect("stamp");
    assert_eq!(stamped, 0);
    assert_eq!(holder_type(&s, &c, "MICHAŁ KICIŃSKI"), None);
}

#[test]
fn founder_stamp_never_overrides_existing_label() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "KRU");
    let doc = report_doc(&s, &c, "https://example.com/kru.xhtml");
    // A manual / dictionary label already sits on the row.
    stake(&s, &c, "Piotr Krupczak", Some("family_foundation"));
    holding(
        &s,
        &c,
        &doc,
        "Piotr Krupczak",
        MgmtRole::Management,
        Some("1716965"),
        None,
    );

    let stamped = s
        .management_holdings()
        .stamp_founder_insiders(&c)
        .expect("stamp");
    assert_eq!(stamped, 0);
    assert_eq!(
        holder_type(&s, &c, "PIOTR KRUPCZAK").as_deref(),
        Some("family_foundation")
    );
}

#[test]
fn founder_stamp_no_match_writes_nothing() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "PKO");
    let doc = report_doc(&s, &c, "https://example.com/pko.xhtml");
    stake(&s, &c, "Nationale-Nederlanden OFE", None);
    holding(
        &s,
        &c,
        &doc,
        "Szymon Kowal",
        MgmtRole::Management,
        Some("0"),
        None,
    );

    let stamped = s
        .management_holdings()
        .stamp_founder_insiders(&c)
        .expect("stamp");
    assert_eq!(stamped, 0);
    assert_eq!(holder_type(&s, &c, "NATIONALE-NEDERLANDEN OFE"), None);
}

#[test]
fn insider_transaction_substrate_feeds_the_same_stamp() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "SNT");
    stake(&s, &c, "Cezary Kozielski", None);
    // Insert an insider_transactions row directly (the T4 substrate) — FK off so
    // the test needs no feed-item scaffolding; the join reads person_normalized.
    {
        let conn = s.checkout_for_tests().expect("conn");
        conn.execute("PRAGMA foreign_keys = OFF", [])
            .expect("pragma");
        conn.execute(
            "INSERT INTO insider_transactions
                (id, company_id, feed_item_id, unit_index, person_name_raw, person_normalized, role, direction)
             VALUES ('itx1', ?1, 'fi_synth', 0, 'Cezary Kozielski', 'CEZARY KOZIELSKI', 'management', 'buy')",
            [&c],
        )
        .expect("insert insider tx");
    }
    let stamped = s
        .management_holdings()
        .stamp_founder_insiders(&c)
        .expect("stamp");
    assert_eq!(stamped, 1);
    assert_eq!(
        holder_type(&s, &c, "CEZARY KOZIELSKI").as_deref(),
        Some("founder_insider")
    );
}

#[test]
fn skin_in_the_game_join_covers_person_and_vehicle() {
    let s = AppState::new(open_in_memory_database().expect("db"));
    let c = company(&s, "SNT");
    let doc = report_doc(&s, &c, "https://example.com/snt.xhtml");
    holding(
        &s,
        &c,
        &doc,
        "Dariusz Korecki",
        MgmtRole::Management,
        Some("100000"),
        None,
    );
    holding(
        &s,
        &c,
        &doc,
        "Cezary Kozielski",
        MgmtRole::Management,
        Some("2047380"),
        Some("Melhus Company Ltd"),
    );

    let skin = s.management_holdings().skin_in_the_game(&c).expect("skin");
    // Direct person identity + the vehicle identity are both corroboration keys.
    let direct = skin
        .get(
            &crate::fundamentals::ownership::classify::canonical_holder_identity("Dariusz Korecki"),
        )
        .expect("direct match");
    assert_eq!(direct.person, "Dariusz Korecki");
    assert!(direct.via.is_none());

    let vehicle = skin
        .get(
            &crate::fundamentals::ownership::classify::canonical_holder_identity(
                "Melhus Company Ltd",
            ),
        )
        .expect("vehicle match");
    assert_eq!(vehicle.person, "Cezary Kozielski");
    assert_eq!(vehicle.via.as_deref(), Some("Melhus Company Ltd"));
}
