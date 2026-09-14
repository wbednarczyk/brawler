use super::*;
use crate::storage::{open_in_memory_database, StorageError};

/// The id of a freshly-created fact, asserting the commit was a new write
/// (not a re-observation) — the shape these unit tests exercise.
fn created_id(commit: StructuredFactCommit) -> String {
    match commit {
        StructuredFactCommit::Created(id) => id,
        other => panic!("expected a newly created fact, got {other:?}"),
    }
}

fn seed_company_and_document(connection: &Connection) -> (String, String) {
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
            [],
        )
        .expect("company");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
             VALUES ('doc1', 'c1', 'espi_attachment', 'https://x/doc1.pdf', 'fetched')",
            [],
        )
        .expect("document");
    ("c1".to_owned(), "doc1".to_owned())
}

/// Runs a job through the same completion path the runner uses and returns
/// the resulting `revenue` proposal (a seeded canonical metric key), so
/// confirm/auto-confirm tests exercise the real proposal->fact plumbing
/// without depending on the AI provider job runner (which lives outside
/// this module).
/// Seeds a pending proposal for an arbitrary canonical `metric_key`/`value`
/// (2025 FY period), so the T4.4 confirm-validation tests can drive a
/// balance-sheet total through the real proposal→fact→validate plumbing.
#[allow(clippy::too_many_arguments)]
/// Seeds an already-confirmed fact for `metric_key` in the 2025 FY period,
/// so a subsequent confirm assembles a multi-fact period the balance-sheet
/// identity can actually evaluate.
fn fact_provenance_row(
    connection: &Connection,
    fact_id: &str,
) -> Option<(String, String, Option<String>, Option<String>)> {
    connection
        .query_row(
            "SELECT source_tier, validation_status, drift_json, citation
             FROM financial_fact_provenance WHERE fact_id = ?1",
            [fact_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .expect("provenance query")
}

/// ADR 0077 G-1 class-closer: exercise every production write path in one
/// DB and assert not a single provenance row carries the retired
/// `validation_status='none'`. Pins the structured (ESEF) and ESPI
/// cover-note tiers — the surviving deterministic write paths.
#[test]
fn no_production_path_writes_validation_status_none() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    // Structured-pipeline (ESEF) emission.
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("structured emit");

    // ESPI cover-note tier emission: its own identifiable
    // extraction_method marker — must clear the G-1 no-`none` guard too.
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2023,
            period_type: "FY",
            period_end: Some("2023-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "900000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "espi_cover_note",
            extraction_method: "espi_cover_note",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Sales revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("cover-note emit");

    // The cover-note path's provenance is identifiable and honest.
    let cover_note_method: String = connection
        .query_row(
            "SELECT f.extraction_method FROM financial_facts f \
             JOIN financial_fact_provenance p ON p.fact_id = f.id \
             WHERE p.source_tier = 'espi_cover_note' AND f.extraction_method = 'espi_cover_note'",
            [],
            |row| row.get(0),
        )
        .expect("the cover-note fact carries an identifiable extraction_method");
    assert_eq!(cover_note_method, "espi_cover_note");

    let none_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_fact_provenance WHERE validation_status = 'none'",
            [],
            |row| row.get(0),
        )
        .expect("count query");
    assert_eq!(
        none_count, 0,
        "no production path may write validation_status='none' (ADR 0077 G-1)"
    );
}

/// ADR 0061: the structured pipeline persists its per-outcome drift
/// alongside the fact, not just returns it for the caller to drop.
#[test]
fn structured_fact_persists_drift_json_when_present() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    let drift = r#"{"addedLabels":[],"removedLabels":["total equity line"],"unitChanged":null}"#;

    let id = created_id(
        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: Some("2025-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "1000000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "esef",
                extraction_method: "api",
                validation_status: "flagged",
                drift_json: Some(drift),
                citation: Some("Przychody netto ze sprzedazy"),
                attribution: None,
                measure_window: None,
                data_quality: None,
                statement_basis: None,
            },
        )
        .expect("record structured fact"),
    );

    let (source_tier, validation_status, stored_drift, citation) =
        fact_provenance_row(&connection, &id).expect("a structured fact must carry provenance");
    assert_eq!(source_tier, "esef");
    assert_eq!(validation_status, "flagged");
    assert_eq!(stored_drift.as_deref(), Some(drift));
    assert_eq!(citation.as_deref(), Some("Przychody netto ze sprzedazy"));
}

/// The provenance `ON CONFLICT(fact_id)` clause must refresh `drift_json`
/// too (not just the columns it already updated) — proven directly against
/// the table rather than via two `record_structured_fact` calls: a second
/// call for the *same* period+metric hits `financial_facts`' own
/// `UNIQUE(period_id, definition_id, ...)` constraint before it would ever
/// reach a repeated `fact_id` (facts are not upserted, only inserted), so
/// that branch is unreached via this function today — this pins the SQL
/// behavior itself so a future caller that *does* reuse a `fact_id` (e.g. a
/// correction/re-provenance path) can rely on it.
#[test]
fn structured_fact_provenance_on_conflict_refreshes_drift_json() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let id = created_id(
        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: Some("2025-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "1000000",
                currency: Some("PLN"),
                confirmation_state: "flagged",
                source_tier: "esef",
                extraction_method: "api",
                validation_status: "flagged",
                drift_json: Some(r#"{"addedLabels":[],"removedLabels":["x"],"unitChanged":null}"#),
                citation: Some("Przychody"),
                attribution: None,
                measure_window: None,
                data_quality: None,
                statement_basis: None,
            },
        )
        .expect("record structured fact"),
    );

    // Re-provenance the same fact (the same `ON CONFLICT(fact_id)` upsert
    // `record_structured_fact` issues) with a resolved, drift-free outcome.
    connection
        .execute(
            "
            INSERT INTO financial_fact_provenance
                (fact_id, source_tier, validation_status, drift_json, citation)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(fact_id) DO UPDATE SET
                source_tier = excluded.source_tier,
                validation_status = excluded.validation_status,
                drift_json = excluded.drift_json,
                citation = excluded.citation,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![id, "pdf", "passed", Option::<&str>::None, "Przychody"],
        )
        .expect("re-provenance the same fact");

    let (_, validation_status, drift_json, _) =
        fact_provenance_row(&connection, &id).expect("a structured fact must carry provenance");
    assert_eq!(validation_status, "passed");
    assert_eq!(
        drift_json, None,
        "the upsert must clear a stale drift flag, not just leave it in place"
    );
}

/// ADR 0086 decision 3: an issuer tier re-observing an aggregator-held slot
/// takes the slot's LABEL over — the fact's evidence becomes the issuer's
/// filing, not the third-party page that happened to agree with it.
#[test]
fn an_issuer_reobservation_upgrades_an_aggregator_slot_label() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("aggregator write");

    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer re-observation");

    let (tier, citation): (String, String) = connection
        .query_row(
            "SELECT p.source_tier, p.citation FROM financial_fact_provenance p",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("provenance row");
    assert_eq!(
        tier, "esef",
        "the issuer tier must take over the slot label"
    );
    assert_eq!(citation, "Revenue", "the evidence must point at the filing");
}

/// ADR 0086 decision 3: an issuer tier DISAGREEING with an aggregator-held
/// slot overwrites it — the issuer's number wins its own slot (T7's
/// stored-wins rule now applies only between peers).
#[test]
fn an_issuer_divergence_overwrites_an_aggregator_slot() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "999000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("aggregator write");

    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000123",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer divergence");

    let (value, tier): (String, String) = connection
        .query_row(
            "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
             JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("fact row");
    assert_eq!(value, "1000123", "the issuer's number must win its slot");
    assert_eq!(tier, "esef");
}

/// Bug #324 (7 real rows on the maintainer's DB): an issuer tier
/// RE-OBSERVING (same value) a lower-tier slot must upgrade BOTH halves
/// of the provenance together — `financial_fact_provenance.source_tier`
/// to the issuer tier AND `financial_facts.extraction_method` to the
/// issuer's own marker (`api`), never leaving the stale lower tier's
/// method label stamped on a slot it no longer belongs to. Uses the ESPI
/// cover-note tier as the lower tier — its own identifiable
/// `extraction_method='espi_cover_note'` marker is distinct from the
/// generic `api` the issuer tier writes.
#[test]
fn an_issuer_reobservation_upgrades_a_lower_tier_slots_extraction_method_too() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "espi_cover_note",
            extraction_method: "espi_cover_note",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Sales revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("cover-note write");

    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer re-observation");

    let (tier, method): (String, String) = connection
        .query_row(
            "SELECT p.source_tier, f.extraction_method \
             FROM financial_facts f JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("fact + provenance row");
    assert_eq!(tier, "esef");
    assert_eq!(
        method, "api",
        "extraction_method must move with the tier upgrade, never stay stale"
    );
}

/// An issuer tier RE-OBSERVING (same value) a lower-tier slot that was
/// written with NO currency must fill in the gap from the incoming ESEF
/// write, which does carry one — never leave the fact permanently
/// currency-less just because the value happened to already agree. Uses
/// the aggregator tier as the lower tier.
#[test]
fn an_issuer_reobservation_fills_a_currency_gap_left_by_a_lower_tier() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: None,
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("aggregator write with no currency");

    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer re-observation");

    let currency: Option<String> = connection
        .query_row("SELECT currency FROM financial_facts", [], |row| row.get(0))
        .expect("fact row");
    assert_eq!(
        currency.as_deref(),
        Some("PLN"),
        "a same-value tier upgrade must fill a currency gap the lower tier left, \
         not leave the fact permanently currency-less"
    );
}

/// Companion to the currency-gap test: a same NUMBER in a CONTRADICTING
/// currency is not an agreement — EUR 1 000 000 and PLN 1 000 000 are
/// different figures sharing digits. It takes the Divergent path: the
/// HIGHER tier corrects the slot under its own currency and evidence.
#[test]
fn a_same_number_in_a_different_currency_is_a_divergence_the_higher_tier_corrects() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("EUR"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("aggregator write with EUR");

    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer re-observation");

    let (currency, tier): (Option<String>, String) = connection
        .query_row(
            "SELECT f.currency, p.source_tier
             FROM financial_facts f
             JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("fact row");
    assert_eq!(
        currency.as_deref(),
        Some("PLN"),
        "a same-number observation in a contradicting currency is a \
         divergence: the higher tier corrects the slot under its OWN \
         currency instead of relabeling the row around the aggregator's EUR"
    );
    assert_eq!(
        tier, "esef",
        "the correcting tier owns the slot's provenance"
    );
}

/// Bug #324, the DIVERGENT-value half: an issuer tier OVERWRITING a
/// lower-tier slot with a different value must sync `extraction_method`
/// alongside the value + tier — the exact `StructuredFactCommit::Upgraded`
/// path whose `update_financial_fact` call never carried an
/// `extraction_method` field. Uses the ESPI cover-note tier as the lower
/// tier — its own identifiable `extraction_method='espi_cover_note'`
/// marker is distinct from the generic `api` the issuer tier writes.
#[test]
fn an_issuer_divergence_upgrades_a_lower_tier_slots_extraction_method_too() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "999000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "espi_cover_note",
            extraction_method: "espi_cover_note",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Sales revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("cover-note write");

    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000123",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer divergence");

    let (value, tier, method): (String, String, String) = connection
        .query_row(
            "SELECT f.value_numeric, p.source_tier, f.extraction_method \
             FROM financial_facts f JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .expect("fact + provenance row");
    assert_eq!(value, "1000123");
    assert_eq!(tier, "esef");
    assert_eq!(
        method, "api",
        "extraction_method must move with the tier upgrade, never stay stale"
    );
}

/// A fact with NO provenance row is a manual entry — untouchable by every
/// automatic path (ADR 0086 decision 3), including an issuer-tier write.
#[test]
fn a_manual_slot_is_never_upgraded_or_overwritten() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "555000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "manual",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("hand-entered"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("seed");
    // Strip the provenance row — a hand-entered fact never gets one.
    connection
        .execute("DELETE FROM financial_fact_provenance", [])
        .expect("strip provenance");

    let commit = record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "560000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("issuer write against a manual slot");

    assert!(
        matches!(commit, StructuredFactCommit::Divergent { .. }),
        "a manual slot is reported as a divergence, never overwritten: {commit:?}"
    );
    let value: String = connection
        .query_row("SELECT value_numeric FROM financial_facts", [], |row| {
            row.get(0)
        })
        .expect("fact");
    assert_eq!(value, "555000", "the hand-entered value must survive");
}

/// F4 (ADR 0086 perf): a multi-fact page writes through the ONE batched call,
/// resolving each period once and returning a commit per input in order. The
/// second identical batch re-observes every catalog slot — proof the batch
/// wrote real rows as one logical unit (not per-fact best-effort).
#[test]
fn record_aggregator_facts_batches_a_page_in_order() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    let make = |value: &'static str, metric: &'static str| StructuredFactInput {
        company_id: &company_id,
        fiscal_year: 2024,
        period_type: "FY",
        period_end: Some("2024-12-31"),
        report_document_id: &document_id,
        metric_key: metric,
        value_numeric: value,
        currency: Some("PLN"),
        confirmation_state: "confirmed",
        source_tier: "html_aggregator",
        extraction_method: "api",
        validation_status: "unreviewed",
        drift_json: None,
        citation: Some("https://biznesradar.example/page | Przychody"),
        attribution: None,
        measure_window: None,
        data_quality: None,
        statement_basis: None,
    };
    // Two catalog facts sharing ONE period + a non-catalog key (order matters).
    let inputs = vec![
        make("1000000", "revenue"),
        make("2000000", "total_assets"),
        make("3", "definitely_not_a_catalog_metric"),
    ];

    let commits = record_aggregator_facts(&connection, &inputs).expect("batch write");
    assert_eq!(commits.len(), 3, "one commit per input, in order");
    assert!(matches!(commits[0], AggregatorFactCommit::Created(_)));
    assert!(matches!(commits[1], AggregatorFactCommit::Created(_)));
    assert!(matches!(commits[2], AggregatorFactCommit::NoDefinition));

    // Both catalog facts actually landed (the batch committed them together).
    let fact_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");
    assert_eq!(fact_count, 2);

    let again = record_aggregator_facts(&connection, &inputs).expect("re-batch");
    assert!(matches!(again[0], AggregatorFactCommit::Reobserved { .. }));
    assert!(matches!(again[1], AggregatorFactCommit::Reobserved { .. }));
    assert!(matches!(again[2], AggregatorFactCommit::NoDefinition));
}

// --- ADR 0093 decision 1: `SourceTier::Agent` threaded through the trust
// ladder — ranked below every issuer tier and above `html_aggregator`. ---

fn agent_input<'a>(
    company_id: &'a str,
    document_id: &'a str,
    value: &'a str,
) -> StructuredFactInput<'a> {
    StructuredFactInput {
        company_id,
        fiscal_year: 2024,
        period_type: "FY",
        period_end: Some("2024-12-31"),
        report_document_id: document_id,
        metric_key: "revenue",
        value_numeric: value,
        currency: Some("PLN"),
        confirmation_state: "confirmed",
        source_tier: "agent",
        extraction_method: "mcp_agent",
        validation_status: "unreviewed",
        drift_json: None,
        citation: Some("XTB RB 18/2026 | Revenue"),
        attribution: None,
        measure_window: None,
        data_quality: None,
        statement_basis: None,
    }
}

fn issuer_input<'a>(
    company_id: &'a str,
    document_id: &'a str,
    value: &'a str,
) -> StructuredFactInput<'a> {
    StructuredFactInput {
        company_id,
        fiscal_year: 2024,
        period_type: "FY",
        period_end: Some("2024-12-31"),
        report_document_id: document_id,
        metric_key: "revenue",
        value_numeric: value,
        currency: Some("PLN"),
        confirmation_state: "confirmed",
        source_tier: "esef",
        extraction_method: "api",
        validation_status: "passed",
        drift_json: None,
        citation: Some("Revenue"),
        attribution: None,
        measure_window: None,
        data_quality: None,
        statement_basis: None,
    }
}

/// (a) ADR 0093 decision 1: an issuer tier RE-OBSERVING an agent-held slot
/// takes the slot's LABEL over — mirrors
/// `an_issuer_reobservation_upgrades_an_aggregator_slot_label`, agent instead
/// of the aggregator.
#[test]
fn an_issuer_reobservation_upgrades_an_agent_slot_label() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "1000000"),
    )
    .expect("agent write");

    record_structured_fact(
        &connection,
        issuer_input(&company_id, &document_id, "1000000"),
    )
    .expect("issuer re-observation");

    let (tier, citation): (String, String) = connection
        .query_row(
            "SELECT p.source_tier, p.citation FROM financial_fact_provenance p",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("provenance row");
    assert_eq!(
        tier, "esef",
        "the issuer tier must take over the agent slot label"
    );
    assert_eq!(citation, "Revenue", "the evidence must point at the filing");
}

/// (a) ADR 0093 decision 1: an issuer tier DISAGREEING with an agent-held
/// slot overwrites it — a mis-extracted agent figure can never block the
/// audited correction.
#[test]
fn an_issuer_divergence_overwrites_an_agent_slot() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "999000"),
    )
    .expect("agent write");

    let commit = record_structured_fact(
        &connection,
        issuer_input(&company_id, &document_id, "1000123"),
    )
    .expect("issuer divergence");
    assert!(
        matches!(
            &commit,
            StructuredFactCommit::Upgraded {
                previous_tier,
                previous_value: Some(v),
                ..
            } if previous_tier == "agent" && v == "999000"
        ),
        "the issuer's number must upgrade the agent slot: {commit:?}"
    );

    let (value, tier): (String, String) = connection
        .query_row(
            "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
             JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("fact row");
    assert_eq!(value, "1000123", "the issuer's number must win its slot");
    assert_eq!(tier, "esef");
}

/// (b) ADR 0093 decision 1: the agent tier never overwrites an issuer-held
/// slot — a disagreement is a `Divergent` outcome, reported, never resolved
/// silently.
#[test]
fn an_agent_divergence_against_an_issuer_slot_is_never_applied() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        issuer_input(&company_id, &document_id, "1000123"),
    )
    .expect("issuer write");

    let commit = record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "999000"),
    )
    .expect("agent divergence against an issuer slot");
    assert!(
        matches!(commit, StructuredFactCommit::Divergent { .. }),
        "an agent write against an issuer slot is reported, never overwritten: {commit:?}"
    );

    let (value, tier): (String, String) = connection
        .query_row(
            "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
             JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("fact row");
    assert_eq!(value, "1000123", "the issuer's value must survive");
    assert_eq!(tier, "esef");
}

/// (b) ADR 0093 decision 1: the agent tier never overwrites a manual
/// (no-provenance) slot either.
#[test]
fn an_agent_write_against_a_manual_slot_is_never_applied() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "555000"),
    )
    .expect("seed");
    // Strip the provenance row — a hand-entered fact never gets one.
    connection
        .execute("DELETE FROM financial_fact_provenance", [])
        .expect("strip provenance");

    let commit = record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "560000"),
    )
    .expect("agent write against a manual slot");
    assert!(
        matches!(commit, StructuredFactCommit::Divergent { .. }),
        "a manual slot is reported as a divergence, never overwritten: {commit:?}"
    );
    let value: String = connection
        .query_row("SELECT value_numeric FROM financial_facts", [], |row| {
            row.get(0)
        })
        .expect("fact");
    assert_eq!(value, "555000", "the hand-entered value must survive");
}

/// (c) ADR 0093 decision 1: the agent tier fills over `html_aggregator` — an
/// agent reads the issuer's own document, the aggregator is third-party.
/// Agreement branch: the slot's LABEL takes the agent's over.
#[test]
fn an_agent_reobservation_upgrades_an_html_aggregator_slot_label() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "1000000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("aggregator write");

    record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "1000000"),
    )
    .expect("agent re-observation");

    let tier: String = connection
        .query_row(
            "SELECT source_tier FROM financial_fact_provenance",
            [],
            |row| row.get(0),
        )
        .expect("provenance row");
    assert_eq!(
        tier, "agent",
        "the agent tier must take over the aggregator slot label"
    );
}

/// (c) ADR 0093 decision 1: the agent tier OVERWRITES a divergent
/// `html_aggregator` slot (outranks it).
#[test]
fn an_agent_divergence_overwrites_an_html_aggregator_slot() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    record_structured_fact(
        &connection,
        StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: "revenue",
            value_numeric: "999000",
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
            attribution: None,
            measure_window: None,
            data_quality: None,
            statement_basis: None,
        },
    )
    .expect("aggregator write");

    record_structured_fact(
        &connection,
        agent_input(&company_id, &document_id, "1000123"),
    )
    .expect("agent divergence");

    let (value, tier): (String, String) = connection
        .query_row(
            "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
             JOIN financial_fact_provenance p ON p.fact_id = f.id",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("fact row");
    assert_eq!(
        value, "1000123",
        "the agent's number must win the aggregator's slot"
    );
    assert_eq!(tier, "agent");
}

/// (2) `outranked_stored_tier_of` explicit precedence pins for the agent
/// tier — no assumption that parse/outranks compose correctly.
#[test]
fn outranked_stored_tier_of_places_agent_between_pdf_and_html_aggregator() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    let fact_id = created_id(
        record_structured_fact(&connection, agent_input(&company_id, &document_id, "1"))
            .expect("agent seed"),
    );

    assert_eq!(
        outranked_stored_tier_of(&connection, &fact_id, "esef").expect("esef outranks"),
        Some("agent".to_owned())
    );
    assert_eq!(
        outranked_stored_tier_of(&connection, &fact_id, "structured_xhtml")
            .expect("structured_xhtml outranks"),
        Some("agent".to_owned())
    );
    assert_eq!(
        outranked_stored_tier_of(&connection, &fact_id, "espi_cover_note")
            .expect("espi_cover_note outranks"),
        Some("agent".to_owned())
    );
    assert_eq!(
        outranked_stored_tier_of(&connection, &fact_id, "pdf").expect("pdf outranks"),
        Some("agent".to_owned())
    );
    assert_eq!(
        outranked_stored_tier_of(&connection, &fact_id, "html_aggregator")
            .expect("html_aggregator does not outrank agent"),
        None
    );
}

// --- ADR 0093 decision 2: `data_quality` canonical vocabulary + the
// preliminary-data lifecycle — coexistence and write-path supersession,
// exercised through the structured path (`record_structured_fact` /
// `create_or_reobserve_financial_fact`). The plain `create_financial_fact`
// path (MCP/UI manual writes) is covered directly in
// `storage/tests/financials.rs`, since both share the same
// `create_financial_fact` stamping logic (T3). ---

fn quality_input<'a>(
    company_id: &'a str,
    document_id: &'a str,
    value: &'a str,
    data_quality: &'a str,
) -> StructuredFactInput<'a> {
    StructuredFactInput {
        company_id,
        fiscal_year: 2024,
        period_type: "FY",
        period_end: Some("2024-12-31"),
        report_document_id: document_id,
        metric_key: "revenue",
        value_numeric: value,
        currency: Some("PLN"),
        confirmation_state: "confirmed",
        source_tier: "agent",
        extraction_method: "mcp_agent",
        validation_status: "unreviewed",
        drift_json: None,
        citation: Some("XTB RB 18/2026 | Revenue"),
        attribution: None,
        measure_window: None,
        data_quality: Some(data_quality),
        statement_basis: None,
    }
}

/// `record_structured_fact` rejects an unknown `data_quality` token as a
/// typed error rather than silently minting a phantom uniqueness slot
/// (`normalize_data_quality`, `storage/financials.rs`).
#[test]
fn record_structured_fact_rejects_an_unknown_data_quality_token() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let error = record_structured_fact(
        &connection,
        quality_input(&company_id, &document_id, "1", "garbage"),
    )
    .expect_err("an unknown data_quality token must never be silently slotted");
    assert!(
        matches!(
            error,
            StorageError::InvalidFinancialsValue {
                key: "data_quality",
                ref value
            } if value == "garbage"
        ),
        "expected a typed invalid-data_quality error, got {error:?}"
    );
}

/// `record_structured_fact` (and every other caller funnelled through
/// `write_fact_provenance_fields`) must
/// REFUSE an explicit source_tier/extraction_method pair the tier's own
/// allowlist (`SourceTier::matches_extraction_method`) does not
/// recognize as coherent — a typed error at RUNTIME, not a
/// `debug_assert!` that silently vanishes in a release build (bug #324's
/// 7 real incoherent rows were never caught by anything before this).
#[test]
fn record_structured_fact_refuses_an_incoherent_tier_method_pair() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    // `esef` paired with `mcp_agent` is incoherent (the agent marker
    // pairs only with tier `agent`). Bug #324's original shape —
    // `esef` + `html_positional` — is now refused EARLIER by the ADR
    // 0095 `RetiredExtractionMethod` guard on the create path, so this
    // test exercises the coherence guard with a live incoherent pair.
    let mut input = quality_input(&company_id, &document_id, "1", "final");
    input.source_tier = "esef";
    input.extraction_method = "mcp_agent";

    let error = record_structured_fact(&connection, input)
        .expect_err("an incoherent source_tier/extraction_method pair must be refused");
    assert!(
        matches!(
            error,
            StorageError::IncoherentFactProvenance {
                ref source_tier,
                ref extraction_method,
                ..
            } if source_tier == "esef" && extraction_method == "mcp_agent"
        ),
        "expected a typed IncoherentFactProvenance error, got {error:?}"
    );

    // No provenance row is left behind by the rejected write (the
    // production write path wraps this in a transaction the caller rolls
    // back on error — `KpiExtractionStore::record_structured_fact`).
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM financial_fact_provenance",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("count"),
        0,
        "a refused write must leave no provenance row"
    );
}

/// ADR 0095 scope expansion: `source_tier='pdf'` is retired outright —
/// migration 0135 deleted every stored `pdf`-tier fact, and no NEW write
/// may ever produce it again, regardless of which extraction_method
/// accompanies it (a runtime refusal in the shared writer, not a
/// `debug_assert` that vanishes in a release build).
#[test]
fn record_structured_fact_refuses_a_retired_pdf_source_tier() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let mut input = quality_input(&company_id, &document_id, "1", "final");
    input.source_tier = "pdf";
    input.extraction_method = "api";

    let error = record_structured_fact(&connection, input)
        .expect_err("a write naming the retired pdf tier must be refused");
    assert!(
        matches!(
            error,
            StorageError::RetiredSourceTier { ref source_tier, .. } if source_tier == "pdf"
        ),
        "expected a typed RetiredSourceTier error naming the tier, got {error:?}"
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM financial_fact_provenance",
                [],
                |row| row.get::<_, i64>(0)
            )
            .expect("count"),
        0,
        "a refused write must leave no provenance row"
    );
}

/// ADR 0098 dec. 7 (#365): `structured_xhtml` joined `pdf` as a legacy
/// read-only tier. Driven through the PUBLIC store wrapper (its
/// transaction is what rolls the whole write back), asserting BOTH the
/// fact row and the provenance row are gone — the private fn creates the
/// fact before provenance, so a provenance-only assertion would pass even
/// if the fact leaked.
#[test]
fn record_structured_fact_refuses_a_retired_structured_xhtml_source_tier() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);
    let state = crate::storage::AppState::new(connection);

    let mut input = quality_input(&company_id, &document_id, "1", "final");
    input.source_tier = "structured_xhtml";
    input.extraction_method = "api";

    let error = state
        .kpi_extraction()
        .record_structured_fact(input)
        .expect_err("a write naming the retired structured_xhtml tier must be refused");
    assert!(
        matches!(
            error,
            StorageError::RetiredSourceTier { ref source_tier, .. }
                if source_tier == "structured_xhtml"
        ),
        "expected a typed RetiredSourceTier error naming the tier, got {error:?}"
    );
    let raw = state.checkout_for_tests().expect("raw");
    for (table, label) in [
        ("financial_facts", "fact"),
        ("financial_fact_provenance", "provenance"),
    ] {
        assert_eq!(
            raw.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                .get::<_, i64>(0))
                .expect("count"),
            0,
            "a refused write must leave no {label} row"
        );
    }
}

/// A `preliminary` fact and a `final` fact for the same metric/period are
/// two DIFFERENT rows in the uniqueness slot (`data_quality` is a slot
/// dimension, 0034) — never a UNIQUE violation, never a silent overwrite.
#[test]
fn preliminary_and_final_coexist_in_the_same_slot() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let preliminary_id = created_id(
        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "492200000", "preliminary"),
        )
        .expect("preliminary write"),
    );
    let final_id = created_id(
        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "495000000", "final"),
        )
        .expect("final write"),
    );

    assert_ne!(preliminary_id, final_id, "distinct rows, same slot");
    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
        .expect("count");
    assert_eq!(count, 2, "both quality variants persist");
}

/// A `final` fact created into a slot whose sibling is `preliminary` stamps
/// `supersedes_id` at it (ADR 0093 decision 2), via the structured path.
#[test]
fn final_created_next_to_preliminary_stamps_supersedes_id_via_structured_path() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let preliminary_id = created_id(
        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "492200000", "preliminary"),
        )
        .expect("preliminary write"),
    );
    let final_id = created_id(
        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "495000000", "final"),
        )
        .expect("final write"),
    );

    let supersedes_id: Option<String> = connection
        .query_row(
            "SELECT supersedes_id FROM financial_facts WHERE id = ?1",
            [&final_id],
            |row| row.get(0),
        )
        .expect("final row");
    assert_eq!(supersedes_id, Some(preliminary_id));
}

/// When BOTH a `preliminary` and an `estimated` sibling occupy the slot, a
/// later `final` fact supersedes the `preliminary` one — the issuer's own
/// preliminary release outranks a third-party estimate (ADR 0093 decision 2).
#[test]
fn final_prefers_the_preliminary_sibling_over_an_estimated_one() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    record_structured_fact(
        &connection,
        quality_input(&company_id, &document_id, "480000000", "estimated"),
    )
    .expect("estimated write");
    let preliminary_id = created_id(
        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "492200000", "preliminary"),
        )
        .expect("preliminary write"),
    );
    let final_id = created_id(
        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "495000000", "final"),
        )
        .expect("final write"),
    );

    let supersedes_id: Option<String> = connection
        .query_row(
            "SELECT supersedes_id FROM financial_facts WHERE id = ?1",
            [&final_id],
            |row| row.get(0),
        )
        .expect("final row");
    assert_eq!(
        supersedes_id,
        Some(preliminary_id),
        "the issuer-published preliminary must win over a third-party estimate"
    );
}

// -----------------------------------------------------------------
// resolve_definition_by_metric_key (#361): sector-aware deterministic
// precedence, shared with the manifest validator's resolver.
// -----------------------------------------------------------------

fn insert_definition(
    connection: &Connection,
    id: &str,
    scope: &str,
    company_id: Option<&str>,
    sector: Option<&str>,
    metric_key: &str,
) {
    connection
        .execute(
            "INSERT INTO kpi_definitions (id, scope, company_id, sector, metric_key, label, value_kind)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5, 'monetary')",
            params![id, scope, company_id, sector, metric_key],
        )
        .expect("kpi_definitions insert");
}

#[test]
fn resolver_prefers_company_scoped_over_canonical_and_sector() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
        .expect("set sector");
    insert_definition(
        &connection,
        "kpidef_revenue__s_banking",
        "sector",
        None,
        Some("banking"),
        "revenue",
    );
    insert_definition(
        &connection,
        "kpidef_revenue__c_c1",
        "company",
        Some(&company_id),
        None,
        "revenue",
    );

    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
    assert_eq!(resolved.as_deref(), Some("kpidef_revenue__c_c1"));
}

#[test]
fn resolver_prefers_canonical_over_matching_sector() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
        .expect("set sector");
    insert_definition(
        &connection,
        "kpidef_revenue__s_banking",
        "sector",
        None,
        Some("banking"),
        "revenue",
    );

    // "revenue" is already seeded canonically by migration 0034.
    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
    assert_eq!(resolved.as_deref(), Some("kpidef_revenue"));
}

#[test]
fn resolver_picks_matching_sector_and_excludes_non_matching_sector() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
        .expect("set sector");
    // A company-specific metric key with NO canonical twin, so only the
    // sector rows compete.
    insert_definition(
        &connection,
        "kpidef_nim__s_banking",
        "sector",
        None,
        Some("banking"),
        "net_interest_margin",
    );
    insert_definition(
        &connection,
        "kpidef_nim__s_industrial",
        "sector",
        None,
        Some("industrial"),
        "net_interest_margin",
    );

    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "net_interest_margin")
            .expect("resolve");
    assert_eq!(
        resolved.as_deref(),
        Some("kpidef_nim__s_banking"),
        "the industrial sector row must be excluded, not merely deprioritized"
    );
}

#[test]
fn resolver_company_without_sector_never_gets_a_sector_scoped_definition() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    // No sector set on the company at all.
    insert_definition(
        &connection,
        "kpidef_nim__s_banking",
        "sector",
        None,
        Some("banking"),
        "net_interest_margin",
    );

    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "net_interest_margin")
            .expect("resolve");
    assert_eq!(resolved, None);
}

/// Dual-axis regression (a): a runtime definition scoped to a RAW
/// directory sector ("Gry") keeps resolving exactly as before the
/// statement-type axis existed.
#[test]
fn resolver_keeps_matching_a_raw_directory_sector_definition() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    super::super::companies::set_company_sector(&connection, &company_id, Some("Gry"))
        .expect("set sector");
    insert_definition(
        &connection,
        "kpidef_arpu__s_gry",
        "sector",
        None,
        Some("Gry"),
        "arpu",
    );

    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "arpu").expect("resolve");
    assert_eq!(resolved.as_deref(), Some("kpidef_arpu__s_gry"));
}

/// Dual-axis regression (b): the `'industrial'` classification default
/// never opens the statement axis — a company with no raw sector still
/// matches nothing, even against a runtime `sector='industrial'` row.
#[test]
fn resolver_never_matches_an_industrial_statement_type_definition() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    insert_definition(
        &connection,
        "kpidef_widget__s_industrial",
        "sector",
        None,
        Some("industrial"),
        "widget_output",
    );

    let resolved = resolve_definition_by_metric_key(&connection, &company_id, "widget_output")
        .expect("resolve");
    assert_eq!(resolved, None);
}

/// Dual-axis (c) — red before the fix: the seeded statement packs use the
/// `statement_type` vocabulary, so a classified issuer resolves its pack
/// even when its raw directory sector says something else (or nothing).
#[test]
fn resolver_matches_the_statement_type_axis_for_a_classified_issuer() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    connection
        .execute(
            "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
            [&company_id],
        )
        .expect("classify");

    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "net_interest_income")
            .expect("resolve");
    assert_eq!(resolved.as_deref(), Some("kpidef_bank_net_interest_income"));
}

/// Dual-axis collision (d): when a raw-sector definition and a
/// statement-type definition both carry the same metric key, the raw
/// match wins by RANK — even when its id sorts lexicographically last.
#[test]
fn a_raw_sector_definition_outranks_the_statement_type_axis() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    super::super::companies::set_company_sector(&connection, &company_id, Some("banki komercyjne"))
        .expect("set sector");
    connection
        .execute(
            "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
            [&company_id],
        )
        .expect("classify");
    // Sorts AFTER the seeded kpidef_bank_net_interest_income — only rank
    // can make it win.
    insert_definition(
        &connection,
        "kpidef_zz_nii__s_raw",
        "sector",
        None,
        Some("banki komercyjne"),
        "net_interest_income",
    );

    let resolved =
        resolve_definition_by_metric_key(&connection, &company_id, "net_interest_income")
            .expect("resolve");
    assert_eq!(resolved.as_deref(), Some("kpidef_zz_nii__s_raw"));
}

/// Dual-axis (e): the SQL predicate and the Rust twin
/// (`sector_definition_matches`, pinned-commit eligibility) agree on the
/// same truth table — mirror fidelity is asserted, not assumed.
#[test]
fn sector_eligibility_truth_table_agrees_between_sql_and_rust() {
    // (definition sector, raw company sector, statement_type, eligible)
    let cases: [(&str, Option<&str>, &str, bool); 5] = [
        ("Gry", Some("Gry"), "industrial", true),
        ("industrial", None, "industrial", false),
        ("banking", None, "banking", true),
        ("banking", Some("banki komercyjne"), "banking", true),
        ("banking", Some("Gry"), "industrial", false),
    ];
    for (definition_sector, raw_sector, statement_type, eligible) in cases {
        assert_eq!(
            sector_definition_matches(Some(definition_sector), raw_sector, statement_type),
            eligible,
            "rust: def={definition_sector} raw={raw_sector:?} statement={statement_type}"
        );

        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        if let Some(raw) = raw_sector {
            super::super::companies::set_company_sector(&connection, &company_id, Some(raw))
                .expect("set sector");
        }
        connection
            .execute(
                "UPDATE companies SET statement_type = ?1 WHERE id = ?2",
                params![statement_type, company_id],
            )
            .expect("classify");
        insert_definition(
            &connection,
            "kpidef_truth_case",
            "sector",
            None,
            Some(definition_sector),
            "truth_case_metric",
        );
        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "truth_case_metric")
                .expect("resolve");
        assert_eq!(
            resolved.is_some(),
            eligible,
            "sql: def={definition_sector} raw={raw_sector:?} statement={statement_type}"
        );
    }
    assert!(!sector_definition_matches(None, Some("Gry"), "banking"));
}

/// Class guardrail (ADR 0099 dec. 6 / #383 sol R1): every key of every
/// extraction-profile pack RESOLVES to a definition for a company of that
/// statement type — a pack can never demand an unresolvable key again.
#[test]
fn every_profile_pack_key_resolves_for_its_statement_type() {
    use crate::storage::kpi_ingest_profiles::{expected_pack, PROFILE_VERSIONS};
    for statement_type in [
        "industrial",
        "banking",
        "insurance",
        "specialty_finance",
        "brokerage",
        "reit",
    ] {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        connection
            .execute(
                "UPDATE companies SET statement_type = ?1 WHERE id = ?2",
                params![statement_type, company_id],
            )
            .expect("classify");
        for profile in PROFILE_VERSIONS {
            for key in expected_pack(profile, statement_type).expect("registered profile") {
                let resolved = resolve_definition_by_metric_key(&connection, &company_id, key)
                    .expect("resolve");
                assert!(
                    resolved.is_some(),
                    "{profile} × {statement_type}: pack key {key} does not resolve"
                );
            }
        }
    }
}

#[test]
fn resolver_falls_back_to_remaining_global_definitions_lexicographically() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    // Neither row is canonical/sector/company -- both are "remaining
    // global", ordered lexicographically by id. Distinct `scope` values
    // (rather than two `user` rows) keep the unique index
    // `(metric_key, scope, company_id, sector)` happy -- the resolver's
    // catch-all bucket cares only that `scope NOT IN ('company', 'sector')`.
    insert_definition(
        &connection,
        "kpidef_custom_b",
        "user",
        None,
        None,
        "custom_metric",
    );
    insert_definition(
        &connection,
        "kpidef_custom_a",
        "legacy",
        None,
        None,
        "custom_metric",
    );

    let resolved = resolve_definition_by_metric_key(&connection, &company_id, "custom_metric")
        .expect("resolve");
    assert_eq!(resolved.as_deref(), Some("kpidef_custom_a"));
}

#[test]
fn resolver_catch_all_excludes_company_bound_rows_for_a_different_company() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    connection
        .execute(
            "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
             VALUES ('c2', 'gpw', 'XYZ', 'GPW:XYZ', 'XYZ SA')",
            [],
        )
        .expect("company b");
    // A 'user' scoped definition bound to company A only -- not
    // 'company'/'sector' scope, so it would fall into the catch-all
    // bucket if that bucket did not also require company_id IS NULL.
    insert_definition(
        &connection,
        "kpidef_x__u_c1",
        "user",
        Some(&company_id),
        None,
        "custom_x",
    );

    let resolved =
        resolve_definition_by_metric_key(&connection, "c2", "custom_x").expect("resolve");
    assert_eq!(
        resolved, None,
        "company A's user-scoped definition must not leak to company B via the catch-all"
    );
}

#[test]
fn resolver_is_deterministic_across_repeated_calls() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, _doc) = seed_company_and_document(&connection);
    super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
        .expect("set sector");
    insert_definition(
        &connection,
        "kpidef_revenue__s_banking",
        "sector",
        None,
        Some("banking"),
        "revenue",
    );
    insert_definition(
        &connection,
        "kpidef_revenue__c_c1",
        "company",
        Some(&company_id),
        None,
        "revenue",
    );

    let first =
        resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
    let second =
        resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
    assert_eq!(first, second);
}

// -------------------------------------------------------------------
// ADR 0100 decision 6 (2nd paragraph), epic #398: the slot-write
// `measure_window` default derives from the resolved definition's
// `period_nature` instead of hard-defaulting to 'flow'. Exercised
// through the REAL structured/aggregator writer paths, not the
// `slot_dims` helper directly.
// -------------------------------------------------------------------

fn stored_measure_window(connection: &Connection, fact_id: &str) -> String {
    connection
        .query_row(
            "SELECT measure_window FROM financial_facts WHERE id = ?1",
            [fact_id],
            |row| row.get(0),
        )
        .expect("fact should exist")
}

fn balance_input<'a>(
    company_id: &'a str,
    document_id: &'a str,
    value: &'a str,
    measure_window: Option<&'a str>,
) -> StructuredFactInput<'a> {
    StructuredFactInput {
        company_id,
        fiscal_year: 2024,
        period_type: "FY",
        period_end: Some("2024-12-31"),
        report_document_id: document_id,
        // `total_assets` is `instant`-natured (migration 0141 backfill).
        metric_key: "total_assets",
        value_numeric: value,
        currency: Some("PLN"),
        confirmation_state: "confirmed",
        source_tier: "esef",
        extraction_method: "api",
        validation_status: "passed",
        drift_json: None,
        citation: Some("ESEF | Assets"),
        attribution: None,
        measure_window,
        data_quality: None,
        statement_basis: None,
    }
}

#[test]
fn none_measure_window_defaults_to_point_in_time_for_an_instant_metric() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let commit = record_structured_fact(
        &connection,
        balance_input(&company_id, &document_id, "1000", None),
    )
    .expect("structured write should succeed");
    let fact_id = created_id(commit);
    assert_eq!(
        stored_measure_window(&connection, &fact_id),
        "point_in_time"
    );
}

#[test]
fn none_measure_window_still_defaults_to_flow_for_a_duration_metric() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    // `revenue` is `duration`-natured (the catalog default).
    let commit =
        record_structured_fact(&connection, issuer_input(&company_id, &document_id, "1000"))
            .expect("structured write should succeed");
    let fact_id = created_id(commit);
    assert_eq!(stored_measure_window(&connection, &fact_id), "flow");
}

#[test]
fn an_explicit_measure_window_contradicting_period_nature_is_a_typed_refusal() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    // `flow` on the `instant`-natured `total_assets` is a genuine class
    // contradiction.
    let error = record_structured_fact(
        &connection,
        balance_input(&company_id, &document_id, "1000", Some("flow")),
    )
    .expect_err("a flow window on an instant metric must be refused");
    assert!(
        matches!(
            error,
            StorageError::MeasureWindowPeriodNatureMismatch {
                ref measure_window,
                ref period_nature,
                ..
            } if measure_window == "flow" && period_nature == "instant"
        ),
        "expected a typed measure_window/period_nature mismatch, got {error:?}"
    );

    // The converse direction: `point_in_time` on the `duration`-natured
    // `revenue` is refused too.
    let mut duration_bad = issuer_input(&company_id, &document_id, "1000");
    duration_bad.measure_window = Some("point_in_time");
    let error = record_structured_fact(&connection, duration_bad)
        .expect_err("point_in_time on a duration metric must be refused");
    assert!(
        matches!(
            error,
            StorageError::MeasureWindowPeriodNatureMismatch {
                ref measure_window,
                ref period_nature,
                ..
            } if measure_window == "point_in_time" && period_nature == "duration"
        ),
        "expected a typed measure_window/period_nature mismatch, got {error:?}"
    );
}

#[test]
fn cumulative_and_trailing_over_a_duration_metric_are_accepted() {
    let connection = open_in_memory_database().expect("db");
    let (company_id, document_id) = seed_company_and_document(&connection);

    let mut cumulative = issuer_input(&company_id, &document_id, "1000");
    cumulative.measure_window = Some("cumulative");
    let commit =
        record_structured_fact(&connection, cumulative).expect("cumulative must be accepted");
    assert_eq!(
        stored_measure_window(&connection, &created_id(commit)),
        "cumulative"
    );

    // A distinct period so the second write lands in a fresh slot rather
    // than re-observing the first.
    let mut trailing = issuer_input(&company_id, &document_id, "1000");
    trailing.fiscal_year = 2023;
    trailing.period_end = Some("2023-12-31");
    trailing.measure_window = Some("trailing");
    let commit = record_structured_fact(&connection, trailing).expect("trailing must be accepted");
    assert_eq!(
        stored_measure_window(&connection, &created_id(commit)),
        "trailing"
    );
}
