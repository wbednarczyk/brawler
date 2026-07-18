//! Real-data recall harness for the management-holdings parser (v0.57 T5,
//! ADR 0083 D6; docs/testing.md real-data-first rule). **Inert in CI**: it
//! measures accuracy over the 15-document hand-labeled ground truth by calling
//! the pure parser directly (no job queue, no persistence).
//!
//! It resolves each labeled document from a **throwaway copy** of the maintainer's
//! real DB (the master is at migration 81 — pre-ownership; `open_database`
//! migrates the copy forward), reads the stored bytes, runs `extract_report` →
//! `parse_management_holdings`, and prints per-document + overall **person recall**
//! (matched labeled persons / labeled persons) and **role recall** (correct role
//! among matched persons), plus honest **share recall**. Person/role are keyed on
//! the canonical holder identity (the production founder-stamping join key), never
//! a surname.
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
//!   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
//!   cargo test -p brawler --lib real_data_management_holdings -- --ignored --nocapture
//! ```
//!
//! `BRAWLER_MGMT_GROUND_TRUTH` overrides the ground-truth path (default
//! `private/realdata/management_holdings_ground_truth.json`).

use std::path::PathBuf;

use serde::Deserialize;

use super::*;
use crate::fundamentals::ownership::classify::canonical_holder_identity;
use crate::report_diff::extraction::{extract_report, SourceFormat};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GroundTruth {
    documents: Vec<GtDocument>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GtDocument {
    ticker: String,
    /// The `report_documents.id` — the ground truth's canonical `matchKey`.
    report_document_id: String,
    #[serde(rename = "match")]
    doc_match: DocumentMatch,
    #[serde(default)]
    parse_class: Option<String>,
    #[serde(default)]
    rows: Vec<GtRow>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DocumentMatch {
    url_contains: Option<String>,
    content_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GtRow {
    #[serde(default)]
    person_raw: Option<String>,
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    shares: Option<i64>,
    #[serde(default)]
    zero_holding_aggregate: Option<bool>,
}

impl GtRow {
    fn is_person(&self) -> bool {
        self.person_raw.is_some() && !self.zero_holding_aggregate.unwrap_or(false)
    }
}

/// Classes the deterministic tier is NOT expected to recover by person (glyph
/// residual, prose-zero aggregates, missing sections, and the displaced-table
/// PDF) — legitimate residual misses, excluded from the ≥95%/≥90% floors.
fn is_residual_class(parse_class: Option<&str>) -> bool {
    matches!(
        parse_class,
        Some(
            "glyph_encoded_residual"
                | "prose_zero_per_organ"
                | "prose_zero_aggregate"
                | "section_not_found_by_holdings_anchors"
                | "no_holdings_table_false_positive_anchors"
                | "prose_plus_displaced_table"
        )
    )
}

fn document_matches(document: &ReportDocument, spec: &DocumentMatch) -> bool {
    spec.url_contains
        .as_deref()
        .is_some_and(|needle| document.url.contains(needle))
        || spec
            .content_hash
            .as_deref()
            .is_some_and(|hash| document.content_hash.as_deref() == Some(hash))
}

fn resolve_document<'a>(
    documents: &'a [ReportDocument],
    doc_id: &str,
    spec: &DocumentMatch,
) -> Option<&'a ReportDocument> {
    // Primary: the canonical report_documents.id (the ground truth's matchKey).
    if let Some(hit) = documents.iter().find(|rd| rd.id == doc_id) {
        return Some(hit);
    }
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

fn pct(matched: usize, total: usize) -> String {
    if total == 0 {
        "n/a".to_owned()
    } else {
        format!("{:.1}%", 100.0 * matched as f64 / total as f64)
    }
}

#[derive(Default)]
struct Stats {
    person_matched: usize,
    person_labeled: usize,
    role_correct: usize,
    role_labeled: usize,
    share_correct: usize,
    share_labeled: usize,
}

impl Stats {
    fn add(&mut self, other: &DocScore) {
        self.person_matched += other.matched;
        self.person_labeled += other.labeled;
        self.role_correct += other.role_ok;
        self.role_labeled += other.role_total;
        self.share_correct += other.share_ok;
        self.share_labeled += other.share_total;
    }
}

#[derive(Default)]
struct DocScore {
    matched: usize,
    labeled: usize,
    role_ok: usize,
    role_total: usize,
    share_ok: usize,
    share_total: usize,
}

#[test]
#[ignore = "real-data validation; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR + the ground-truth file"]
fn real_data_management_holdings_recall() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_management_holdings_recall: set BRAWLER_REAL_DB to a throwaway copy"
        );
        return;
    };
    let ground_truth_path = std::env::var("BRAWLER_MGMT_GROUND_TRUTH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("private/realdata/management_holdings_ground_truth.json")
        });
    let Ok(raw) = std::fs::read_to_string(&ground_truth_path) else {
        eprintln!(
            "SKIP real_data_management_holdings_recall: no ground-truth file at {}",
            ground_truth_path.display()
        );
        return;
    };
    let ground_truth: GroundTruth =
        serde_json::from_str(&raw).expect("management-holdings ground-truth JSON must parse");

    let connection = open_database(&db_path).expect("open real db (migrates the throwaway copy)");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };
    let companies = state.list_companies().expect("list companies");

    let mut resolved = 0usize;
    let mut all = Stats::default();
    let mut parseable = Stats::default();

    eprintln!(
        "== real-data management-holdings recall: {} labeled document(s) ==",
        ground_truth.documents.len()
    );

    for doc in &ground_truth.documents {
        let label = format!(
            "{} [{}]",
            doc.ticker,
            doc.parse_class.as_deref().unwrap_or("?")
        );
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
        let Some(report_document) =
            resolve_document(&documents, &doc.report_document_id, &doc.doc_match)
        else {
            eprintln!(
                "SKIP {label}: no report_documents row matched {:?}",
                doc.doc_match
            );
            continue;
        };
        let Some(local_path) = report_document.local_path.as_deref() else {
            eprintln!("SKIP {label}: matched document has no stored file");
            continue;
        };
        let path = state.data_dir().join(local_path);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(err) => {
                eprintln!("SKIP {label}: read {} failed: {err}", path.display());
                continue;
            }
        };

        let format = SourceFormat::resolve(report_document.content_type.as_deref(), local_path);
        let extracted = extract_report(&bytes, format);
        if std::env::var("BRAWLER_MGMT_DUMP_TICKER").as_deref() == Ok(doc.ticker.as_str()) {
            let mut text = String::new();
            for s in &extracted.sections {
                text.push_str(&format!(
                    "== SECTION {} [{}] ==\n{}\n",
                    s.ordinal, s.heading, s.body
                ));
            }
            let _ = std::fs::write(
                format!(
                    "/tmp/mgmt_dump_{}_{}.txt",
                    doc.ticker,
                    doc.parse_class.as_deref().unwrap_or("x")
                ),
                text,
            );
        }
        let outcome = crate::fundamentals::management_holdings::parse_management_holdings(
            &extracted.sections,
            format,
        );
        resolved += 1;

        // Greedy 1:1 person matching by canonical identity.
        let labeled_persons: Vec<&GtRow> = doc.rows.iter().filter(|r| r.is_person()).collect();
        let mut consumed = vec![false; outcome.rows.len()];
        let mut score = DocScore {
            labeled: labeled_persons.len(),
            ..DocScore::default()
        };
        let mut missed: Vec<String> = Vec::new();
        for labeled in &labeled_persons {
            let person = labeled.person_raw.as_deref().unwrap_or("");
            let key = canonical_holder_identity(person);
            let hit = outcome.rows.iter().enumerate().find(|(i, emitted)| {
                !consumed[*i] && canonical_holder_identity(&emitted.person_raw) == key
            });
            match hit {
                Some((i, emitted)) => {
                    consumed[i] = true;
                    score.matched += 1;
                    if let Some(role) = labeled.role.as_deref() {
                        score.role_total += 1;
                        if emitted.role.map(|r| r.as_str()) == Some(role) {
                            score.role_ok += 1;
                        }
                    }
                    if let Some(shares) = labeled.shares {
                        score.share_total += 1;
                        if emitted
                            .shares
                            .as_deref()
                            .and_then(|s| s.parse::<i64>().ok())
                            == Some(shares)
                        {
                            score.share_ok += 1;
                        }
                    }
                }
                None => missed.push(person.to_owned()),
            }
        }

        eprintln!(
            "{label:<50} fmt={:<5} state={:<20} person={:>6} ({}/{})  role={:>6} ({}/{})  share={:>6} ({}/{})",
            format.as_str(),
            format!("{:?}", outcome.state),
            pct(score.matched, score.labeled),
            score.matched,
            score.labeled,
            pct(score.role_ok, score.role_total),
            score.role_ok,
            score.role_total,
            pct(score.share_ok, score.share_total),
            score.share_ok,
            score.share_total,
        );
        if !missed.is_empty() {
            eprintln!("    MISSED persons: {missed:?}");
            let emitted: Vec<String> = outcome
                .rows
                .iter()
                .map(|r| format!("{}={:?}", r.person_raw, r.shares))
                .collect();
            eprintln!("    EMITTED: {emitted:?}");
        }

        all.add(&score);
        if !is_residual_class(doc.parse_class.as_deref()) {
            parseable.add(&score);
        }
    }

    eprintln!("-- overall (all classes) --");
    eprintln!(
        "person = {} ({}/{})  role = {} ({}/{})  share = {} ({}/{})",
        pct(all.person_matched, all.person_labeled),
        all.person_matched,
        all.person_labeled,
        pct(all.role_correct, all.role_labeled),
        all.role_correct,
        all.role_labeled,
        pct(all.share_correct, all.share_labeled),
        all.share_correct,
        all.share_labeled,
    );
    eprintln!("-- parseable classes (the ≥95% person / ≥90% role floor set) --");
    eprintln!(
        "person = {} ({}/{})  role = {} ({}/{})  share = {} ({}/{})",
        pct(parseable.person_matched, parseable.person_labeled),
        parseable.person_matched,
        parseable.person_labeled,
        pct(parseable.role_correct, parseable.role_labeled),
        parseable.role_correct,
        parseable.role_labeled,
        pct(parseable.share_correct, parseable.share_labeled),
        parseable.share_correct,
        parseable.share_labeled,
    );
    eprintln!(
        "-- resolved {}/{} documents --",
        resolved,
        ground_truth.documents.len()
    );

    // Acceptance floors over the deterministic-parseable classes (residual classes
    // — glyph / prose-zero / missing / displaced — are legitimate misses).
    if parseable.person_labeled > 0 {
        let person_recall = parseable.person_matched as f64 / parseable.person_labeled as f64;
        let role_recall = if parseable.role_labeled > 0 {
            parseable.role_correct as f64 / parseable.role_labeled as f64
        } else {
            1.0
        };
        assert!(
            person_recall >= 0.95,
            "person recall {person_recall:.3} below the 0.95 acceptance floor (parseable classes)"
        );
        assert!(
            role_recall >= 0.90,
            "role recall {role_recall:.3} below the 0.90 acceptance floor (parseable classes)"
        );
    }
}

/// F-A3 live proof (owner dogfooding 2026-07-17): KRU had ZERO management-holdings
/// rows on the live DB because its holdings live ONLY in the SzD management report
/// (`doc_kind='other'`), which the periodic-only selection skipped. This asserts
/// the fixed selection now picks KRU's SzD AND that parsing it yields real holder
/// rows — the two halves of the live gap, end to end on the real bytes.
///
/// ```text
/// BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
///   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
///   cargo test -p brawler --lib real_data_kru_management_holdings_recovered -- --ignored --nocapture
/// ```
#[test]
#[ignore = "real-data F-A3 proof; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR"]
fn real_data_kru_management_holdings_recovered() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_kru_management_holdings_recovered: set BRAWLER_REAL_DB");
        return;
    };
    let connection = open_database(&db_path).expect("open real db");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };
    let Some(kru) = state
        .list_companies()
        .expect("companies")
        .into_iter()
        .find(|c| c.ticker.eq_ignore_ascii_case("KRU"))
    else {
        eprintln!("SKIP: no KRU company in the throwaway DB");
        return;
    };

    // 1) The fixed selection now enqueues the SzD (a `doc_kind='other'` document).
    let selected = state
        .management_holdings()
        .documents_needing_management_extraction(Some(&kru.id))
        .expect("selection");
    let documents = state
        .list_report_documents_by_company(&kru.id)
        .expect("documents");
    let szd = documents
        .iter()
        .find(|d| {
            d.url.to_lowercase().contains("szd_grupa_kruk")
                || d.title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains("z działalności"))
        })
        .expect("KRU SzD document present in the corpus");
    assert_eq!(
        szd.doc_kind.as_deref(),
        Some("other"),
        "SzD is a doc_kind=other filing"
    );
    assert!(
        selected.iter().any(|s| s.report_document_id == szd.id),
        "KRU's SzD must be selected for management-holdings extraction (F-A3)"
    );

    // 2) Parsing the SzD's real bytes yields holder rows (the live gap was selection,
    //    not the parser — the recall harness already scores KRU at 95.8%).
    let local_path = szd.local_path.as_deref().expect("SzD stored file");
    let bytes = std::fs::read(state.data_dir().join(local_path)).expect("read SzD");
    let format = SourceFormat::resolve(szd.content_type.as_deref(), local_path);
    let extracted = extract_report(&bytes, format);
    let outcome = crate::fundamentals::management_holdings::parse_management_holdings(
        &extracted.sections,
        format,
    );
    eprintln!(
        "== KRU SzD parse: state={:?}, {} row(s): {:?} ==",
        outcome.state,
        outcome.rows.len(),
        outcome
            .rows
            .iter()
            .map(|r| r.person_raw.as_str())
            .collect::<Vec<_>>()
    );
    assert!(
        !outcome.rows.is_empty(),
        "KRU's SzD must parse to management-holdings rows, got state {:?}",
        outcome.state
    );
}

/// Corpus PRECISION guard (F-A2, owner dogfooding 2026-07-17). Recall is measured
/// over 15 labeled documents; junk is a whole-corpus property. This runs the real
/// parser over EVERY selectable document of EVERY company (the same selection the
/// live extraction uses — periodic statements + SzD management reports) and asserts
/// that ZERO emitted person rows are junk: the plausibility gate leaves no bypass
/// path, and none of the notorious live-ABE junk tokens survive.
///
/// Run it manually (throwaway copy, like the recall harness):
///
/// ```text
/// BRAWLER_REAL_DB=private/realdata/worktest.sqlite3 \
///   BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
///   cargo test -p brawler --lib real_data_management_holdings_junk_rate -- --ignored --nocapture
/// ```
#[test]
#[ignore = "real-data precision guard; needs BRAWLER_REAL_DB + BRAWLER_REAL_DATA_DIR"]
fn real_data_management_holdings_junk_rate() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!("SKIP real_data_management_holdings_junk_rate: set BRAWLER_REAL_DB");
        return;
    };
    let connection = open_database(&db_path).expect("open real db (migrates the throwaway copy)");
    let state = match std::env::var("BRAWLER_REAL_DATA_DIR") {
        Ok(dir) => AppState::with_data_dir(connection, PathBuf::from(dir)),
        Err(_) => AppState::new(connection),
    };
    let mut companies = state.list_companies().expect("list companies");
    // Optional ticker filter (comma-separated) so the guard can run over a bounded
    // representative set — including the live junk producer (ABE) — when a
    // whole-corpus PDF sweep is too slow for a single run. Unset = the full corpus.
    if let Ok(filter) = std::env::var("BRAWLER_MGMT_JUNK_TICKERS") {
        let wanted: Vec<String> = filter
            .split(',')
            .map(|t| t.trim().to_uppercase())
            .filter(|t| !t.is_empty())
            .collect();
        companies.retain(|c| wanted.iter().any(|w| c.ticker.eq_ignore_ascii_case(w)));
    }

    // Independent of the gate predicate: substrings no natural person carries, drawn
    // from the live-ABE junk harvest — a belt-and-suspenders regression tripwire.
    // Whole-word junk indicators no natural-person name carries (an EMITTED person
    // row containing any of these is definitively junk). Independent of the gate
    // predicate — a genuine second opinion. NB: use precise stems, never a bare
    // " Sp" (which false-matches "Sprawozdawczości").
    const JUNK_SUBSTRINGS: [&str; 12] = [
        "Skarb",
        "Państwa",
        "Marketing",
        "Limited",
        "Izby",
        "Dyrektor",
        "Zleceniodawca",
        "Wystawca",
        "Europejska",
        "Sprawozdaw",
        "Finansow",
        "Standard",
    ];

    let mut docs_parsed = 0usize;
    let mut emitted_rows = 0usize;
    let mut junk_rows: Vec<(String, String)> = Vec::new(); // (ticker, name)
    let mut substring_hits: Vec<(String, String)> = Vec::new();

    for company in &companies {
        let documents = state
            .list_report_documents_by_company(&company.id)
            .expect("list report documents");
        for rd in &documents {
            if rd.fetch_status != "fetched" {
                continue;
            }
            let Some(local_path) = rd.local_path.as_deref() else {
                continue;
            };
            // Mirror the live selection: periodic statements + SzD management reports.
            let is_periodic = matches!(
                rd.doc_kind.as_deref(),
                Some("periodic_ssf") | Some("periodic_jsf")
            );
            let is_mgmt = crate::fundamentals::extraction::classify::is_management_report(
                rd.title.as_deref().unwrap_or(""),
                &rd.url,
            );
            if !is_periodic && !is_mgmt {
                continue;
            }
            let path = state.data_dir().join(local_path);
            let Ok(bytes) = std::fs::read(&path) else {
                continue;
            };
            let format = SourceFormat::resolve(rd.content_type.as_deref(), local_path);
            let extracted = extract_report(&bytes, format);
            let outcome = crate::fundamentals::management_holdings::parse_management_holdings(
                &extracted.sections,
                format,
            );
            docs_parsed += 1;
            for row in &outcome.rows {
                emitted_rows += 1;
                if crate::fundamentals::management_holdings::is_implausible_person(
                    &row.person_raw,
                    row.shares.as_deref(),
                ) {
                    junk_rows.push((company.ticker.clone(), row.person_raw.clone()));
                }
                if JUNK_SUBSTRINGS
                    .iter()
                    .any(|needle| row.person_raw.contains(needle))
                {
                    substring_hits.push((company.ticker.clone(), row.person_raw.clone()));
                }
            }
        }
    }

    let junk_rate = if emitted_rows == 0 {
        0.0
    } else {
        100.0 * junk_rows.len() as f64 / emitted_rows as f64
    };
    eprintln!(
        "== corpus junk-rate: {} doc(s) parsed, {} person row(s) emitted, {} junk ({:.2}%) ==",
        docs_parsed,
        emitted_rows,
        junk_rows.len(),
        junk_rate
    );
    if !junk_rows.is_empty() {
        eprintln!("  GATE-BYPASS junk rows: {junk_rows:?}");
    }
    if !substring_hits.is_empty() {
        eprintln!("  KNOWN-JUNK-SUBSTRING rows: {substring_hits:?}");
    }

    assert!(
        junk_rows.is_empty(),
        "the plausibility gate must leave no bypass path: {} junk row(s) emitted",
        junk_rows.len()
    );
    assert!(
        substring_hits.is_empty(),
        "known-junk substrings leaked into emitted rows: {substring_hits:?}"
    );
}
