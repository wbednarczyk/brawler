//! Guard (S3, testing.md § Source-tree guards; data-model.md § Model
//! principles, guardrail `d60305c`): "latest" selection must sort by the
//! DOMAIN date, never `created_at` — a backfill/re-ingest can give an OLD
//! record a NEWER `created_at`.
//!
//! Scans every production `.rs` file under `src-tauri/src/**` for a SQL
//! `ORDER BY` clause (case-insensitive, `OVER (…)` windows, comments
//! excluded) whose leading key is `created_at`, checked against the frozen,
//! ratchet-only allowlist ([`ALLOWED`]: `(file, fn, ordinal, Reason)`).
//!
//! Second scan (#496): every Rust comparator that ranks on `created_at`
//! (`sort_by`/`sort_by_key`/`max_by`/`min_by`/`Reverse(`/`.cmp(`/
//! `partial_cmp(` within one statement-sized window of a `.created_at`
//! field read), checked against [`ALLOWED_COMPARATORS`] — the SQL scan
//! could not see `history_sweep`'s sibling rank, the KPI-table cell pick or
//! the report-diff representative, all of which ranked on insert time.
//!
//! Ceilings: literal-split fragments are invisible; the enclosing-fn lookup
//! is a text heuristic (no brace-depth tracking); a comparator built from a
//! `created_at` value bound to a local first (`let t = x.created_at; …
//! .cmp(&t)`), a bare `a.created_at > b.created_at`, or a `created_at`
//! merely read near an unrelated `.cmp(` (pinned as
//! `NotRankedOnCreatedAt`) are the window scan's known blind spots.

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use super::scan::{
    classified_non_code_spans, is_test_file, source_files, strip_test_spans, SpanKind,
};
use super::{extract_test_fn_name, is_fn_signature};

/// Why a leading-`created_at` `ORDER BY` site is allowed to stay as-is
/// (data-model.md § Model principles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Reason {
    /// A queue/run/job-processing order: `created_at` legitimately models
    /// enqueue/attempt/claim chronology (when the work arrived), not a
    /// domain "latest report/event" selection.
    QueueOrRunChronology,
    /// User-authored content (a note, an evidence link, an alert rule) with
    /// no other domain date — `created_at` (authoring time) IS the
    /// meaningful order here.
    LocalAuthoringOrder,
    /// A point-in-time assessment/evaluation snapshot, ordered by when it
    /// was computed — `created_at` is the domain date for a snapshot.
    AssessmentSnapshot,
    /// Attachments/rows sharing ONE publication event — `created_at` orders
    /// artifacts of the SAME event, not competing "latest" candidates.
    SharedPublicationEvent,
    /// A genuine offender, not fixed yet — tracked by the named issue.
    /// Unused since #496 paid the three pinned debts; kept so the next
    /// offender must still carry its issue (never a bare allow).
    #[allow(dead_code)]
    Debt(&'static str),
    /// Comparator scan only: the statement-sized window matched a comparator
    /// that ranks on ANOTHER key while `created_at` is merely read nearby
    /// (heuristic ceiling, reviewed by hand).
    NotRankedOnCreatedAt,
}

/// Every leading-`created_at` `ORDER BY` site this guard finds today,
/// reviewed and pinned. Per-fn ratchet (wave-1 idiom, `storage_writes.rs`):
/// an entry whose site no longer matches must be deleted (an improvement can
/// never silently re-regress); a new offender outside this list fails loud.
///
/// `(file, fn, ordinal, reason)` — `file` is repo-relative under
/// `src-tauri/src/` (no `src/` prefix, matching `storage_writes.rs`'s
/// `FROZEN_UNTRANSACTED_WRITERS` convention); `ordinal` is the 1-based index
/// of the site within that fn (today, every listed fn has exactly one).
const ALLOWED: &[(&str, &str, u8, Reason)] = &[
    // QueueOrRunChronology — `created_at` orders queue/run/attempt
    // chronology, not a "latest domain thing" selection.
    (
        "storage/kpi_ingest_runs.rs",
        "list_runs",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/kpi_ingest_runs.rs",
        "list_pending_runs",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/kpi_ingest_runs.rs",
        "claim_next",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/history_sweeps.rs",
        "get_latest_history_sweep",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/transcripts.rs",
        "list_transcript_jobs",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/transcripts.rs",
        "find_existing_transcript_job",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/activity_reads.rs",
        "queued_transcript_jobs",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/autopilot.rs",
        "list_runs",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/pipeline_reextraction.rs",
        "create_batch_with_job_if_none_active",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/pipeline_reextraction.rs",
        "get_latest_batch",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/signals.rs",
        "list_signals_needing_event_date",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/insider.rs",
        "load_pending",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/ownership.rs",
        "list_extraction_residuals",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/ownership.rs",
        "documents_needing_ownership_extraction",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/ownership.rs",
        "residuals_needing_ocr",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/ownership.rs",
        "load_pending_major_holdings",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/management_holdings.rs",
        "list_residuals",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/management_holdings.rs",
        "documents_needing_management_extraction",
        1,
        Reason::QueueOrRunChronology,
    ),
    (
        "storage/report_documents.rs",
        "list_pending_attachments",
        1,
        Reason::QueueOrRunChronology,
    ),
    // LocalAuthoringOrder — user-authored content with no other domain date.
    (
        "storage/attention.rs",
        "list_alert_rules",
        1,
        Reason::LocalAuthoringOrder,
    ),
    (
        "storage/notebooks.rs",
        "notebook_entry_origins",
        1,
        Reason::LocalAuthoringOrder,
    ),
    (
        "storage/research.rs",
        "list_evidence_links",
        1,
        Reason::LocalAuthoringOrder,
    ),
    (
        "storage/import_export/export.rs",
        "export_evidence_links",
        1,
        Reason::LocalAuthoringOrder,
    ),
    (
        "storage/import_export/export.rs",
        "export_notebook_origins",
        1,
        Reason::LocalAuthoringOrder,
    ),
    (
        "storage/report_expectations.rs",
        "list_report_expectations",
        1,
        Reason::LocalAuthoringOrder,
    ),
    // AssessmentSnapshot — a point-in-time evaluation, ordered by when it ran.
    (
        "storage/quality_frameworks.rs",
        "list_framework_evaluations",
        1,
        Reason::AssessmentSnapshot,
    ),
    (
        "storage/quality_frameworks.rs",
        "qualitative_verdict_changes",
        1,
        Reason::AssessmentSnapshot,
    ),
    // SharedPublicationEvent — attachments of one feed item share the event.
    (
        "storage/report_documents.rs",
        "list_by_origin",
        1,
        Reason::SharedPublicationEvent,
    ),
];

/// One found leading-`created_at` `ORDER BY` site: its location (repo-relative
/// path under `src-tauri/src/`, e.g. `"storage/foo.rs"`) plus the enclosing
/// fn's name and its 1-based ordinal among sites found in the SAME fn.
#[derive(Debug, PartialEq, Eq)]
struct Site {
    file: String,
    function: String,
    ordinal: u8,
}

/// Whether the text right after "ORDER BY" is a leading `created_at` key:
/// bare `created_at`, an aliased `x.created_at`, or
/// `datetime(created_at)`/`datetime(x.created_at)`. Reads exactly one token
/// (a dotted identifier, or a `datetime(...)` call) — a following `DESC`/
/// `ASC`, `,`, `)`, `LIMIT`, or the string literal's own closing quote is
/// whatever non-identifier character stops the read, so none of those need
/// separate handling.
fn leading_key_is_created_at(after_order_by: &str) -> bool {
    let trimmed = after_order_by.trim_start();
    if let Some(rest) = trimmed.strip_prefix("datetime(") {
        let Some(close) = rest.find(')') else {
            return false;
        };
        let inner = rest[..close].trim();
        return inner == "created_at" || inner.ends_with(".created_at");
    }
    let is_ident_char = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.';
    let key_len = trimmed.find(|c| !is_ident_char(c)).unwrap_or(trimmed.len());
    let key = &trimmed[..key_len];
    key == "created_at" || key.ends_with(".created_at")
}

/// Byte offsets of every "ORDER BY" occurrence in `content` whose leading
/// sort key is `created_at` — commentary (a `//`/`/* */` span) is excluded;
/// a match inside a string literal (real SQL text) or bare code is examined
/// identically, since [`leading_key_is_created_at`] reads only the one
/// token it needs and stops at the first non-identifier character either
/// way (whitespace, `,`, `)`, or the literal's own closing quote).
fn order_by_marker() -> &'static regex::Regex {
    static MARKER: OnceLock<regex::Regex> = OnceLock::new();
    // SQLite keywords are case-insensitive; `\s+` covers a marker split across
    // lines (`ORDER\n  BY`).
    MARKER.get_or_init(|| regex::Regex::new(r"(?i)order\s+by").expect("valid regex"))
}

fn leading_created_at_order_by_offsets(content: &str) -> Vec<usize> {
    let spans = classified_non_code_spans(content);
    let mut offsets = Vec::new();
    for m in order_by_marker().find_iter(content) {
        let pos = m.start();
        let after = m.end();

        let in_comment = spans
            .iter()
            .any(|(start, end, kind)| *kind == SpanKind::Comment && *start <= pos && pos < *end);
        if in_comment {
            continue; // commentary, not a real query
        }
        if leading_key_is_created_at(&content[after..]) {
            offsets.push(pos);
        }
    }
    offsets
}

/// The nearest enclosing `fn <name>` name for a byte offset in `content` —
/// the nearest preceding line matching `is_fn_signature`, scanning backward
/// (text-level: no brace-depth tracking, matching the same heuristic every
/// other guard in this module uses).
fn enclosing_fn_name(content: &str, offset: usize) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut acc = 0usize;
    let mut line_idx = lines.len().saturating_sub(1);
    for (idx, line) in lines.iter().enumerate() {
        let line_end = acc + line.len() + 1; // +1 for the '\n'
        if offset < line_end {
            line_idx = idx;
            break;
        }
        acc = line_end;
    }
    (0..=line_idx)
        .rev()
        .find(|&i| is_fn_signature(lines[i]))
        .map(|i| extract_test_fn_name(lines[i]))
}

/// Every leading-`created_at` `ORDER BY` site in one file's (already
/// test-span-stripped) content, `ordinal`s assigned sequentially per fn in
/// the order found.
fn sites_in_file(rel: &str, content: &str) -> Vec<Site> {
    let mut per_fn_counts: HashMap<String, u8> = HashMap::new();
    let mut sites = Vec::new();
    for offset in leading_created_at_order_by_offsets(content) {
        let Some(function) = enclosing_fn_name(content, offset) else {
            continue; // heuristic ceiling: no enclosing fn found; none of today's sites hit this
        };
        let ordinal = per_fn_counts.entry(function.clone()).or_insert(0);
        *ordinal += 1;
        sites.push(Site {
            file: rel.to_string(),
            function,
            ordinal: *ordinal,
        });
    }
    sites
}

/// Every leading-`created_at` `ORDER BY` site across the production source
/// tree (test files and test spans excluded).
fn find_sites() -> Vec<Site> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut sites = Vec::new();
    for path in source_files(&src_dir) {
        let rel = path
            .strip_prefix(&src_dir)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/");
        if is_test_file(&rel) {
            continue;
        }
        let raw = std::fs::read_to_string(&path).expect("readable source file");
        let content = strip_test_spans(&raw);
        sites.extend(sites_in_file(&rel, &content));
    }
    sites
}

/// Guard (S3): see module docs. Frozen-allowlist idiom — a NEW offender
/// outside [`ALLOWED`] fails loud; an allowlisted site that no longer
/// matches must be deleted (ratchet shrinks only); a fn with MORE offending
/// sites than pinned ordinals for it fails too (an unreviewed new site in an
/// already-listed fn).
#[test]
fn recency_selection_never_leads_with_created_at() {
    let sites = find_sites();
    let mut violations = Vec::new();

    for site in &sites {
        let allowed = ALLOWED.iter().any(|(file, function, ordinal, _reason)| {
            *file == site.file && *function == site.function && *ordinal == site.ordinal
        });
        if !allowed {
            violations.push(format!(
                "{}:{} (site #{}): ORDER BY leads with created_at — domain \
                 recency must order by the DOMAIN date, never created_at \
                 (data-model.md § Model principles, guardrail d60305c). Either \
                 fix the query to sort on the domain date, or add a reviewed \
                 entry to ALLOWED with a Reason (Reason::Debt needs a tracked \
                 issue)",
                site.file, site.function, site.ordinal
            ));
        }
    }

    for (file, function, ordinal, reason) in ALLOWED {
        let still_matches = sites
            .iter()
            .any(|s| s.file == *file && s.function == *function && s.ordinal == *ordinal);
        if !still_matches {
            violations.push(format!(
                "{file}:{function} (site #{ordinal}, {reason:?}): no longer a \
                 leading-created_at ORDER BY site — delete this entry from \
                 ALLOWED so it can never silently regress back (per-site \
                 ratchet)"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "recency source-tree guard (S3):\n{}",
        violations.join("\n")
    );
}

#[cfg(test)]
mod scanner_tests {
    use super::{enclosing_fn_name, leading_created_at_order_by_offsets, sites_in_file};
    use crate::source_tree_guards::scan::strip_test_spans;

    fn fn_names_for(source: &str) -> Vec<String> {
        leading_created_at_order_by_offsets(source)
            .into_iter()
            .filter_map(|offset| enclosing_fn_name(source, offset))
            .collect()
    }

    #[test]
    fn matches_a_bare_created_at_leading_key() {
        let src = "fn f() {\n    let sql = \"SELECT * FROM t ORDER BY created_at DESC\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn matches_an_aliased_created_at_leading_key() {
        let src = "fn f() {\n    let sql = \"SELECT * FROM t ORDER BY x.created_at, id\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn matches_a_datetime_wrapped_created_at_leading_key() {
        let src =
            "fn f() {\n    let sql = \"SELECT * FROM t ORDER BY datetime(f.created_at) DESC, id\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn matches_a_multiline_clause() {
        let src = "fn f() {\n    let sql = \"SELECT * FROM t\n         ORDER BY\n         candidate.created_at, id\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn matches_a_lowercase_marker() {
        let src = "fn f() {\n    let sql = \"select * from t order by created_at desc\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn matches_a_marker_split_across_lines() {
        let src = "fn f() {\n    let sql = \"SELECT * FROM t ORDER\n  BY created_at DESC\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn matches_inside_a_window_function() {
        let src =
            "fn f() {\n    let sql = \"SELECT ROW_NUMBER() OVER (ORDER BY created_at DESC) FROM t\";\n}\n";
        assert_eq!(fn_names_for(src), vec!["f".to_string()]);
    }

    #[test]
    fn assigns_sequential_ordinals_for_two_sites_in_the_same_fn() {
        let src = "fn f() {\n    let a = \"SELECT * FROM t ORDER BY created_at DESC\";\n    let b = \"SELECT * FROM u ORDER BY created_at ASC\";\n}\n";
        let sites = sites_in_file("storage/x.rs", src);
        let ordinals: Vec<u8> = sites.iter().map(|s| s.ordinal).collect();
        assert_eq!(
            ordinals,
            vec![1, 2],
            "ordinals must be assigned sequentially per fn"
        );
        assert!(
            sites.iter().all(|s| s.function == "f"),
            "both sites belong to the same enclosing fn"
        );
    }

    #[test]
    fn does_not_match_a_domain_date_leading_key_with_created_at_as_a_tie_breaker() {
        let src =
            "fn f() {\n    let sql = \"SELECT * FROM t ORDER BY as_of DESC, created_at DESC, id DESC\";\n}\n";
        assert!(
            fn_names_for(src).is_empty(),
            "a domain-date leading key with created_at only as a tie-breaker must not match"
        );
    }

    #[test]
    fn does_not_match_order_by_inside_a_comment() {
        let src = "fn f() {\n    // ORDER BY created_at DESC (just an example in a comment)\n    let x = 1;\n}\n";
        assert!(
            fn_names_for(src).is_empty(),
            "a comment mentioning the phrase is not a real query"
        );
    }

    #[test]
    fn does_not_match_order_by_inside_a_cfg_test_block() {
        let src = "fn real() {}\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let sql = \"SELECT * FROM t ORDER BY created_at DESC\";\n    }\n}\n";
        let stripped = strip_test_spans(src);
        assert!(
            fn_names_for(&stripped).is_empty(),
            "a #[cfg(test)] mod's content must be excluded from the production scan"
        );
    }
}

// ---------------------------------------------------------------------------
// Rust-comparator scan (#496): `created_at` ranked in Rust, not SQL.
// ---------------------------------------------------------------------------

/// Every Rust site that ranks on `created_at` today, reviewed and pinned
/// `(file, fn, sites, Reason)` — `sites` is how many distinct comparison
/// expressions in that fn read `created_at`; same ratchet idiom as
/// [`ALLOWED`]: a listed fn that vanishes must be deleted, a fn with MORE
/// sites than pinned fails loud (an unreviewed comparator inside an exempted
/// fn), a new fn fails loud.
const ALLOWED_COMPARATORS: &[(&str, &str, u8, Reason)] = &[
    // The attachments of ONE filing, newest registered first — artifacts of
    // the same publication event, not competing "latest" candidates.
    (
        "jobs/insider_attachment.rs",
        "process_filing",
        1,
        Reason::SharedPublicationEvent,
    ),
    // `entries.sort_by_key(Reverse(modified))` ranks backup files by mtime;
    // `entry.created_at` is only READ two lines later (window false positive).
    (
        "storage/backup.rs",
        "collect_status",
        1,
        Reason::NotRankedOnCreatedAt,
    ),
];

/// How far (bytes) around a `.created_at` read a comparator marker still
/// counts as ranking on it — one statement-sized window, so a multi-line
/// `right\n.created_at\n.cmp(&left.created_at)` is one site and a
/// `format!("{}", x.created_at)` two statements away is not.
const COMPARATOR_WINDOW: usize = 160;

fn created_at_read_marker() -> &'static regex::Regex {
    static MARKER: OnceLock<regex::Regex> = OnceLock::new();
    MARKER.get_or_init(|| regex::Regex::new(r"\.created_at\b").expect("valid regex"))
}

fn comparator_marker() -> &'static regex::Regex {
    static MARKER: OnceLock<regex::Regex> = OnceLock::new();
    MARKER.get_or_init(|| {
        regex::Regex::new(
            r"sort_by\(|sort_by_key\(|sort_by_cached_key\(|sort_unstable_by\(|sort_unstable_by_key\(|max_by\(|max_by_key\(|min_by\(|min_by_key\(|Reverse\(|\.cmp\(|partial_cmp\(",
        )
        .expect("valid regex")
    })
}

/// Byte offsets of every `.created_at` field read in code (comments and
/// string literals excluded — SQL text is the other scan's job) that has a
/// comparator marker within [`COMPARATOR_WINDOW`] bytes on either side.
/// Reads closer than the window to the previous hit are folded into it (one
/// comparison expression reads the field twice).
fn created_at_comparator_offsets(content: &str) -> Vec<usize> {
    // Blank every comment and string literal to spaces (same byte length, so
    // offsets stay valid): neither a `// sort_by(created_at)` remark nor a SQL
    // literal's text may count as a marker or as a field read.
    let mut code = content.as_bytes().to_vec();
    for (start, end, _kind) in classified_non_code_spans(content) {
        for byte in &mut code[start..end] {
            if !byte.is_ascii_whitespace() {
                *byte = b' ';
            }
        }
    }
    let code = String::from_utf8(code).expect("blanking keeps ASCII/UTF-8 boundaries intact");

    let mut offsets: Vec<usize> = Vec::new();
    for m in created_at_read_marker().find_iter(&code) {
        let pos = m.start();
        let lo = pos.saturating_sub(COMPARATOR_WINDOW);
        let hi = (m.end() + COMPARATOR_WINDOW).min(code.len());
        // Clamp to char boundaries (the window is a byte count).
        let lo = (lo..=pos)
            .find(|&i| code.is_char_boundary(i))
            .unwrap_or(pos);
        let hi = (m.end()..=hi)
            .rev()
            .find(|&i| code.is_char_boundary(i))
            .unwrap_or(m.end());
        if !comparator_marker().is_match(&code[lo..hi]) {
            continue;
        }
        if offsets
            .last()
            .is_some_and(|&prev| pos - prev <= COMPARATOR_WINDOW)
        {
            continue; // the same comparison reading the field again
        }
        offsets.push(pos);
    }
    offsets
}

/// `(file, fn, sites)` for every fn in one file's (test-span-stripped)
/// content that ranks on `created_at` — `sites` counts distinct comparison
/// expressions (window-folded), so a second comparator added to an exempted
/// fn is visible to the ratchet.
fn comparator_sites_in_file(rel: &str, content: &str) -> Vec<(String, String, u8)> {
    let mut sites: Vec<(String, String, u8)> = Vec::new();
    for offset in created_at_comparator_offsets(content) {
        let Some(function) = enclosing_fn_name(content, offset) else {
            continue;
        };
        match sites.iter_mut().find(|(_, f, _)| *f == function) {
            Some(entry) => entry.2 += 1,
            None => sites.push((rel.to_string(), function, 1)),
        }
    }
    sites
}

fn find_comparator_sites() -> Vec<(String, String, u8)> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = manifest_dir.join("src");
    let mut sites = Vec::new();
    for path in source_files(&src_dir) {
        let rel = path
            .strip_prefix(&src_dir)
            .expect("under src")
            .to_string_lossy()
            .replace('\\', "/");
        if is_test_file(&rel) || rel.starts_with("source_tree_guards/") {
            continue;
        }
        let raw = std::fs::read_to_string(&path).expect("readable source file");
        let content = strip_test_spans(&raw);
        sites.extend(comparator_sites_in_file(&rel, &content));
    }
    sites
}

/// Guard (#496): the SQL rule applies to Rust ranking too — see
/// [`ALLOWED_COMPARATORS`] and the module docs.
#[test]
fn recency_selection_never_ranks_on_created_at_in_rust() {
    let sites = find_comparator_sites();
    let mut violations = Vec::new();

    for (file, function, count) in &sites {
        let pinned = ALLOWED_COMPARATORS
            .iter()
            .find(|(f, func, _sites, _reason)| f == file && func == function)
            .map(|(_, _, sites, _)| *sites);
        match pinned {
            Some(pinned) if pinned >= *count => {}
            _ => violations.push(format!(
                "{file}:{function} ({count} comparison site(s), pinned {}): a Rust \
                 comparator ranks on created_at — domain recency must order by the \
                 DOMAIN date (a document's disclosure_key(), a fact's \
                 canonical_fact_rank(), a period's end date), never created_at \
                 (data-model.md § Model principles, guardrail d60305c, #496). Fix \
                 the comparator, or add a reviewed entry to ALLOWED_COMPARATORS \
                 with a Reason",
                pinned.map_or("none".to_owned(), |p| p.to_string())
            )),
        }
    }
    for (file, function, pinned, reason) in ALLOWED_COMPARATORS {
        let found = sites
            .iter()
            .find(|(f, func, _)| f == file && func == function)
            .map(|(_, _, count)| *count);
        match found {
            Some(count) if count == *pinned => {}
            Some(count) => violations.push(format!(
                "{file}:{function} ({reason:?}): {count} comparison site(s) but {pinned} \
                 pinned — ratchet the pin down to {count} (per-site ratchet)"
            )),
            None => violations.push(format!(
                "{file}:{function} ({reason:?}): no longer a created_at comparator \
                 site — delete this entry from ALLOWED_COMPARATORS (per-site ratchet)"
            )),
        }
    }

    assert!(
        violations.is_empty(),
        "recency source-tree guard (#496, Rust comparators):\n{}",
        violations.join("\n")
    );
}

#[cfg(test)]
mod comparator_scanner_tests {
    use super::{comparator_sites_in_file, created_at_comparator_offsets};

    #[test]
    fn a_reverse_on_created_at_inside_a_rank_fn_is_one_site() {
        let source = "fn rank(d: &Doc) -> (u8, std::cmp::Reverse<String>) {\n    (0, std::cmp::Reverse(d.created_at.clone()))\n}\n";
        assert_eq!(
            comparator_sites_in_file("x.rs", source),
            vec![("x.rs".to_string(), "rank".to_string(), 1)]
        );
    }

    #[test]
    fn a_multi_line_cmp_reading_the_field_twice_is_one_site() {
        let source = "fn pick(c: &mut Vec<Doc>) {\n    c.sort_by(|left, right| {\n        right\n            .created_at\n            .cmp(&left.created_at)\n    });\n}\n";
        assert_eq!(created_at_comparator_offsets(source).len(), 1);
        assert_eq!(
            comparator_sites_in_file("x.rs", source),
            vec![("x.rs".to_string(), "pick".to_string(), 1)]
        );
    }

    #[test]
    fn two_separate_comparisons_in_one_fn_count_as_two_sites() {
        let source = concat!(
            "fn pick(c: &mut Vec<Doc>, d: &mut Vec<Doc>) {\n",
            "    c.sort_by_cached_key(|x| x.created_at.clone());\n",
            "    let _ = c.len(); let _ = c.len(); let _ = c.len(); let _ = c.len();\n",
            "    let _ = c.len(); let _ = c.len(); let _ = c.len(); let _ = c.len();\n",
            "    let _ = c.len(); let _ = c.len(); let _ = c.len(); let _ = c.len();\n",
            "    d.sort_by_key(|x| std::cmp::Reverse(x.created_at.clone()));\n",
            "}\n",
        );
        assert_eq!(
            comparator_sites_in_file("x.rs", source),
            vec![("x.rs".to_string(), "pick".to_string(), 2)]
        );
    }

    #[test]
    fn a_plain_read_a_comment_and_a_sql_literal_are_not_sites() {
        let source = concat!(
            "fn label(d: &Doc) -> String {\n",
            "    // sort_by(created_at) would be wrong here\n",
            "    let sql = \"SELECT 1 ORDER BY created_at\"; let _ = sql;\n",
            "    format!(\"{}\", d.created_at)\n",
            "}\n",
        );
        assert!(comparator_sites_in_file("x.rs", source).is_empty());
    }
}
