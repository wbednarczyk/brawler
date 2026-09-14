//! Layer 1 → Layer 2 projection for the ESEF tier (ADR 0100 decisions 1, 2,
//! 3, 4, 7; epic #398, #508).
//!
//! A pure function over Layer 1 raw tagged-fact rows
//! (`crate::storage::NewTaggedFact` — the exact shape [`super::layer1`]
//! produces and `report_tagged_facts` stores 1:1): dimensionless-only with a
//! concept-level primary-statement filter -> namespace-gated crosswalk
//! resolution -> full write-slot duplicate resolution per (statement basis,
//! metric key, duration window), in that order, BEFORE any of the pipeline's
//! existing metric-key-only collapses (`esef::dedup_longest_duration`,
//! `extraction::fact_set_for_period`, `jobs::structured_extraction`'s
//! `seen_keys`) — those would otherwise let XML document order pick the
//! winner among several legitimately-tagged occurrences of one concept.
//!
//! The primary-statement selector is CONCEPT-level by nature (sol review
//! finding 2): a presentation linkbase relates concepts to roles, so no
//! per-occurrence statement ranking is expressible — same-concept repeats
//! resolve by value equality (repeat vs typed conflict), never by role.
//!
//! Only the requested `period_end` is projected (ADR 0100 decision 7):
//! comparative periods stay in Layer 1, never written by this module's
//! caller. [`super::pipeline`] also calls this function (read-only, output
//! never persisted) to build the comparative cross-check's own candidate set
//! for a PRIOR period end, so that check is resolved by the same
//! order-independent rule rather than by document order too.
//!
//! Validation is NOT this module's job: [`super::pipeline::run_pipeline`]
//! runs every candidate through the existing `validate`/`validate_tier` gate
//! before any write (ADR 0100 decision 5) — this module only decides WHICH
//! candidates exist.
//!
//! The full write slot (period, definition, statement basis, attribution,
//! variant, measure window, data quality) collapses to `(metric_key,
//! period_end)` for THIS tier specifically: ESEF facts are always
//! `consolidated`/`reported`/default-quality, and `measure_window` derives
//! 1:1 from the crosswalk's `period_nature` at the storage write boundary
//! (`storage::financials::resolve_measure_window`) — never re-derived here,
//! per the epic brief.

use std::collections::{BTreeMap, BTreeSet};

use rust_decimal::Decimal;

use super::super::ifrs_crosswalk;
use super::super::{ExtractedFact, FactPeriod, SourceTier, StatementBasis};
use crate::storage::NewTaggedFact;

/// Whether this occurrence's concept participates in any PRIMARY-STATEMENT
/// presentation role (balance / income / comprehensive_income / cash_flow) —
/// decision 3's selector. `None`-equivalent roles (decision 3's `other`,
/// which also covers a bare, non-package instance with no linkbase to attach
/// roles from at all) mean "not projected", never a guess.
///
/// This is a CONCEPT-LEVEL test, and it can be nothing stronger (sol review
/// finding 2): an XBRL presentation linkbase relates CONCEPTS to roles, so
/// the parser attaches one identical role set to every occurrence of a
/// concept — two occurrences of `ProfitLoss` (the income-statement line and
/// the cash-flow reconciliation's opening line) are indistinguishable by
/// role. Same-concept repeats therefore resolve by VALUE EQUALITY in step 3
/// (equal → one re-observed fact; divergent → a typed conflict), never by a
/// per-occurrence statement ranking the linkbase cannot express.
fn is_primary_statement(fact: &NewTaggedFact) -> bool {
    fact.roles.iter().any(|r| {
        matches!(
            r.role_kind.as_str(),
            "balance" | "income" | "comprehensive_income" | "cash_flow"
        )
    })
}

/// Instance-level basis EVIDENCE from the entry path (ADR 0100 decision 1,
/// #508) — distinct from the store's two-valued [`StatementBasis`] because an
/// instance's path can carry BOTH a standalone and a consolidated token at
/// once (a genuinely unreadable case), which must never collapse into a
/// silent default the way "neither token" legitimately does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstanceBasis {
    Consolidated,
    Standalone,
    /// Both a standalone and a consolidated token matched — never guessed,
    /// excluded from projection and counted (`ambiguous_basis_skipped`).
    Ambiguous,
}

/// Statement basis EVIDENCE for one instance of a multi-document package,
/// derived from its entry path (epic #398 corpus evidence: TXT ships
/// standalone and consolidated filings under Polish-named folders in one
/// package; ESEF itself is a consolidated-IFRS mandate, so consolidated is
/// the default when neither token is present). `unconsolidated` is a
/// STANDALONE token whose substring would otherwise also match the bare
/// `consolidated` token, so every `unconsolidated` occurrence is stripped
/// before the consolidated check runs (ADR 0100 decision 1, #508).
fn basis_of(package_entry_path: &str) -> InstanceBasis {
    let path = package_entry_path.to_lowercase();
    let has_unconsolidated = path.contains("unconsolidated");
    let remainder = path.replace("unconsolidated", "");
    let standalone_hit = has_unconsolidated
        || remainder.contains("jednostkow")
        || remainder.contains("separate")
        || remainder.contains("standalone");
    let consolidated_hit =
        remainder.contains("skonsolidowan") || remainder.contains("consolidated");
    match (standalone_hit, consolidated_hit) {
        (true, true) => InstanceBasis::Ambiguous,
        (true, false) => InstanceBasis::Standalone,
        (false, true) | (false, false) => InstanceBasis::Consolidated,
    }
}

/// Whether `instance` (the entry-path evidence) is the document's SELECTED
/// primary basis — `Ambiguous` never matches either, and NOTHING matches a
/// `None` selection (no eligible primary-statement fact anywhere in the
/// document, #508 decision 2).
fn instance_matches_selected(instance: InstanceBasis, selected: Option<StatementBasis>) -> bool {
    matches!(
        (instance, selected),
        (
            InstanceBasis::Consolidated,
            Some(StatementBasis::Consolidated)
        ) | (InstanceBasis::Standalone, Some(StatementBasis::Standalone))
    )
}

fn decimal_of(fact: &NewTaggedFact) -> Option<Decimal> {
    fact.value_numeric.as_deref().and_then(|s| s.parse().ok())
}

fn period_of(fact: &NewTaggedFact) -> FactPeriod {
    if fact.period_type == "instant" {
        FactPeriod::Instant(fact.period_end.clone())
    } else {
        FactPeriod::Duration {
            start: fact.period_start.clone().unwrap_or_default(),
            end: fact.period_end.clone(),
        }
    }
}

/// One projected candidate, keeping a trace back to every Layer 1 occurrence
/// it resolved from (ADR 0100 decision 4: "retaining links to every
/// contributing Layer-1 row") — one identity for a lone occurrence, several
/// for a deterministic repeat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedFact {
    pub fact: ExtractedFact,
    pub contributing_fact_identities: Vec<String>,
}

/// A full-slot duplicate whose contributing Layer 1 occurrences disagree —
/// recorded, never projected (ADR 0100 decision 4). Never resolved by
/// document order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotConflict {
    pub metric_key: String,
    pub period_end: String,
    /// The document's ONE selected basis (#508 decision 2) — the other
    /// filing in a mixed-basis package is dropped before slotting, so it can
    /// never produce a conflict here at all.
    pub statement_basis: StatementBasis,
    pub contributing_fact_identities: Vec<String>,
}

/// One period-end's projection result.
#[derive(Debug, Clone, Default)]
pub struct ProjectionResult {
    pub facts: Vec<ProjectedFact>,
    pub conflicts: Vec<SlotConflict>,
    /// Dimensional rows at this period end — stay in Layer 1, unprojected.
    pub dimensional_skipped: usize,
    /// Rows with none of the four primary-statement roles (decision 3's
    /// `other`, including "no role data at all").
    pub non_primary_statement_skipped: usize,
    /// Distinct concepts with no crosswalk entry — counted, never silently
    /// dropped from Layer 1 (decision 1/2), just not (yet) nameable.
    pub uncrosswalked_concepts: BTreeSet<String>,
    /// Occurrences (not distinct concepts) behind `uncrosswalked_concepts` —
    /// the fact-level count the Coverage read model needs (epic #398 slice:
    /// `storage::report_tagged_facts::coverage_counts`'s "awaiting a name"
    /// bucket), since a dashboard count of raw numbers must not collapse to
    /// the distinct-concept count a repeated position would understate.
    pub uncrosswalked_fact_count: usize,
    /// Duration occurrences dropped because a LONGER span ending at the same
    /// date exists in the slot (Q3's 3-month figure next to its 9-month
    /// year-to-date — cumulative GPW reporting keeps the longest window;
    /// sol review finding 3). Counted, never a conflict.
    pub shorter_window_skipped: usize,
    /// Crosswalk-resolved, primary-statement occurrences dropped because
    /// their instance's basis is NOT the document's selected primary basis
    /// (ADR 0100 decision 2, #508) — the other filing in a mixed-basis
    /// package, kept as Layer 1 evidence but never projected.
    pub non_primary_basis_skipped: usize,
    /// Crosswalk-resolved, primary-statement occurrences dropped because
    /// their instance's entry path carries BOTH a standalone and a
    /// consolidated token (ADR 0100 decision 1, #508) — genuinely unreadable
    /// basis evidence, never guessed.
    pub ambiguous_basis_skipped: usize,
}

/// Step 1's eligibility test (dimensionless, with a usable value) as a
/// reusable predicate — shared by `project_period` and `select_primary_basis`
/// so the two can never drift (ADR 0100 decision 2, #508).
fn has_usable_value(f: &NewTaggedFact) -> bool {
    !f.is_dimensional && decimal_of(f).is_some()
}

/// Step 1's role predicate (with the no-linkbase fallback) as a reusable
/// function.
fn passes_role_filter(f: &NewTaggedFact, has_presentation_linkbase: bool) -> bool {
    !has_presentation_linkbase || is_primary_statement(f)
}

/// A fresh concept -> crosswalk-entry map, built once per caller (`entries()`
/// is a static table; this is just the lookup index over it).
fn build_crosswalk(
) -> std::collections::HashMap<&'static str, &'static ifrs_crosswalk::CrosswalkEntry> {
    ifrs_crosswalk::entries()
        .iter()
        .map(|entry| (entry.concept, entry))
        .collect()
}

/// Step 2's crosswalk resolution as a reusable function (sol review finding
/// 1 still applies: only a STANDARD IFRS namespace concept ever resolves).
fn resolve_crosswalk<'a>(
    f: &NewTaggedFact,
    crosswalk: &std::collections::HashMap<&str, &'a ifrs_crosswalk::CrosswalkEntry>,
) -> Option<&'a ifrs_crosswalk::CrosswalkEntry> {
    if ifrs_crosswalk::is_standard_ifrs_namespace(&f.concept_namespace_uri) {
        crosswalk.get(f.concept_local_name.as_str()).copied()
    } else {
        None
    }
}

/// The document's ONE primary statement basis (ADR 0100 decision 2, #508),
/// selected ONCE over every Layer 1 row the document carries — never the
/// period-filtered survivors a single `project_period` call sees, so the
/// current AND comparative projections agree. Applies the EXACT primary-
/// statement role filter and crosswalk resolution `project_period` itself
/// applies (never a second copy): among facts that pass both, primary is
/// `Consolidated` if any comes from a consolidated instance, else
/// `Standalone` if any, else `None` (nothing eligible at all — the caller
/// projects nothing). An instance whose entry path is `Ambiguous` never
/// contributes evidence either way — it is excluded from selection exactly
/// as it is excluded from projection.
pub fn select_primary_basis(
    layer1_facts: &[NewTaggedFact],
    has_presentation_linkbase: bool,
) -> Option<StatementBasis> {
    let crosswalk = build_crosswalk();
    let mut any_consolidated = false;
    let mut any_standalone = false;
    for f in layer1_facts {
        if !has_usable_value(f) || !passes_role_filter(f, has_presentation_linkbase) {
            continue;
        }
        if resolve_crosswalk(f, &crosswalk).is_none() {
            continue;
        }
        match basis_of(&f.package_entry_path) {
            InstanceBasis::Consolidated => any_consolidated = true,
            InstanceBasis::Standalone => any_standalone = true,
            InstanceBasis::Ambiguous => {}
        }
    }
    if any_consolidated {
        Some(StatementBasis::Consolidated)
    } else if any_standalone {
        Some(StatementBasis::Standalone)
    } else {
        None
    }
}

/// Projects Layer 1 rows for exactly one `period_end` into Layer 2 ESEF
/// candidates, per ADR 0100 decision 4 (as amended after sol's review):
/// 1. dimensionless only, with a usable value, concept-level
///    primary-statement filter;
/// 2. crosswalk resolution (standard IFRS namespace required);
/// 3. full write-slot duplicate resolution per (statement basis, metric):
///    longest duration window wins over shorter spans sharing its end date,
///    then value equality separates a repeat from a typed conflict.
///
/// `has_presentation_linkbase` is document-level evidence (ADR 0100 decision
/// 3 regression fix, epic #398): a bare iXBRL instance (not inside a ZIP
/// package) carries no `*_pre.xml` to attach roles from, so EVERY one of its
/// facts would fail step 1's role filter and the document would silently
/// project zero facts — a real regression for a document class that simply
/// cannot supply the evidence the strict selector needs. When `false`, the
/// role filter is skipped and every dimensionless, valued candidate proceeds
/// straight to crosswalk resolution — the pre-epic selection for this tier.
/// A package that DOES carry a linkbase (`true`) keeps the strict path.
///
/// Pure: `facts` is whatever generation the caller has in hand (freshly
/// extracted, or read back from `report_tagged_facts`) — this function never
/// touches storage and applies no validation gate (that is the caller's job,
/// decision 5).
pub fn project_period(
    facts: &[NewTaggedFact],
    period_end: &str,
    has_presentation_linkbase: bool,
    basis: Option<StatementBasis>,
) -> ProjectionResult {
    let mut result = ProjectionResult::default();

    // ---- Step 1: this period, dimensionless only, with a usable value -----
    // A row whose context never resolved or whose value never parsed has
    // nothing to project regardless of role — it stays visible in Layer 1
    // (decision 9), just not counted as a primary-statement candidate here.
    let mut candidates: Vec<&NewTaggedFact> = Vec::new();
    for f in facts.iter().filter(|f| f.period_end == period_end) {
        if f.is_dimensional {
            result.dimensional_skipped += 1;
            continue;
        }
        if decimal_of(f).is_none() {
            continue;
        }
        if passes_role_filter(f, has_presentation_linkbase) {
            candidates.push(f);
        } else {
            result.non_primary_statement_skipped += 1;
        }
    }

    // There is deliberately NO per-occurrence statement-precedence step here
    // (sol review finding 2): the presentation linkbase relates CONCEPTS to
    // roles, so every occurrence of one concept carries an identical role
    // set and no ranking can tell them apart. The selector above is the
    // whole of decision 3; same-concept repeats resolve by value equality in
    // step 3 below. An earlier revision carried a ranking step whose only
    // passing evidence was a test fabricating per-occurrence role vectors
    // the parser cannot produce — removed as unimplementable, not merely
    // unimplemented (ADR 0100 decision 3, amended).
    let precedence_survivors = candidates;

    // ---- Step 2: crosswalk resolution --------------------------------------
    let crosswalk = build_crosswalk();
    // Grouped by metric key: every surviving occurrence already shares the
    // document's ONE selected basis (decision 2, #508) — a mixed-basis
    // package's OTHER instance is dropped below, before it ever reaches this
    // map, so the slot no longer needs basis as part of its key.
    // `period_start` is handled inside the group below.
    // The group value carries the resolved entry's `value_kind` alongside its
    // facts (#509 decision 1) — currency is decided from THIS, never from the
    // raw unit measure, so a `count`/`percentage`/... concept can never reach
    // `currency: Some(<unit>)` merely because Layer 1 happened to observe a
    // unit-shaped string. `value_kind` is uniform per slot (entries sharing a
    // `metric_key` agree on it — asserted by a crosswalk invariant test), so
    // the first occurrence's is authoritative for the whole group.
    let mut by_slot: BTreeMap<&'static str, (&'static str, Vec<&NewTaggedFact>)> = BTreeMap::new();
    for f in precedence_survivors {
        // The crosswalk names STANDARD taxonomy concepts only (sol review
        // finding 1): an issuer-extension concept that reuses a standard
        // local name (issuer-namespace `Revenue`) must never resolve to the
        // global key — it stays uncrosswalked until the owner promotes it
        // under its issuer-qualified identity (ADR 0100 decisions 2/10).
        let Some(entry) = resolve_crosswalk(f, &crosswalk) else {
            result
                .uncrosswalked_concepts
                .insert(f.concept_local_name.clone());
            result.uncrosswalked_fact_count += 1;
            continue;
        };
        // Decision 2 (#508): only the document's selected basis slots; the
        // other filing's occurrences (and any genuinely ambiguous instance)
        // are dropped HERE, never mixed into the same-metric slot.
        match basis_of(&f.package_entry_path) {
            InstanceBasis::Ambiguous => result.ambiguous_basis_skipped += 1,
            instance if !instance_matches_selected(instance, basis) => {
                result.non_primary_basis_skipped += 1;
            }
            _ => {
                by_slot
                    .entry(entry.metric_key)
                    .or_insert_with(|| (entry.value_kind, Vec::new()))
                    .1
                    .push(f);
            }
        }
    }

    // ---- Step 3: full write-slot duplicate resolution ----------------------
    // Within one (basis, metric) slot, several DURATION spans can
    // legitimately end at the target period_end — a Q3 filing tags the
    // 3-month quarter AND the 9-month year-to-date figure with the same end
    // date. GPW interim reporting is cumulative, so the LONGEST span
    // (earliest period_start) is the reported figure for the period and the
    // shorter windows are counted, never conflated into a conflict (parity
    // with the retired parser's `dedup_longest_duration`; sol review
    // finding 3). Only after that does value equality separate a repeat
    // from a typed conflict.
    for (metric_key, (value_kind, mut occurrences)) in by_slot {
        let longest_start: Option<String> = occurrences
            .iter()
            .filter(|f| f.period_type != "instant")
            .filter_map(|f| f.period_start.clone())
            .min();
        if let Some(longest) = longest_start.as_deref() {
            let before = occurrences.len();
            occurrences.retain(|f| {
                f.period_type == "instant" || f.period_start.as_deref() == Some(longest)
            });
            result.shorter_window_skipped += before - occurrences.len();
        }

        let first = occurrences[0];
        let first_value = decimal_of(first).expect("filtered to a usable value in step 1");
        let all_agree = occurrences
            .iter()
            .all(|f| decimal_of(f) == Some(first_value) && f.unit_measure == first.unit_measure);
        let identities: Vec<String> = occurrences
            .iter()
            .map(|f| f.fact_identity.clone())
            .collect();
        if all_agree {
            result.facts.push(ProjectedFact {
                fact: ExtractedFact {
                    metric_key: metric_key.to_owned(),
                    value: first_value,
                    period: period_of(first),
                    basis,
                    // #509 decision 1: currency by `value_kind`, never a raw
                    // unit copy — a non-monetary fact (count, percentage, ...)
                    // must never reach the store's currency column at all.
                    currency: if value_kind == "monetary" {
                        first.unit_measure.clone()
                    } else {
                        None
                    },
                    tier: SourceTier::Esef,
                    citation: first.concept_local_name.clone(),
                },
                contributing_fact_identities: identities,
            });
        } else {
            result.conflicts.push(SlotConflict {
                metric_key: metric_key.to_owned(),
                period_end: period_end.to_owned(),
                // `by_slot` only ever holds facts `instance_matches_selected`
                // accepted (step 2), which requires `basis: Some(_)` — a
                // non-empty slot proves a basis was actually selected.
                statement_basis: basis.expect("non-empty slot implies a selected basis"),
                contributing_fact_identities: identities,
            });
        }
    }

    result
}

#[cfg(test)]
mod projection_tests;
