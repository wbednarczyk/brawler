//! Real-data recall/precision harness for the ownership shareholders-table
//! parser (v0.56 T1, ADR 0072; docs/testing.md real-data-first rule). Mirrors
//! [`super::real_data_extraction`]: **inert in CI**, it measures accuracy over a
//! small hand-labeled ground-truth set of real report documents by calling the
//! pure parser directly (no job queue, no persistence).
//!
//! It resolves each labeled document from a throwaway copy of the maintainer's
//! real DB, reads the stored report bytes, runs `extract_report` →
//! `parse_shareholders`, and prints per-document and overall **row recall**
//! (matched labeled rows / labeled rows) and **row precision** (matched /
//! emitted holder rows on labeled docs), keyed by `SourceFormat`, plus the
//! per-document misses so the owner can eyeball failures. Sanity assertion only
//! (some document resolved) — no quality floor in the spike; the metrics decide
//! the T2/T3 tier split.
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo test -p brawler --lib real_data_ownership_recall_precision -- --ignored --nocapture
//! ```
//!
//! `BRAWLER_OWNERSHIP_GROUND_TRUTH` overrides the ground-truth path (default
//! `private/realdata/ownership_ground_truth.json`). See docs/testing.md for the
//! JSON shape.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::*;
use crate::fundamentals::ownership::{parse_shareholders, OwnershipRow};
use crate::report_diff::extraction::{extract_report, SourceFormat};

// ---------------------------------------------------------------------------
// Ground-truth JSON shape (docs/testing.md)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruthDocument {
    /// Company ticker (matched case-insensitively against `companies.ticker`).
    ticker: String,
    #[serde(rename = "match")]
    doc_match: DocumentMatch,
    /// ISO `YYYY-MM-DD` disclosure date (printed for context; not used to match).
    #[serde(default)]
    as_of: Option<String>,
    rows: Vec<GroundTruthRow>,
}

/// One hand-labeled holder row. Percentages are decimal strings; either may be
/// null for a one-sided disclosure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruthRow {
    holder: String,
    #[serde(default)]
    capital_pct: Option<String>,
    #[serde(default)]
    votes_pct: Option<String>,
}

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

/// Resolve the labeled document among a company's rows: a `urlContains` needle
/// can match several siblings (the report + its `.xades` signature + other
/// attachments share a URL stem), so among the matches prefer a row that is
/// actually fetched with a stored file. Falls back to any match so the SKIP
/// diagnostic still names the row that matched.
fn resolve_document<'a>(
    documents: &'a [ReportDocument],
    spec: &DocumentMatch,
) -> Option<&'a ReportDocument> {
    let matches: Vec<&ReportDocument> = documents
        .iter()
        .filter(|rd| document_matches(rd, spec))
        .collect();
    matches
        .iter()
        .find(|rd| rd.fetch_status == "fetched" && rd.local_path.is_some())
        .copied()
        .or_else(|| matches.first().copied())
}

// ---------------------------------------------------------------------------
// Row matching (holder-name normalization + per-side pct tolerance)
// ---------------------------------------------------------------------------

/// Harness-local holder normalization for metric matching. Real text layers
/// break words with arbitrary spaces ("Nationale-Nederla nden"), so matching
/// is fully whitespace-insensitive: case-fold, drop parenthesized qualifiers
/// ("(dawniej R22 S.A.)", "(zarządzane fundusze łącznie)"), remove all
/// whitespace and punctuation, then strip a minimal legal-form suffix set.
/// Deliberately separate from `storage::ownership::normalize_holder_name`
/// (which only upper-cases) — full normalization is T5's job; this only keeps
/// the spike's recall from being deflated by "PZU" vs "PZU S.A.".
fn normalize_holder(raw: &str) -> String {
    let mut s = raw.to_lowercase();
    // Drop parenthesized qualifiers (also unbalanced trailing ones).
    while let (Some(open), close) = (s.find('('), s.find(')')) {
        match close {
            Some(close) if close > open => s.replace_range(open..=close, " "),
            _ => {
                s.truncate(open);
                break;
            }
        }
    }
    let squeezed: String = s.chars().filter(|c| c.is_alphanumeric()).collect();
    const SUFFIXES: [&str; 8] = [
        "spółkaakcyjna",
        "spolkaakcyjna",
        "spzoo",
        "sa",
        "se",
        "bv",
        "nv",
        "ltd",
    ];
    let mut normalized = squeezed;
    for suffix in SUFFIXES {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            if !stripped.is_empty() {
                normalized = stripped.to_owned();
                break;
            }
        }
    }
    normalized
}

/// Absolute-tolerance (0.01) percentage comparison on a single side. A null
/// label side imposes no requirement; a labeled side that the parser did not
/// emit is a miss.
fn side_matches(labeled: Option<&str>, emitted: Option<&str>) -> bool {
    match labeled {
        None => true,
        Some(l) => match (
            l.parse::<f64>(),
            emitted.and_then(|e| e.parse::<f64>().ok()),
        ) {
            (Ok(l), Some(e)) => (l - e).abs() <= 0.01,
            _ => false,
        },
    }
}

fn row_matches(labeled: &GroundTruthRow, emitted: &OwnershipRow) -> bool {
    normalize_holder(&labeled.holder) == normalize_holder(&emitted.holder_raw)
        && side_matches(
            labeled.capital_pct.as_deref(),
            emitted.capital_pct.as_deref(),
        )
        && side_matches(labeled.votes_pct.as_deref(), emitted.votes_pct.as_deref())
}

fn pct(matched: usize, total: usize) -> String {
    if total == 0 {
        "n/a".to_owned()
    } else {
        format!("{:.1}%", 100.0 * matched as f64 / total as f64)
    }
}

#[derive(Default)]
struct FormatStats {
    matched: usize,
    labeled: usize,
    emitted: usize,
}

#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR + an ownership ground-truth file"]
fn real_data_ownership_recall_precision() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_ownership_recall_precision: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };

    let ground_truth_path = std::env::var("BRAWLER_OWNERSHIP_GROUND_TRUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/ownership_ground_truth.json")
        });
    let Ok(raw) = std::fs::read_to_string(&ground_truth_path) else {
        eprintln!(
            "SKIP real_data_ownership_recall_precision: no ground-truth file at {}",
            ground_truth_path.display()
        );
        return;
    };
    let ground_truth: GroundTruth = serde_json::from_str(&raw)
        .expect("ownership ground-truth JSON must parse (see docs/testing.md)");
    if ground_truth.documents.is_empty() {
        eprintln!(
            "SKIP real_data_ownership_recall_precision: ground truth at {} has no documents",
            ground_truth_path.display()
        );
        return;
    }

    let connection = open_database(&db_path).expect("open real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };

    let companies = state.list_companies().expect("list companies");

    let mut resolved = 0usize;
    let mut by_format: BTreeMap<&'static str, FormatStats> = BTreeMap::new();

    eprintln!(
        "== real-data ownership recall/precision: {} labeled document(s) ==",
        ground_truth.documents.len()
    );

    for doc in &ground_truth.documents {
        let as_of = doc.as_of.as_deref().unwrap_or("?");
        let label = format!("{} @{}", doc.ticker, as_of);

        if doc.doc_match.url_contains.is_none() && doc.doc_match.content_hash.is_none() {
            eprintln!("SKIP {label}: match spec has neither urlContains nor contentHash");
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
        let Some(report_document) = resolve_document(&documents, &doc.doc_match) else {
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
        let extracted = extract_report(&bytes, format);
        let outcome = parse_shareholders(&extracted.sections, format);

        // Debug aid: BRAWLER_OWNERSHIP_DUMP=<dir> writes each document's
        // extracted section text so parser misses can be diagnosed offline.
        if let Ok(dump_dir) = std::env::var("BRAWLER_OWNERSHIP_DUMP") {
            let mut text = String::new();
            for s in &extracted.sections {
                text.push_str(&format!(
                    "===== SECTION {} [{}] =====\n",
                    s.ordinal, s.heading
                ));
                text.push_str(&s.body);
                text.push('\n');
            }
            let _ = std::fs::create_dir_all(&dump_dir);
            let _ = std::fs::write(
                PathBuf::from(&dump_dir).join(format!("{}.txt", doc.ticker)),
                text,
            );
        }

        // Greedy 1:1 matching of labeled rows against emitted holder rows.
        let mut consumed = vec![false; outcome.rows.len()];
        let mut matched = 0usize;
        let mut missed: Vec<&str> = Vec::new();
        for labeled in &doc.rows {
            let hit = outcome
                .rows
                .iter()
                .enumerate()
                .find(|(i, emitted)| !consumed[*i] && row_matches(labeled, emitted));
            match hit {
                Some((i, _)) => {
                    consumed[i] = true;
                    matched += 1;
                }
                None => missed.push(labeled.holder.as_str()),
            }
        }
        let spurious: Vec<String> = outcome
            .rows
            .iter()
            .enumerate()
            .filter(|(i, _)| !consumed[*i])
            .map(|(_, r)| {
                format!(
                    "{} [{}|{}]",
                    r.holder_raw,
                    r.capital_pct.as_deref().unwrap_or("-"),
                    r.votes_pct.as_deref().unwrap_or("-")
                )
            })
            .collect();

        let fmt_key = format.as_str();
        eprintln!(
            "{label:<20} fmt={fmt_key:<5} state={:<16} recall={:>7} ({matched}/{labeled_n})  precision={:>7} ({matched}/{emitted_n})",
            format!("{:?}", outcome.state),
            pct(matched, doc.rows.len()),
            pct(matched, outcome.rows.len()),
            labeled_n = doc.rows.len(),
            emitted_n = outcome.rows.len(),
        );
        if !missed.is_empty() {
            eprintln!("    MISSED  labeled holders: {missed:?}");
        }
        if !spurious.is_empty() {
            eprintln!("    SPURIOUS emitted holders: {spurious:?}");
        }
        if !missed.is_empty() || !spurious.is_empty() {
            eprintln!(
                "    ANCHOR: {:?}",
                outcome.matched_heading.as_deref().unwrap_or("-")
            );
        }

        let entry = by_format.entry(fmt_key).or_default();
        entry.matched += matched;
        entry.labeled += doc.rows.len();
        entry.emitted += outcome.rows.len();
        resolved += 1;
    }

    eprintln!("-- per format --");
    let mut total = FormatStats::default();
    for (fmt, stats) in &by_format {
        eprintln!(
            "{fmt:<5} recall={:>7} ({}/{})  precision={:>7} ({}/{})",
            pct(stats.matched, stats.labeled),
            stats.matched,
            stats.labeled,
            pct(stats.matched, stats.emitted),
            stats.matched,
            stats.emitted,
        );
        total.matched += stats.matched;
        total.labeled += stats.labeled;
        total.emitted += stats.emitted;
    }
    eprintln!(
        "-- overall: resolved={}/{} documents  recall={} ({}/{})  precision={} ({}/{}) --",
        resolved,
        ground_truth.documents.len(),
        pct(total.matched, total.labeled),
        total.matched,
        total.labeled,
        pct(total.matched, total.emitted),
        total.matched,
        total.emitted,
    );

    // Harness-sanity only (no quality floor in the spike).
    assert!(
        resolved > 0,
        "expected at least one ground-truth document to resolve to a real report file \
         (check BRAWLER_REAL_DATA_DIR, tickers, and match specs)"
    );
}

/// Real-data backfill evidence (v0.56 phase-1 gate): run the T3 catch-up +
/// queue against a **throwaway copy** of the maintainer's real DB and print
/// per-company stake coverage, holder-type classification counts, and the
/// residual list. Inert in CI — skips unless `BRAWLER_REAL_DB` (throwaway
/// copy; migrations 0082–0084 will be applied to it) and
/// `BRAWLER_REAL_DATA_DIR` are set:
///
/// ```text
/// cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
/// BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
///   BRAWLER_REAL_DATA_DIR=/path/to/tauri/data \
///   cargo test -p brawler --lib real_data_ownership_backfill -- --ignored --nocapture
/// ```
#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB (throwaway copy) + BRAWLER_REAL_DATA_DIR"]
fn real_data_ownership_backfill() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_ownership_backfill: set BRAWLER_REAL_DB to a throwaway copy");
        return;
    };
    let Ok(data_dir) = std::env::var("BRAWLER_REAL_DATA_DIR") else {
        eprintln!("SKIP real_data_ownership_backfill: set BRAWLER_REAL_DATA_DIR");
        return;
    };

    let connection = crate::storage::open_database(&db_path).expect("open real db copy");
    let state = AppState::with_data_dir(connection, PathBuf::from(data_dir));

    let enqueued =
        crate::jobs::ownership_extraction::enqueue_ownership_extraction_catch_up(&state, None);
    let processed = crate::jobs::handlers::build_worker(state.clone())
        .run_until_idle()
        .expect("drain queue");
    eprintln!("== ownership backfill: enqueued={enqueued} processed={processed} ==");

    let companies = state.companies().list_companies().expect("companies");
    let mut with_stakes = 0usize;
    let mut total_stakes = 0usize;
    let mut classified = 0usize;
    let mut unclassified = 0usize;
    let mut residuals: Vec<(String, String, String)> = Vec::new();
    for company in &companies {
        for r in state
            .ownership()
            .list_extraction_residuals(&company.id)
            .expect("residuals")
        {
            residuals.push((
                company.ticker.clone(),
                r.report_document_id.clone(),
                r.parse_state.clone(),
            ));
        }
        let current = state
            .ownership()
            .current_state_with_free_float(&company.id)
            .expect("current state");
        if current.stakes.is_empty() {
            continue;
        }
        with_stakes += 1;
        total_stakes += current.stakes.len();
        let (c, u): (Vec<_>, Vec<_>) = current.stakes.iter().partition(|s| s.holder_type.is_some());
        classified += c.len();
        unclassified += u.len();
        eprintln!(
            "{:<6} holders={:<3} classified={:<3} free_float={:?}",
            company.ticker,
            current.stakes.len(),
            c.len(),
            current.free_float_pct
        );
    }
    eprintln!(
        "-- companies with stakes: {with_stakes}/{} --",
        companies.len()
    );
    eprintln!(
        "-- total current-state holder rows: {total_stakes} (classified {classified} / unclassified {unclassified}) --"
    );
    eprintln!(
        "-- residual documents (AI/OCR tier): {} --",
        residuals.len()
    );
    for (ticker, doc, parse_state) in residuals.iter().take(20) {
        eprintln!("   residual: {ticker} doc={doc} state={parse_state}");
    }
    assert!(processed > 0, "backfill processed at least one document");
}
