//! Real-data **shape-inventory scan** (epic #40 S6; ADR 0091 decision 4).
//!
//! Public CI can never see the maintainer's database (personal investment
//! research; 121 MB that would be permanent in git history). So the real DB
//! contributes exactly one thing to the public repo: an **anonymized inventory
//! of the data-state SHAPES it exhibits** — which entity states and combinations
//! occur, never a single byte of what they are about. The synthetic corpus
//! ([`super::shape_corpus`]) then grows named cases until it covers that
//! inventory, and public CI runs the S4/S5 honesty invariants over the synthetic
//! corpus through the same read models.
//!
//! # The hard privacy boundary
//!
//! Every emitted descriptor is a **key + domain**, built from machine
//! vocabularies only (`trigger_type`, `evidence_type`, `acceptance`,
//! `reason_code`, tier, statement type, health status — the enum-ish columns the
//! CHECK constraints define). Titles, tickers, names, URLs, ids, dates and free
//! text never reach a descriptor, and [`shape_token`] is a hard filter, not a
//! convention: a value that is not a lowercase machine token collapses to
//! `other` instead of being emitted. Counts stay on stderr; the committed file
//! carries the shape SET, which is the contract the coverage gate reads.
//!
//! # Generative, not a checklist
//!
//! Keys are built from the values actually observed, so a NEW trigger type,
//! acceptance or reason code in the real DB produces a NEW inventory key on the
//! next scan — which then reddens the coverage gate until the corpus grows a
//! case for it. A hand-maintained catalog could only miss such a shape silently.
//!
//! # Running it (local, owner's machine only)
//!
//! ```text
//! make shape-inventory-scan            # copy → scan → write src/test/scenarios/shape-inventory.json
//! ```
//!
//! or by hand, like the S4 harness:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/honesty-worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/honesty-worktest.sqlite3 \
//!   cargo test -p brawler --lib real_data_shape_inventory -- --ignored --nocapture
//! ```
//!
//! Like every `real_data_*` harness it SKIPS loudly without `BRAWLER_REAL_DB`,
//! and refuses the master snapshot / the live application database (opening a
//! database applies migrations).

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::real_data_honesty::{
    rendered_statement, EMITTING_ACCEPTANCES, JOINED_EVIDENCE_TYPES, PRODUCTION_CLAIMING_REASONS,
};
use super::*;
use crate::commands::company_health::compute_company_health;
use crate::fundamentals::health::{AltmanScore, PiotroskiScore};

/// The entity domains a shape can concern. One closed vocabulary shared by the
/// scan, the committed inventory, the synthetic corpus and the TS contract test
/// — a descriptor in an unknown domain is a typo, not a discovery.
pub(super) const SHAPE_DOMAINS: [&str; 5] =
    ["attention", "company", "extraction", "health", "source"];

/// Collapse a raw column value to a **machine token** safe to put in a public
/// key, or `other` when it is not one.
///
/// The filter is the privacy boundary in code: only `[a-z0-9_]` survives, so a
/// column that unexpectedly carries free text (a title, a name, a URL, an id
/// with digits and dashes) can never be spliced into a committed descriptor. It
/// is deliberately conservative — an `other` bucket is a visible prompt to look,
/// a leaked token would be silent.
pub(super) fn shape_token(raw: &str) -> String {
    let trimmed = raw.trim();
    let looks_machine = !trimmed.is_empty()
        && trimmed.len() <= 40
        && trimmed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if looks_machine {
        trimmed.replace('_', "-")
    } else {
        "other".to_owned()
    }
}

/// The inventory under construction: shape key → (domain, occurrences).
#[derive(Default)]
pub(super) struct ShapeScan {
    pub(super) shapes: BTreeMap<String, (&'static str, usize)>,
    /// Raw values that failed [`shape_token`], by domain — a count only, so a
    /// surprise is visible without printing what the surprise was.
    pub(super) non_token_values: BTreeMap<&'static str, usize>,
}

impl ShapeScan {
    fn note(&mut self, domain: &'static str, key: impl AsRef<str>) {
        let key = key.as_ref();
        debug_assert!(
            SHAPE_DOMAINS.contains(&domain),
            "unknown shape domain {domain}"
        );
        let entry = self.shapes.entry(key.to_owned()).or_insert((domain, 0));
        entry.1 += 1;
    }

    /// Note a shape whose key ends in a value read from the database. Routes
    /// through [`shape_token`] and records the miss when the value is not a
    /// machine token.
    fn note_valued(&mut self, domain: &'static str, prefix: &str, raw: &str) {
        let token = shape_token(raw);
        if token == "other" {
            *self.non_token_values.entry(domain).or_default() += 1;
        }
        self.note(domain, format!("{prefix}-{token}"));
    }
}

/// Walk every read model that feeds an honesty metric and record the shapes the
/// database exhibits. Read-only: nothing here writes, and nothing here reads a
/// column the read models do not already expose.
pub(super) fn scan_shapes(state: &AppState) -> ShapeScan {
    let mut scan = ShapeScan::default();

    // --- attention: THE Today read model ------------------------------------
    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput {
            company_id: None,
            include_dismissed: true,
        })
        .expect("list attention events");
    let mut companies_with_events: BTreeSet<String> = BTreeSet::new();
    for event in &events {
        scan.note_valued("attention", "attention-trigger", &event.trigger_type);
        scan.note_valued("attention", "attention-evidence", &event.evidence_type);
        // The severity vocabulary as the CONTRACT serializes it, not a Debug
        // rendering — the same token the frontend routes on.
        let severity = serde_json::to_value(event.severity)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default();
        scan.note_valued("attention", "attention-severity", &severity);
        match &event.company_id {
            Some(company_id) => {
                companies_with_events.insert(company_id.clone());
                scan.note("attention", "attention-scope-company");
            }
            None => scan.note("attention", "attention-scope-workspace"),
        }
        scan.note(
            "attention",
            match event.rule_id {
                Some(_) => "attention-from-user-rule",
                None => "attention-system-raised",
            },
        );

        // The statement the row actually renders (the real frontend rule).
        let statement = rendered_statement(event.evidence_title.as_deref());
        scan.note(
            "attention",
            match statement {
                Some(_) => "attention-statement-concrete",
                None => "attention-statement-generic-fallback",
            },
        );
        scan.note(
            "attention",
            match (event.evidence_title.as_deref(), statement.as_deref()) {
                (None, _) => "attention-title-absent",
                (Some(_), None) => "attention-title-filename-only",
                (Some(title), Some(statement)) if title.trim() != statement => {
                    "attention-title-filename-prefixed"
                }
                (Some(_), Some(_)) => "attention-title-plain-prose",
            },
        );
        if JOINED_EVIDENCE_TYPES.contains(&event.evidence_type.as_str()) {
            scan.note(
                "attention",
                match event.evidence_title {
                    Some(_) => "attention-evidence-resolved",
                    None => "attention-evidence-orphaned",
                },
            );
        }
        scan.note(
            "attention",
            match event.evidence_detail {
                Some(_) => "attention-evidence-detail-present",
                None => "attention-evidence-detail-absent",
            },
        );
        if event.dismissed {
            scan.note("attention", "attention-dismissed");
        } else if event.seen {
            scan.note("attention", "attention-seen-not-dismissed");
        } else {
            scan.note("attention", "attention-unseen");
        }
    }

    // --- per company: extraction outcomes, health, the document/fact backbone -
    for company in &state.list_companies().expect("list companies") {
        let outcomes = state
            .fundamentals_provenance()
            .list_extraction_outcomes(&company.id)
            .expect("list extraction outcomes");
        for outcome in &outcomes {
            scan.note_valued("extraction", "extraction-acceptance", &outcome.acceptance);
            scan.note_valued("extraction", "extraction-reason", &outcome.reason_code);
            match &outcome.tier {
                Some(tier) => scan.note_valued("extraction", "extraction-tier", tier),
                None => scan.note("extraction", "extraction-tier-absent"),
            }
            let succeeded = EMITTING_ACCEPTANCES.contains(&outcome.acceptance.as_str());
            let claims_production =
                PRODUCTION_CLAIMING_REASONS.contains(&outcome.reason_code.as_str());
            match (outcome.fact_count == 0, succeeded, claims_production) {
                (false, _, _) => scan.note("extraction", "extraction-facts-recorded"),
                // The S5 dishonest residue: a success that recorded nothing and
                // claims an emission instead of naming a why (#243 repairs the
                // stored rows; the corpus deliberately never reproduces it).
                (true, true, true) => {
                    scan.note("extraction", "extraction-zero-facts-claiming-emission")
                }
                (true, true, false) => {
                    scan.note("extraction", "extraction-zero-facts-with-named-reason")
                }
                (true, false, _) => scan.note("extraction", "extraction-facts-zero-not-accepted"),
            }
            scan.note(
                "extraction",
                match outcome.attempt_count > 1 {
                    true => "extraction-retried",
                    false => "extraction-first-attempt",
                },
            );
            if outcome.structure_changed {
                scan.note("extraction", "extraction-structure-changed");
            }
            if outcome.drift_json.is_some() {
                scan.note("extraction", "extraction-drift-report-present");
            }
            if outcome.detail_json.is_some() {
                scan.note("extraction", "extraction-detail-present");
            }
        }

        // --- health ---------------------------------------------------------
        if let Ok(health) = compute_company_health(state, &company.id) {
            scan.note_valued("health", "health-statement-type", &health.statement_type);
            scan.note(
                "health",
                match health.latest {
                    Some(_) => "health-latest-period-present",
                    None => "health-latest-period-absent",
                },
            );
            scan.note(
                "health",
                match health.history.len() {
                    0 => "health-history-empty",
                    1 => "health-history-single-period",
                    _ => "health-history-multi-period",
                },
            );
            for period in &health.history {
                scan.note(
                    "health",
                    match &period.piotroski {
                        PiotroskiScore::Headline { .. } => "health-piotroski-headline",
                        PiotroskiScore::InsufficientData { missing, .. } if missing.is_empty() => {
                            "health-piotroski-insufficient-unnamed"
                        }
                        PiotroskiScore::InsufficientData { .. } => {
                            "health-piotroski-insufficient-named"
                        }
                        PiotroskiScore::NotApplicable { reason } if reason.trim().is_empty() => {
                            "health-piotroski-not-applicable-unreasoned"
                        }
                        PiotroskiScore::NotApplicable { .. } => {
                            "health-piotroski-not-applicable-reasoned"
                        }
                    },
                );
                scan.note(
                    "health",
                    match &period.altman {
                        AltmanScore::Headline { .. } => "health-altman-headline",
                        AltmanScore::InsufficientData { missing, .. } if missing.is_empty() => {
                            "health-altman-insufficient-unnamed"
                        }
                        AltmanScore::InsufficientData { .. } => "health-altman-insufficient-named",
                        AltmanScore::NotApplicable { reason } if reason.trim().is_empty() => {
                            "health-altman-not-applicable-unreasoned"
                        }
                        AltmanScore::NotApplicable { .. } => {
                            "health-altman-not-applicable-reasoned"
                        }
                    },
                );
            }
        }

        // --- the company backbone the metrics above stand on -----------------
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        let periods = state
            .list_financial_periods(ListFinancialPeriodsInput {
                company_id: company.id.clone(),
                fiscal_year: None,
            })
            .expect("list financial periods");
        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list financial facts");

        scan.note(
            "company",
            match documents.is_empty() {
                true => "company-without-report-documents",
                false => "company-with-report-documents",
            },
        );
        scan.note(
            "company",
            match periods.is_empty() {
                true => "company-without-financial-periods",
                false => "company-with-financial-periods",
            },
        );
        scan.note(
            "company",
            match facts.is_empty() {
                true => "company-without-financial-facts",
                false => "company-with-financial-facts",
            },
        );
        // The named example from the S6 card: documents captured, nothing
        // extracted from them yet.
        if !documents.is_empty() && facts.is_empty() {
            scan.note("company", "company-documents-without-facts");
        }
        scan.note(
            "company",
            match outcomes.is_empty() {
                true => "company-without-extraction-outcomes",
                false => "company-with-extraction-outcomes",
            },
        );
        scan.note(
            "company",
            match companies_with_events.contains(&company.id) {
                true => "company-with-attention-events",
                false => "company-without-attention-events",
            },
        );
    }

    // --- sources: the poor-state surface S2/S3 made reachable ----------------
    for adapter in &state.list_source_adapters().expect("list source adapters") {
        scan.note_valued("source", "source-health", &adapter.health_status);
        scan.note_valued("source", "source-role", &adapter.role);
        scan.note(
            "source",
            match adapter.enabled {
                true => "source-enabled",
                false => "source-disabled",
            },
        );
        scan.note(
            "source",
            match adapter.last_error {
                Some(_) => "source-last-error-present",
                None => "source-last-error-absent",
            },
        );
        scan.note(
            "source",
            match adapter.last_success_at {
                Some(_) => "source-fetched-successfully-before",
                None => "source-never-succeeded",
            },
        );
        if adapter
            .last_detail_items_failed
            .is_some_and(|failed| failed > 0)
        {
            scan.note("source", "source-detail-fetch-failures");
        }
    }

    scan
}

/// A one-line generic description per key prefix, so the committed file reads as
/// documentation. Prose about the SHAPE CLASS only — it must stay true of any
/// database, which is exactly what makes it safe to commit.
fn describe(key: &str) -> &'static str {
    match key {
        k if k.starts_with("attention-trigger-") => "attention event fired by this trigger type",
        k if k.starts_with("attention-evidence-detail-") => {
            "attention event with/without a secondary evidence datum"
        }
        k if k.starts_with("attention-evidence-resolved")
            || k.starts_with("attention-evidence-orphaned") =>
        {
            "attention event whose joined evidence does/does not resolve"
        }
        k if k.starts_with("attention-evidence-") => "attention event carrying this evidence type",
        k if k.starts_with("attention-severity-") => "attention event at this computed severity",
        k if k.starts_with("attention-scope-") => "attention event at this scope",
        k if k.starts_with("attention-title-") => "attention event evidence title in this form",
        k if k.starts_with("attention-statement-") => "attention row rendering in this form",
        k if k.starts_with("attention-") => "attention event in this lifecycle/origin state",
        k if k.starts_with("extraction-acceptance-") => {
            "extraction outcome recorded with this acceptance"
        }
        k if k.starts_with("extraction-reason-") => {
            "extraction outcome with this typed reason code"
        }
        k if k.starts_with("extraction-tier-") => "extraction outcome produced by this tier",
        k if k.starts_with("extraction-") => "extraction outcome in this effect/attempt state",
        k if k.starts_with("health-statement-type-") => {
            "health read model for a company of this statement type"
        }
        k if k.starts_with("health-history-") || k.starts_with("health-latest-") => {
            "health read model with this period coverage"
        }
        k if k.starts_with("health-") => "health period score in this state",
        k if k.starts_with("company-") => "company with this document/fact/event coverage",
        k if k.starts_with("source-") => "source adapter in this health/liveness state",
        _ => "data-state shape observed in the real database",
    }
}

/// Where the scan writes: the canonical shared scenario directory, reached
/// through the build.rs-resolved env (never a literal `../` path — the mutants
/// sandbox copies only `src-tauri/`, `source_tree_guards`).
pub(super) const INVENTORY_FILE_NAME: &str = "shape-inventory.json";

#[test]
#[ignore = "real-data shape-inventory scan; needs BRAWLER_REAL_DB (a throwaway copy)"]
fn real_data_shape_inventory_scan() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_shape_inventory_scan: set BRAWLER_REAL_DB to a THROWAWAY copy of the \
             owner's database (see private/realdata/README.md)"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP real_data_shape_inventory_scan: no database at {db_path}");
        return;
    }
    // Same refusal as the S4 harness: opening a database APPLIES MIGRATIONS, so
    // the master snapshot and the live application database are off limits.
    let file_name = std::path::Path::new(&db_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    assert!(
        file_name != "brawler.sqlite3" && !db_path.starts_with("/mnt/d/"),
        "refusing to run: {db_path} is the master snapshot or the live application database. \
         This harness migrates — copy it first (private/realdata/README.md)."
    );

    let connection = open_database(&db_path).expect("open throwaway real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    let scan = scan_shapes(&state);

    eprintln!("== S6 shape inventory (anonymized descriptors; counts are stderr-only) ==");
    eprintln!("db={db_path}");
    eprintln!("shapes observed = {}", scan.shapes.len());
    for (key, (domain, count)) in &scan.shapes {
        eprintln!("   {domain:<10} {key:<48} x{count}");
    }
    if !scan.non_token_values.is_empty() {
        eprintln!("-- values that were NOT machine tokens (collapsed to `other`) --");
        for (domain, count) in &scan.non_token_values {
            eprintln!("   {domain:<10} {count}");
        }
    }

    // A scan that saw nothing must not produce an inventory certifying that the
    // corpus covers everything (the S4 "measured nothing = false green" rule).
    assert!(
        scan.shapes.len() > 20,
        "expected a rich shape inventory from the real database, got {} — wrong copy?",
        scan.shapes.len()
    );

    let shapes: Vec<serde_json::Value> = scan
        .shapes
        .iter()
        .map(|(key, (domain, _))| {
            serde_json::json!({
                "key": key,
                "domain": domain,
                "description": describe(key),
            })
        })
        .collect();
    let inventory = serde_json::json!({
        "$comment": concat!(
            "ANONYMIZED SHAPE DESCRIPTORS ONLY (ADR 0091 dec. 4, epic #40 S6). Which data-state ",
            "shapes the maintainer's real database exhibits — never titles, tickers, names, ids, ",
            "dates or any row content. Regenerate with `make shape-inventory-scan`; the coverage ",
            "gate over the synthetic corpus lives in src-tauri/src/storage/tests/shape_corpus.rs."
        ),
        "domains": SHAPE_DOMAINS,
        "shapes": shapes,
    });
    let serialized = format!(
        "{}\n",
        serde_json::to_string_pretty(&inventory).expect("serialize inventory")
    );

    let canonical = std::path::Path::new(env!("BRAWLER_SCENARIOS_DIR")).join(INVENTORY_FILE_NAME);
    let out_path = if canonical.parent().is_some_and(|parent| parent.is_dir()) {
        canonical
    } else {
        let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        std::fs::create_dir_all(&out_dir).expect("create target dir");
        out_dir.join(INVENTORY_FILE_NAME)
    };
    std::fs::write(&out_path, serialized).expect("write shape inventory");
    eprintln!("inventory written to {}", out_path.display());
    eprintln!(
        "review the diff before committing — nothing in it may be traceable to a real company"
    );
}

#[test]
fn shape_token_refuses_anything_that_is_not_a_machine_token() {
    // The privacy boundary, asserted: machine vocabularies pass through, and
    // every content-shaped value collapses instead of reaching a public file.
    assert_eq!(shape_token("accepted_via_witness"), "accepted-via-witness");
    assert_eq!(shape_token("job_failed"), "job-failed");
    assert_eq!(shape_token("esef"), "esef");
    for content in [
        "CDR",
        "CD PROJEKT S.A.",
        "Raport bieżący 12/2026",
        "https://www.gpw.pl/komunikaty?id=1",
        "2410_Passus_PL-sig.pdf",
        "01H8XY9Z-4C2A-4E1B",
        "2026-07-29T09:12:00Z",
        "wyniki finansowe",
        "",
        "   ",
    ] {
        assert_eq!(
            shape_token(content),
            "other",
            "{content:?} must never be spliced into a committed shape key"
        );
    }
}
