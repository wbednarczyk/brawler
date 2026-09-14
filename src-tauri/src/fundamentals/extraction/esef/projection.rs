//! Layer 1 → Layer 2 projection for the ESEF tier (ADR 0100 decisions 1, 3,
//! 4, 7; epic #398).
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

/// Statement basis for one instance of a multi-document package, derived
/// from its entry path (epic #398 corpus evidence: TXT ships standalone and
/// consolidated filings under Polish-named folders in one package; ESEF
/// itself is a consolidated-IFRS mandate, so consolidated is the default).
/// A heuristic over the only per-instance evidence the package carries —
/// applied per instance so a standalone filing is never silently stamped
/// consolidated (sol review finding 3).
fn basis_of(package_entry_path: &str) -> StatementBasis {
    let path = package_entry_path.to_lowercase();
    if path.contains("jednostkow")
        || path.contains("separate")
        || path.contains("standalone")
        || path.contains("unconsolidated")
    {
        StatementBasis::Standalone
    } else {
        StatementBasis::Consolidated
    }
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
    /// The instance's derived basis — a standalone and a consolidated filing
    /// in one package are separate slots and never conflict with each other.
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
        if !has_presentation_linkbase {
            // No linkbase evidence for this document at all — the role
            // filter cannot be evaluated, so every dimensionless/valued row
            // is a candidate (never a guess at which role it would have
            // carried).
            candidates.push(f);
        } else if is_primary_statement(f) {
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
    let crosswalk: std::collections::HashMap<&str, &ifrs_crosswalk::CrosswalkEntry> =
        ifrs_crosswalk::entries()
            .iter()
            .map(|entry| (entry.concept, entry))
            .collect();
    // Grouped by the REAL slot dimensions this tier varies on (sol review
    // finding 3): statement basis (a multi-document package can carry a
    // standalone AND a consolidated filing — never merged) and metric key.
    // `period_start` is handled inside the group below.
    // The group value carries the resolved entry's `value_kind` alongside its
    // facts (#509 decision 1) — currency is decided from THIS, never from the
    // raw unit measure, so a `count`/`percentage`/... concept can never reach
    // `currency: Some(<unit>)` merely because Layer 1 happened to observe a
    // unit-shaped string. `value_kind` is uniform per slot (entries sharing a
    // `metric_key` agree on it — asserted by a crosswalk invariant test), so
    // the first occurrence's is authoritative for the whole group.
    let mut by_slot: BTreeMap<(&'static str, &'static str), (&'static str, Vec<&NewTaggedFact>)> =
        BTreeMap::new();
    for f in precedence_survivors {
        // The crosswalk names STANDARD taxonomy concepts only (sol review
        // finding 1): an issuer-extension concept that reuses a standard
        // local name (issuer-namespace `Revenue`) must never resolve to the
        // global key — it stays uncrosswalked until the owner promotes it
        // under its issuer-qualified identity (ADR 0100 decisions 2/10).
        let entry = if ifrs_crosswalk::is_standard_ifrs_namespace(&f.concept_namespace_uri) {
            crosswalk.get(f.concept_local_name.as_str())
        } else {
            None
        };
        match entry {
            Some(entry) => {
                by_slot
                    .entry((basis_of(&f.package_entry_path).as_str(), entry.metric_key))
                    .or_insert_with(|| (entry.value_kind, Vec::new()))
                    .1
                    .push(f);
            }
            None => {
                result
                    .uncrosswalked_concepts
                    .insert(f.concept_local_name.clone());
                result.uncrosswalked_fact_count += 1;
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
    for ((basis_str, metric_key), (value_kind, mut occurrences)) in by_slot {
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

        let basis = if basis_str == "standalone" {
            StatementBasis::Standalone
        } else {
            StatementBasis::Consolidated
        };
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
                    basis: Some(basis),
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
                statement_basis: basis,
                contributing_fact_identities: identities,
            });
        }
    }

    result
}

#[cfg(test)]
mod projection_tests;
