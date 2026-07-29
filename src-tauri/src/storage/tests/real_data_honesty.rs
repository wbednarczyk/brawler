//! Real-data **honesty** harness (epic #40 S4/S5; ADR 0091 decisions 4-5).
//!
//! Measures, on the maintainer's real database, how honest the app is about what
//! actually happened: does a Today row state something CONCRETE, does its
//! evidence still resolve, does a raw filename ever stand in for prose — and
//! (S5) does a recorded SUCCESS admit it produced nothing, does a missing number
//! say what is missing.
//!
//! **Inert in CI** — like [`super::real_data_extraction`] it skips unless
//! `BRAWLER_REAL_DB` points at a THROWAWAY copy of the real DB, so `make check`
//! never runs it. The real database never enters the public repo or default CI
//! (ADR 0091 decision 4): the only thing that gets committed is the aggregate
//! baseline (`realdata-honesty-baseline.json`) — counts and percentages, never
//! a title, a ticker, or any other row content. Nothing in this file prints or
//! serializes row content; keep it that way.
//!
//! Metrics are computed **through the real read models**
//! ([`AttentionStore::list_attention_events`],
//! [`FundamentalsProvenanceStore::list_extraction_outcomes`],
//! [`crate::commands::company_health::compute_company_health`]) and the real
//! frontend statement rule — never a parallel SQL path — so the numbers move
//! exactly when the app the owner uses moves:
//!
//! | metric | meaning | gate |
//! |---|---|---|
//! | `specificity_pct` | share of title-capable events whose row states something concrete | ratcheted floor |
//! | `orphaned_evidence` | events whose evidence resolves to nothing (snapshot NULL ∧ join empty) | ratcheted ceiling |
//! | `filename_as_statement` | rendered statements that are a raw filename | hard `0` |
//! | `zero_effect_successes` | outcome rows that succeeded, recorded zero facts, and claim an emission instead of naming a why | ratcheted ceiling |
//! | `silent_missing_metrics` | health read-model outputs reporting a number as missing without naming what | ratcheted ceiling |
//!
//! The S5 pair measures the STORED record. The same class is enforced hard, in
//! `make check` and with no real data, by the per-shape zero-effects invariants
//! over [`crate::effects_honesty::ExplainsEffect`] — a fixed defect still leaves
//! a residue in old rows that only this harness can see.
//!
//! The floors/ceilings live in the committed baseline and are enforced by
//! `scripts/check/realdata-ratchet.mjs` (run by `make realdata-honesty-check`),
//! not by this test: the harness reports, the ratchet judges.
//!
//! Run it manually:
//!
//! ```text
//! cp private/realdata/brawler.sqlite3 private/realdata/honesty-worktest.sqlite3
//! BRAWLER_REAL_DB=private/realdata/honesty-worktest.sqlite3 \
//!   cargo test -p brawler --lib real_data_honesty -- --ignored --nocapture
//! ```
//!
//! or, with the copy + ratchet in one step, `make realdata-honesty-check`.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use regex::Regex;

use super::*;
use crate::storage::attention::{
    EVIDENCE_AUTOPILOT_RUN, EVIDENCE_COMPANY_SIGNAL, EVIDENCE_JOB, EVIDENCE_SOURCE_RECONCILIATION,
    TRIGGER_PRICE_ENTERS_RANGE, TRIGGER_PRICE_WEEK52_LOW,
};

// ---------------------------------------------------------------------------
// The filename pattern — Rust mirror of the canonical TypeScript source
// ---------------------------------------------------------------------------

/// The canonical filename-pattern source. The TS module is the single source of
/// truth (ADR 0091 decision 5); the constants below are a mirror, kept honest
/// by [`filename_pattern_mirrors_canonical_typescript`]. Reached via the
/// build.rs env indirection, never a literal `../` path — the mutants sandbox
/// copies only `src-tauri/` (#110, `source_tree_guards`).
const CANONICAL_TS_SOURCE: &str = include_str!(env!("BRAWLER_DOCUMENT_TITLE_TS"));

/// Rust mirror of `FILENAME_EXTENSION` in `src/screens/Today/documentTitle.ts`.
/// Case-insensitive via an inline flag, because the TS literal carries `/i`.
pub(crate) const FILENAME_EXTENSION_PATTERN: &str = r"(?i)\.(?:xhtml|html|htm|pdf|zip)";

/// Rust mirror of `LEADING_SEPARATORS` in the same TS module — the separators /
/// glue trimmed off the front of the human remainder after a split.
const LEADING_SEPARATORS_PATTERN: &str = r"^[\s–—:;,.-]+";

pub(super) fn filename_extension() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(FILENAME_EXTENSION_PATTERN).expect("valid filename pattern"))
}

fn leading_separators() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(LEADING_SEPARATORS_PATTERN).expect("valid separator pattern"))
}

/// The statement a Today row actually renders for an evidence title — the Rust
/// mirror of `splitDocumentTitle(...).statement` (`documentTitle.ts`), which
/// `AttentionRow.tsx` applies before composing the row's sentence. `None` means
/// the row falls back to generic copy (no title, or a filename-only title).
///
/// Measuring the raw `evidence_title` instead would lie in both directions: a
/// glued `"…​.xhtmlJednostkowe Sprawozdanie…"` renders CLEAN prose (so it is not
/// a filename-as-statement), and a filename-only title renders generic copy (so
/// it is not specific).
pub(super) fn rendered_statement(evidence_title: Option<&str>) -> Option<String> {
    let trimmed = evidence_title.unwrap_or_default().trim();
    if trimmed.is_empty() {
        return None;
    }
    let Some(matched) = filename_extension().find(trimmed) else {
        return Some(trimmed.to_owned());
    };
    let remainder = &trimmed[matched.end()..];
    let remainder = leading_separators().replace(remainder, "");
    let remainder = remainder.trim();
    (!remainder.is_empty()).then(|| remainder.to_owned())
}

/// Evidence types whose title is resolved by one of the guarded `LEFT JOIN`s in
/// [`list_attention_events`] — the only rows for which "the evidence resolved to
/// nothing" is a meaningful statement. `daily_quote` has no join (a price event
/// states its rule's range), so it is not orphanable.
pub(super) const JOINED_EVIDENCE_TYPES: [&str; 4] = [
    EVIDENCE_COMPANY_SIGNAL,
    EVIDENCE_SOURCE_RECONCILIATION,
    EVIDENCE_AUTOPILOT_RUN,
    EVIDENCE_JOB,
];

// ---------------------------------------------------------------------------
// Effects honesty (epic #40 S5) — the stored vocabulary
// ---------------------------------------------------------------------------

/// The `fundamentals_extraction_outcomes.acceptance` values that mean the run
/// SUCCEEDED — it accepted a set and persisted it. Mirrors
/// [`crate::fundamentals::extraction::pipeline::Acceptance::emits`]; the CHECK
/// in migration `0105` is the authoritative vocabulary.
pub(super) const EMITTING_ACCEPTANCES: [&str; 3] =
    ["accepted", "accepted_via_witness", "accepted_unreviewed"];

/// The `reason_code` values that CLAIM production rather than explain its
/// absence. Every other code in the vocabulary (`validation_failed`,
/// `structure_drift`, `witness_disagreement`, `no_deterministic_tier`,
/// `no_period_derived`, `document_unreadable`) names a why, so a zero-fact row
/// carrying one of those is honest — it says what stopped it.
pub(super) const PRODUCTION_CLAIMING_REASONS: [&str; 2] = ["emitted", "witness_fallback"];

/// Triggers excluded from the specificity denominator **by definition**: a price
/// event's statement IS its rule's range / the 52-week-low label — generic copy
/// is the correct, honest rendering, not a failure to be specific. Mirrors the
/// price entries of `GENERIC_FALLBACK_KEYS` in
/// `tests/live/ux-checkpoint.live.spec.ts` ("Price range", "52-week low").
pub(super) const NON_TITLE_TRIGGERS: [&str; 2] =
    [TRIGGER_PRICE_ENTERS_RANGE, TRIGGER_PRICE_WEEK52_LOW];

// ---------------------------------------------------------------------------
// Parity + port gates (these run in `make check` — no real data needed)
// ---------------------------------------------------------------------------

/// Extract a `const <name> = /<body>/<flags>;` literal (exported or not) from the
/// canonical TS module. Deliberately dumb: any reshaping of the declaration
/// reddens here rather than silently reading the wrong regex.
fn typescript_regex_literal(name: &str) -> (String, String) {
    let needle = format!("const {name} = /");
    let start = CANONICAL_TS_SOURCE
        .find(&needle)
        .unwrap_or_else(|| panic!("`{needle}` not found in documentTitle.ts"));
    let rest = &CANONICAL_TS_SOURCE[start + needle.len()..];
    let close = rest
        .find('/')
        .unwrap_or_else(|| panic!("unterminated regex literal for {name}"));
    let body = rest[..close].to_owned();
    let flags: String = rest[close + 1..]
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .collect();
    (body, flags)
}

/// ADR 0091 decision 5: ONE source of truth for the filename pattern across
/// languages. The canonical TS regex and the Rust mirror must stay identical —
/// otherwise the specificity/filename metrics and the UI disagree about what
/// counts as a filename, and the harness certifies an honesty the user never
/// sees. Idiom: the `include_str!` classification gate in `mcp/registry.rs`.
#[test]
fn filename_pattern_mirrors_canonical_typescript() {
    let (extension_body, extension_flags) = typescript_regex_literal("FILENAME_EXTENSION");
    assert_eq!(
        extension_flags, "i",
        "canonical FILENAME_EXTENSION lost its /i flag — the Rust mirror's inline (?i) no longer \
         mirrors it (src/screens/Today/documentTitle.ts)"
    );
    assert_eq!(
        FILENAME_EXTENSION_PATTERN,
        format!("(?i){extension_body}"),
        "FILENAME_EXTENSION drifted between the canonical TS source and the Rust mirror \
         (ADR 0091 dec. 5). Update FILENAME_EXTENSION_PATTERN to match \
         src/screens/Today/documentTitle.ts — never the other way round."
    );

    let (separators_body, separators_flags) = typescript_regex_literal("LEADING_SEPARATORS");
    assert_eq!(
        separators_flags, "",
        "canonical LEADING_SEPARATORS gained regex flags the Rust mirror does not carry"
    );
    assert_eq!(
        LEADING_SEPARATORS_PATTERN, separators_body,
        "LEADING_SEPARATORS drifted between the canonical TS source and the Rust mirror \
         (ADR 0091 dec. 5)."
    );

    // Both mirrors must actually compile as Rust regexes (a TS-only construct
    // would pass the string comparison and blow up at measurement time).
    assert!(filename_extension().is_match("report.PDF"));
    assert!(!filename_extension().is_match("Wyniki za I kwartał 2026"));
    assert!(leading_separators().is_match(" — statement"));
}

/// The Rust port of `splitDocumentTitle` reproduces the four documented cases of
/// the canonical TS module (glued / filename-only / human-only / dots-but-no-
/// extension). If the port drifts, every metric below silently measures a
/// different app than the one the owner runs.
#[test]
fn rendered_statement_mirrors_split_document_title() {
    // Glued filename + human title -> the human part is the statement.
    assert_eq!(
        rendered_statement(Some("Y24_25_Sprawozdanie.xhtmlJednostkowe Sprawozdanie")).as_deref(),
        Some("Jednostkowe Sprawozdanie")
    );
    // Filename-only -> no statement (the row falls back to generic copy).
    assert_eq!(rendered_statement(Some("2410_Passus_PL-sig.pdf")), None);
    // Human-only -> passes through unchanged.
    assert_eq!(
        rendered_statement(Some("Wstępne wyniki za czerwiec 2026")).as_deref(),
        Some("Wstępne wyniki za czerwiec 2026")
    );
    // Dots without a known extension never split.
    assert_eq!(
        rendered_statement(Some("Raport nr 12.2026 o wynikach")).as_deref(),
        Some("Raport nr 12.2026 o wynikach")
    );
    // A second extension in the remainder IS a filename-as-statement — the case
    // the hard metric exists to catch.
    assert_eq!(
        rendered_statement(Some("a.zip b.pdf")).as_deref(),
        Some("b.pdf")
    );
    assert_eq!(rendered_statement(None), None);
    assert_eq!(rendered_statement(Some("   ")), None);
}

// ---------------------------------------------------------------------------
// The real-data harness
// ---------------------------------------------------------------------------

#[derive(Default)]
pub(super) struct HonestyTally {
    pub(super) events_total: usize,
    /// Events whose trigger can carry an evidence title (specificity denominator).
    pub(super) specificity_denominator: usize,
    /// …of those, the ones whose rendered statement says something concrete.
    pub(super) specificity_numerator: usize,
    /// Events whose evidence type resolves through a guarded join (orphan denominator).
    pub(super) orphan_denominator: usize,
    /// …of those, the ones whose evidence resolved to nothing at all.
    pub(super) orphaned_evidence: usize,
    /// Rendered statements that are still a raw filename. Must be zero.
    pub(super) filename_as_statement: usize,
    /// Per-trigger `(total, without a concrete statement)` — counts only, never content.
    pub(super) by_trigger: BTreeMap<String, (usize, usize)>,
    /// Per-evidence-type `(total, orphaned)` — counts only, never content.
    pub(super) by_evidence_type: BTreeMap<String, (usize, usize)>,
}

impl HonestyTally {
    /// Share of title-capable events whose row states something concrete.
    /// An empty denominator is 100% by definition (nothing could have been
    /// vague) — callers guard the degenerate-corpus case separately.
    pub(super) fn specificity_pct(&self) -> f64 {
        if self.specificity_denominator == 0 {
            100.0
        } else {
            (self.specificity_numerator as f64 / self.specificity_denominator as f64) * 100.0
        }
    }
}

/// The epic #40 S5 half: does a **success** admit it produced nothing, and does
/// a **missing** number say what is missing?
#[derive(Default)]
pub(super) struct EffectsTally {
    /// Extraction outcome rows read (the zero-effect denominator).
    pub(super) outcomes_total: usize,
    /// Rows whose acceptance succeeded, whose fact count is zero, and whose
    /// typed reason claims production instead of explaining its absence.
    pub(super) zero_effect_successes: usize,
    /// Companies whose health read model was computed.
    pub(super) companies_scored: usize,
    /// Health read-model outputs that report a number as missing without saying
    /// WHAT is missing (or why it does not apply).
    pub(super) silent_missing_metrics: usize,
    /// Per-`reason_code` `(zero-fact successes, total successes)` — counts only.
    pub(super) by_reason_code: BTreeMap<String, (usize, usize)>,
    /// Per-silence-kind counts, so the ratchet's number is traceable to a shape.
    pub(super) by_silence_kind: BTreeMap<&'static str, usize>,
}

/// Silence kinds, for the per-shape breakdown. Machine tokens, counts only.
pub(super) mod silence {
    /// A company with no scored FY period at all — `CompanyHealth.latest` is
    /// `None` and the shape carries no field saying why (no annual period? no
    /// facts? not applicable?). The one gap the S5 measurement found in the
    /// health contract itself.
    pub const NO_SCORED_PERIOD: &str = "no_scored_period";
    /// `InsufficientData` whose `missing` list is empty — the read model knows
    /// it could not score and does not name a single missing input.
    pub const PIOTROSKI_MISSING_UNNAMED: &str = "piotroski_insufficient_without_missing_list";
    pub const ALTMAN_MISSING_UNNAMED: &str = "altman_insufficient_without_missing_list";
    /// `NotApplicable` with a blank reason.
    pub const PIOTROSKI_NOT_APPLICABLE_UNREASONED: &str = "piotroski_not_applicable_without_reason";
    pub const ALTMAN_NOT_APPLICABLE_UNREASONED: &str = "altman_not_applicable_without_reason";
}

/// Measure the S4 attention-honesty trio through THE read model the Today
/// stream uses ([`AttentionStore::list_attention_events`]) and the real frontend
/// statement rule — never a parallel query (ADR 0091 dec. 4). Dismissed rows are
/// included so the measurement covers the whole fired corpus rather than only
/// what the owner has not cleared yet.
///
/// Shared with the **synthetic shape corpus** (epic #40 S6,
/// [`super::shape_corpus`]), which runs these exact invariants in default CI
/// with no real data: one measurement implementation, two datasets — otherwise
/// the public gate could certify an honesty the local ratchet does not measure.
pub(super) fn measure_attention(state: &AppState) -> HonestyTally {
    let events = state
        .attention()
        .list_attention_events(AttentionEventListInput {
            company_id: None,
            include_dismissed: true,
        })
        .expect("list attention events");

    let mut tally = HonestyTally {
        events_total: events.len(),
        ..HonestyTally::default()
    };

    for event in &events {
        let statement = rendered_statement(event.evidence_title.as_deref());
        let concrete = statement.is_some();

        let trigger_entry = tally
            .by_trigger
            .entry(event.trigger_type.clone())
            .or_default();
        trigger_entry.0 += 1;
        if !concrete {
            trigger_entry.1 += 1;
        }

        if !NON_TITLE_TRIGGERS.contains(&event.trigger_type.as_str()) {
            tally.specificity_denominator += 1;
            if concrete {
                tally.specificity_numerator += 1;
            }
        }

        if JOINED_EVIDENCE_TYPES.contains(&event.evidence_type.as_str()) {
            tally.orphan_denominator += 1;
            let evidence_entry = tally
                .by_evidence_type
                .entry(event.evidence_type.clone())
                .or_default();
            evidence_entry.0 += 1;
            // `evidence_title` is `snapshot.or(join)`, so `None` on a joined
            // evidence type means BOTH resolved to nothing: the fire-time
            // snapshot is absent (legacy row) and the evidence row is gone.
            if event.evidence_title.is_none() {
                tally.orphaned_evidence += 1;
                evidence_entry.1 += 1;
            }
        }

        if statement
            .as_deref()
            .is_some_and(|s| filename_extension().is_match(s))
        {
            tally.filename_as_statement += 1;
        }
    }

    tally
}

/// Measure the S5 effects-honesty pair through the real read models
/// ([`FundamentalsProvenanceStore::list_extraction_outcomes`] and
/// [`compute_company_health`]) — never a parallel SQL path, so both numbers move
/// exactly when the app the owner runs moves. Shared with the synthetic shape
/// corpus for the same reason as [`measure_attention`].
pub(super) fn measure_effects(state: &AppState) -> EffectsTally {
    use crate::commands::company_health::compute_company_health;
    use crate::fundamentals::health::{AltmanScore, PiotroskiScore};

    let mut tally = EffectsTally::default();
    let companies = state.list_companies().expect("list companies");

    for company in &companies {
        // --- zero-effect successes -----------------------------------------
        let outcomes = state
            .fundamentals_provenance()
            .list_extraction_outcomes(&company.id)
            .expect("list extraction outcomes");
        tally.outcomes_total += outcomes.len();
        for outcome in &outcomes {
            if !EMITTING_ACCEPTANCES.contains(&outcome.acceptance.as_str()) {
                continue;
            }
            let entry = tally
                .by_reason_code
                .entry(outcome.reason_code.clone())
                .or_default();
            entry.1 += 1;
            if outcome.fact_count == 0
                && PRODUCTION_CLAIMING_REASONS.contains(&outcome.reason_code.as_str())
            {
                tally.zero_effect_successes += 1;
                entry.0 += 1;
            }
        }

        // --- silent missing metrics ----------------------------------------
        let Ok(health) = compute_company_health(state, &company.id) else {
            continue;
        };
        tally.companies_scored += 1;
        let mut silent = |kind: &'static str| {
            tally.silent_missing_metrics += 1;
            *tally.by_silence_kind.entry(kind).or_default() += 1;
        };
        if health.latest.is_none() {
            silent(silence::NO_SCORED_PERIOD);
        }
        for period in &health.history {
            match &period.piotroski {
                PiotroskiScore::InsufficientData { missing, .. } if missing.is_empty() => {
                    silent(silence::PIOTROSKI_MISSING_UNNAMED)
                }
                PiotroskiScore::NotApplicable { reason } if reason.trim().is_empty() => {
                    silent(silence::PIOTROSKI_NOT_APPLICABLE_UNREASONED)
                }
                _ => {}
            }
            match &period.altman {
                AltmanScore::InsufficientData { missing, .. } if missing.is_empty() => {
                    silent(silence::ALTMAN_MISSING_UNNAMED)
                }
                AltmanScore::NotApplicable { reason } if reason.trim().is_empty() => {
                    silent(silence::ALTMAN_NOT_APPLICABLE_UNREASONED)
                }
                _ => {}
            }
        }
    }

    tally
}

#[test]
#[ignore = "real-data honesty harness; needs BRAWLER_REAL_DB (a throwaway copy)"]
fn real_data_honesty_metrics() {
    let Ok(db_path) = std::env::var("BRAWLER_REAL_DB") else {
        eprintln!(
            "SKIP real_data_honesty_metrics: set BRAWLER_REAL_DB to a THROWAWAY copy of the \
             owner's database (see private/realdata/README.md)"
        );
        return;
    };
    if !std::path::Path::new(&db_path).is_file() {
        eprintln!("SKIP real_data_honesty_metrics: no database at {db_path}");
        return;
    }

    // Guardrail, not a convenience: opening the database APPLIES MIGRATIONS.
    // Running this against the master snapshot would migrate the reference copy
    // every later measurement is compared to, and running it against the live
    // application database would mutate the owner's real data.
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

    // The SHARED measurement (see [`measure_attention`]) — the same code the
    // synthetic shape corpus runs in public CI, so neither dataset can be judged
    // by a rule the other has not seen.
    let tally = measure_attention(&state);
    let specificity_pct = tally.specificity_pct();

    eprintln!("== S4 real-data honesty metrics (aggregate counts only) ==");
    eprintln!("db={db_path}");
    eprintln!(
        "attention events (incl. dismissed) = {}",
        tally.events_total
    );
    eprintln!(
        "specificity_pct       = {specificity_pct:.1}%  ({}/{} title-capable events state \
         something concrete)",
        tally.specificity_numerator, tally.specificity_denominator
    );
    eprintln!(
        "orphaned_evidence     = {}  (of {} events with joined evidence)",
        tally.orphaned_evidence, tally.orphan_denominator
    );
    eprintln!(
        "filename_as_statement = {}  (must be 0)",
        tally.filename_as_statement
    );
    eprintln!("-- by trigger: total / without a concrete statement --");
    for (trigger, (total, generic)) in &tally.by_trigger {
        eprintln!("   {trigger:<24} {total:>5} / {generic:>5}");
    }
    eprintln!("-- by evidence type: total / orphaned --");
    for (evidence_type, (total, orphaned)) in &tally.by_evidence_type {
        eprintln!("   {evidence_type:<24} {total:>5} / {orphaned:>5}");
    }

    // --- S5: effects honesty ------------------------------------------------
    let effects = measure_effects(&state);
    eprintln!("== S5 effects honesty (aggregate counts only) ==");
    eprintln!(
        "zero_effect_successes  = {}  (of {} recorded extraction outcomes)",
        effects.zero_effect_successes, effects.outcomes_total
    );
    eprintln!(
        "silent_missing_metrics = {}  (over {} scored companies)",
        effects.silent_missing_metrics, effects.companies_scored
    );
    eprintln!("-- successful outcomes by reason code: zero-fact / total --");
    for (reason_code, (zero, total)) in &effects.by_reason_code {
        eprintln!("   {reason_code:<24} {zero:>5} / {total:>5}");
    }
    eprintln!("-- silence by kind --");
    for (kind, count) in &effects.by_silence_kind {
        eprintln!("   {kind:<44} {count:>5}");
    }

    // Aggregates ONLY (ADR 0091 dec. 4) — this file is read by the ratchet and
    // may be pasted into a PR; it must never carry a title, ticker, or id.
    let metrics = serde_json::json!({
        "specificity_pct": (specificity_pct * 10.0).round() / 10.0,
        "orphaned_evidence": tally.orphaned_evidence,
        "filename_as_statement": tally.filename_as_statement,
        "zero_effect_successes": effects.zero_effect_successes,
        "silent_missing_metrics": effects.silent_missing_metrics,
        "context": {
            "events_total": tally.events_total,
            "specificity_numerator": tally.specificity_numerator,
            "specificity_denominator": tally.specificity_denominator,
            "orphan_denominator": tally.orphan_denominator,
            "outcomes_total": effects.outcomes_total,
            "companies_scored": effects.companies_scored,
        }
    });
    let out_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    std::fs::create_dir_all(&out_dir).expect("create target dir");
    let out_path = out_dir.join("realdata-honesty-metrics.json");
    std::fs::write(
        &out_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&metrics).expect("serialize metrics")
        ),
    )
    .expect("write honesty metrics");
    eprintln!("metrics written to {}", out_path.display());

    // Sanity: a harness that measured nothing must not certify honesty (a
    // degenerate 100% on an empty corpus is the classic false green).
    assert!(
        tally.events_total > 0,
        "expected at least one attention event on the real database — the harness measured \
         nothing (wrong copy?)"
    );
    // The one HARD gate here (the ratchet owns the other two): a raw filename
    // must NEVER be a row's statement on real data (owner dogfooding 2026-07-23).
    assert_eq!(
        tally.filename_as_statement, 0,
        "a Today row statement is a raw filename on real data — a filename is metadata, not prose"
    );
    // Same sanity rule as above for the S5 half: an effects measurement over an
    // empty corpus certifies nothing.
    assert!(
        effects.outcomes_total > 0 && effects.companies_scored > 0,
        "expected recorded extraction outcomes and at least one scored company — the effects \
         half measured nothing (wrong copy?)"
    );
}
