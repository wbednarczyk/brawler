//! Real-data recall/precision harness for the structured fundamentals
//! extraction pipeline (Radicle 380024f; ADR 0061 guardrail — "a `#[ignore]`
//! real-data harness measures recall/precision on the owner's filings before
//! any default flip"; also the gate for the ADR 0060 decision-3 document-tier
//! default-model change).
//!
//! **Inert in CI** — like [`super::autopilot::autopilot_real_data_validation`],
//! it skips unless `BRAWLER_REAL_DB` points at a throwaway copy of the
//! maintainer's real DB, so `make check` never runs it.
//!
//! Where `autopilot_real_data_validation` is a pipeline *smoke* (drains the
//! durable queue, asserts a terminal state), this harness measures *accuracy*:
//! for a small hand-labeled ground-truth set of real report documents, it
//! calls the pure [`run_pipeline`] directly (no job queue, no persistence,
//! same as [`crate::jobs::structured_extraction::run_structured_extraction`]
//! minus the write side) and compares the emitted facts against the labels.
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo test -p brawler --lib real_data_extraction_recall_precision -- --ignored --nocapture
//! ```
//!
//! See `docs/testing.md` for the ground-truth JSON format and the env-var
//! contract.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use super::*;
use crate::fundamentals::extraction::pipeline::{run_pipeline, PipelineInput};
use crate::fundamentals::validation::Tolerance;
use crate::report_diff::extraction::SourceFormat;

// ---------------------------------------------------------------------------
// Ground-truth JSON shape
// ---------------------------------------------------------------------------

/// One labeled report document. See `docs/testing.md` for the on-disk shape
/// and a worked example.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruthDocument {
    /// Company ticker (matched case-insensitively against `companies.ticker`).
    ticker: String,
    #[serde(rename = "match")]
    doc_match: DocumentMatch,
    /// ISO `YYYY-MM-DD` period end the labeled facts belong to.
    period_end: String,
    /// `Q1`/`Q2`/`Q3`/`H1`/`FY` etc — matches `financial_periods.period_type`.
    period_type: String,
    fiscal_year: i64,
    /// `metric_key` -> expected value, as a decimal string in signed base units.
    facts: BTreeMap<String, String>,
}

/// Identifies which `report_documents` row a ground-truth entry labels.
/// Either field may be set; at least one must be for the entry to resolve.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DocumentMatch {
    url_contains: Option<String>,
    content_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruth {
    documents: Vec<GroundTruthDocument>,
}

fn document_matches(document: &ReportDocument, spec: &DocumentMatch) -> bool {
    if let Some(needle) = spec.url_contains.as_deref() {
        if document.url.contains(needle) {
            return true;
        }
    }
    if let Some(hash) = spec.content_hash.as_deref() {
        if document.content_hash.as_deref() == Some(hash) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Small local mirrors of private pipeline helpers
// ---------------------------------------------------------------------------

/// Mirrors `jobs::structured_extraction::prior_period_end` (private to that
/// module): the immediately-prior period's end date, by decrementing the
/// leading year. Duplicated here rather than exported, since it is a 3-line
/// pure helper and this harness must not widen the production API surface.
fn prior_period_end(period_end: &str) -> Option<String> {
    let year: i64 = period_end.get(0..4)?.parse().ok()?;
    Some(format!("{:04}{}", year - 1, period_end.get(4..)?))
}

/// Mirrors `Tolerance::accepts` (private to `fundamentals::validation`) and
/// `cross_check_prior`'s residual/scale convention (`residual = actual -
/// expected`, `scale = actual.abs().max(expected.abs())`) exactly, so a
/// "match" here means what the pipeline's own validation gate would accept.
fn tolerance_accepts(tol: &Tolerance, actual: Decimal, expected: Decimal) -> bool {
    let residual = actual - expected;
    let scale = actual.abs().max(expected.abs());
    let allowed = (scale * tol.relative).max(tol.absolute);
    residual.abs() <= allowed
}

fn pct(matched: usize, total: usize) -> String {
    if total == 0 {
        "n/a".to_owned()
    } else {
        format!("{:.1}%", 100.0 * matched as f64 / total as f64)
    }
}

#[derive(Default)]
struct TierStats {
    matched: usize,
    labeled: usize,
    emitted_for_labeled: usize,
}

/// Real-data recall/precision harness (CLAUDE.md real-data-validation-
/// precedes-implementation guardrail; ADR 0061 guardrail). **Inert in CI** —
/// skips unless `BRAWLER_REAL_DB` and a ground-truth file are present.
///
/// `BRAWLER_REAL_DATA_DIR` points at the Tauri data dir holding the actual
/// fetched report files (same convention as `autopilot_real_data_validation`);
/// without it, `AppState::new` falls back to a temp data dir, so every
/// document fails to resolve (the sanity assert below then fails loudly
/// rather than silently reporting 0/0).
///
/// `BRAWLER_GROUND_TRUTH` overrides the ground-truth path (default
/// `private/realdata/ground_truth.json`, resolved relative to the repo root).
///
/// This is a harness-sanity gate only (no quality floor): it asserts the
/// ground truth is reachable and *some* labeled fact matched. Precision/
/// recall thresholds are a deliberate follow-up before any default flip
/// (ADR 0061 guardrail; ADR 0060 decision 3) — see `docs/testing.md`.
#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR + a ground-truth file"]
fn real_data_extraction_recall_precision() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_extraction_recall_precision: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };

    let ground_truth_path = std::env::var("BRAWLER_GROUND_TRUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/ground_truth.json")
        });
    let Ok(raw) = std::fs::read_to_string(&ground_truth_path) else {
        eprintln!(
            "SKIP real_data_extraction_recall_precision: no ground-truth file at {}",
            ground_truth_path.display()
        );
        return;
    };
    let ground_truth: GroundTruth =
        serde_json::from_str(&raw).expect("ground-truth JSON must parse (see docs/testing.md)");
    if ground_truth.documents.is_empty() {
        eprintln!(
            "SKIP real_data_extraction_recall_precision: ground truth at {} has no documents",
            ground_truth_path.display()
        );
        return;
    }

    // open_database applies migrations, same as autopilot_real_data_validation.
    let connection = open_database(&db_path).expect("open real db");
    // Point at the real Tauri data dir so report bytes can be read (same
    // convention as autopilot_real_data_validation's BRAWLER_REAL_DATA_DIR).
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    let companies = state.list_companies().expect("list companies");
    let tol = Tolerance::default();

    let mut resolved = 0usize;
    let mut total_labeled = 0usize;
    let mut total_matched = 0usize;
    let mut total_emitted_for_labeled = 0usize;
    let mut total_unlabeled_emitted = 0usize;
    let mut tier_stats: BTreeMap<&'static str, TierStats> = BTreeMap::new();

    eprintln!(
        "== real-data extraction recall/precision: {} labeled document(s) ==",
        ground_truth.documents.len()
    );

    for doc in &ground_truth.documents {
        let label = format!("{} {}", doc.ticker, doc.period_end);

        if doc.doc_match.url_contains.is_none() && doc.doc_match.content_hash.is_none() {
            eprintln!("SKIP {label}: ground-truth entry has neither urlContains nor contentHash");
            continue;
        }

        let Some(company) = companies
            .iter()
            .find(|c| c.ticker.eq_ignore_ascii_case(&doc.ticker))
        else {
            eprintln!("SKIP {label}: no company with ticker {}", doc.ticker);
            continue;
        };

        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        let Some(report_document) = documents
            .iter()
            .find(|rd| document_matches(rd, &doc.doc_match))
        else {
            eprintln!(
                "SKIP {label}: no report_documents row matched {:?}",
                doc.doc_match
            );
            continue;
        };

        let Some(local_path) = report_document.local_path.as_deref() else {
            eprintln!("SKIP {label}: matched report document has no stored file (local_path)");
            continue;
        };
        let path = state.data_dir().join(local_path);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("SKIP {label}: failed to read {}: {err}", path.display());
                continue;
            }
        };

        let format = SourceFormat::resolve(report_document.content_type.as_deref(), local_path);
        let layer1_facts: Option<Vec<crate::storage::NewTaggedFact>> = match format {
            SourceFormat::Xhtml => Some(
                crate::jobs::structured_extraction::compute_layer1_generation(
                    &bytes,
                    crate::jobs::structured_extraction::DocumentRoute::IxbrlInstance,
                )
                .facts,
            ),
            SourceFormat::Pdf => {
                // The PDF fact-extraction arm is retired (ADR 0086 dec. 1); a
                // PDF-only document has no surviving tier to feed.
                eprintln!("SKIP {label}: PDF-only document, no surviving extraction tier");
                continue;
            }
        };

        // The honest live configuration: whatever prior-period data this
        // company already has, exactly as run_structured_extraction reads it.
        let prior_end = prior_period_end(&doc.period_end);
        let prior = state
            .financials()
            .stored_fact_set(&company.id, doc.fiscal_year - 1, &doc.period_type)
            .expect("stored fact set");

        let input = PipelineInput {
            period_end: &doc.period_end,
            layer1_facts: layer1_facts.as_deref(),
            // This harness's `SourceFormat` has no ZIP-package concept (unlike
            // `t7_cbf_corpus`'s `run_new`) — every xhtml document routes bare,
            // so it never carries linkbase evidence.
            has_presentation_linkbase: false,
            prior: prior.as_ref(),
            prior_period_end: prior_end.as_deref(),
            expected_keys: None,
        };
        let outcome = run_pipeline(&input);

        let mut emitted: BTreeMap<&str, Decimal> = BTreeMap::new();
        for fact in &outcome.facts {
            emitted.insert(&fact.metric_key, fact.value);
        }

        let mut labeled = 0usize;
        let mut matched = 0usize;
        let mut emitted_for_labeled = 0usize;
        for (key, expected_str) in &doc.facts {
            labeled += 1;
            let expected = Decimal::from_str(expected_str)
                .unwrap_or_else(|e| panic!("bad decimal '{expected_str}' for {key}: {e}"));
            match emitted.get(key.as_str()) {
                Some(&actual) => {
                    emitted_for_labeled += 1;
                    if tolerance_accepts(&tol, actual, expected) {
                        matched += 1;
                    } else {
                        eprintln!("  MISMATCH {key}: expected {expected} got {actual}");
                    }
                }
                None => eprintln!("  MISSING  {key}: expected {expected}"),
            }
        }
        let unlabeled_emitted = emitted
            .keys()
            .filter(|k| !doc.facts.contains_key(**k))
            .count();

        let tier = outcome.tier.map(|t| t.as_str()).unwrap_or("none");
        eprintln!(
            "{label:<24} tier={tier:<16} acceptance={:<20} recall={:>7} ({matched}/{labeled})  precision={:>7} ({matched}/{emitted_for_labeled})  unlabeled_emitted={unlabeled_emitted}",
            outcome.acceptance.as_str(),
            pct(matched, labeled),
            pct(matched, emitted_for_labeled),
        );

        let entry = tier_stats.entry(tier).or_default();
        entry.matched += matched;
        entry.labeled += labeled;
        entry.emitted_for_labeled += emitted_for_labeled;

        resolved += 1;
        total_labeled += labeled;
        total_matched += matched;
        total_emitted_for_labeled += emitted_for_labeled;
        total_unlabeled_emitted += unlabeled_emitted;
    }

    eprintln!("-- per tier --");
    for (tier, stats) in &tier_stats {
        eprintln!(
            "{tier:<16} recall={:>7} ({}/{})  precision={:>7} ({}/{})",
            pct(stats.matched, stats.labeled),
            stats.matched,
            stats.labeled,
            pct(stats.matched, stats.emitted_for_labeled),
            stats.matched,
            stats.emitted_for_labeled,
        );
    }
    eprintln!(
        "-- overall: resolved={}/{} documents  recall={} ({total_matched}/{total_labeled})  precision={} ({total_matched}/{total_emitted_for_labeled})  unlabeled_emitted={total_unlabeled_emitted} --",
        resolved,
        ground_truth.documents.len(),
        pct(total_matched, total_labeled),
        pct(total_matched, total_emitted_for_labeled),
    );

    // Harness-sanity only (no quality floor yet — see docs/testing.md).
    assert!(
        resolved > 0,
        "expected at least one ground-truth document to resolve to a real report file \
         (check BRAWLER_REAL_DATA_DIR, tickers, and match specs)"
    );
    assert!(
        total_matched > 0,
        "overall recall must be > 0 across the resolved documents (got {total_matched}/{total_labeled})"
    );
}

// ===========================================================================
// A4 — corpus-wide deterministic sweep (plan v0.59 A4, card 380024f)
// ===========================================================================
//
// The labeled harness above measures precision where hand labels exist. It
// cannot answer the question ADR 0084 left open — *how much of the owner's
// real filing corpus can the deterministic-only pipeline actually read?* —
// because labelling 3 800 documents by hand is not a thing anyone will do.
//
// This sweep answers that question honestly by separating two claims that a
// single "coverage %" would silently merge:
//
//   * **Production counts** — what each tier attempted, what the validation
//     gate accepted or flagged (by typed `reason_code`), and how many of a
//     company's expected primary KPIs ended up filled. Measured on every
//     attempted document. No precision claim is attached to these: nobody has
//     checked the values against the filings.
//   * **Ground-truth-backed precision** — only where hand labels exist (the
//     espi-wdf corpus, and any labeled report documents in the JSON above).
//
// And, per the ADR 0085 amendment (2026-07-21), it reports **filing-sourced
// facts separately from aggregator-sourced ones**: a coverage number that
// silently counts BiznesRadar fallback values overstates how well Brawler
// reads filings, which is the only number the parked decisions can rest on.

use std::collections::{BTreeSet, HashMap};

use crate::fundamentals::extraction::espi_cover_note::parse_espi_cover_note;
use crate::fundamentals::extraction::StatementBasis;
use crate::jobs::structured_extraction::{
    derive_report_period, is_esef_route, route_document, run_structured_extraction, DocumentRoute,
};

/// Default number of documents the sweep attempts. The full corpus is ~3 800
/// stored files, many of them multi-megabyte PDFs whose text extraction
/// dominates the runtime; the default keeps a manual run to minutes, and
/// `BRAWLER_A4_LIMIT=0` lifts the cap for a full overnight pass.
const DEFAULT_SWEEP_LIMIT: usize = 250;

/// Aggregated counters for one sweep.
#[derive(Default)]
struct SweepTally {
    eligible: usize,
    /// Eligible documents by stored `doc_kind` — without this split the
    /// "no period derivable" number is uninterpretable: a governance statement
    /// legitimately has no reporting period, a `periodic_ssf` that has none is
    /// a genuine gap.
    eligible_by_kind: BTreeMap<String, usize>,
    no_period_derived: usize,
    no_period_by_kind: BTreeMap<String, usize>,
    attempted: usize,
    errored: usize,
    by_route: BTreeMap<&'static str, usize>,
    /// Runs that read at least one tracked fact out of the document. NOT
    /// `StructuredExtractionResult::emitted`, which means "created a NEW fact
    /// row": on the owner's database most slots are already filled by earlier
    /// production runs, so every successful re-read looks like zero emissions.
    /// Re-observing the same value IS the pipeline reading the document.
    route_read: BTreeMap<&'static str, usize>,
    /// Documents that yielded at least one tracked fact, and the fact total —
    /// the document-level recall the KPI-profile denominator cannot give.
    documents_with_facts: usize,
    facts_read_total: usize,
    by_tier: BTreeMap<String, usize>,
    by_acceptance: BTreeMap<String, usize>,
    by_reason: BTreeMap<String, usize>,
    facts_by_provenance_tier: BTreeMap<String, usize>,
}

/// One `(company, fiscal_year, period_type)` slot the sweep attempted, with the
/// metric keys the pipeline read for it (union across that slot's documents).
#[derive(Default)]
struct SlotCoverage {
    read_keys: BTreeSet<String>,
    aggregator_keys: BTreeSet<String>,
}

/// The tiers that read the ISSUER's own filing. `html_aggregator` is
/// deliberately absent: it is a third party's transcription, and ADR 0085's
/// amendment requires it to be counted apart from anything read out of a filing.
fn is_filing_tier(tier: &str) -> bool {
    matches!(
        tier,
        "esef" | "structured_xhtml" | "espi_cover_note" | "pdf"
    )
}

/// Ground-truth-backed precision over the hand-labeled espi-wdf cover-note
/// corpus (15 documents / 347 facts). Returns `(matched, labeled, emitted)`, or
/// `None` when the owner's corpus is absent.
///
/// The pinned acceptance gate for this corpus lives in
/// `fundamentals::extraction::espi_cover_note` (`ground_truth_corpus_347_of_347`);
/// this recomputes it only so the A4 summary can state one precision figure that
/// is genuinely label-backed, next to the production counts that are not.
fn wdf_corpus_precision(spike_dir: &std::path::Path) -> Option<(usize, usize, usize)> {
    #[derive(Deserialize)]
    struct GtFact {
        metric_key: String,
        scope: String,
        value: String,
    }
    #[derive(Deserialize)]
    struct GtPeriod {
        to: String,
    }
    #[derive(Deserialize)]
    struct GtDocument {
        name: String,
        period: GtPeriod,
        facts: Vec<GtFact>,
    }
    #[derive(Deserialize)]
    struct Gt {
        documents: Vec<GtDocument>,
    }

    let raw = std::fs::read_to_string(spike_dir.join("ground_truth.json")).ok()?;
    let gt: Gt = serde_json::from_str(&raw).ok()?;

    let (mut matched, mut labeled, mut emitted) = (0usize, 0usize, 0usize);
    for doc in &gt.documents {
        let body =
            std::fs::read_to_string(spike_dir.join("corpus").join(format!("{}.txt", doc.name)))
                .ok()?;
        let parsed = parse_espi_cover_note(&body, &doc.period.to);

        let mut produced: BTreeMap<(String, &'static str), Decimal> = BTreeMap::new();
        for fact in &parsed.facts {
            let scope = match fact.basis {
                Some(StatementBasis::Standalone) => "standalone",
                _ => "consolidated",
            };
            produced
                .entry((fact.metric_key.clone(), scope))
                .or_insert(fact.value);
        }

        for label in &doc.facts {
            labeled += 1;
            let want = Decimal::from_str(&label.value).ok()?;
            let scope: &str = &label.scope;
            if let Some(got) = produced
                .iter()
                .find(|((k, s), _)| k == &label.metric_key && *s == scope)
                .map(|(_, v)| *v)
            {
                if (got - want).abs() < Decimal::from_str("0.005").ok()? {
                    matched += 1;
                }
            }
        }
        emitted += produced.len();
    }
    Some((matched, labeled, emitted))
}

/// **A4 — the deterministic pipeline's real coverage number** (ADR 0061
/// guardrail: "a `#[ignore]` real-data harness measures recall/precision on the
/// owner's filings before any default flip"; ADR 0084 made this the *official*
/// coverage number by retiring the AI tier; ADR 0085's amendment requires the
/// filing/aggregator split).
///
/// **Inert in CI** — skips with a printed message unless `BRAWLER_REAL_DB` and
/// `BRAWLER_REAL_DATA_DIR` are set. It writes facts, provenance and outcome rows
/// (it runs the real job-layer path, which is the only way to measure the typed
/// `reason_code` vocabulary), so it **refuses to run against the master
/// snapshot or the live app database** — point it at a throwaway copy.
///
/// Run it manually from `src-tauri/`:
///
/// ```text
/// cp private/realdata/brawler.sqlite3 private/realdata/a4_worktest.sqlite3
/// BRAWLER_REAL_DB=../private/realdata/a4_worktest.sqlite3 \
///   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
///   BRAWLER_A4_LIMIT=250 \
///   cargo nextest run -p brawler deterministic_pipeline_real_data_sweep \
///     --run-ignored all --no-capture
/// ```
///
/// Asserts only harness sanity (something was attempted, and the reason-code
/// vocabulary stayed typed). It deliberately pins **no** coverage floor: the
/// point of A4 is to report the real numbers, and a harness that asserts the
/// number it wants would be worse than no harness at all.
#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB (a throwaway copy) + BRAWLER_REAL_DATA_DIR"]
fn deterministic_pipeline_real_data_sweep() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP deterministic_pipeline_real_data_sweep: set BRAWLER_REAL_DB to a THROWAWAY copy \
             of the owner's database (see private/realdata/README.md)"
        );
        return;
    };
    let Ok(data_dir) = std::env::var("BRAWLER_REAL_DATA_DIR") else {
        eprintln!(
            "SKIP deterministic_pipeline_real_data_sweep: set BRAWLER_REAL_DATA_DIR to the Tauri \
             data dir holding the fetched report files"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP deterministic_pipeline_real_data_sweep: no database at {db_path}");
        return;
    }

    // Guardrail, not a convenience: this harness WRITES (facts, provenance,
    // outcomes). Running it against the master snapshot would corrupt the
    // reference copy every later measurement is compared to, and running it
    // against the live app database would corrupt the owner's real data.
    let file_name = std::path::Path::new(&db_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_owned();
    assert!(
        file_name != "brawler.sqlite3" && !db_path.starts_with("/mnt/d/"),
        "refusing to run: {db_path} is the master snapshot or the live application database. \
         This harness writes — copy it first (private/realdata/README.md)."
    );

    let derive_only = std::env::var("BRAWLER_A4_DERIVE_ONLY").as_deref() == Ok("1");
    let limit = std::env::var("BRAWLER_A4_LIMIT")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(DEFAULT_SWEEP_LIMIT);

    let connection = open_database(&db_path).expect("open throwaway real db");
    let state = AppState::with_data_dir(connection, PathBuf::from(&data_dir));
    // The witness is OFF unless explicitly enabled: a default run must be
    // offline and repeatable. With it off, "aggregator-sourced" below is 0 by
    // construction and the run measures purely how well filings are read —
    // which is the number ADR 0085's amendment asks for. `BRAWLER_A4_LIVE_WITNESS=1`
    // installs the real HTTP fetcher for a run that also exercises the fallback.
    let live_witness = std::env::var("BRAWLER_A4_LIVE_WITNESS").as_deref() == Ok("1");
    let state = if live_witness {
        state.with_fundamentals_witness_fetcher(std::sync::Arc::new(
            crate::source_adapters::biznesradar_fundamentals::HttpFundamentalsWitnessFetcher,
        ))
    } else {
        state
    };

    let definitions = state
        .financials()
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("kpi definitions");
    let metric_key_by_definition: HashMap<String, String> = definitions
        .iter()
        .map(|d| (d.id.clone(), d.metric_key.clone()))
        .collect();

    let companies = state.list_companies().expect("list companies");
    let mut tally = SweepTally::default();
    let mut slots: BTreeMap<(String, i64, String), SlotCoverage> = BTreeMap::new();
    let mut expected_by_company: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let started = std::time::Instant::now();

    eprintln!("== A4 deterministic fundamentals sweep ==");
    eprintln!("db={db_path}");
    eprintln!("data_dir={data_dir}");
    eprintln!(
        "companies={}  document cap={}",
        companies.len(),
        if limit == 0 {
            "unlimited".to_owned()
        } else {
            limit.to_string()
        }
    );

    'companies: for company in &companies {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        let expected = state
            .financials()
            .expected_primary_metric_keys(&company.id)
            .expect("expected primary keys")
            .unwrap_or_default();
        if !expected.is_empty() {
            expected_by_company.insert(company.id.clone(), expected);
        }

        // Fact ids this company's runs touched, so provenance (and therefore
        // the filing/aggregator split) can be resolved in one read per company.
        let mut touched: Vec<(String, (String, i64, String))> = Vec::new();

        for document in &documents {
            if document.fetch_status != "fetched" || document.local_path.is_none() {
                continue;
            }
            tally.eligible += 1;
            let kind = document
                .doc_kind
                .clone()
                .unwrap_or_else(|| "(unclassified)".to_owned());
            *tally.eligible_by_kind.entry(kind.clone()).or_default() += 1;
            if limit != 0 && tally.attempted >= limit {
                continue;
            }

            // The same period derivation production uses. `None` is itself a
            // measured gap (the document is stored but the pipeline can't tell
            // which period it belongs to), not a silent skip.
            let Some((fiscal_year, period_type, period_end)) =
                derive_report_period(&state, document)
            else {
                tally.no_period_derived += 1;
                *tally.no_period_by_kind.entry(kind).or_default() += 1;
                continue;
            };

            let route = if is_esef_route(document) {
                "esef_route"
            } else {
                "pdf_route"
            };
            *tally.by_route.entry(route).or_default() += 1;
            tally.attempted += 1;

            // `BRAWLER_A4_DERIVE_ONLY=1` measures the period-derivation stage
            // over the WHOLE corpus without paying for extraction (card
            // fc692da): the derivation is upstream of every tier, so its gap
            // ("stored documents the pipeline cannot place in a period") is the
            // number a widening of the grammar has to move, and a full
            // extraction sweep is an overnight run.
            if derive_only {
                continue;
            }

            let result = run_structured_extraction(
                &state,
                &company.id,
                &document.id,
                fiscal_year,
                period_type,
                &period_end,
                MODE_AUTOPILOT,
            );
            let slot = (company.id.clone(), fiscal_year, period_type.to_owned());

            match result {
                Ok(result) => {
                    let read = result.produced_fact_ids.len() + result.skipped_fact_ids.len();
                    if read > 0 {
                        *tally.route_read.entry(route).or_default() += 1;
                        tally.documents_with_facts += 1;
                        tally.facts_read_total += read;
                    }
                    *tally
                        .by_tier
                        .entry(
                            result
                                .tier
                                .map(|t| t.as_str().to_owned())
                                .unwrap_or_else(|| "none".to_owned()),
                        )
                        .or_default() += 1;
                    *tally
                        .by_acceptance
                        .entry(result.acceptance.as_str().to_owned())
                        .or_default() += 1;
                    // Produced (new) AND skipped (re-observed at an already
                    // filled slot) both mean "the pipeline read this value out
                    // of this document today" — which is what coverage asks.
                    for fact_id in result
                        .produced_fact_ids
                        .iter()
                        .chain(&result.skipped_fact_ids)
                    {
                        touched.push((fact_id.clone(), slot.clone()));
                    }
                }
                Err(error) => {
                    tally.errored += 1;
                    eprintln!(
                        "  ERROR {} {} {}: {error}",
                        company.ticker, period_end, document.id
                    );
                }
            }

            // The typed reason code is only durable on the outcome row — the
            // in-memory result carries the acceptance, not the disambiguated
            // reason (validation_failed vs structure_drift vs
            // witness_disagreement vs witness_fallback).
            if let Ok(Some(outcome)) = state
                .fundamentals_provenance()
                .get_extraction_outcome_for_slot(
                    &company.id,
                    &document.id,
                    fiscal_year,
                    period_type,
                    &period_end,
                )
            {
                *tally.by_reason.entry(outcome.reason_code).or_default() += 1;
            }

            if limit != 0 && tally.attempted >= limit {
                // Resolve this company's provenance before leaving the loop.
                resolve_touched(
                    &state,
                    &touched,
                    &metric_key_by_definition,
                    &company.id,
                    &mut slots,
                    &mut tally,
                );
                break 'companies;
            }
        }

        resolve_touched(
            &state,
            &touched,
            &metric_key_by_definition,
            &company.id,
            &mut slots,
            &mut tally,
        );
    }

    // ---- recall against the expected primary-KPI profile -------------------
    let (mut expected_slots, mut filled_slots, mut filled_by_aggregator) = (0usize, 0usize, 0usize);
    let mut slots_with_profile = 0usize;
    let mut slots_without_profile = 0usize;
    for ((company_id, _, _), coverage) in &slots {
        match expected_by_company.get(company_id) {
            Some(expected) => {
                slots_with_profile += 1;
                expected_slots += expected.len();
                filled_slots += expected.intersection(&coverage.read_keys).count();
                filled_by_aggregator += expected.intersection(&coverage.aggregator_keys).count();
            }
            None => slots_without_profile += 1,
        }
    }

    let facts_total: usize = tally.facts_by_provenance_tier.values().sum();
    let filing_facts: usize = tally
        .facts_by_provenance_tier
        .iter()
        .filter(|(tier, _)| is_filing_tier(tier))
        .map(|(_, n)| *n)
        .sum();
    let aggregator_facts: usize = tally
        .facts_by_provenance_tier
        .get("html_aggregator")
        .copied()
        .unwrap_or(0);
    let legacy_facts = facts_total - filing_facts - aggregator_facts;

    // ---- ground-truth-backed precision ------------------------------------
    let spike_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("private/realdata/spikes/espi-wdf");
    let wdf = wdf_corpus_precision(&spike_dir);

    // ---- the summary block -------------------------------------------------
    eprintln!();
    eprintln!("======================================================================");
    eprintln!("A4 SUMMARY — deterministic fundamentals pipeline on real filings");
    eprintln!("======================================================================");
    eprintln!(
        "documents eligible (fetched + stored file) : {}",
        tally.eligible
    );
    eprintln!(
        "  no period derivable                      : {}",
        tally.no_period_derived
    );
    eprintln!(
        "  attempted                                : {}",
        tally.attempted
    );
    eprintln!(
        "  hard errors                              : {}",
        tally.errored
    );
    eprintln!(
        "  elapsed                                  : {:?}",
        started.elapsed()
    );

    eprintln!();
    eprintln!("-- eligible documents by stored doc_kind (period-derivation gap) --");
    for (kind, eligible) in &tally.eligible_by_kind {
        let no_period = tally.no_period_by_kind.get(kind).copied().unwrap_or(0);
        eprintln!(
            "{kind:<20} eligible={eligible:<6} no_period={no_period:<6} ({})",
            pct(no_period, *eligible)
        );
    }

    eprintln!();
    eprintln!("-- attempts by route (what the pipeline was handed) --");
    for (route, attempted) in &tally.by_route {
        let read = tally.route_read.get(route).copied().unwrap_or(0);
        eprintln!(
            "{route:<14} attempted={attempted:<6} read_facts={read:<6} read_rate={}",
            pct(read, *attempted)
        );
    }
    eprintln!(
        "DOCUMENT-LEVEL RECALL: {}/{} attempted documents yielded >=1 tracked fact ({}), \
         {:.1} facts per reading document",
        tally.documents_with_facts,
        tally.attempted,
        pct(tally.documents_with_facts, tally.attempted),
        if tally.documents_with_facts == 0 {
            0.0
        } else {
            tally.facts_read_total as f64 / tally.documents_with_facts as f64
        },
    );

    eprintln!();
    eprintln!("-- resolved tier (which tier actually read the document) --");
    for (tier, count) in &tally.by_tier {
        eprintln!("{tier:<20} {count}");
    }

    eprintln!();
    eprintln!("-- validation gate verdict --");
    for (acceptance, count) in &tally.by_acceptance {
        eprintln!("{acceptance:<24} {count}");
    }

    eprintln!();
    eprintln!("-- flagged-gap breakdown by typed reason_code (ADR 0084 §6) --");
    for (reason, count) in &tally.by_reason {
        eprintln!("{reason:<24} {count}");
    }

    eprintln!();
    eprintln!("-- facts read, by provenance tier (ADR 0085 amendment split) --");
    for (tier, count) in &tally.facts_by_provenance_tier {
        eprintln!("{tier:<20} {count}");
    }
    eprintln!(
        "FILING-SOURCED     {filing_facts}   ({} of facts read)",
        pct(filing_facts, facts_total)
    );
    eprintln!(
        "AGGREGATOR-SOURCED {aggregator_facts}   ({} of facts read)",
        pct(aggregator_facts, facts_total)
    );
    if !live_witness {
        eprintln!(
            "  NOTE: the BiznesRadar witness is NOT installed in this run \
             (BRAWLER_A4_LIVE_WITNESS=1 enables it), so the aggregator count is 0 BY \
             CONSTRUCTION, not by measurement. Every figure above is filing-sourced."
        );
    }
    if legacy_facts > 0 {
        eprintln!(
            "legacy/unknown tier {legacy_facts}   (pre-ADR-0084 rows, neither counted above)"
        );
    }

    eprintln!();
    eprintln!("-- RECALL vs the expected primary-KPI profile --");
    if slots_with_profile == 0 {
        // Stated plainly rather than approximated with an invented denominator:
        // `expected_primary_metric_keys` derives the profile from `kpi_relevance`
        // rows ranked `primary`, and the owner has never assigned any. The
        // completeness gate (ADR 0061 dec. 4d) is therefore INERT on real data —
        // itself a finding, not a harness defect.
        eprintln!(
            "NOT MEASURABLE on this database: {slots_without_profile} slot(s) attempted, none of \
             their companies has a primary-ranked `kpi_relevance` row, so there is no expected-KPI \
             denominator. Use DOCUMENT-LEVEL RECALL above instead."
        );
        eprintln!(
            "Consequence worth reporting: the ADR 0061 dec. 4d completeness downgrade never fires \
             in production either — no `AcceptedUnreviewed` verdict above came from a hollow parse."
        );
    } else {
        eprintln!("slots attempted with a KPI profile : {slots_with_profile}");
        eprintln!("slots attempted with NO profile    : {slots_without_profile}");
        eprintln!(
            "expected KPI slots                 : {expected_slots}
filled by the pipeline             : {filled_slots}  -> recall {}",
            pct(filled_slots, expected_slots)
        );
        eprintln!("  of which aggregator-sourced      : {filled_by_aggregator}");
        eprintln!(
            "  filing-sourced recall            : {}",
            pct(filled_slots - filled_by_aggregator, expected_slots)
        );
    }

    eprintln!();
    eprintln!("-- PRECISION (ground-truth-backed ONLY) --");
    match wdf {
        Some((matched, labeled, emitted)) => eprintln!(
            "espi-wdf cover-note corpus (hand-labeled, 15 docs): \
             recall {} ({matched}/{labeled})  precision {} ({matched}/{emitted})",
            pct(matched, labeled),
            pct(matched, emitted),
        ),
        None => eprintln!("espi-wdf cover-note corpus: NOT AVAILABLE (no labels loaded)"),
    }
    eprintln!(
        "labeled report documents (BRAWLER_GROUND_TRUTH): see \
         real_data_extraction_recall_precision — separate harness"
    );
    eprintln!(
        "ALL OTHER NUMBERS ABOVE ARE PRODUCTION COUNTS, NOT PRECISION: {facts_total} facts were \
         read with no ground truth behind them, so no precision is claimed for them."
    );
    eprintln!("======================================================================");

    // Harness sanity only — deliberately NO coverage floor (see the doc comment).
    assert!(
        tally.attempted > 0,
        "no document was attempted — check BRAWLER_REAL_DATA_DIR and the database copy"
    );
    // The reason vocabulary is a contract (ADR 0084 §6): an untyped/prose reason
    // reaching the durable row would make every flagged-gap number above
    // uninterpretable, so the sweep reddens rather than reporting it silently.
    const KNOWN_REASONS: &[&str] = &[
        "emitted",
        "validation_failed",
        "structure_drift",
        "witness_disagreement",
        "witness_fallback",
        "no_deterministic_tier",
        "no_period_derived",
        "document_unreadable",
        "facts_superseded",
        "value_divergence",
    ];
    for reason in tally.by_reason.keys() {
        assert!(
            KNOWN_REASONS.contains(&reason.as_str()),
            "untyped extraction reason_code {reason:?} reached the outcome row"
        );
    }
}

/// Resolves one company's touched fact ids into `(slot -> metric keys)` plus the
/// provenance-tier histogram, in one facts read and one provenance read.
fn resolve_touched(
    state: &AppState,
    touched: &[(String, (String, i64, String))],
    metric_key_by_definition: &HashMap<String, String>,
    company_id: &str,
    slots: &mut BTreeMap<(String, i64, String), SlotCoverage>,
    tally: &mut SweepTally,
) {
    if touched.is_empty() {
        return;
    }
    let facts = state
        .financials()
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        })
        .expect("list financial facts");
    let metric_key_by_fact: HashMap<&str, &str> = facts
        .iter()
        .filter_map(|f| {
            metric_key_by_definition
                .get(&f.definition_id)
                .map(|key| (f.id.as_str(), key.as_str()))
        })
        .collect();

    let ids: Vec<String> = touched.iter().map(|(id, _)| id.clone()).collect();
    let provenance = state
        .fundamentals_provenance()
        .get_many(&ids)
        .expect("fact provenance");
    let tier_by_fact: HashMap<&str, &str> = provenance
        .iter()
        .map(|p| (p.fact_id.as_str(), p.source_tier.as_str()))
        .collect();

    for (fact_id, slot) in touched {
        let Some(metric_key) = metric_key_by_fact.get(fact_id.as_str()) else {
            continue;
        };
        let tier = tier_by_fact
            .get(fact_id.as_str())
            .copied()
            .unwrap_or("unknown");
        *tally
            .facts_by_provenance_tier
            .entry(tier.to_owned())
            .or_default() += 1;
        let entry = slots.entry(slot.clone()).or_default();
        entry.read_keys.insert((*metric_key).to_owned());
        if tier == "html_aggregator" {
            entry.aggregator_keys.insert((*metric_key).to_owned());
        }
    }
}

/// **Magic-byte container routing — the `eb71488` before/after** (card
/// `eb71488`, v0.59). Scans every stored `.pdf` document, finds the ones whose
/// BYTES are not a PDF (XML/ZIP/HTML under a `.pdf` name), and runs the new
/// container-routing extraction over them, reporting how many now produce facts
/// versus land an honest typed outcome. The "before" is fixed by the premise:
/// these documents failed 100% when the extension sent them to the PDF reader.
///
/// **Inert in CI** — skips unless `BRAWLER_REAL_DB` (a throwaway copy) and
/// `BRAWLER_REAL_DATA_DIR` are set. Writes facts/outcomes to the copy, so it
/// refuses the master snapshot and the live app database.
///
/// ```text
/// cp private/realdata/brawler.sqlite3 private/realdata/eb_worktest.sqlite3
/// BRAWLER_REAL_DB=../private/realdata/eb_worktest.sqlite3 \
///   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
///   cargo nextest run -p brawler mislabeled_container_routing_before_after \
///     --run-ignored all --no-capture
/// ```
#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB (a throwaway copy) + BRAWLER_REAL_DATA_DIR"]
fn mislabeled_container_routing_before_after() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP mislabeled_container_routing_before_after: set BRAWLER_REAL_DB to a copy");
        return;
    };
    let Ok(data_dir) = std::env::var("BRAWLER_REAL_DATA_DIR") else {
        eprintln!("SKIP mislabeled_container_routing_before_after: set BRAWLER_REAL_DATA_DIR");
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP mislabeled_container_routing_before_after: no database at {db_path}");
        return;
    }
    // Same write-guard as the A4 sweep: never the master snapshot / live db.
    assert!(
        std::path::Path::new(&db_path)
            .file_name()
            .and_then(|n| n.to_str())
            != Some("brawler.sqlite3")
            && !db_path.starts_with("/mnt/d/"),
        "refusing to run against the master snapshot or live db — copy it first"
    );

    let connection = open_database(&db_path).expect("open throwaway real db");
    let state = AppState::with_data_dir(connection, PathBuf::from(&data_dir));
    // Witness OFF: an offline, repeatable measurement of how the ROUTING alone
    // changes coverage (no network fetch, no aggregator-sourced facts).
    let companies = state.list_companies().expect("list companies");

    let mut scanned = 0usize;
    let mut mislabeled = 0usize;
    let mut by_route: BTreeMap<&'static str, usize> = BTreeMap::new();
    let mut periodic_mislabeled = 0usize;
    let mut now_produce_facts = 0usize;
    let mut honest_outcome = 0usize;
    let mut no_period = 0usize;

    eprintln!("== eb71488 magic-byte container routing: before/after ==");
    eprintln!("db={db_path}");

    for company in &companies {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        for document in &documents {
            if document.fetch_status != "fetched" {
                continue;
            }
            let Some(local_path) = document.local_path.as_deref() else {
                continue;
            };
            if !local_path.to_ascii_lowercase().ends_with(".pdf") {
                continue;
            }
            let Ok(bytes) = std::fs::read(state.data_dir().join(local_path)) else {
                continue;
            };
            scanned += 1;
            let route = route_document(&bytes);
            if route == DocumentRoute::Pdf {
                continue; // a genuine PDF — routes exactly as before, no change.
            }
            mislabeled += 1;
            let route_name = match route {
                DocumentRoute::Positional => "positional",
                DocumentRoute::IxbrlInstance => "ixbrl_instance",
                DocumentRoute::ZipPackage => "zip_package",
                DocumentRoute::Unsupported(c) => c.as_str(),
                DocumentRoute::Pdf => unreachable!(),
            };
            *by_route.entry(route_name).or_default() += 1;

            let kind = document.doc_kind.as_deref().unwrap_or("(unclassified)");
            let periodic = matches!(kind, "periodic_ssf" | "periodic_jsf");
            if periodic {
                periodic_mislabeled += 1;
            }

            let Some((fiscal_year, period_type, period_end)) =
                derive_report_period(&state, document)
            else {
                no_period += 1;
                eprintln!(
                    "  [{route_name:>14}] kind={kind:<14} NO PERIOD  doc={}",
                    document.id
                );
                continue;
            };

            let result = run_structured_extraction(
                &state,
                &company.id,
                &document.id,
                fiscal_year,
                period_type,
                &period_end,
                MODE_AUTOPILOT,
            );
            match result {
                Ok(r) if !r.produced_fact_ids.is_empty() || !r.skipped_fact_ids.is_empty() => {
                    now_produce_facts += 1;
                    eprintln!(
                        "  [{route_name:>14}] kind={kind:<14} FACTS tier={:?} produced={} reobserved={}  {} {}",
                        r.tier,
                        r.produced_fact_ids.len(),
                        r.skipped_fact_ids.len(),
                        company.ticker,
                        period_end
                    );
                }
                Ok(r) => {
                    honest_outcome += 1;
                    eprintln!(
                        "  [{route_name:>14}] kind={kind:<14} NO-FACTS acceptance={} tier={:?}  {} {}",
                        r.acceptance.as_str(),
                        r.tier,
                        company.ticker,
                        period_end
                    );
                }
                Err(err) => {
                    honest_outcome += 1;
                    eprintln!("  [{route_name:>14}] kind={kind:<14} OUTCOME {err}");
                }
            }
        }
    }

    eprintln!("---");
    eprintln!("scanned .pdf documents:        {scanned}");
    eprintln!("mislabeled (bytes != PDF):     {mislabeled}");
    eprintln!("  by route: {by_route:?}");
    eprintln!("periodic among mislabeled:     {periodic_mislabeled}");
    eprintln!("  -> now produce facts:        {now_produce_facts}");
    eprintln!("  -> honest typed outcome:     {honest_outcome}");
    eprintln!("  -> no derivable period:      {no_period}");

    // Harness sanity only (no floor): the corpus MUST contain mislabeled files
    // (this is the whole premise of the card); the assertion catches a scan that
    // silently read nothing.
    assert!(
        scanned > 0,
        "no .pdf documents were scanned — check BRAWLER_REAL_DATA_DIR"
    );
    assert!(
        mislabeled > 0,
        "expected mislabeled containers in the corpus (card eb71488 measured 45; 19 periodic)"
    );
}

// ===========================================================================
// #182 — ESEF / positional ground-truth scorer (DIAGNOSTIC — floors deferred
// to measurement v2, 2026-08-05 methodology audit)
// ===========================================================================
//
// Exhaustive, basis-aware precision/recall over the hand-labeled #182 corpus
// (private/realdata/spikes/esef-positional-gt/, HANDOVER.md): 32 real report
// documents (15 ESEF, 17 html_positional, CDR-dominated per the corpus's own
// `honest_limitations` note) against the corpus's merged ground-truth rows
// (label_esef.py's independent iXBRL parser for the ESEF tier; two
// independent LLM passes reconciled for the positional tier — pass1 x pass2
// agreement -> `machine`, disagreement or a single pass -> `needs_owner`).
// Exact row/verified counts drift with the corpus and are NOT pinned here —
// read them from the generated `scoring-report.json` (or the printed summary
// this test emits) rather than a stale comment. Committed measurement table +
// audit-verdict summary: docs/testing.md § "#182 ESEF / positional
// ground-truth scorer".
//
// ADR 0095: the html_positional/`pdf`-tier extraction route is RETIRED and
// migration 0135 deletes every stored `pdf`-tier fact. The positional arm
// below scores against a permanently-empty app-side set — its GT rows stay
// historical evidence (never re-labeled). What it asserts unconditionally is
// that the retirement took effect on real, production-shaped data (zero
// `pdf`-tier facts remain in the scored DB) — a stored-state auditor, not a
// ratchet.
//
// Unlike every harness above, this one never runs the pipeline: it scores
// facts ALREADY STORED in the database against the labels, via the real read
// models (`FinancialsStore::list_financial_facts` +
// `FundamentalsProvenanceStore::get_many`) — the #182 question is "how good
// is what's actually on the owner's database today", not "what would a fresh
// run produce".
//
// **Inert in CI** — skips with a printed message unless the corpus dir (or
// `BRAWLER_GT_DIR`) and `ground_truth.json` are both present. Writes nothing
// to the corpus's `db-snapshot.sqlite3`: it copies to a throwaway
// `scoring-worktest.sqlite3` first and opens ONLY the copy through the normal
// `open_database` path, so every pending migration (0134 tier/method repair,
// 0135 December year-shift repair) applies to the copy and the snapshot stays
// a clean reference forever.
//
// **Refinement round (three metric-design artifacts the raw first pass
// conflated with real pipeline gaps — fixed here, not just noted):**
//
// 1. **Key scope.** Only metric keys in the intersection of the GT labeler's
//    covered set (`GT182_COVERED_BASE_KEYS` — the 15 mapped concepts) and the
//    app's canonical keys are scored. An app fact for a key outside that set
//    (e.g. `long_term_debt`, which the corpus never labels at all) is
//    excluded entirely and tallied `out_of_gt_scope` — never `SPURIOUS`.
// 2. **Variant translation** (`gt182_app_comparable_key`). `label_esef.py`'s
//    `CONCEPT_MAP` deliberately keeps `ifrs-full:ProfitLoss` /
//    `ifrs-full:Equity` as `net_profit__profitloss` / `total_equity__equity`
//    rather than collapsing them at label time ("so both variants are
//    visible in the proposed labels"); the app's own ESEF extractor
//    (`fundamentals/extraction/esef.rs`: `"ProfitLoss" => "net_profit"`,
//    `"Equity" => "total_equity"`) maps the SAME bare concepts to the SAME
//    app keys, so those two variants translate to `net_profit` /
//    `total_equity` at score time. `net_profit__owners` /
//    `total_equity__owners` (`ifrs-full:…AttributableToOwnersOfParent`) have
//    no app-side counterpart at all — the app has no NCI-split KPI — so they
//    are NOT app-claimable: tallied `app_has_no_concept`, excluded from both
//    denominators, never scored as MISSING.
// 3. **Period scope.** The recall/precision denominator is the document's
//    CURRENT period only (`MANIFEST.json`'s own `period_end` for that
//    document — the period the pipeline was asked to extract). GT rows at a
//    comparative period (the prior-year column an ESEF/positional statement
//    also carries) go to a `comparative_coverage` observation bucket per
//    tier instead — `machine`-verified rows only, split stored-vs-not — since
//    whether the app ALSO retains the comparative column is a genuinely
//    different question from whether it read the current filing correctly.
//    `SPURIOUS` is scoped the same way: an app fact only counts against
//    precision when its OWN period is the document's current period.
//
// Basis stays part of the match key throughout (HANDOVER.md: DBC x2, CAR,
// DNP and CDR's "for_2024" filing are STANDALONE, not consolidated), near-
// misses (`< 0.5%` relative diff) stay an evidence-only annotation on
// `WRONG_VALUE`, and `needs_owner`/`uncertain` GT rows stay excluded from
// every denominator (`UNVERIFIED`) — none of that changed.

/// One row of `ground_truth.json` (schema fixed by the #182 contract).
#[derive(Debug, Deserialize)]
struct Gt182Row {
    file: String,
    ticker: String,
    /// `esef` | `positional`.
    tier: String,
    /// Translated to the app's canonical `financial_facts.metric_key` via
    /// `gt182_app_comparable_key` at score time — see the module doc comment
    /// (refinement 2) for the `__profitloss`/`__equity`/`__owners` handling.
    mapped_key: String,
    period_end: String,
    #[serde(default)]
    #[allow(dead_code)] // carried through to the evidence JSON, not part of the match key
    period_start: Option<String>,
    statement_basis: String,
    value: String,
    currency: String,
    source: String,
    /// `machine` (independent-parser / pass1 x pass2 agreement) | `needs_owner`.
    verification: String,
    uncertain: bool,
}

/// `ground_truth.json` may be a bare array (like the `proposed_labels_*.json`
/// intermediates it is merged from) or `{"rows": [...]}` — accept either
/// rather than guess which the merge step settled on.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Gt182File {
    Bare(Vec<Gt182Row>),
    Wrapped { rows: Vec<Gt182Row> },
}

/// One `MANIFEST.json` document entry — only the fields the scorer needs;
/// serde ignores the rest (sha256, byte counts, validation_status_counts…).
#[derive(Debug, Deserialize)]
struct Gt182ManifestDoc {
    /// == `report_documents.id` == `financial_facts.source_document_ref`.
    document_id: String,
    file_name: String,
    /// The period the pipeline was asked to extract for this document — the
    /// PERIOD SCOPE anchor (refinement 3): a GT row at this `period_end` is
    /// "current"; any other `period_end` for the same document is
    /// "comparative".
    period_end: String,
}

#[derive(Debug, Deserialize)]
struct Gt182Manifest {
    esef_tier: Vec<Gt182ManifestDoc>,
    positional_tier: Vec<Gt182ManifestDoc>,
}

/// The GT labeler's covered concept set (task contract, refinement 1) — the
/// 15 mapped concepts `label_esef.py`/the positional LLM passes both target.
/// Any app fact whose `metric_key` is outside this set (e.g. `long_term_debt`)
/// is out of the corpus's scope entirely, not a scoring gap.
const GT182_COVERED_BASE_KEYS: &[&str] = &[
    "revenue",
    "gross_profit",
    "operating_profit",
    "net_profit",
    "eps_basic",
    "eps_diluted",
    "operating_cash_flow",
    "investing_cash_flow",
    "financing_cash_flow",
    "total_assets",
    "current_assets",
    "current_liabilities",
    "total_liabilities",
    "total_equity",
    "cash",
];

/// Translates a ground-truth `mapped_key` to the app's canonical metric key
/// (refinement 2), or `None` when the concept has no app-side counterpart at
/// all. `net_profit__profitloss` / `total_equity__equity` collapse to the
/// bare `net_profit` / `total_equity` app keys (the app's ESEF extractor maps
/// the SAME underlying `ifrs-full:ProfitLoss` / `ifrs-full:Equity` concepts
/// there); `net_profit__owners` / `total_equity__owners`
/// (`…AttributableToOwnersOfParent`) are NOT app-claimable — the app has no
/// NCI-split KPI — and return `None`.
fn gt182_app_comparable_key(mapped_key: &str) -> Option<&str> {
    match mapped_key {
        "net_profit__profitloss" => Some("net_profit"),
        "total_equity__equity" => Some("total_equity"),
        "net_profit__owners" | "total_equity__owners" => None,
        other if GT182_COVERED_BASE_KEYS.contains(&other) => Some(other),
        _ => None,
    }
}

/// `(report_documents.id, metric_key, period_end, statement_basis)` — the
/// exhaustive match key. `statement_basis` is part of the key on purpose:
/// per HANDOVER.md, DBC (x2), CAR, DNP and CDR's "for_2024" filing are
/// STANDALONE statements, and collapsing basis would silently pair a
/// consolidated app fact with a standalone label or vice versa.
type Gt182Key = (String, String, String, String);

#[derive(Debug, Clone)]
struct Gt182AppValue {
    value: Decimal,
    currency: Option<String>,
    ticker: String,
    fact_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gt182ScoreRow {
    tier: String,
    ticker: String,
    document_id: String,
    metric_key: String,
    period_end: String,
    statement_basis: String,
    app_value: Option<String>,
    gt_value: Option<String>,
    app_currency: Option<String>,
    gt_currency: Option<String>,
    /// `MATCH` | `WRONG_VALUE` | `MISSING` | `SPURIOUS` | `CURRENCY_MISSING` |
    /// `CURRENCY_MISMATCH` | `UNVERIFIED` (current-period, in-scope,
    /// denominator-affecting) | `APP_HAS_NO_CONCEPT` (refinement 2, `__owners`
    /// variants) | `COMPARATIVE` (refinement 3, non-current period —
    /// informational only, see `comparative_*` on the tier aggregate).
    verdict: String,
    /// `WRONG_VALUE` whose relative difference is `< 0.5%` — informational
    /// only; the verdict itself uses exact decimal equality (tolerance 0).
    near_miss: bool,
    gt_source: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gt182TierAggregate {
    tier: String,
    match_count: usize,
    wrong_value_count: usize,
    missing_count: usize,
    spurious_count: usize,
    /// Value-equal to the GT row, but the app fact carries NO currency at
    /// all (`financial_facts.currency` is `NULL`) — a distinct failure from
    /// `CURRENCY_MISMATCH`.
    currency_missing_count: usize,
    /// Value-equal to the GT row, but the app fact's currency differs from
    /// the GT row's currency.
    currency_mismatch_count: usize,
    unverified_count: usize,
    /// `MATCH / (MATCH + WRONG_VALUE + SPURIOUS + CURRENCY_MISSING + CURRENCY_MISMATCH)`.
    precision: Option<f64>,
    /// `MATCH / (MATCH + WRONG_VALUE + MISSING + CURRENCY_MISSING + CURRENCY_MISMATCH)`.
    recall: Option<f64>,
    /// Denominator behind `precision` (0 when `precision` is `None`) —
    /// counts-first reporting: every ratio is printed as `numerator/denominator`.
    precision_denominator: usize,
    /// Denominator behind `recall` (0 when `recall` is `None`).
    recall_denominator: usize,
    /// Refinement 1: app facts whose `metric_key` is outside
    /// `GT182_COVERED_BASE_KEYS` — excluded entirely, never `SPURIOUS`.
    out_of_gt_scope_count: usize,
    /// Refinement 2: GT rows whose concept the app cannot claim at all
    /// (`net_profit__owners` / `total_equity__owners`) — excluded from both
    /// denominators.
    app_has_no_concept_count: usize,
    /// Refinement 3: `machine`-verified, in-scope GT rows at a comparative
    /// (non-current) period — informational, excluded from both denominators.
    comparative_total: usize,
    /// Of `comparative_total`, how many the app ALSO has a fact for at that
    /// exact (document, metric, period, basis) key — existence only, not a
    /// value check.
    comparative_stored: usize,
    /// Of `comparative_total`, how many the app has no fact for at all.
    comparative_missing: usize,
    /// Counts-first reporting (audit): per-ticker breakdown so a
    /// concentrated tier (e.g. positional is 12/17 CD Projekt documents)
    /// doesn't hide behind one tier-wide number. Sorted by ticker.
    ticker_counts: Vec<Gt182TickerCounts>,
}

/// Per-ticker verdict counts within one tier — see `Gt182TierAggregate::ticker_counts`.
#[derive(Debug, Default, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Gt182TickerCounts {
    ticker: String,
    match_count: usize,
    wrong_value_count: usize,
    missing_count: usize,
    spurious_count: usize,
    currency_missing_count: usize,
    currency_mismatch_count: usize,
    unverified_count: usize,
}

/// Per-tier accumulator for the three refinement observation buckets, built
/// while indexing app facts / grouping GT rows, then folded into the tier's
/// [`Gt182TierAggregate`] and evidence rows after [`gt182_score_tier`] runs.
#[derive(Debug, Default)]
struct Gt182SideBuckets {
    out_of_gt_scope: usize,
    app_has_no_concept: usize,
    comparative_total: usize,
    comparative_stored: usize,
    comparative_missing: usize,
    /// `APP_HAS_NO_CONCEPT` + `COMPARATIVE` evidence rows for this tier.
    extra_rows: Vec<Gt182ScoreRow>,
}

#[derive(Debug, Serialize)]
struct Gt182Report {
    generated_at_unix: u64,
    gt_dir: String,
    /// Ground-truth rows whose `file` did not resolve via `MANIFEST.json`
    /// (excluded from scoring — a corpus-metadata mismatch, not a verdict).
    unresolved_gt_rows: usize,
    rows: Vec<Gt182ScoreRow>,
    aggregates: Vec<Gt182TierAggregate>,
}

/// Relative difference `|actual - expected| / max(|actual|, |expected|)`,
/// the same scale convention `tolerance_accepts` above uses. Used ONLY for
/// the `near_miss` evidence annotation — the match verdict is exact equality.
fn gt182_relative_diff(actual: Decimal, expected: Decimal) -> Decimal {
    let scale = actual.abs().max(expected.abs());
    if scale.is_zero() {
        Decimal::ZERO
    } else {
        (actual - expected).abs() / scale
    }
}

/// PERIOD SCOPE (refinement 3): the subset of `app_facts` whose OWN period
/// (the key's `period_end`) is the document's current period per
/// `doc_current_period_end` — the only app facts a `SPURIOUS` verdict can be
/// charged against. A comparative-period app fact is real (and used by the
/// `comparative_coverage` observation bucket), just never a precision hit.
fn gt182_current_period_only(
    app_facts: &BTreeMap<Gt182Key, Gt182AppValue>,
    doc_current_period_end: &HashMap<String, String>,
) -> BTreeMap<Gt182Key, Gt182AppValue> {
    app_facts
        .iter()
        .filter(|(key, _)| {
            let (doc_id, _, period_end, _) = key;
            doc_current_period_end.get(doc_id).map(String::as_str) == Some(period_end.as_str())
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// Scores one tier exhaustively over the UNION of ground-truth keys and
/// app-fact keys, per the #182 verdict contract. Both inputs are ALREADY
/// scoped by the caller to the current-period, app-claimable slots
/// (refinements 1-3: key scope, variant translation, period scope) — this
/// function only decides MATCH/WRONG_VALUE/MISSING/SPURIOUS/CURRENCY_MISSING/
/// CURRENCY_MISMATCH/UNVERIFIED within that scope; the
/// `out_of_gt_scope`/`app_has_no_concept`/`comparative_*` buckets are
/// populated by the caller, not here.
///
/// - `UNVERIFIED` — the GT row is `needs_owner` or `uncertain` (excluded from
///   both denominators, listed for owner arbitration).
/// - `MATCH` — a `machine`-verified GT row whose app fact has the SAME value
///   AND the SAME currency (audit blocker, currency-aware matching).
/// - `CURRENCY_MISSING` / `CURRENCY_MISMATCH` — value-equal to the GT row,
///   but the app fact's currency is `NULL` / present-but-different. Counted
///   in both denominators (a value-only match is not a full match) but never
///   in `MATCH` itself.
/// - `WRONG_VALUE` — a `machine`-verified GT row with a present, value-
///   differing app fact.
/// - `MISSING` — a `machine`-verified GT row with no app fact at that key
///   (recall hit).
/// - `SPURIOUS` — an app fact at a key with NO ground-truth row at all,
///   machine or `needs_owner` (precision hit).
fn gt182_score_tier(
    tier: &str,
    gt_rows: &BTreeMap<Gt182Key, &Gt182Row>,
    app_facts: &BTreeMap<Gt182Key, Gt182AppValue>,
) -> (Vec<Gt182ScoreRow>, Gt182TierAggregate) {
    let near_threshold = Decimal::new(5, 3); // 0.005 = 0.5%
    let mut keys: BTreeSet<Gt182Key> = gt_rows.keys().cloned().collect();
    keys.extend(app_facts.keys().cloned());

    let mut rows = Vec::with_capacity(keys.len());
    let mut agg = Gt182TierAggregate {
        tier: tier.to_owned(),
        ..Default::default()
    };
    let mut ticker_counts: BTreeMap<String, Gt182TickerCounts> = BTreeMap::new();

    for key in keys {
        let (document_id, metric_key, period_end, statement_basis) = key.clone();
        let gt = gt_rows.get(&key).copied();
        let app = app_facts.get(&key);
        let ticker = app
            .map(|a| a.ticker.clone())
            .or_else(|| gt.map(|g| g.ticker.clone()))
            .unwrap_or_default();

        let (verdict, near_miss) = match gt {
            Some(gt_row) if gt_row.verification != "machine" || gt_row.uncertain => {
                ("UNVERIFIED", false)
            }
            Some(gt_row) => {
                let expected = Decimal::from_str(&gt_row.value).unwrap_or_else(|e| {
                    panic!(
                        "bad ground-truth decimal '{}' for {} {}: {e}",
                        gt_row.value, gt_row.ticker, gt_row.mapped_key
                    )
                });
                match app {
                    // CURRENCY-AWARE MATCHING (audit blocker): a value match
                    // alone is not enough — currency must agree too, or the
                    // two "equal" numbers may be in different units.
                    Some(app_val) if app_val.value == expected => match &app_val.currency {
                        Some(app_currency) if app_currency == &gt_row.currency => ("MATCH", false),
                        Some(_different) => ("CURRENCY_MISMATCH", false),
                        None => ("CURRENCY_MISSING", false),
                    },
                    Some(app_val) => {
                        let near = gt182_relative_diff(app_val.value, expected) < near_threshold;
                        ("WRONG_VALUE", near)
                    }
                    None => ("MISSING", false),
                }
            }
            None => {
                // Union-of-keys invariant: a key absent from `gt_rows` was
                // only added because it is present in `app_facts`.
                debug_assert!(app.is_some(), "key with neither a gt row nor an app fact");
                ("SPURIOUS", false)
            }
        };

        let ticker_entry =
            ticker_counts
                .entry(ticker.clone())
                .or_insert_with(|| Gt182TickerCounts {
                    ticker: ticker.clone(),
                    ..Default::default()
                });
        match verdict {
            "MATCH" => {
                agg.match_count += 1;
                ticker_entry.match_count += 1;
            }
            "WRONG_VALUE" => {
                agg.wrong_value_count += 1;
                ticker_entry.wrong_value_count += 1;
            }
            "MISSING" => {
                agg.missing_count += 1;
                ticker_entry.missing_count += 1;
            }
            "SPURIOUS" => {
                agg.spurious_count += 1;
                ticker_entry.spurious_count += 1;
            }
            "CURRENCY_MISSING" => {
                agg.currency_missing_count += 1;
                ticker_entry.currency_missing_count += 1;
            }
            "CURRENCY_MISMATCH" => {
                agg.currency_mismatch_count += 1;
                ticker_entry.currency_mismatch_count += 1;
            }
            "UNVERIFIED" => {
                agg.unverified_count += 1;
                ticker_entry.unverified_count += 1;
            }
            _ => unreachable!("exhaustive verdict match above"),
        }

        rows.push(Gt182ScoreRow {
            tier: tier.to_owned(),
            ticker,
            document_id,
            metric_key,
            period_end,
            statement_basis,
            app_value: app.map(|a| a.value.to_string()),
            gt_value: gt.map(|g| g.value.clone()),
            app_currency: app.and_then(|a| a.currency.clone()),
            gt_currency: gt.map(|g| g.currency.clone()),
            verdict: verdict.to_owned(),
            near_miss,
            gt_source: gt.map(|g| g.source.clone()),
        });
    }

    rows.sort_by(|a, b| {
        (
            a.ticker.as_str(),
            a.metric_key.as_str(),
            a.period_end.as_str(),
        )
            .cmp(&(
                b.ticker.as_str(),
                b.metric_key.as_str(),
                b.period_end.as_str(),
            ))
    });

    // Currency-verdict keys are "an app fact was produced, but it doesn't
    // fully match" — they count in BOTH denominators, same as WRONG_VALUE:
    // a real GT row was there to recall, and a real app assertion was made
    // to grade for precision.
    let denom_precision = agg.match_count
        + agg.wrong_value_count
        + agg.spurious_count
        + agg.currency_missing_count
        + agg.currency_mismatch_count;
    let denom_recall = agg.match_count
        + agg.wrong_value_count
        + agg.missing_count
        + agg.currency_missing_count
        + agg.currency_mismatch_count;
    agg.precision_denominator = denom_precision;
    agg.recall_denominator = denom_recall;
    agg.precision = (denom_precision > 0).then(|| agg.match_count as f64 / denom_precision as f64);
    agg.recall = (denom_recall > 0).then(|| agg.match_count as f64 / denom_recall as f64);
    agg.ticker_counts = ticker_counts.into_values().collect();

    (rows, agg)
}

/// **#182 B2 — the ground-truth scorer, a DIAGNOSTIC, not a ratchet.** See
/// the module doc comment above for the full contract. **Inert in CI** —
/// skips (loudly) unless the corpus dir, `ground_truth.json`, and
/// `db-snapshot.sqlite3` are present.
///
/// Run it manually from `src-tauri/`, or via `make realdata-gt-score`:
///
/// ```text
/// cargo test esef_positional_ground_truth_scores -- --ignored --nocapture
/// ```
///
/// **Status (2026-08-05 methodology audit): floors deferred to measurement
/// v2.** This scorer grades facts ALREADY STORED in a DB snapshot — a
/// parser change does NOT move these numbers, only a fresh corpus/extraction
/// run would — so pinning a regression floor on stored-state numbers would
/// have gated the wrong thing. The audit also found the corpus is output-
/// conditioned (GT rows only exist for what the pipeline already extracted),
/// currency-blind (fixed here — see `CURRENCY_MISSING`/`CURRENCY_MISMATCH`),
/// CDR-concentrated in the positional tier, and scored per-slot rather than
/// per-document. See the "#182 is a DIAGNOSTIC" comment at the bottom of
/// this function for the full list.
///
/// **Policy:** asserts harness sanity only (ground truth non-empty, both
/// tiers scored at least one key, scoring report written) — no precision/
/// recall floor. `BRAWLER_GT_REQUIRED=1` (`make realdata-gt-check`, the
/// owner-only closure gate) turns every "corpus absent" skip, a zero-GT-rows-
/// at-current-period manifest document, and a zero-denominator tier into a
/// hard failure instead of a loud warning; unset, all three stay loud but
/// non-failing (default local run / CI, which never sees the private corpus,
/// ADR 0091).
#[test]
#[ignore = "requires the owner's private #182 corpus"]
fn esef_positional_ground_truth_scores() {
    // `BRAWLER_GT_REQUIRED=1` — the owner-only closure gate (`make
    // realdata-gt-check`): turns every "SKIP, corpus absent" path below, the
    // zero-GT-rows-at-current-period manifest guard, and the zero-denominator
    // check further down into a hard panic instead of a loud warning. Off by
    // default (and in CI, which never sees the private corpus, ADR 0091) so
    // the plain `make realdata-gt-score` run stays a diagnostic, non-gating
    // report (this harness has no precision/recall floor — see the function
    // doc comment).
    let required = std::env::var("BRAWLER_GT_REQUIRED")
        .map(|v| v == "1")
        .unwrap_or(false);
    macro_rules! skip_or_require {
        ($($arg:tt)*) => {{
            let msg = format!($($arg)*);
            if required {
                panic!("REQUIRED esef_positional_ground_truth_scores: {msg}");
            }
            eprintln!("SKIP esef_positional_ground_truth_scores: {msg}");
            return;
        }};
    }

    let gt_dir = std::env::var("BRAWLER_GT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/spikes/esef-positional-gt")
        });
    if !gt_dir.is_dir() {
        skip_or_require!(
            "no corpus dir at {} \
             (set BRAWLER_GT_DIR — this harness is inert without the owner's private #182 corpus)",
            gt_dir.display()
        );
    }

    let gt_path = gt_dir.join("ground_truth.json");
    let Ok(raw) = std::fs::read_to_string(&gt_path) else {
        skip_or_require!(
            "no ground_truth.json at {} \
             (the #182 label-merge job may not have produced it yet)",
            gt_path.display()
        );
    };
    let gt_rows: Vec<Gt182Row> = match serde_json::from_str::<Gt182File>(&raw) {
        Ok(Gt182File::Bare(rows)) => rows,
        Ok(Gt182File::Wrapped { rows }) => rows,
        Err(err) => panic!(
            "ground_truth.json at {} failed to parse: {err}",
            gt_path.display()
        ),
    };

    let manifest_path = gt_dir.join("MANIFEST.json");
    let manifest_raw = std::fs::read_to_string(&manifest_path).unwrap_or_else(|e| {
        panic!(
            "MANIFEST.json missing at {} despite ground_truth.json being present \
             (corpus integrity bug, not an env-gate case): {e}",
            manifest_path.display()
        )
    });
    let manifest: Gt182Manifest = serde_json::from_str(&manifest_raw).unwrap_or_else(|e| {
        panic!(
            "MANIFEST.json at {} failed to parse: {e}",
            manifest_path.display()
        )
    });

    // Adversarial-review should-fix #8: a duplicate `file_name` or
    // `document_id` in MANIFEST.json (within a tier, or reused across the
    // two tiers) would silently overwrite an earlier corpus document's
    // mapping in the maps built below — a real corpus-integrity bug, not an
    // "absent corpus" case, so it is a hard failure regardless of
    // `BRAWLER_GT_REQUIRED`, exactly like the "MANIFEST.json missing" panic
    // above.
    {
        let mut seen_files: HashMap<&str, &str> = HashMap::new();
        let mut seen_doc_ids: HashMap<&str, &str> = HashMap::new();
        let mut dup_errors: Vec<String> = Vec::new();
        for (tier_name, docs) in [
            ("esef_tier", &manifest.esef_tier),
            ("positional_tier", &manifest.positional_tier),
        ] {
            for doc in docs {
                if let Some(prev_tier) = seen_files.insert(doc.file_name.as_str(), tier_name) {
                    dup_errors.push(format!(
                        "duplicate MANIFEST file_name '{}': already seen in {prev_tier}, \
                         reused in {tier_name}",
                        doc.file_name
                    ));
                }
                if let Some(prev_tier) = seen_doc_ids.insert(doc.document_id.as_str(), tier_name) {
                    dup_errors.push(format!(
                        "duplicate MANIFEST document_id '{}': already seen in {prev_tier}, \
                         reused in {tier_name}",
                        doc.document_id
                    ));
                }
            }
        }
        assert!(
            dup_errors.is_empty(),
            "MANIFEST.json at {} has duplicate filenames/document ids (corpus integrity bug, \
             not an env-gate case):\n  {}",
            manifest_path.display(),
            dup_errors.join("\n  ")
        );
    }

    let mut file_to_doc_id: HashMap<String, String> = HashMap::new();
    let mut esef_doc_ids: BTreeSet<String> = BTreeSet::new();
    let mut positional_doc_ids: BTreeSet<String> = BTreeSet::new();
    // PERIOD SCOPE anchor (refinement 3): the period the pipeline was asked
    // to extract for each corpus document — a GT row at this period is
    // "current", any other period for the same document is "comparative".
    let mut doc_current_period_end: HashMap<String, String> = HashMap::new();
    for doc in &manifest.esef_tier {
        file_to_doc_id.insert(doc.file_name.clone(), doc.document_id.clone());
        esef_doc_ids.insert(doc.document_id.clone());
        doc_current_period_end.insert(doc.document_id.clone(), doc.period_end.clone());
    }
    for doc in &manifest.positional_tier {
        file_to_doc_id.insert(doc.file_name.clone(), doc.document_id.clone());
        positional_doc_ids.insert(doc.document_id.clone());
        doc_current_period_end.insert(doc.document_id.clone(), doc.period_end.clone());
    }
    let all_corpus_doc_ids: BTreeSet<String> =
        esef_doc_ids.union(&positional_doc_ids).cloned().collect();

    // ---- MANIFEST/corpus period-agreement guard (audit blocker, #182) -----
    // A document whose MANIFEST.json `period_end` disagrees with the corpus's
    // OWN ground-truth labeling (every GT row for it lands at some OTHER
    // period) silently contributes ZERO current-period signal — exactly the
    // DNP bug found in the 2026-08-05 audit (period_end was 2026-12-31, a
    // year-shift bug also repaired in the DB by migration 0135; every one of
    // DNP's 28 GT rows fell into the comparative bucket instead of scoring).
    // Count GT rows per document at the document's DECLARED current period —
    // untranslated, unfiltered by verification — a manifest/corpus
    // consistency check, not a scoring-refinement one.
    let mut current_period_gt_rows_by_doc: HashMap<&str, usize> = HashMap::new();
    for row in &gt_rows {
        let Some(doc_id) = file_to_doc_id.get(&row.file) else {
            continue;
        };
        if doc_current_period_end
            .get(doc_id.as_str())
            .map(String::as_str)
            == Some(row.period_end.as_str())
        {
            *current_period_gt_rows_by_doc
                .entry(doc_id.as_str())
                .or_insert(0) += 1;
        }
    }
    for doc_id in &all_corpus_doc_ids {
        let count = current_period_gt_rows_by_doc
            .get(doc_id.as_str())
            .copied()
            .unwrap_or(0);
        if count == 0 {
            let msg = format!(
                "manifest document {doc_id} has ZERO ground-truth rows at its declared current \
                 period {} — likely a MANIFEST.json period_end / corpus-labeling disagreement \
                 (see the #182 DNP fix, 2026-08-05); every GT row for this document is silently \
                 falling into the comparative bucket instead of scoring",
                doc_current_period_end
                    .get(doc_id.as_str())
                    .map(String::as_str)
                    .unwrap_or("?")
            );
            if required {
                panic!("REQUIRED esef_positional_ground_truth_scores: {msg}");
            }
            eprintln!("WARN esef_positional_ground_truth_scores: {msg}");
        }
    }

    // ---- copy db-snapshot.sqlite3 -> a throwaway worktest copy -----------
    // NEVER open db-snapshot.sqlite3 itself: it is the immutable reference
    // every future scoring run is compared against.
    let snapshot_path = gt_dir.join("db-snapshot.sqlite3");
    if !snapshot_path.is_file() {
        skip_or_require!("no db-snapshot.sqlite3 at {}", gt_dir.display());
    }
    // Adversarial-review should-fix #8: a FIXED `scoring-worktest.sqlite3`
    // path inside the shared corpus directory means two concurrent runs (or
    // a crashed prior run's leftover WAL/SHM) can collide or cross-
    // contaminate each other. Each run gets its OWN scratch directory in the
    // system temp dir, named with the process id + a monotonic timestamp —
    // never written into `gt_dir` at all. Best-effort cleanup at the end;
    // leftovers are inert scratch files, never the corpus itself.
    let run_id = format!(
        "{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let worktest_dir = std::env::temp_dir().join(format!("brawler-gt182-worktest-{run_id}"));
    std::fs::create_dir_all(&worktest_dir).unwrap_or_else(|e| {
        panic!(
            "create worktest scratch dir {}: {e}",
            worktest_dir.display()
        )
    });
    let worktest_path = worktest_dir.join("scoring-worktest.sqlite3");
    std::fs::copy(&snapshot_path, &worktest_path)
        .unwrap_or_else(|e| panic!("copy db-snapshot.sqlite3 -> scoring-worktest.sqlite3: {e}"));
    for suffix in ["-wal", "-shm"] {
        let src = PathBuf::from(format!("{}{suffix}", snapshot_path.display()));
        let dst = PathBuf::from(format!("{}{suffix}", worktest_path.display()));
        if src.is_file() {
            std::fs::copy(&src, &dst).unwrap_or_else(|e| panic!("copy {suffix} sidecar: {e}"));
        }
        // A unique per-run scratch dir never has a stale sidecar to clear.
    }

    // Opens the COPY; `open_database` applies every pending migration
    // (incl. 0134/0135 in this working tree) to it.
    let connection = open_database(&worktest_path).unwrap_or_else(|e| {
        panic!("open scoring-worktest.sqlite3 (migrations must apply cleanly to the copy): {e}")
    });

    // ADR 0095 stored-state audit: the html_positional/`pdf`-tier extraction
    // route is retired and migration 0135 deletes every stored `pdf`-tier
    // fact — unconditionally, on every real DB, not just this corpus's own
    // documents. This is a correctness invariant about the migration, not a
    // diagnostic about extraction quality, so it is asserted regardless of
    // `BRAWLER_GT_REQUIRED`.
    let remaining_pdf_tier_facts: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM financial_facts f \
             LEFT JOIN financial_fact_provenance p ON p.fact_id = f.id \
             WHERE f.extraction_method = 'html_positional' OR p.source_tier = 'pdf'",
            [],
            |row| row.get(0),
        )
        .expect("count remaining pdf-tier facts");
    assert_eq!(
        remaining_pdf_tier_facts, 0,
        "ADR 0095: migration 0135 must delete every pdf-tier fact (html_positional \
         extraction_method or source_tier='pdf') from the scored DB — found {remaining_pdf_tier_facts} still present"
    );

    let state = AppState::new(connection);

    // ---- index every stored fact the corpus documents carry, per tier ----
    // (both current AND comparative periods — the comparative-coverage
    // bucket below needs comparative-period app facts too; period scoping
    // happens later, only for the precision/recall denominator.)
    let mut esef_app: BTreeMap<Gt182Key, Gt182AppValue> = BTreeMap::new();
    let mut positional_app: BTreeMap<Gt182Key, Gt182AppValue> = BTreeMap::new();
    // Per-tier accumulator for the refinement observation buckets (out-of-
    // scope keys here; app-has-no-concept + comparative-coverage rows are
    // added while grouping the GT rows, below).
    let mut side: HashMap<&str, Gt182SideBuckets> = HashMap::new();

    let companies = state.list_companies().expect("list companies");
    for company in &companies {
        let periods = state
            .financials()
            .list_financial_periods(ListFinancialPeriodsInput {
                company_id: company.id.clone(),
                fiscal_year: None,
            })
            .expect("list financial periods");
        let period_end_by_id: HashMap<&str, &str> = periods
            .iter()
            .filter_map(|p| p.period_end_date.as_deref().map(|end| (p.id.as_str(), end)))
            .collect();

        let facts = state
            .financials()
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .expect("list financial facts");
        let relevant: Vec<&FinancialFact> = facts
            .iter()
            .filter(|f| {
                f.source_document_ref
                    .as_deref()
                    .is_some_and(|r| all_corpus_doc_ids.contains(r))
            })
            .collect();
        if relevant.is_empty() {
            continue;
        }

        let ids: Vec<String> = relevant.iter().map(|f| f.id.clone()).collect();
        let provenance = state
            .fundamentals_provenance()
            .get_many(&ids)
            .expect("fact provenance");
        let tier_by_fact: HashMap<&str, &str> = provenance
            .iter()
            .map(|p| (p.fact_id.as_str(), p.source_tier.as_str()))
            .collect();

        for fact in relevant {
            let doc_id = fact.source_document_ref.clone().unwrap_or_default();
            let Some(period_end) = period_end_by_id.get(fact.period_id.as_str()) else {
                eprintln!(
                    "  WARN {} fact {} (doc {doc_id}): period {} has no period_end_date, skipped",
                    company.ticker, fact.id, fact.period_id
                );
                continue;
            };
            let Ok(value) = Decimal::from_str(&fact.value_numeric) else {
                eprintln!(
                    "  WARN {} fact {} (doc {doc_id}): bad decimal '{}', skipped",
                    company.ticker, fact.id, fact.value_numeric
                );
                continue;
            };
            // esef tier: provenance.source_tier == 'esef' (matches the
            // MANIFEST's own esef_tier selection rule).
            let source_tier = tier_by_fact.get(fact.id.as_str()).copied();
            let is_esef_tier_fact = source_tier == Some("esef") && esef_doc_ids.contains(&doc_id);
            // positional tier: extraction_method == 'html_positional'
            // regardless of stored tier (MANIFEST's positional_tier rule;
            // ADR-documented #324 tier/method disagreement).
            let is_positional_tier_fact =
                fact.extraction_method == "html_positional" && positional_doc_ids.contains(&doc_id);
            if !is_esef_tier_fact && !is_positional_tier_fact {
                continue;
            }

            // KEY SCOPE (refinement 1): a metric key outside the GT
            // labeler's covered set (e.g. `long_term_debt`) is out of the
            // corpus's scope entirely — never SPURIOUS, just tallied.
            if !GT182_COVERED_BASE_KEYS.contains(&fact.metric_key.as_str()) {
                if is_esef_tier_fact {
                    side.entry("esef").or_default().out_of_gt_scope += 1;
                }
                if is_positional_tier_fact {
                    side.entry("positional").or_default().out_of_gt_scope += 1;
                }
                continue;
            }

            let key: Gt182Key = (
                doc_id.clone(),
                fact.metric_key.clone(),
                period_end.to_string(),
                fact.statement_basis.clone(),
            );
            let app_value = Gt182AppValue {
                value,
                currency: fact.currency.clone(),
                ticker: company.ticker.clone(),
                fact_id: fact.id.clone(),
            };

            if is_esef_tier_fact {
                if let Some(prior) = esef_app.insert(key.clone(), app_value.clone()) {
                    eprintln!(
                        "  WARN duplicate esef app-fact key ({doc_id}, {}, {period_end}, {}): \
                         {} superseded by {}",
                        fact.metric_key, fact.statement_basis, prior.fact_id, fact.id
                    );
                }
            }
            if is_positional_tier_fact {
                if let Some(prior) = positional_app.insert(key, app_value) {
                    eprintln!(
                        "  WARN duplicate positional app-fact key ({doc_id}, {}, {period_end}, {}): \
                         {} superseded by {}",
                        fact.metric_key, fact.statement_basis, prior.fact_id, fact.id
                    );
                }
            }
        }
    }

    // ---- group ground-truth rows by tier, resolved + translated + period-
    // scoped (refinements 1-3) ----------------------------------------------
    // Corpus-integrity violations (sol review, ADR 0095 round): every way a
    // ground-truth row can silently FALL OUT of the denominator — an unknown
    // tier, a `file` the manifest cannot resolve, a post-translation key
    // collision — is collected here and FAILS the run under
    // `BRAWLER_GT_REQUIRED=1` (after the report publishes, so the evidence
    // survives). A closure gate that can pass by quietly shrinking what it
    // measures is not a gate.
    let mut integrity_violations: Vec<String> = Vec::new();
    let mut gt_current_by_tier: HashMap<&str, BTreeMap<Gt182Key, &Gt182Row>> = HashMap::new();
    let mut unresolved: Vec<&Gt182Row> = Vec::new();
    for row in &gt_rows {
        if row.tier != "esef" && row.tier != "positional" {
            eprintln!(
                "  WARN unknown ground-truth tier '{}' for {} {}, skipped",
                row.tier, row.ticker, row.mapped_key
            );
            integrity_violations.push(format!(
                "unknown ground-truth tier '{}' for {} {} (row fell out of every denominator)",
                row.tier, row.ticker, row.mapped_key
            ));
            continue;
        }
        let Some(doc_id) = file_to_doc_id.get(&row.file) else {
            unresolved.push(row);
            continue;
        };

        // VARIANT TRANSLATION (refinement 2): `__owners` rows have no
        // app-side concept at all — a distinct observation bucket, never a
        // MISSING recall hit.
        let Some(translated) = gt182_app_comparable_key(&row.mapped_key) else {
            let bucket = side.entry(row.tier.as_str()).or_default();
            bucket.app_has_no_concept += 1;
            bucket.extra_rows.push(Gt182ScoreRow {
                tier: row.tier.clone(),
                ticker: row.ticker.clone(),
                document_id: doc_id.clone(),
                metric_key: row.mapped_key.clone(),
                period_end: row.period_end.clone(),
                statement_basis: row.statement_basis.clone(),
                app_value: None,
                gt_value: Some(row.value.clone()),
                app_currency: None,
                gt_currency: Some(row.currency.clone()),
                verdict: "APP_HAS_NO_CONCEPT".to_owned(),
                near_miss: false,
                gt_source: Some(row.source.clone()),
            });
            continue;
        };

        // PERIOD SCOPE (refinement 3): only the document's CURRENT period
        // (MANIFEST's own `period_end` for it) counts toward precision/
        // recall; a comparative-period row is an informational observation.
        let is_current = doc_current_period_end
            .get(doc_id.as_str())
            .map(String::as_str)
            == Some(row.period_end.as_str());
        let app_map = if row.tier == "esef" {
            &esef_app
        } else {
            &positional_app
        };
        if !is_current {
            let app_key: Gt182Key = (
                doc_id.clone(),
                translated.to_owned(),
                row.period_end.clone(),
                row.statement_basis.clone(),
            );
            let app_at_key = app_map.get(&app_key);
            let bucket = side.entry(row.tier.as_str()).or_default();
            if row.verification == "machine" && !row.uncertain {
                bucket.comparative_total += 1;
                if app_at_key.is_some() {
                    bucket.comparative_stored += 1;
                } else {
                    bucket.comparative_missing += 1;
                }
            }
            bucket.extra_rows.push(Gt182ScoreRow {
                tier: row.tier.clone(),
                ticker: row.ticker.clone(),
                document_id: doc_id.clone(),
                metric_key: translated.to_owned(),
                period_end: row.period_end.clone(),
                statement_basis: row.statement_basis.clone(),
                app_value: app_at_key.map(|a| a.value.to_string()),
                gt_value: Some(row.value.clone()),
                app_currency: app_at_key.and_then(|a| a.currency.clone()),
                gt_currency: Some(row.currency.clone()),
                verdict: "COMPARATIVE".to_owned(),
                near_miss: false,
                gt_source: Some(row.source.clone()),
            });
            continue;
        }

        let key: Gt182Key = (
            doc_id.clone(),
            translated.to_owned(),
            row.period_end.clone(),
            row.statement_basis.clone(),
        );
        let bucket = gt_current_by_tier.entry(row.tier.as_str()).or_default();
        if let Some(prior) = bucket.insert(key, row) {
            eprintln!(
                "  WARN duplicate ground-truth key (post-translation) in {} tier: {} {} -> {} {} \
                 {} ({} superseded by {})",
                row.tier,
                row.ticker,
                row.mapped_key,
                translated,
                row.period_end,
                row.statement_basis,
                prior.source,
                row.source
            );
            integrity_violations.push(format!(
                "duplicate post-translation ground-truth key in {} tier: {} {} -> {} {} {} \
                 (one label silently overwrote another)",
                row.tier,
                row.ticker,
                row.mapped_key,
                translated,
                row.period_end,
                row.statement_basis
            ));
        }
    }
    if !unresolved.is_empty() {
        integrity_violations.push(format!(
            "{} ground-truth row(s) reference a `file` MANIFEST.json cannot resolve \
             (excluded from every denominator)",
            unresolved.len()
        ));
        eprintln!(
            "-- {} ground-truth row(s) reference a `file` not in MANIFEST.json \
             (excluded from scoring) --",
            unresolved.len()
        );
        for row in &unresolved {
            eprintln!(
                "  {} {} {} : {}",
                row.ticker, row.mapped_key, row.period_end, row.file
            );
        }
    }

    // Current-period-only app-fact views (refinement 3: SPURIOUS is scoped
    // to an app fact's OWN period matching its document's current period).
    let esef_app_current = gt182_current_period_only(&esef_app, &doc_current_period_end);
    let positional_app_current =
        gt182_current_period_only(&positional_app, &doc_current_period_end);

    let empty_esef_gt = BTreeMap::new();
    let empty_positional_gt = BTreeMap::new();
    let esef_gt = gt_current_by_tier.get("esef").unwrap_or(&empty_esef_gt);
    let positional_gt = gt_current_by_tier
        .get("positional")
        .unwrap_or(&empty_positional_gt);

    let (esef_rows, mut esef_agg) = gt182_score_tier("esef", esef_gt, &esef_app_current);
    let (positional_rows, mut positional_agg) =
        gt182_score_tier("positional", positional_gt, &positional_app_current);

    // Fold the three observation buckets into each tier's aggregate.
    let default_side = Gt182SideBuckets::default();
    for (tier_name, agg) in [("esef", &mut esef_agg), ("positional", &mut positional_agg)] {
        let bucket = side.get(tier_name).unwrap_or(&default_side);
        agg.out_of_gt_scope_count = bucket.out_of_gt_scope;
        agg.app_has_no_concept_count = bucket.app_has_no_concept;
        agg.comparative_total = bucket.comparative_total;
        agg.comparative_stored = bucket.comparative_stored;
        agg.comparative_missing = bucket.comparative_missing;
    }

    let mut all_rows = esef_rows;
    all_rows.extend(positional_rows);
    for tier_name in ["esef", "positional"] {
        if let Some(bucket) = side.get(tier_name) {
            all_rows.extend(bucket.extra_rows.iter().cloned());
        }
    }
    all_rows.sort_by(|a, b| {
        (
            a.tier.as_str(),
            a.ticker.as_str(),
            a.metric_key.as_str(),
            a.period_end.as_str(),
        )
            .cmp(&(
                b.tier.as_str(),
                b.ticker.as_str(),
                b.metric_key.as_str(),
                b.period_end.as_str(),
            ))
    });
    let aggregates = vec![esef_agg, positional_agg];

    // ---- the evidence table (full, sorted tier -> ticker -> metric) ------
    eprintln!();
    eprintln!("======================================================================");
    eprintln!("#182 GROUND-TRUTH SCORER — ESEF / positional (DIAGNOSTIC, no floors — measurement v2 pending)");
    eprintln!("======================================================================");
    eprintln!("gt_dir={}", gt_dir.display());
    eprintln!(
        "ground-truth rows: {} ({} unresolved to a MANIFEST document)",
        gt_rows.len(),
        unresolved.len()
    );
    eprintln!();
    eprintln!(
        "{:<12}{:<8}{:<26}{:<12}{:<14}{:>16}{:>16}  {:<10}{:<10}VERDICT",
        "TIER", "TICKER", "METRIC", "PERIOD", "BASIS", "APP VALUE", "GT VALUE", "APP CUR", "GT CUR"
    );
    for row in &all_rows {
        eprintln!(
            "{:<12}{:<8}{:<26}{:<12}{:<14}{:>16}{:>16}  {:<10}{:<10}{}{}",
            row.tier,
            row.ticker,
            row.metric_key,
            row.period_end,
            row.statement_basis,
            row.app_value.as_deref().unwrap_or("-"),
            row.gt_value.as_deref().unwrap_or("-"),
            row.app_currency.as_deref().unwrap_or("-"),
            row.gt_currency.as_deref().unwrap_or("-"),
            row.verdict,
            if row.near_miss { " (near)" } else { "" },
        );
    }

    // COUNTS-FIRST reporting (audit): every ratio prints as numerator/
    // denominator alongside the percentage — never a bare percentage that
    // hides how few (or how concentrated) the underlying facts are.
    eprintln!();
    eprintln!("-- per-tier aggregates (current period only, in-scope + app-claimable keys) --");
    for agg in &aggregates {
        let precision_str = match agg.precision {
            Some(p) => format!(
                "{}/{} ({:.1}%)",
                agg.match_count,
                agg.precision_denominator,
                p * 100.0
            ),
            None => format!("n/a (0/{})", agg.precision_denominator),
        };
        let recall_str = match agg.recall {
            Some(r) => format!(
                "{}/{} ({:.1}%)",
                agg.match_count,
                agg.recall_denominator,
                r * 100.0
            ),
            None => format!("n/a (0/{})", agg.recall_denominator),
        };
        eprintln!(
            "{:<12} MATCH={:<5} WRONG_VALUE={:<5} MISSING={:<5} SPURIOUS={:<5} \
             CURRENCY_MISSING={:<5} CURRENCY_MISMATCH={:<5} UNVERIFIED={:<5}",
            agg.tier,
            agg.match_count,
            agg.wrong_value_count,
            agg.missing_count,
            agg.spurious_count,
            agg.currency_missing_count,
            agg.currency_mismatch_count,
            agg.unverified_count,
        );
        eprintln!("             precision={precision_str}  recall={recall_str}");
    }
    eprintln!();
    eprintln!(
        "-- per-ticker match counts (visibility into tier concentration, e.g. positional's CDR share) --"
    );
    for agg in &aggregates {
        eprintln!("{}:", agg.tier);
        for t in &agg.ticker_counts {
            eprintln!(
                "  {:<8} MATCH={:<5} WRONG_VALUE={:<5} MISSING={:<5} SPURIOUS={:<5} \
                 CURRENCY_MISSING={:<5} CURRENCY_MISMATCH={:<5} UNVERIFIED={:<5}",
                t.ticker,
                t.match_count,
                t.wrong_value_count,
                t.missing_count,
                t.spurious_count,
                t.currency_missing_count,
                t.currency_mismatch_count,
                t.unverified_count,
            );
        }
    }
    eprintln!();
    eprintln!("-- observation buckets (informational, off the precision/recall denominators) --");
    for agg in &aggregates {
        eprintln!(
            "{:<12} out_of_gt_scope={:<5} app_has_no_concept={:<5}  comparative: total={:<5} \
             stored={:<5} missing={:<5}",
            agg.tier,
            agg.out_of_gt_scope_count,
            agg.app_has_no_concept_count,
            agg.comparative_total,
            agg.comparative_stored,
            agg.comparative_missing,
        );
    }
    eprintln!("======================================================================");

    // ---- write the structured report for the orchestrator's owner-
    // arbitration render ----------------------------------------------------
    let generated_at_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let report = Gt182Report {
        generated_at_unix,
        gt_dir: gt_dir.display().to_string(),
        unresolved_gt_rows: unresolved.len(),
        rows: all_rows,
        aggregates: aggregates.clone(),
    };
    // Adversarial-review should-fix #8: publish atomically — write to a
    // per-run temp file in the SAME directory (so the rename below is on one
    // filesystem, hence atomic), then rename onto the stable published path.
    // A reader (or a concurrent run) can never observe a partially-written
    // `scoring-report.json`.
    let report_path = gt_dir.join("scoring-report.json");
    let report_tmp_path = gt_dir.join(format!("scoring-report.json.tmp-{run_id}"));
    let report_json = serde_json::to_string_pretty(&report).expect("serialize scoring report");
    std::fs::write(&report_tmp_path, report_json)
        .unwrap_or_else(|e| panic!("write {}: {e}", report_tmp_path.display()));
    std::fs::rename(&report_tmp_path, &report_path).unwrap_or_else(|e| {
        panic!(
            "publish {} <- {}: {e}",
            report_path.display(),
            report_tmp_path.display()
        )
    });
    eprintln!("scoring report written to {}", report_path.display());

    // Best-effort cleanup of the per-run worktest scratch dir — inert
    // leftovers on failure are never the corpus itself (see above).
    let _ = std::fs::remove_dir_all(&worktest_dir);

    // ---- harness sanity ----------------------------------------------------
    assert!(
        !gt_rows.is_empty(),
        "ground_truth.json at {} has no rows",
        gt_path.display()
    );
    for agg in &aggregates {
        // Adversarial-review should-fix #8: the total must count EVERY
        // verdict bucket, including the currency-aware ones — a tier that
        // scored only CURRENCY_MISSING/CURRENCY_MISMATCH rows (a real,
        // if unlikely, shape) was wrongly flagged as "scored nothing" before
        // this fix, since those two verdicts were missing from the sum.
        let total = agg.match_count
            + agg.wrong_value_count
            + agg.missing_count
            + agg.spurious_count
            + agg.unverified_count
            + agg.currency_missing_count
            + agg.currency_mismatch_count;
        assert!(
            total > 0,
            "{} tier scored nothing (0 keys) — check MANIFEST document-id resolution",
            agg.tier
        );
    }

    // ---- #182 is a DIAGNOSTIC, not a ratchet — NO precision/recall floors -
    // An external methodology audit (2026-08-05) found floors premature and
    // pulled them from this change; they land in measurement v2 once the
    // audit's blockers are addressed. Two are fixed in THIS change (currency-
    // aware matching above; the DNP MANIFEST.json period_end correction); the
    // remaining ones are structural to how the corpus was built and need a
    // v2 measurement design, not a code fix here:
    //   - output-conditioned corpus: GT rows are only labeled for facts/
    //     periods visible in documents the pipeline had ALREADY extracted,
    //     which biases recall upward for whatever it already covers — the
    //     corpus cannot see what the pipeline never touched.
    //   - no fresh extraction: this harness scores a point-in-time DB
    //     snapshot, never a live pipeline run — a parser change does NOT
    //     move these numbers (only a fresh corpus/extraction run would), so
    //     these numbers are evidence about STORED STATE, not about the
    //     current parser.
    //   - CDR concentration: 12 of the positional tier's 17 documents are CD
    //     Projekt, so a tier-wide number is mostly CDR's number — see the
    //     per-ticker breakdown printed above and in `scoring-report.json`.
    //   - document-level vs single-slot estimand: precision/recall are
    //     computed per (document, metric, period, basis) SLOT, not per
    //     document, so a handful of chatty documents can dominate a tier.
    // Until v2 lands, this harness asserts sanity only (below) plus, under
    // `BRAWLER_GT_REQUIRED=1`, that neither denominator is zero (a genuine
    // corpus/snapshot integrity signal, distinct from "the numbers are low").
    for agg in &aggregates {
        // ADR 0095: the positional tier's app side is now PERMANENTLY empty
        // (migration 0135 deletes every pdf-tier fact; asserted DB-wide
        // above) — a zero precision denominator for it is the correct,
        // expected steady state from now on, never a corpus/snapshot
        // integrity signal. Only `esef` still has a live app side that a
        // zero denominator would say something real about.
        if agg.precision.is_none() && agg.tier != "positional" {
            let msg = format!(
                "{} tier's precision denominator (MATCH+WRONG_VALUE+SPURIOUS+CURRENCY_MISSING+\
                 CURRENCY_MISMATCH) is zero — the corpus/db-snapshot looks stale or broken, not \
                 merely absent",
                agg.tier
            );
            if required {
                panic!("REQUIRED esef_positional_ground_truth_scores: {msg}");
            }
            eprintln!("WARN esef_positional_ground_truth_scores: {msg} (non-required run)");
        } else if agg.precision.is_none() {
            eprintln!(
                "INFO esef_positional_ground_truth_scores: positional tier's precision \
                 denominator is zero — expected post-ADR-0095 (the tier's app side is \
                 permanently empty), not a corpus/snapshot integrity signal"
            );
        }
        if agg.recall.is_none() {
            let msg = format!(
                "{} tier's recall denominator (MATCH+WRONG_VALUE+MISSING+CURRENCY_MISSING+\
                 CURRENCY_MISMATCH) is zero — the corpus/db-snapshot looks stale or broken, not \
                 merely absent",
                agg.tier
            );
            if required {
                panic!("REQUIRED esef_positional_ground_truth_scores: {msg}");
            }
            eprintln!("WARN esef_positional_ground_truth_scores: {msg} (non-required run)");
        }
    }

    // Corpus-integrity enforcement (sol review): rows that silently fell out
    // of the denominators fail the closure gate — AFTER the report published,
    // so the run's evidence survives for diagnosis.
    if !integrity_violations.is_empty() {
        let listing = integrity_violations.join("\n  - ");
        if required {
            panic!(
                "REQUIRED esef_positional_ground_truth_scores: {} corpus-integrity \
                 violation(s) silently shrank the scoring denominator:\n  - {listing}",
                integrity_violations.len()
            );
        }
        eprintln!(
            "WARN esef_positional_ground_truth_scores: {} corpus-integrity violation(s) \
             (non-required run):\n  - {listing}",
            integrity_violations.len()
        );
    }
}

/// Concept-harvest command (ADR 0100 decision 2, epic #398): prints every
/// Layer 1 (`report_tagged_facts`) concept with no crosswalk entry
/// (`fundamentals::extraction::ifrs_crosswalk`), ranked by distinct-company
/// count with its observed `period_type`(s) — the input for the next
/// crosswalk seed migration. Read-only: no writes, so (unlike the sweep
/// harnesses above) it is safe to point directly at a real database, though a
/// throwaway copy is still recommended.
///
/// **Inert in CI** — skips with a printed message unless `BRAWLER_REAL_DB` is
/// set. Prints no floor/assertion beyond harness sanity: this is a diagnostic
/// report, not a gate (the `esef_positional_ground_truth_scores` pattern).
///
/// Run it manually from `src-tauri/`:
///
/// ```text
/// BRAWLER_REAL_DB=../private/realdata/brawler.sqlite3 \
///   cargo nextest run -p brawler concept_harvest_report \
///     --run-ignored all --no-capture
/// ```
#[test]
#[ignore = "developer diagnostic; needs BRAWLER_REAL_DB"]
fn concept_harvest_report() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP concept_harvest_report: set BRAWLER_REAL_DB to a copy of the owner's database \
             (see private/realdata/README.md)"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP concept_harvest_report: no database at {db_path}");
        return;
    }

    // Read-only for real (sol review finding 5): `open_database` applies
    // every pending migration on open, so this diagnostic would MUTATE the
    // database it claims only to read. A schema too old for the query fails
    // loudly at the query instead.
    let connection = open_database_readonly(&db_path).expect("open real db read-only");
    let state = AppState::new(connection);
    // Harness sanity BEFORE reading the result: an empty Layer 1 store makes
    // "no uncrosswalked concepts" indistinguishable from "nothing was ever
    // captured", and the reassuring branch below would then report a curated
    // vocabulary that was never exercised. A check that cannot fail is worse
    // than no check, so a store with zero tagged facts is a hard failure.
    let captured = state
        .list_companies()
        .expect("companies")
        .iter()
        .any(|company| {
            !state
                .report_tagged_facts()
                .facts_for_company(&company.id)
                .expect("layer 1 facts")
                .is_empty()
        });
    assert!(
        captured,
        "concept_harvest_report: {db_path} holds no report_tagged_facts rows — \
         re-extract before harvesting, or the report is vacuously clean"
    );

    let harvested = state
        .report_tagged_facts()
        .harvest_uncrosswalked_concepts()
        .expect("harvest query");

    if harvested.is_empty() {
        println!(
            "concept_harvest_report: every observed Layer 1 concept already has a crosswalk entry"
        );
        return;
    }

    println!(
        "concept_harvest_report: {} uncrosswalked concept(s), ranked by distinct-company count",
        harvested.len()
    );
    println!("{:>6}  {:<12}  concept", "issuers", "period_type");
    for row in &harvested {
        println!(
            "{:>6}  {:<12}  {}",
            row.company_count,
            row.period_types.join("|"),
            row.concept_local_name
        );
    }
}

/// Corpus-wide concept harvest (ADR 0100 decision 2, epic #398) — the input
/// for a crosswalk seed migration that is honest about cross-issuer evidence.
///
/// `concept_harvest_report` above reads whatever Layer 1 already holds, which
/// on a live database is only the reports the app has re-extracted so far.
/// Curating from that is the selectivity trap the two-layer split exists to
/// avoid: a concept seen at ONE issuer is not evidence of a shared vocabulary.
/// This harness instead walks EVERY stored report document on disk, computes
/// the Layer 1 generation the pipeline would produce, and ranks uncrosswalked
/// concepts by how many distinct issuers tag them — the whole corpus, not the
/// re-extracted slice. It never writes: the database is read for its document
/// inventory only, the packages are read from `data_dir`.
///
/// **Inert in CI** — skips unless `BRAWLER_REAL_DB` is set.
///
/// ```text
/// BRAWLER_REAL_DB=../private/realdata/brawler.sqlite3 \
///   cargo test corpus_concept_harvest_report -- --ignored --nocapture
/// ```
#[test]
#[ignore = "developer diagnostic; needs BRAWLER_REAL_DB"]
fn corpus_concept_harvest_report() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP corpus_concept_harvest_report: set BRAWLER_REAL_DB to a copy of the owner's \
             database (see private/realdata/README.md)"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP corpus_concept_harvest_report: no database at {db_path}");
        return;
    }

    // Read-only for real (sol review finding 5), same as above.
    let connection = open_database_readonly(&db_path).expect("open real db read-only");
    // The packages live in the real Tauri data dir, not next to the database
    // copy (same convention as the recall/precision harness above).
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };
    let data_dir = state.data_dir();

    let crosswalked: std::collections::HashSet<&str> =
        crate::fundamentals::extraction::ifrs_crosswalk::entries()
            .iter()
            .map(|entry| entry.concept)
            .collect();

    // concept → (distinct issuers, occurrences, period types, one sample issuer)
    let mut seen: BTreeMap<
        String,
        (
            std::collections::BTreeSet<String>,
            i64,
            std::collections::BTreeSet<String>,
            bool,
        ),
    > = BTreeMap::new();
    let mut packages_read = 0usize;
    let mut packages_without_instance = 0usize;

    for company in state.list_companies().expect("companies") {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("report documents");
        for document in &documents {
            let Some(local_path) = document.local_path.as_deref() else {
                continue;
            };
            let Ok(bytes) = std::fs::read(data_dir.join(local_path)) else {
                continue;
            };
            let route = crate::jobs::structured_extraction::route_document(&bytes);
            let generation =
                crate::jobs::structured_extraction::compute_layer1_generation(&bytes, route);
            if !generation.has_instance {
                if matches!(
                    route,
                    crate::jobs::structured_extraction::DocumentRoute::ZipPackage
                ) {
                    packages_without_instance += 1;
                }
                continue;
            }
            packages_read += 1;
            for fact in &generation.facts {
                // Dimensional occurrences are stored but never projected, so
                // they are not naming candidates (decision 1).
                if fact.is_dimensional {
                    continue;
                }
                let entry = seen
                    .entry(fact.concept_local_name.clone())
                    .or_insert_with(|| {
                        (
                            std::collections::BTreeSet::new(),
                            0,
                            std::collections::BTreeSet::new(),
                            false,
                        )
                    });
                entry.0.insert(company.ticker.clone());
                entry.1 += 1;
                entry.2.insert(fact.period_type.clone());
                // Standard taxonomy vs issuer extension. Only a standard
                // concept may take a global canonical key — an extension is a
                // company's own vocabulary and must stay issuer-qualified
                // (ADR 0100 decision 2), so the two never share a ranking.
                entry.3 |= fact.concept_namespace_uri.contains("/ifrs/")
                    || fact.concept_namespace_uri.contains("ifrs-full");
            }
        }
    }

    assert!(
        packages_read > 0,
        "corpus_concept_harvest_report: no report document under {} yielded an iXBRL instance — \
         point BRAWLER_REAL_DB at a database whose report_documents are on this disk",
        data_dir.display()
    );

    let mut uncurated: Vec<_> = seen
        .iter()
        .filter(|(concept, _)| !crosswalked.contains(concept.as_str()))
        .collect();
    uncurated.sort_by(|a, b| {
        b.1 .0
            .len()
            .cmp(&a.1 .0.len())
            .then_with(|| b.1 .1.cmp(&a.1 .1))
            .then_with(|| a.0.cmp(b.0))
    });

    println!(
        "corpus_concept_harvest_report: {packages_read} instance-bearing document(s), \
         {packages_without_instance} package(s) with no instance, {} distinct dimensionless \
         concept(s), {} of them uncrosswalked",
        seen.len(),
        uncurated.len()
    );
    println!(
        "{:>7}  {:>6}  {:<10}  {:<9}  concept",
        "issuers", "occurs", "period", "standard"
    );
    for (concept, (issuers, occurrences, period_types, in_statement)) in uncurated {
        println!(
            "{:>7}  {:>6}  {:<10}  {:<9}  {concept}",
            issuers.len(),
            occurrences,
            period_types.iter().cloned().collect::<Vec<_>>().join("|"),
            if *in_statement { "ifrs" } else { "ext" },
        );
    }
}
