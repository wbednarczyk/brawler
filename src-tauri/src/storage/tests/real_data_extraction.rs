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
use serde::Deserialize;

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
        let esef_bytes: Option<Vec<u8>> = match format {
            SourceFormat::Xhtml => Some(bytes),
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
            esef_bytes: esef_bytes.as_deref(),
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
