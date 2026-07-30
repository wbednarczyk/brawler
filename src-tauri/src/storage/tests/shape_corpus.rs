//! The **synthetic shape corpus** (epic #40 S6; ADR 0091 decision 4) — the
//! public half of the honesty layer.
//!
//! The real database can never reach public CI, so CI gets a corpus of seed data
//! that reproduces every data-state SHAPE the real database exhibits, and the S4
//! honesty invariants run over it **through the same read models and the same
//! measurement code** ([`super::real_data_honesty::measure_attention`] /
//! [`super::real_data_honesty::measure_effects`]). No real data, no secrets, no
//! ratchet — hard asserts on default runners.
//!
//! # Two gates, one corpus
//!
//! | gate | asks |
//! |---|---|
//! | [`synthetic_corpus_covers_every_real_data_shape`] | does the corpus REACH every shape the real DB has? |
//! | [`synthetic_corpus_holds_the_honesty_invariants`] | is the app honest ON that corpus? |
//!
//! The coverage gate is **observational, not declarative**: it seeds the corpus,
//! re-runs the very scan that produced the inventory
//! ([`super::real_data_shape_inventory::scan_shapes`]), and compares shape SETS.
//! A case cannot claim a coverage it does not produce, and the report names each
//! uncovered key — the no-silent-caps rule. An inventory shape the corpus cannot
//! reach must be listed, with a reason, in `shape-inventory-missing.json`; an
//! entry that the corpus *does* reach reddens too, so an excuse cannot outlive
//! the gap it excused.
//!
//! # Named cases
//!
//! [`CASES`] is the factory: an ordered list of `(name, seed fn)`. Each case
//! seeds one coherent situation through the real write paths, dropping to raw
//! SQL only for shapes no current write path can produce (a legacy row, a
//! classifier outcome with no runtime setter) — the same idiom the migration
//! tests use. The gate scans after each case, so the report can attribute every
//! shape to the case that first realized it.
//!
//! Growing the corpus: add a case, run the gate, read the attribution table.

use std::collections::{BTreeMap, BTreeSet};

use super::real_data_honesty::{measure_attention, measure_effects};
use super::real_data_shape_inventory::{scan_shapes, INVENTORY_FILE_NAME, SHAPE_DOMAINS};
use super::*;
use crate::storage::attention::{
    insert_attention_event, insert_system_attention_event, EVIDENCE_AUTOPILOT_RUN,
    EVIDENCE_COMPANY_SIGNAL, EVIDENCE_SOURCE_RECONCILIATION,
};

/// The committed inventory of shapes the maintainer's real database exhibits —
/// the contract this corpus must cover. Reached through the build.rs-resolved
/// env, never a literal `../` path (`source_tree_guards`, #110).
const INVENTORY_JSON: &str = include_str!(concat!(
    env!("BRAWLER_SCENARIOS_DIR"),
    "/shape-inventory.json"
));

/// Shapes the corpus deliberately does not reach, each with its reason. The
/// gate treats an entry as an accepted gap — and reddens when the corpus reaches
/// it anyway, so a stale excuse cannot hide.
const MISSING_JSON: &str = include_str!(concat!(
    env!("BRAWLER_SCENARIOS_DIR"),
    "/shape-inventory-missing.json"
));

// ---------------------------------------------------------------------------
// Reading the committed contract
// ---------------------------------------------------------------------------

/// One inventory entry: the anonymized descriptor, as committed.
struct InventoryShape {
    key: String,
    domain: String,
}

fn inventory() -> Vec<InventoryShape> {
    let parsed: serde_json::Value =
        serde_json::from_str(INVENTORY_JSON).expect("shape-inventory.json parses");
    parsed["shapes"]
        .as_array()
        .expect("shape-inventory.json has a `shapes` array")
        .iter()
        .map(|entry| InventoryShape {
            key: entry["key"].as_str().expect("shape key").to_owned(),
            domain: entry["domain"].as_str().expect("shape domain").to_owned(),
        })
        .collect()
}

fn missing_list() -> BTreeMap<String, String> {
    let parsed: serde_json::Value =
        serde_json::from_str(MISSING_JSON).expect("shape-inventory-missing.json parses");
    parsed["missing"]
        .as_object()
        .expect("shape-inventory-missing.json has a `missing` object")
        .iter()
        .map(|(key, reason)| {
            (
                key.clone(),
                reason.as_str().expect("a reason string").to_owned(),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Seeding helpers — the corpus's own vocabulary
// ---------------------------------------------------------------------------

/// Deterministic, obviously-synthetic issuers. Nothing here is modelled on a
/// real company: the corpus proves SHAPES, and a plausible-looking issuer would
/// only invite someone to read it as real data.
fn seed_company(state: &AppState, ticker: &str, name: &str) -> Company {
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            display_name: name.to_owned(),
            isin: Some(format!("PLTEST0{:05}", ticker.len() * 1111)),
            cik: None,
            lei: None,
        })
        .expect("company should be created")
}

/// The FY period + confirmed facts a health score reads. `metrics` are
/// `(metric_key, value)` pairs from the canonical catalog.
fn seed_fy(state: &AppState, company_id: &str, fiscal_year: i64, metrics: &[(&str, &str)]) {
    let period = state
        .create_financial_period(NewFinancialPeriod {
            company_id: company_id.to_owned(),
            fiscal_year,
            period_type: "FY".to_owned(),
            period_end_date: Some(format!("{fiscal_year}-12-31")),
            report_evidence_ref: None,
        })
        .expect("financial period should create");
    let definitions = state
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: Some("canonical".to_owned()),
            sector: None,
            company_id: None,
        })
        .expect("canonical definitions should list");
    for (metric_key, value) in metrics {
        let definition = definitions
            .iter()
            .find(|d| &d.metric_key == metric_key)
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
                annotation: None,
                source_document_ref: None,
            })
            .expect("financial fact should create");
    }
}

/// The full Altman Z″ input set for one FY (X1..X4 numerators + denominators).
fn altman_inputs(scale: i64) -> Vec<(&'static str, String)> {
    vec![
        ("working_capital", (30_000 * scale).to_string()),
        ("retained_earnings", (40_000 * scale).to_string()),
        ("operating_profit", (12_000 * scale).to_string()),
        ("total_equity", (90_000 * scale).to_string()),
        ("total_assets", (200_000 * scale).to_string()),
        ("total_liabilities", (110_000 * scale).to_string()),
    ]
}

fn seed_report_document(state: &AppState, id: &str, company_id: &str, title: &str) {
    let connection = state.checkout().expect("database connection");
    connection
        .execute(
            "INSERT INTO report_documents (id, company_id, source_type, url, title, fetch_status)
             VALUES (?1, ?2, 'espi_attachment', ?3, ?4, 'metadata_only')",
            params![
                id,
                company_id,
                format!("https://example.test/{id}.xhtml"),
                title
            ],
        )
        .expect("seed a report document");
}

/// A company-scoped alert rule, so rule-backed attention events have an owner.
///
/// One rule per event on purpose: [`insert_attention_event`] applies the
/// per-rule DAILY THROTTLE (`DAILY_THROTTLE_PER_RULE`), so several same-day
/// events sharing a rule silently collapse into one — which is exactly how the
/// first run of the coverage gate lost the `seen`/`dismissed` lifecycle shapes.
fn seed_signal_rule(state: &AppState, company_id: &str, category: &str) -> String {
    state
        .attention()
        .create_alert_rule(NewAlertRule {
            trigger_type: crate::storage::attention::TRIGGER_SIGNAL_CATEGORY.to_owned(),
            signal_category: Some(category.to_owned()),
            price_min: None,
            price_max: None,
            scope_type: "company".to_owned(),
            scope_ref: company_id.to_owned(),
        })
        .expect("alert rule should create")
        .id
}

fn now_rfc3339() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .expect("format now")
}

// ---------------------------------------------------------------------------
// The named cases
// ---------------------------------------------------------------------------

/// A scored industrial issuer with the complete Altman input set on two of three
/// FY periods, captured documents and an accepted extraction.
///
/// Shapes: `health-altman-headline`, `health-history-multi-period`,
/// `health-latest-period-present`, `health-statement-type-industrial`,
/// `health-piotroski-insufficient-named`, `company-with-*`.
fn case_scored_industrial_issuer(state: &AppState) {
    let company = seed_company(state, "SYNA", "Synthetic Alpha Works S.A.");
    for (year, scale) in [(2024, 3), (2023, 2)] {
        let metrics = altman_inputs(scale);
        let borrowed: Vec<(&str, &str)> = metrics
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect();
        seed_fy(state, &company.id, year, &borrowed);
    }
    // The oldest FY carries only a subset: Altman still scores a headline is not
    // possible here, so it lands on the NAMED-missing branch — the honest state.
    seed_fy(
        state,
        &company.id,
        2022,
        &[("total_assets", "100000"), ("revenue", "80000")],
    );

    seed_report_document(
        state,
        "doc_syna_fy2024",
        &company.id,
        "Skonsolidowany raport roczny 2024",
    );
    state
        .fundamentals_provenance()
        .record_extraction_outcome(NewExtractionOutcome {
            company_id: &company.id,
            report_document_id: "doc_syna_fy2024",
            fiscal_year: 2024,
            period_type: "FY",
            period_end: "2024-12-31",
            tier: Some("esef"),
            acceptance: "accepted",
            reason_code: "emitted",
            detail_json: Some(r#"{"checks":[]}"#),
            drift_json: None,
            structure_changed: false,
            fact_count: 6,
        })
        .expect("record an accepted outcome");
}

/// A second industrial issuer whose facts cover only part of the Altman set —
/// the read model must NAME what is missing rather than fall silent.
///
/// Shapes: `health-altman-insufficient-named`, `company-without-report-documents`,
/// `company-without-extraction-outcomes`, `company-without-attention-events`.
fn case_partially_covered_industrial_issuer(state: &AppState) {
    let company = seed_company(state, "SYNB", "Synthetic Beta Industrials S.A.");
    seed_fy(
        state,
        &company.id,
        2024,
        &[
            ("total_assets", "500000"),
            ("total_liabilities", "300000"),
            ("revenue", "410000"),
        ],
    );
    seed_fy(
        state,
        &company.id,
        2023,
        &[("total_assets", "480000"), ("revenue", "390000")],
    );
}

/// Issuers whose statement type puts both scores out of scope. The reason a
/// score does not apply is the statement type itself — never a blank.
///
/// `statement_type` has no runtime setter: it is a classifier outcome the
/// sector-derivation migrations (`0095`, `0098`) write. Seeding it directly is
/// the migration-test idiom, not a shortcut around a write path.
///
/// Shapes: `health-statement-type-{banking,insurance,specialty-finance}`,
/// `health-piotroski-not-applicable-reasoned`,
/// `health-altman-not-applicable-reasoned`.
fn case_non_industrial_statement_types(state: &AppState) {
    for (ticker, name, statement_type) in [
        ("SYNC", "Synthetic Gamma Bank S.A.", "banking"),
        ("SYND", "Synthetic Delta Insurance S.A.", "insurance"),
        (
            "SYNE",
            "Synthetic Epsilon Finance S.A.",
            "specialty_finance",
        ),
    ] {
        let company = seed_company(state, ticker, name);
        {
            let connection = state.checkout().expect("database connection");
            connection
                .execute(
                    "UPDATE companies SET statement_type = ?1 WHERE id = ?2",
                    params![statement_type, company.id],
                )
                .expect("seed the classifier's statement type");
        }
        seed_fy(
            state,
            &company.id,
            2024,
            &[("total_assets", "900000"), ("revenue", "120000")],
        );
        seed_fy(
            state,
            &company.id,
            2023,
            &[("total_assets", "850000"), ("revenue", "110000")],
        );
    }
}

/// Rule-backed stream rows in every lifecycle state, each stating something
/// concrete.
///
/// Shapes: `attention-trigger-signal-category`,
/// `attention-evidence-company-signal`, `attention-from-user-rule`,
/// `attention-scope-company`, `attention-statement-concrete`,
/// `attention-title-plain-prose`, `attention-unseen`,
/// `attention-seen-not-dismissed`, `attention-dismissed`,
/// `attention-severity-notable`, `company-with-attention-events`.
fn case_rule_backed_stream_rows(state: &AppState) {
    let company = seed_company(state, "SYNF", "Synthetic Zeta Holdings S.A.");
    seed_fy(
        state,
        &company.id,
        2024,
        &[("total_assets", "300000"), ("revenue", "250000")],
    );
    for (category, evidence_ref, title) in [
        (
            "dividend",
            "sig_unseen",
            "Rekomendacja zarządu w sprawie wypłaty dywidendy",
        ),
        (
            "significant_contract",
            "sig_seen",
            "Zawarcie znaczącej umowy z odbiorcą przemysłowym",
        ),
        (
            "report_delay",
            "sig_dismissed",
            "Zmiana terminu publikacji raportu okresowego",
        ),
    ] {
        let rule_id = seed_signal_rule(state, &company.id, category);
        let connection = state.checkout().expect("database connection");
        let created = insert_attention_event(
            &connection,
            &rule_id,
            crate::storage::attention::TRIGGER_SIGNAL_CATEGORY,
            &company.id,
            EVIDENCE_COMPANY_SIGNAL,
            evidence_ref,
            Some(title),
        )
        .expect("insert a rule-backed event");
        assert!(created, "{evidence_ref} was suppressed by dedup/throttle");
    }

    // The lifecycle states, through the real commands.
    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput {
            company_id: Some(company.id.clone()),
            include_dismissed: true,
        })
        .expect("list events");
    for event in &events {
        if event.evidence_ref.ends_with("_seen") {
            state
                .attention()
                .mark_attention_event_seen(&event.id)
                .expect("mark seen");
        }
        if event.evidence_ref.ends_with("_dismissed") {
            state
                .attention()
                .dismiss_attention_event(&event.id)
                .expect("dismiss");
        }
    }
}

/// System-raised rows: a missed-report reconciliation (urgent, with a secondary
/// datum) and a completed autopilot run.
///
/// Shapes: `attention-system-raised`,
/// `attention-trigger-source-reconciliation`,
/// `attention-evidence-source-reconciliation`, `attention-severity-urgent`,
/// `attention-trigger-autopilot-run-completed`,
/// `attention-evidence-autopilot-run`, `attention-evidence-detail-present`,
/// `attention-evidence-resolved`.
fn case_system_raised_rows(state: &AppState) {
    let company = seed_company(state, "SYNG", "Synthetic Eta Materials S.A.");
    seed_fy(
        state,
        &company.id,
        2024,
        &[("total_assets", "220000"), ("revenue", "180000")],
    );
    seed_report_document(
        state,
        "doc_syng_fy2024",
        &company.id,
        "Skonsolidowany raport roczny 2024 grupy",
    );
    let connection = state.checkout().expect("database connection");

    // A reconciliation result + its system event: the read model resolves the
    // missed report's witness title and the source that missed it.
    connection
        .execute(
            "INSERT INTO source_reconciliation_results
                (id, witness_adapter_id, company_id, report_number, report_type, disclosure_date,
                 witness_title, status)
             VALUES ('rec_syng', 'bankier-company-komunikaty', ?1, '12/2026', 'ESPI',
                     '2026-06-01', 'Raport bieżący o zawarciu umowy inwestycyjnej', 'espi_only')",
            params![company.id],
        )
        .expect("seed a reconciliation result");
    insert_system_attention_event(
        &connection,
        crate::storage::attention::TRIGGER_SOURCE_RECONCILIATION,
        Some(&company.id),
        EVIDENCE_SOURCE_RECONCILIATION,
        "rec_syng",
        Some("Raport bieżący o zawarciu umowy inwestycyjnej"),
        &now_rfc3339(),
    )
    .expect("insert a reconciliation event");

    // An autopilot run + its completion event: the run's raw status is the
    // secondary datum, the processed document's title the statement.
    connection
        .execute(
            "INSERT INTO autopilot_run (id, company_id, report_document_id, mode, status, stage)
             VALUES ('run_syng', ?1, 'doc_syng_fy2024', 'autopilot', 'succeeded', 'notify')",
            params![company.id],
        )
        .expect("seed an autopilot run");
    insert_system_attention_event(
        &connection,
        crate::storage::attention::TRIGGER_AUTOPILOT_RUN_COMPLETED,
        Some(&company.id),
        EVIDENCE_AUTOPILOT_RUN,
        "run_syng",
        Some("Skonsolidowany raport roczny 2024 grupy"),
        &now_rfc3339(),
    )
    .expect("insert an autopilot completion event");
}

/// The two title shapes the rendering rule exists for: a filename glued in front
/// of the human title (the statement is the human part), and a legacy row whose
/// evidence has been pruned and whose fire-time snapshot predates the snapshot
/// column (the row falls back to generic copy — honestly).
///
/// A pruned-evidence row cannot be produced by any current write path (the
/// writer always snapshots), so it is seeded at SQL level exactly as the
/// migration tests seed pre-migration shapes.
///
/// Shapes: `attention-title-filename-prefixed`, `attention-title-absent`,
/// `attention-statement-generic-fallback`, `attention-evidence-orphaned`,
/// `attention-evidence-detail-absent`.
fn case_degraded_evidence_titles(state: &AppState) {
    let company = seed_company(state, "SYNH", "Synthetic Theta Logistics S.A.");
    seed_fy(
        state,
        &company.id,
        2024,
        &[("total_assets", "140000"), ("revenue", "120000")],
    );
    let rule_id = seed_signal_rule(state, &company.id, "dividend");
    let connection = state.checkout().expect("database connection");

    // Glued filename + human title — what an ESEF attachment label looks like.
    insert_attention_event(
        &connection,
        &rule_id,
        crate::storage::attention::TRIGGER_SIGNAL_CATEGORY,
        &company.id,
        EVIDENCE_COMPANY_SIGNAL,
        "sig_glued",
        Some("Y24_25_Sprawozdanie.xhtmlJednostkowe Sprawozdanie Finansowe"),
    )
    .expect("insert a glued-title event");

    // The legacy shape: no snapshot, and the joined evidence row is gone.
    connection
        .execute(
            "INSERT INTO attention_events
                (id, rule_id, trigger_type, company_id, evidence_type, evidence_ref, fired_at,
                 evidence_title)
             VALUES ('attn_syn_orphan', ?1, 'signal_category', ?2, 'company_signal',
                     'sig_pruned_by_retention', ?3, NULL)",
            params![rule_id, company.id, now_rfc3339()],
        )
        .expect("seed a pruned-evidence legacy row");
}

/// The extraction-outcome vocabulary as the real database exhibits it: every
/// acceptance, every typed reason, every tier — and, on the two production-
/// claiming reasons, a fact count that evidences the claim.
///
/// Shapes: `extraction-acceptance-*`, `extraction-reason-*`, `extraction-tier-*`,
/// `extraction-facts-recorded`, `extraction-facts-zero-not-accepted`,
/// `extraction-first-attempt`, `extraction-retried`,
/// `extraction-structure-changed`, `extraction-drift-report-present`,
/// `extraction-detail-present`, `company-with-extraction-outcomes`.
fn case_extraction_outcome_vocabulary(state: &AppState) {
    let company = seed_company(state, "SYNI", "Synthetic Iota Chemicals S.A.");
    seed_fy(
        state,
        &company.id,
        2024,
        &[("total_assets", "700000"), ("revenue", "600000")],
    );

    // (document, year, tier, acceptance, reason, facts, structure_changed, drift, detail)
    type Row = (
        &'static str,
        i64,
        Option<&'static str>,
        &'static str,
        &'static str,
        i64,
        bool,
        Option<&'static str>,
        Option<&'static str>,
    );
    let rows: [Row; 6] = [
        // A parsed filing that produced facts — the honest emission.
        (
            "doc_syni_a",
            2024,
            Some("pdf"),
            "accepted",
            "emitted",
            9,
            false,
            None,
            Some(r#"{"checks":[]}"#),
        ),
        // Values sourced from the aggregator because no tier read them.
        (
            "doc_syni_b",
            2023,
            Some("html_aggregator"),
            "accepted_via_witness",
            "witness_fallback",
            4,
            false,
            None,
            Some(r#"{"witness":"aggregator"}"#),
        ),
        // Accepted without review — still evidenced by a fact count.
        (
            "doc_syni_c",
            2022,
            Some("esef"),
            "accepted_unreviewed",
            "emitted",
            7,
            false,
            None,
            None,
        ),
        // Nothing could read the document at all.
        (
            "doc_syni_d",
            2021,
            None,
            "empty",
            "no_deterministic_tier",
            0,
            false,
            None,
            None,
        ),
        // Read, but the identity checks failed.
        (
            "doc_syni_e",
            2020,
            Some("pdf"),
            "flagged",
            "validation_failed",
            0,
            false,
            None,
            Some(r#"{"failed":["balance_identity"]}"#),
        ),
        // The layout moved under the extractor.
        (
            "doc_syni_f",
            2019,
            Some("esef"),
            "flagged",
            "structure_drift",
            0,
            true,
            Some(r#"{"drift":"section_moved"}"#),
            None,
        ),
    ];
    for (document, year, tier, acceptance, reason, facts, structure_changed, drift, detail) in rows
    {
        seed_report_document(
            state,
            document,
            &company.id,
            &format!("Raport okresowy za rok {year}"),
        );
        state
            .fundamentals_provenance()
            .record_extraction_outcome(NewExtractionOutcome {
                company_id: &company.id,
                report_document_id: document,
                fiscal_year: year,
                period_type: "FY",
                period_end: &format!("{year}-12-31"),
                tier,
                acceptance,
                reason_code: reason,
                detail_json: detail,
                drift_json: drift,
                structure_changed,
                fact_count: facts,
            })
            .expect("record an extraction outcome");
    }

    // The witness disagreed with the filing — re-recording the SAME slot is what
    // makes it a retry (`attempt_count > 1`), exactly as a re-run does.
    seed_report_document(
        state,
        "doc_syni_g",
        &company.id,
        "Raport okresowy za rok 2018",
    );
    for _ in 0..2 {
        state
            .fundamentals_provenance()
            .record_extraction_outcome(NewExtractionOutcome {
                company_id: &company.id,
                report_document_id: "doc_syni_g",
                fiscal_year: 2018,
                period_type: "FY",
                period_end: "2018-12-31",
                tier: Some("pdf"),
                acceptance: "flagged",
                reason_code: "witness_disagreement",
                detail_json: Some(r#"{"residual":"0.04"}"#),
                drift_json: None,
                structure_changed: false,
                fact_count: 0,
            })
            .expect("record a retried extraction outcome");
    }
}

/// Source adapters in both health states the Sources screen renders: one that
/// last fetched cleanly, one carrying a last error (the poor-state seed S2/S3
/// made reachable in the browser layer, here at storage level).
///
/// Shapes: `source-health-healthy`, `source-health-attention`,
/// `source-last-error-present`, `source-last-error-absent`,
/// `source-fetched-successfully-before`, `source-enabled`, `source-role-primary`,
/// `source-role-witness`.
fn case_source_adapter_health_states(state: &AppState) {
    let adapters = state
        .list_source_adapters()
        .expect("list source adapters (seeded from the registry)");
    let primary = adapters
        .iter()
        .find(|adapter| adapter.role == "primary")
        .expect("a primary adapter is seeded from the registry");
    let witness = adapters
        .iter()
        .find(|adapter| adapter.role == "witness")
        .expect("a witness adapter is seeded from the registry");

    let connection = state.checkout().expect("database connection");
    crate::storage::ingestion::record_source_outcome(
        &connection,
        &primary.id,
        "2026-06-02T05:00:00Z",
        12,
        3,
        3,
        0,
    )
    .expect("record a clean fetch");
    crate::storage::ingestion::record_source_outcome(
        &connection,
        &witness.id,
        "2026-06-02T05:05:00Z",
        8,
        1,
        1,
        0,
    )
    .expect("record a clean witness fetch");
    drop(connection);

    // The degraded one: a last error flips health to `attention`.
    state
        .record_source_adapter_error(&primary.id, "HTTP 503 from the upstream feed")
        .expect("record an adapter error");
}

/// One named case: a human-readable name and the function that seeds it.
type ShapeCase = (&'static str, fn(&AppState));

/// The corpus, in application order. Order is fixed and meaningful only for the
/// attribution report — each case seeds its own issuers, so no case depends on
/// another's data.
const CASES: [ShapeCase; 8] = [
    ("scored-industrial-issuer", case_scored_industrial_issuer),
    (
        "partially-covered-industrial-issuer",
        case_partially_covered_industrial_issuer,
    ),
    (
        "non-industrial-statement-types",
        case_non_industrial_statement_types,
    ),
    ("rule-backed-stream-rows", case_rule_backed_stream_rows),
    ("system-raised-rows", case_system_raised_rows),
    ("degraded-evidence-titles", case_degraded_evidence_titles),
    (
        "extraction-outcome-vocabulary",
        case_extraction_outcome_vocabulary,
    ),
    (
        "source-adapter-health-states",
        case_source_adapter_health_states,
    ),
];

/// Materialize the whole corpus on a fresh in-memory database (ADR 0048: seed
/// builders, never a checked-in binary snapshot).
fn build_corpus() -> AppState {
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    for (_, seed) in CASES {
        seed(&state);
    }
    state
}

// ---------------------------------------------------------------------------
// Gate 1 — coverage
// ---------------------------------------------------------------------------

#[test]
fn synthetic_corpus_covers_every_real_data_shape() {
    // Seed case by case and re-scan, so the report can say WHICH case first
    // realized each shape — and, for a gap, that no case realized it at all.
    let connection = open_in_memory_database().expect("database should initialize");
    let state = AppState::new(connection);
    let mut first_seen: BTreeMap<String, &'static str> = BTreeMap::new();
    let mut covered: BTreeSet<String> = BTreeSet::new();
    for (name, seed) in CASES {
        seed(&state);
        for key in scan_shapes(&state).shapes.into_keys() {
            if covered.insert(key.clone()) {
                first_seen.insert(key, name);
            }
        }
    }

    let inventory = inventory();
    let missing = missing_list();

    let uncovered: Vec<&InventoryShape> = inventory
        .iter()
        .filter(|shape| !covered.contains(&shape.key) && !missing.contains_key(&shape.key))
        .collect();
    let stale_excuses: Vec<&String> = missing
        .keys()
        .filter(|key| covered.contains(*key))
        .collect();
    let phantom_excuses: Vec<&String> = missing
        .keys()
        .filter(|key| !inventory.iter().any(|shape| &&shape.key == key))
        .collect();

    eprintln!("== synthetic shape corpus: {} cases ==", CASES.len());
    eprintln!(
        "inventory {} shapes | corpus reaches {} | accepted gaps {}",
        inventory.len(),
        inventory
            .iter()
            .filter(|shape| covered.contains(&shape.key))
            .count(),
        missing.len()
    );
    for shape in &inventory {
        let by = first_seen
            .get(&shape.key)
            .copied()
            .or_else(|| missing.contains_key(&shape.key).then_some("(accepted gap)"))
            .unwrap_or("!! UNCOVERED");
        eprintln!("   {:<10} {:<48} {by}", shape.domain, shape.key);
    }

    assert!(
        uncovered.is_empty(),
        "the synthetic corpus does not reach {} inventory shape(s) — add a named case to CASES, or \
         list each with an honest reason in src/test/scenarios/shape-inventory-missing.json \
         (epic #40 S6, ADR 0091 dec. 4):\n{}",
        uncovered.len(),
        uncovered
            .iter()
            .map(|shape| format!("  {} ({})", shape.key, shape.domain))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        stale_excuses.is_empty(),
        "shape-inventory-missing.json excuses shape(s) the corpus now DOES reach — delete the \
         entries so the gap list keeps meaning something:\n{}",
        stale_excuses
            .iter()
            .map(|key| format!("  {key}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        phantom_excuses.is_empty(),
        "shape-inventory-missing.json excuses shape(s) that are not in the inventory at all — a \
         typo, or a shape the last scan dropped:\n{}",
        phantom_excuses
            .iter()
            .map(|key| format!("  {key}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The inventory is a committed contract, not a scratch file: it must stay
/// well-formed, sorted, unique and inside the closed domain vocabulary, whether
/// it was regenerated by the scan or edited by hand.
#[test]
fn committed_inventory_is_well_formed() {
    let inventory = inventory();
    assert!(
        inventory.len() > 20,
        "shape-inventory.json is suspiciously small ({} shapes) — regenerate it with \
         `make shape-inventory-scan`",
        inventory.len()
    );
    let keys: Vec<&str> = inventory.iter().map(|shape| shape.key.as_str()).collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(
        keys, sorted,
        "shape-inventory.json must stay key-sorted so a rescan produces a reviewable diff"
    );
    let unique: BTreeSet<&str> = keys.iter().copied().collect();
    assert_eq!(
        unique.len(),
        keys.len(),
        "duplicate shape key in the inventory"
    );
    for shape in &inventory {
        assert!(
            SHAPE_DOMAINS.contains(&shape.domain.as_str()),
            "{} is in unknown domain {:?} — the vocabulary is SHAPE_DOMAINS",
            shape.key,
            shape.domain
        );
        assert!(
            shape.key.starts_with(&format!("{}-", shape.domain)),
            "{} does not start with its own domain prefix — the key would not be traceable",
            shape.key
        );
    }
    for (key, reason) in missing_list() {
        assert!(
            reason.trim().len() > 20,
            "the accepted gap {key} carries no real reason ({reason:?}) — the missing-list is a \
             record of WHY, not a mute allowlist"
        );
    }
    // The file name the scan writes and the file the gate reads are the same.
    assert!(INVENTORY_JSON.contains(INVENTORY_FILE_NAME.trim_end_matches(".json")));
}

// ---------------------------------------------------------------------------
// Gate 2 — the honesty invariants, on the synthetic corpus, in default CI
// ---------------------------------------------------------------------------

#[test]
fn synthetic_corpus_holds_the_honesty_invariants() {
    let state = build_corpus();

    let attention = measure_attention(&state);
    let effects = measure_effects(&state);

    eprintln!("== honesty invariants on the synthetic corpus (no real data) ==");
    eprintln!(
        "events={} specificity={:.1}% orphaned={} filename_as_statement={}",
        attention.events_total,
        attention.specificity_pct(),
        attention.orphaned_evidence,
        attention.filename_as_statement
    );
    eprintln!(
        "outcomes={} zero_effect_successes={} companies={} silent_missing_metrics={}",
        effects.outcomes_total,
        effects.zero_effect_successes,
        effects.companies_scored,
        effects.silent_missing_metrics
    );

    // A measurement over an empty corpus certifies nothing — the same
    // false-green guard the real-data harness carries.
    assert!(
        attention.events_total > 0
            && attention.specificity_denominator > 0
            && attention.orphan_denominator > 0,
        "the corpus produced no attention events to measure"
    );
    assert!(
        effects.outcomes_total > 0 && effects.companies_scored > 0,
        "the corpus produced no extraction outcomes or scored companies to measure"
    );

    // (1) A raw filename is metadata, never a row's statement.
    assert_eq!(
        attention.filename_as_statement, 0,
        "a Today row statement is a raw filename on the synthetic corpus — the rendering rule \
         must trim it (src/screens/Today/documentTitle.ts)"
    );
    // (2) Every title that carries prose must render as a statement. The corpus
    // contains exactly the rows that legitimately fall back to generic copy (a
    // pruned-evidence legacy row), so anything beyond that is a regression in
    // the statement rule.
    let generic_fallbacks = attention.specificity_denominator - attention.specificity_numerator;
    assert_eq!(
        generic_fallbacks, attention.orphaned_evidence,
        "a title-capable row fell back to generic copy without having lost its evidence — the \
         only honest reason to say nothing concrete is having nothing concrete left to say"
    );
    // (3) A recorded SUCCESS that produced nothing must name a why (ADR 0091
    // amendment: hard where the corpus is under our control).
    assert_eq!(
        effects.zero_effect_successes, 0,
        "an extraction outcome claims an emission it recorded no facts for. Real rows in this \
         state exist and are ratcheted down (#243); the synthetic corpus must never seed one"
    );
    // (4) A missing number must say WHAT is missing.
    assert_eq!(
        effects.silent_missing_metrics, 0,
        "a health read-model output reports a number as missing without naming it: {:?}",
        effects.by_silence_kind
    );
}

/// The run-summary half of the same class ([`crate::effects_honesty`]): a
/// summary shape that had inputs, produced nothing and names no reason is the
/// dishonest state — no situation in the corpus may reach it.
///
/// The stored outcome row and the UI summary are different projections of one
/// run: the row records the facts AT the slot (produced plus re-observed, S5's
/// `slot_fact_count`), the summary distinguishes the two. So this test does not
/// pretend to reconstruct the original summary — it enumerates, for every stored
/// row, BOTH readings the row is compatible with (all facts newly produced / all
/// facts re-observed) and asserts neither is `Unexplained`. A shape that can be
/// dishonest under either reading is a shape defect.
#[test]
fn synthetic_corpus_run_summaries_explain_their_effect() {
    use crate::commands::fundamentals_extraction::StructuredExtractionSummary;
    use crate::effects_honesty::{EffectVerdict, ExplainsEffect};

    let ids = |count: i64| -> Vec<String> { (0..count).map(|i| format!("fact_{i}")).collect() };

    let state = build_corpus();
    let mut checked = 0usize;
    for company in state.list_companies().expect("list companies") {
        for outcome in state
            .fundamentals_provenance()
            .list_extraction_outcomes(&company.id)
            .expect("list extraction outcomes")
        {
            let emitted = super::real_data_honesty::EMITTING_ACCEPTANCES
                .contains(&outcome.acceptance.as_str());
            let as_produced = StructuredExtractionSummary {
                acceptance: outcome.acceptance.clone(),
                tier: outcome.tier.clone(),
                emitted,
                produced_fact_ids: ids(outcome.fact_count),
                skipped_fact_ids: Vec::new(),
                divergent_count: 0,
                reason_code: Some(outcome.reason_code.clone()),
            };
            let as_reobserved = StructuredExtractionSummary {
                acceptance: outcome.acceptance.clone(),
                tier: outcome.tier.clone(),
                emitted,
                produced_fact_ids: Vec::new(),
                skipped_fact_ids: ids(outcome.fact_count),
                divergent_count: 0,
                reason_code: Some(outcome.reason_code.clone()),
            };
            checked += 1;
            for (reading, summary) in [
                ("all facts newly produced", &as_produced),
                ("all facts re-observed", &as_reobserved),
            ] {
                assert!(
                    !summary.effect_verdict().is_unexplained(),
                    "a corpus outcome read as {reading} maps to an Unexplained run summary — the \
                     shape cannot account for its own emptiness (acceptance={:?}, reason={:?}, \
                     facts={})",
                    outcome.acceptance,
                    outcome.reason_code,
                    outcome.fact_count
                );
            }
            if outcome.fact_count > 0 {
                assert_eq!(
                    as_produced.effect_verdict(),
                    EffectVerdict::Produced,
                    "an outcome whose facts were newly produced must read as Produced"
                );
            }
        }
    }
    assert!(checked > 0, "no run summary was checked");
}
