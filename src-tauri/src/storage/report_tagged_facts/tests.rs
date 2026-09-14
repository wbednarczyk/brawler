use super::*;
use crate::storage::open_in_memory_database;

fn seed_company_and_document(connection: &Connection, company: &str, doc: &str) {
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES (?1, 'gpw', ?1, ?2, ?3)",
            params![company, format!("GPW:{company}"), format!("{company} SA")],
        )
        .expect("company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
            params![doc, company, format!("https://x/{doc}.zip")],
        )
        .expect("document");
}

fn basic_fact(package: &str, identity: &str, concept: &str) -> NewTaggedFact {
    NewTaggedFact {
        package_entry_path: package.to_owned(),
        fact_identity: identity.to_owned(),
        identity_kind: "xml_id".to_owned(),
        concept_namespace_uri: "http://xbrl.ifrs.org/taxonomy/2021-03-24/ifrs-full".to_owned(),
        concept_local_name: concept.to_owned(),
        context_ref: "c1".to_owned(),
        period_type: "duration".to_owned(),
        period_start: Some("2024-01-01".to_owned()),
        period_end: "2024-12-31".to_owned(),
        unit_ref: Some("u1".to_owned()),
        unit_measure: Some("iso4217:PLN".to_owned()),
        value_raw: "1000".to_owned(),
        value_numeric: Some("1000".to_owned()),
        scale: Some(0),
        sign: None,
        decimals: Some("0".to_owned()),
        is_dimensional: false,
        dimensions_json: None,
        parse_status: "ok".to_owned(),
        parse_error: None,
        roles: Vec::new(),
    }
}

fn extraction_with(facts: Vec<NewTaggedFact>) -> TaggedFactExtraction {
    TaggedFactExtraction {
        source_content_hash: Some("hash1".to_owned()),
        extractor_version: 1,
        state: "extracted".to_owned(),
        encountered_count: facts.len() as i64,
        stored_count: facts.len() as i64,
        dimensional_count: 0,
        no_linkbase_fallback_count: 0,
        facts,
    }
}

#[test]
fn replace_writes_facts_and_extraction_record() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let extraction = extraction_with(vec![basic_fact(
        "reports/instance.xhtml",
        "xml_id:f1",
        "ProfitLoss",
    )]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

    let facts = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].concept_local_name, "ProfitLoss");
    assert_eq!(facts[0].report_document_id, "doc1");
    assert_eq!(facts[0].company_id, "c1");

    let record = get_extraction(&connection, "doc1")
        .expect("extraction")
        .expect("some");
    assert_eq!(record.source_content_hash.as_deref(), Some("hash1"));
    assert_eq!(record.extractor_version, 1);
    assert_eq!(record.stored_count, 1);
}

#[test]
fn replace_is_atomic_and_idempotent_one_generation() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let extraction = extraction_with(vec![basic_fact(
        "reports/instance.xhtml",
        "xml_id:f1",
        "ProfitLoss",
    )]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("first");
    let first_id = get_facts(&connection, "doc1").expect("facts")[0].id.clone();

    // Writing the SAME extraction again must yield one generation: old
    // rows deleted, new ones inserted (fresh ids), never doubled.
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("second");
    let facts = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(facts.len(), 1, "must not double the generation");
    assert_ne!(
        facts[0].id, first_id,
        "replace mints a fresh id, it does not reuse the old row"
    );
}

#[test]
fn a_mid_transaction_failure_leaves_the_previous_generation_intact() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let first = extraction_with(vec![basic_fact(
        "reports/instance.xhtml",
        "xml_id:f1",
        "ProfitLoss",
    )]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &first).expect("first replace");
    let before = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(before.len(), 1);

    // A poisoned second generation: two facts sharing the SAME identity
    // within the same (document, package_entry_path) — the UNIQUE
    // constraint fails partway through the insert loop, forcing the
    // whole transaction (including the leading DELETE) to roll back.
    let poisoned = extraction_with(vec![
        basic_fact("reports/instance.xhtml", "xml_id:dup", "Revenue"),
        basic_fact("reports/instance.xhtml", "xml_id:dup", "OperatingProfit"),
    ]);
    let error = replace_tagged_facts(&mut connection, "doc1", "c1", &poisoned)
        .expect_err("duplicate fact_identity within one generation must fail");
    assert!(matches!(error, StorageError::Sqlite(_)));

    let after = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(
        after, before,
        "a failed rebuild must never leave a half-generation — the prior generation survives untouched"
    );
}

#[test]
fn unique_key_admits_same_concept_and_context_twice_when_identity_differs() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    // Real shape (ADR 0100 decision 4): ProfitLoss tagged 2x at an
    // identical (concept, context) — distinguished by fact_identity.
    let mut a = basic_fact("reports/instance.xhtml", "xml_id:f1", "ProfitLoss");
    a.context_ref = "ctxA".to_owned();
    let mut b = basic_fact("reports/instance.xhtml", "xml_id:f2", "ProfitLoss");
    b.context_ref = "ctxA".to_owned();

    let extraction = extraction_with(vec![a, b]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction)
        .expect("distinct identities at the same concept/context must both be stored");

    let facts = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(facts.len(), 2);
}

#[test]
fn two_package_instances_can_share_a_fact_identity() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let a = basic_fact("reports/instanceA.xhtml", "xml_id:f1", "ProfitLoss");
    let b = basic_fact("reports/instanceB.xhtml", "xml_id:f1", "ProfitLoss");

    let extraction = extraction_with(vec![a, b]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction)
        .expect("differing package_entry_path must not collide on the same fact_identity");

    let facts = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(facts.len(), 2);
}

#[test]
fn a_failed_normalization_occurrence_is_stored_never_dropped() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let mut broken = basic_fact("reports/instance.xhtml", "xml_id:f1", "ProfitLoss");
    broken.value_raw = "N/A".to_owned();
    broken.value_numeric = None;
    broken.parse_status = "unparsed_value".to_owned();
    broken.parse_error = Some("could not parse 'N/A' as a decimal".to_owned());

    let extraction = extraction_with(vec![broken]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

    let facts = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(facts.len(), 1, "decision 9: never a silent drop");
    assert_eq!(facts[0].value_numeric, None);
    assert_eq!(facts[0].parse_status, "unparsed_value");
    assert!(facts[0].parse_error.is_some());
}

#[test]
fn freshness_check_skips_unchanged_and_rebuilds_on_hash_or_version_change() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let extraction = extraction_with(vec![basic_fact(
        "reports/instance.xhtml",
        "xml_id:f1",
        "ProfitLoss",
    )]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

    assert!(extraction_is_current(&connection, "doc1", "hash1", 1).expect("current"));
    assert!(
        !extraction_is_current(&connection, "doc1", "hash1", 2).expect("version changed"),
        "an extractor_version bump must invalidate"
    );
    assert!(
        !extraction_is_current(&connection, "doc1", "hash2", 1).expect("hash changed"),
        "recapture (a changed source_content_hash) must invalidate"
    );
}

#[test]
fn one_fact_with_several_roles_round_trips() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let mut fact = basic_fact("reports/instance.xhtml", "xml_id:f1", "ProfitLoss");
    fact.roles = vec![
        NewTaggedFactRole {
            role_uri: "ias_1_role-320000".to_owned(),
            role_kind: "income".to_owned(),
        },
        NewTaggedFactRole {
            role_uri: "ias_1_role-610000".to_owned(),
            role_kind: "equity_changes".to_owned(),
        },
    ];

    let extraction = extraction_with(vec![fact]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction).expect("replace");

    let facts = get_facts(&connection, "doc1").expect("facts");
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].roles.len(), 2);
    let kinds: Vec<&str> = facts[0]
        .roles
        .iter()
        .map(|r| r.role_kind.as_str())
        .collect();
    assert!(kinds.contains(&"income"));
    assert!(kinds.contains(&"equity_changes"));
}

#[test]
fn harvest_returns_uncrosswalked_concepts_ranked_by_company_count_and_omits_crosswalked_ones() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    seed_company_and_document(&connection, "c2", "doc2");
    seed_company_and_document(&connection, "c3", "doc3");

    // "Assets" IS in the crosswalk (moved verbatim from esef.rs) — must
    // never appear in the harvest output, however many companies tag it.
    let crosswalked = extraction_with(vec![basic_fact(
        "reports/instance.xhtml",
        "xml_id:a",
        "Assets",
    )]);
    replace_tagged_facts(&mut connection, "doc1", "c1", &crosswalked).expect("c1 replace");
    replace_tagged_facts(&mut connection, "doc2", "c2", &crosswalked).expect("c2 replace");

    // A novel concept absent from the crosswalk, tagged by one company —
    // must appear, ranked, with its observed period_type.
    let novel = extraction_with(vec![basic_fact(
        "reports/instance.xhtml",
        "xml_id:n",
        "SomeNovelExtensionConceptNotInTheCrosswalk",
    )]);
    replace_tagged_facts(&mut connection, "doc3", "c3", &novel).expect("c3 replace");

    let harvested = harvest_uncrosswalked_concepts(&connection).expect("harvest");

    assert!(
        harvested.iter().all(|h| h.concept_local_name != "Assets"),
        "a crosswalked concept must never appear in the harvest: {harvested:?}"
    );

    let novel_row = harvested
        .iter()
        .find(|h| h.concept_local_name == "SomeNovelExtensionConceptNotInTheCrosswalk")
        .expect("the novel concept must be present");
    assert_eq!(novel_row.company_count, 1);
    assert_eq!(novel_row.period_types, vec!["duration".to_owned()]);
}

fn role(kind: &str) -> NewTaggedFactRole {
    NewTaggedFactRole {
        role_uri: format!("http://x/role/{kind}"),
        role_kind: kind.to_owned(),
    }
}

fn seed_document(connection: &Connection, doc: &str, company: &str) {
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES (?1, ?2, 'espi_attachment', ?3, 'fetched')",
            params![doc, company, format!("https://x/{doc}.zip")],
        )
        .expect("document");
}

#[test]
fn get_facts_for_company_spans_every_document_that_company_owns() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    seed_document(&connection, "doc2", "c1");
    seed_company_and_document(&connection, "c2", "doc3");

    replace_tagged_facts(
        &mut connection,
        "doc1",
        "c1",
        &extraction_with(vec![basic_fact("reports/i.xhtml", "f1", "Assets")]),
    )
    .expect("doc1");
    replace_tagged_facts(
        &mut connection,
        "doc2",
        "c1",
        &extraction_with(vec![basic_fact("reports/i.xhtml", "f2", "Revenue")]),
    )
    .expect("doc2");
    replace_tagged_facts(
        &mut connection,
        "doc3",
        "c2",
        &extraction_with(vec![basic_fact("reports/i.xhtml", "f3", "Equity")]),
    )
    .expect("doc3");

    let facts = get_facts_for_company(&connection, "c1").expect("facts");
    assert_eq!(facts.len(), 2, "must span both of c1's documents");
    assert!(facts.iter().all(|f| f.company_id == "c1"));
    assert!(facts
        .iter()
        .any(|f| f.concept_local_name == "Assets" && f.report_document_id == "doc1"));
    assert!(facts
        .iter()
        .any(|f| f.concept_local_name == "Revenue" && f.report_document_id == "doc2"));
}

/// One document, one instant balance-sheet concept ("Assets" — crosswalked,
/// via the balance role) plus one novel dimensionless concept with no
/// crosswalk entry and a primary-statement role ("awaiting a name") plus
/// one dimensional row plus one note-level (no primary-statement role)
/// row — exercises every non-conflicting bucket in one pass.
#[test]
fn coverage_counts_buckets_a_documents_facts_by_projection_outcome() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let mut projected_fact = basic_fact("reports/i.xhtml", "f1", "Assets");
    projected_fact.period_type = "instant".to_owned();
    projected_fact.roles = vec![role("balance")];

    let mut awaiting_name_fact = basic_fact("reports/i.xhtml", "f2", "SomeNovelBalanceConcept");
    awaiting_name_fact.period_type = "instant".to_owned();
    awaiting_name_fact.roles = vec![role("balance")];

    let mut dimensional_fact = basic_fact("reports/i.xhtml", "f3", "Assets");
    dimensional_fact.is_dimensional = true;
    dimensional_fact.roles = vec![role("balance")];

    let mut note_level_fact = basic_fact("reports/i.xhtml", "f4", "SomeNoteConcept");
    note_level_fact.roles = vec![role("other")];

    replace_tagged_facts(
        &mut connection,
        "doc1",
        "c1",
        &extraction_with(vec![
            projected_fact,
            awaiting_name_fact,
            dimensional_fact,
            note_level_fact,
        ]),
    )
    .expect("replace");

    let counts = coverage_counts(&connection, "c1").expect("coverage counts");
    assert_eq!(counts.raw_stored, 4);
    assert_eq!(counts.dimensional, 1);
    assert_eq!(counts.projected, 1, "the crosswalked Assets fact");
    assert_eq!(counts.awaiting_name, 1, "the novel uncrosswalked concept");
    assert_eq!(counts.note_level, 1, "the no-primary-statement-role fact");
    assert_eq!(counts.conflicting, 0);
}

/// Test H (#508 decision 5): a mixed-basis document's non-primary-basis
/// and ambiguous occurrences both roll into ONE `other_basis` count. The
/// contract's asymmetric fixture: a consolidated instance carrying the
/// balanced identity (`Assets 100 = Liabilities 60 + Equity 40`) plus a
/// standalone instance duplicating `Assets` (50) AND carrying its own
/// exclusive `CurrentLiabilities` (30) — both standalone occurrences are
/// crosswalk-resolved, primary-statement facts dropped for basis alone —
/// PLUS a mapped occurrence from a genuinely AMBIGUOUS instance (entry path
/// carrying both a consolidated and a standalone token, astra r1 #3), so
/// `other_basis == 3`, never conflated with `note_level`/`conflicting`.
#[test]
fn coverage_counts_counts_non_primary_basis_occurrences_as_other_basis() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let mut consolidated_assets = basic_fact("reports/skonsolidowane/i.xhtml", "f1", "Assets");
    consolidated_assets.period_type = "instant".to_owned();
    consolidated_assets.roles = vec![role("balance")];
    consolidated_assets.value_numeric = Some("100".to_owned());

    let mut liabilities = basic_fact("reports/skonsolidowane/i.xhtml", "f2", "Liabilities");
    liabilities.period_type = "instant".to_owned();
    liabilities.roles = vec![role("balance")];
    liabilities.value_numeric = Some("60".to_owned());

    let mut equity = basic_fact("reports/skonsolidowane/i.xhtml", "f3", "Equity");
    equity.period_type = "instant".to_owned();
    equity.roles = vec![role("balance")];
    equity.value_numeric = Some("40".to_owned());

    let mut standalone_assets = basic_fact("reports/jednostkowe/i.xhtml", "f4", "Assets");
    standalone_assets.period_type = "instant".to_owned();
    standalone_assets.roles = vec![role("balance")];
    standalone_assets.value_numeric = Some("50".to_owned());

    let mut standalone_current_liabilities =
        basic_fact("reports/jednostkowe/i.xhtml", "f5", "CurrentLiabilities");
    standalone_current_liabilities.period_type = "instant".to_owned();
    standalone_current_liabilities.roles = vec![role("balance")];
    standalone_current_liabilities.value_numeric = Some("30".to_owned());

    // Astra r1 #3: a mapped occurrence from a genuinely AMBIGUOUS instance —
    // its entry path carries BOTH a consolidated and a standalone token, so
    // it is excluded from projection and counted, but never as a
    // non-primary-basis drop.
    let mut ambiguous_equity = basic_fact(
        "reports/skonsolidowane-i-jednostkowe/i.xhtml",
        "f6",
        "Equity",
    );
    ambiguous_equity.period_type = "instant".to_owned();
    ambiguous_equity.roles = vec![role("balance")];
    ambiguous_equity.value_numeric = Some("999".to_owned());

    replace_tagged_facts(
        &mut connection,
        "doc1",
        "c1",
        &extraction_with(vec![
            consolidated_assets,
            liabilities,
            equity,
            standalone_assets,
            standalone_current_liabilities,
            ambiguous_equity,
        ]),
    )
    .expect("replace");

    let counts = coverage_counts(&connection, "c1").expect("coverage counts");
    assert_eq!(counts.raw_stored, 6);
    assert_eq!(counts.projected, 3, "the three consolidated facts");
    assert_eq!(
        counts.other_basis, 3,
        "the standalone Assets duplicate + its exclusive CurrentLiabilities \
         + the mapped ambiguous Equity occurrence"
    );
    assert_eq!(counts.conflicting, 0, "different bases never conflict");
    assert_eq!(counts.note_level, 0);
    assert_eq!(counts.awaiting_name, 0);
}

/// sol round 4: a PRESENT derived-period row wins unconditionally. When
/// the declared reporting date is absent from the tagged dates, the
/// honest outcome is ZERO selected and everything comparative — never a
/// silent switch to the latest tagged date, which would let a
/// subsequent-events note instant impersonate the reporting period.
#[test]
fn coverage_counts_never_substitutes_a_tagged_date_for_the_declared_reporting_date() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let mut fact = basic_fact("reports/i.xhtml", "f1", "Assets");
    fact.period_type = "instant".to_owned();
    fact.roles = vec![role("balance")];
    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction_with(vec![fact]))
        .expect("replace");

    // The document DECLARES 2024-12-30; the only tagged date is
    // 2024-12-31 (basic_fact's period_end).
    connection
        .execute(
            "INSERT INTO document_derived_periods
                 (report_document_id, has_period, fiscal_year, period_type, period_end,
                  derivation_version)
             VALUES ('doc1', 1, 2024, 'FY', '2024-12-30', 1)",
            [],
        )
        .expect("seed derived period");

    let counts = coverage_counts(&connection, "c1").expect("coverage counts");
    assert_eq!(
        counts.projected, 0,
        "no tagged fact carries the declared reporting date — nothing may claim selection"
    );
    assert_eq!(
        counts.comparative, 1,
        "the tagged date is honestly a non-reporting period"
    );
}

#[test]
fn coverage_counts_reports_a_genuine_value_disagreement_as_conflicting_never_projected() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");

    let mut a = basic_fact("reports/i.xhtml", "f1", "Assets");
    a.period_type = "instant".to_owned();
    a.roles = vec![role("balance")];
    a.value_numeric = Some("1000".to_owned());
    let mut b = basic_fact("reports/i.xhtml", "f2", "Assets");
    b.period_type = "instant".to_owned();
    b.roles = vec![role("balance")];
    b.value_numeric = Some("1500".to_owned());

    replace_tagged_facts(&mut connection, "doc1", "c1", &extraction_with(vec![a, b]))
        .expect("replace");

    let counts = coverage_counts(&connection, "c1").expect("coverage counts");
    assert_eq!(counts.raw_stored, 2);
    // Fact-level accounting (sol round 2, finding 8): the ONE conflict
    // slot explains BOTH raw rows, so both are counted here — the
    // buckets account for numbers, not slots.
    assert_eq!(counts.conflicting, 2);
    assert_eq!(
        counts.projected, 0,
        "a genuine disagreement must never resolve into a projected count"
    );
}

#[test]
fn harvest_for_company_ranks_by_the_global_company_count_and_carries_this_companys_occurrences() {
    let mut connection = open_in_memory_database().expect("db");
    seed_company_and_document(&connection, "c1", "doc1");
    seed_company_and_document(&connection, "c2", "doc2");

    // "WidelyTaggedNovelConcept": tagged by c1 AND c2 — global company_count 2.
    let mut wide_a = basic_fact("reports/i.xhtml", "wide_a", "WidelyTaggedNovelConcept");
    wide_a.period_type = "instant".to_owned();
    wide_a.roles = vec![role("balance")];
    let mut wide_b = basic_fact("reports/i.xhtml", "wide_b", "WidelyTaggedNovelConcept");
    wide_b.period_type = "instant".to_owned();
    wide_b.roles = vec![role("balance")];

    // "NarrowNovelConcept": tagged by c1 only, twice (occurrence_count 2).
    let mut narrow_a = basic_fact("reports/i.xhtml", "narrow_a", "NarrowNovelConcept");
    narrow_a.roles = vec![role("income")];
    let mut narrow_b = basic_fact("reports/i.xhtml", "narrow_b", "NarrowNovelConcept");
    narrow_b.roles = vec![role("income")];

    replace_tagged_facts(
        &mut connection,
        "doc1",
        "c1",
        &extraction_with(vec![wide_a, narrow_a, narrow_b]),
    )
    .expect("c1 replace");
    replace_tagged_facts(
        &mut connection,
        "doc2",
        "c2",
        &extraction_with(vec![wide_b]),
    )
    .expect("c2 replace");

    let rows =
        harvest_uncrosswalked_concepts_for_company(&connection, "c1").expect("harvest for company");

    assert_eq!(rows.len(), 2, "only c1's own concepts: {rows:?}");
    assert_eq!(
        rows[0].concept_local_name, "WidelyTaggedNovelConcept",
        "ranked by the GLOBAL company count first, {rows:?}"
    );
    assert_eq!(rows[0].company_count, 2);
    assert_eq!(rows[0].occurrence_count, 1, "c1 tagged it once");
    assert_eq!(rows[0].statement_group, "balance");
    assert_eq!(rows[0].period_nature, "instant");

    let narrow = &rows[1];
    assert_eq!(narrow.concept_local_name, "NarrowNovelConcept");
    assert_eq!(narrow.company_count, 1);
    assert_eq!(narrow.occurrence_count, 2, "c1 tagged it twice");
    assert_eq!(narrow.statement_group, "income");
    assert_eq!(narrow.period_nature, "duration");
}
