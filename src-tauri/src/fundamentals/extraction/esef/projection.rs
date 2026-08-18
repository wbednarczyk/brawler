//! Layer 1 → Layer 2 projection for the ESEF tier (ADR 0100 decisions 1, 3,
//! 4, 7; epic #398).
//!
//! A pure function over Layer 1 raw tagged-fact rows
//! (`crate::storage::NewTaggedFact` — the exact shape [`super::layer1`]
//! produces and `report_tagged_facts` stores 1:1): dimensionless-only ->
//! primary-statement role selection with statement precedence -> crosswalk
//! resolution -> full write-slot duplicate resolution, in that order, BEFORE
//! any of the pipeline's existing metric-key-only collapses
//! (`esef::dedup_longest_duration`, `extraction::fact_set_for_period`,
//! `jobs::structured_extraction`'s `seen_keys`) — those would otherwise let
//! XML document order pick the winner among several legitimately-tagged
//! occurrences of the same concept.
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

/// Statement-role precedence for a profit-and-loss concept tagged under
/// several roles (ADR 0100 decision 4): the income statement's own occurrence
/// wins over the comprehensive-income statement, which wins over the
/// cash-flow reconciliation (which may carry an inverted `sign`). `balance`
/// is a disjoint statement family that never competes with these three in
/// real data — kept in the same total order so the rule is one function, not
/// a special case.
fn role_precedence(role_kind: &str) -> Option<u8> {
    match role_kind {
        "income" => Some(0),
        "comprehensive_income" => Some(1),
        "cash_flow" => Some(2),
        "balance" => Some(3),
        _ => None,
    }
}

/// The best (lowest-rank) primary-statement role this occurrence carries, or
/// `None` when none of its roles classify as balance/income/comprehensive_
/// income/cash_flow — decision 3's `other` (which also covers a bare,
/// non-package instance with no linkbase to attach roles from at all) is the
/// same "not projected" outcome either way, never a guess.
fn best_role(fact: &NewTaggedFact) -> Option<u8> {
    fact.roles
        .iter()
        .filter_map(|r| role_precedence(&r.role_kind))
        .min()
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
}

/// Projects Layer 1 rows for exactly one `period_end` into Layer 2 ESEF
/// candidates, per ADR 0100 decision 4's required order:
/// 1. dimensionless only;
/// 2. primary-statement role filter + statement precedence;
/// 3. crosswalk resolution;
/// 4. full write-slot duplicate resolution (repeat vs typed conflict).
///
/// `has_presentation_linkbase` is document-level evidence (ADR 0100 decision
/// 3 regression fix, epic #398): a bare iXBRL instance (not inside a ZIP
/// package) carries no `*_pre.xml` to attach roles from, so EVERY one of its
/// facts would fail step 2's role filter and the document would silently
/// project zero facts — a real regression for a document class that simply
/// cannot supply the evidence the strict selector needs. When `false`, step
/// 2 is skipped entirely (no role requirement, no precedence grouping) and
/// every dimensionless, valued candidate proceeds straight to crosswalk
/// resolution — the pre-epic selection for this tier. A package that DOES
/// carry a linkbase (`true`) keeps the strict path unchanged.
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
        } else if best_role(f).is_some() {
            candidates.push(f);
        } else {
            result.non_primary_statement_skipped += 1;
        }
    }

    // ---- Step 2: statement precedence, grouped by concept -----------------
    // Only meaningful when role data exists to break a tie on; with no
    // linkbase every candidate survives unchanged (the fallback above).
    let precedence_survivors: Vec<&NewTaggedFact> = if has_presentation_linkbase {
        let mut by_concept: BTreeMap<&str, Vec<&NewTaggedFact>> = BTreeMap::new();
        for f in candidates {
            by_concept
                .entry(f.concept_local_name.as_str())
                .or_default()
                .push(f);
        }
        let mut survivors = Vec::new();
        for occurrences in by_concept.into_values() {
            let min_rank = occurrences
                .iter()
                .filter_map(|f| best_role(f))
                .min()
                .expect("every member of this group passed the role filter above");
            survivors.extend(
                occurrences
                    .into_iter()
                    .filter(|f| best_role(f) == Some(min_rank)),
            );
        }
        survivors
    } else {
        candidates
    };

    // ---- Step 3: crosswalk resolution --------------------------------------
    let crosswalk: std::collections::HashMap<&str, &ifrs_crosswalk::CrosswalkEntry> =
        ifrs_crosswalk::entries()
            .iter()
            .map(|entry| (entry.concept, entry))
            .collect();
    let mut by_metric: BTreeMap<&'static str, Vec<&NewTaggedFact>> = BTreeMap::new();
    for f in precedence_survivors {
        match crosswalk.get(f.concept_local_name.as_str()) {
            Some(entry) => by_metric.entry(entry.metric_key).or_default().push(f),
            None => {
                result
                    .uncrosswalked_concepts
                    .insert(f.concept_local_name.clone());
                result.uncrosswalked_fact_count += 1;
            }
        }
    }

    // ---- Step 4: full write-slot duplicate resolution ----------------------
    // The slot is (metric_key, period_end) here — see the module doc for why
    // the other slot dimensions are constant for this tier.
    for (metric_key, occurrences) in by_metric {
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
                    // ESEF is a consolidated-IFRS mandate (parity with the
                    // Layer 2 `parse_esef` parser).
                    basis: Some(StatementBasis::Consolidated),
                    currency: first.unit_measure.clone(),
                    tier: SourceTier::Esef,
                    citation: first.concept_local_name.clone(),
                },
                contributing_fact_identities: identities,
            });
        } else {
            result.conflicts.push(SlotConflict {
                metric_key: metric_key.to_owned(),
                period_end: period_end.to_owned(),
                contributing_fact_identities: identities,
            });
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::NewTaggedFactRole;

    fn role(kind: &str) -> NewTaggedFactRole {
        NewTaggedFactRole {
            role_uri: format!("http://x/role/{kind}"),
            role_kind: kind.to_owned(),
        }
    }

    /// A minimal balance-sheet fact builder: instant, dimensionless, PLN,
    /// classified `balance` unless overridden.
    fn balance_fact(concept: &str, identity: &str, value: &str) -> NewTaggedFact {
        NewTaggedFact {
            fact_identity: identity.to_owned(),
            concept_local_name: concept.to_owned(),
            period_type: "instant".to_owned(),
            period_end: "2025-12-31".to_owned(),
            unit_measure: Some("PLN".to_owned()),
            value_numeric: Some(value.to_owned()),
            roles: vec![role("balance")],
            ..Default::default()
        }
    }

    /// A minimal duration (P&L) fact builder.
    fn duration_fact(concept: &str, identity: &str, value: &str, role_kind: &str) -> NewTaggedFact {
        NewTaggedFact {
            fact_identity: identity.to_owned(),
            concept_local_name: concept.to_owned(),
            period_type: "duration".to_owned(),
            period_start: Some("2025-01-01".to_owned()),
            period_end: "2025-12-31".to_owned(),
            unit_measure: Some("PLN".to_owned()),
            value_numeric: Some(value.to_owned()),
            roles: vec![role(role_kind)],
            ..Default::default()
        }
    }

    fn d(v: &str) -> Decimal {
        v.parse().unwrap()
    }

    #[test]
    fn income_occurrence_wins_over_the_cash_flow_reconciliation_with_an_inverted_sign() {
        let facts = vec![
            duration_fact("ProfitLoss", "income_occ", "100", "income"),
            duration_fact("ProfitLoss", "cfo_occ", "-100", "cash_flow"),
        ];
        let projected = project_period(&facts, "2025-12-31", true);

        assert_eq!(projected.facts.len(), 1);
        let net_profit = &projected.facts[0];
        assert_eq!(net_profit.fact.metric_key, "net_profit");
        assert_eq!(
            net_profit.fact.value,
            d("100"),
            "the income statement's value must win, never the inverted cash-flow one"
        );
        assert_eq!(
            net_profit.contributing_fact_identities,
            vec!["income_occ".to_owned()]
        );
        assert!(
            projected.conflicts.is_empty(),
            "precedence must resolve this before it ever reaches slot dedup"
        );
    }

    #[test]
    fn a_single_statement_filing_with_no_income_role_projects_from_comprehensive_income() {
        // LPP-shaped: only the ias_1_role-410000 comprehensive-income role
        // exists in the package; no 310000/320000 income role at all.
        let facts = vec![duration_fact(
            "ProfitLoss",
            "ci_occ",
            "250",
            "comprehensive_income",
        )];
        let projected = project_period(&facts, "2025-12-31", true);

        assert_eq!(projected.facts.len(), 1);
        assert_eq!(projected.facts[0].fact.metric_key, "net_profit");
        assert_eq!(projected.facts[0].fact.value, d("250"));
    }

    #[test]
    fn two_rows_one_slot_identical_values_project_one_fact_marked_a_repeat() {
        let facts = vec![
            balance_fact("Assets", "occ1", "1000"),
            balance_fact("Assets", "occ2", "1000"),
        ];
        let projected = project_period(&facts, "2025-12-31", true);

        assert_eq!(projected.facts.len(), 1);
        assert_eq!(projected.facts[0].fact.metric_key, "total_assets");
        let mut ids = projected.facts[0].contributing_fact_identities.clone();
        ids.sort();
        assert_eq!(ids, vec!["occ1".to_owned(), "occ2".to_owned()]);
        assert!(projected.conflicts.is_empty());
    }

    #[test]
    fn two_rows_one_slot_differing_values_project_no_fact_and_record_a_typed_conflict() {
        let facts = vec![
            balance_fact("Assets", "occ1", "1000"),
            balance_fact("Assets", "occ2", "1500"),
        ];
        let projected = project_period(&facts, "2025-12-31", true);

        assert!(
            projected.facts.is_empty(),
            "a genuine value disagreement must never resolve by document order"
        );
        assert_eq!(projected.conflicts.len(), 1);
        let conflict = &projected.conflicts[0];
        assert_eq!(conflict.metric_key, "total_assets");
        assert_eq!(conflict.period_end, "2025-12-31");
        let mut ids = conflict.contributing_fact_identities.clone();
        ids.sort();
        assert_eq!(ids, vec!["occ1".to_owned(), "occ2".to_owned()]);
    }

    #[test]
    fn a_dimensional_row_is_never_projected_and_is_counted() {
        let mut dimensional = balance_fact("Assets", "dim1", "999");
        dimensional.is_dimensional = true;
        let facts = vec![dimensional];
        let projected = project_period(&facts, "2025-12-31", true);

        assert!(projected.facts.is_empty());
        assert_eq!(projected.dimensional_skipped, 1);
    }

    #[test]
    fn an_uncrosswalked_concept_is_never_projected_and_is_counted() {
        let facts = vec![balance_fact(
            "SomeCompanyExtensionConceptNotInTheCrosswalk",
            "unk1",
            "42",
        )];
        let projected = project_period(&facts, "2025-12-31", true);

        assert!(projected.facts.is_empty());
        assert!(projected
            .uncrosswalked_concepts
            .contains("SomeCompanyExtensionConceptNotInTheCrosswalk"));
    }

    #[test]
    fn a_row_with_no_primary_statement_role_is_never_projected_and_is_counted() {
        let facts = vec![duration_fact("Assets", "eq1", "10", "equity_changes")];
        let projected = project_period(&facts, "2025-12-31", true);

        assert!(projected.facts.is_empty());
        assert_eq!(projected.non_primary_statement_skipped, 1);
    }

    #[test]
    fn a_row_with_no_role_at_all_is_never_projected_and_is_counted() {
        let mut no_role = balance_fact("Assets", "noroleocc", "10");
        no_role.roles = Vec::new();
        let facts = vec![no_role];
        let projected = project_period(&facts, "2025-12-31", true);

        assert!(projected.facts.is_empty());
        assert_eq!(projected.non_primary_statement_skipped, 1);
    }

    #[test]
    fn only_the_requested_period_end_is_projected() {
        let facts = vec![
            balance_fact("Assets", "current", "100"),
            NewTaggedFact {
                period_end: "2024-12-31".to_owned(),
                ..balance_fact("Assets", "comparative", "90")
            },
        ];
        let projected = project_period(&facts, "2025-12-31", true);

        assert_eq!(projected.facts.len(), 1);
        assert_eq!(
            projected.facts[0].contributing_fact_identities,
            vec!["current".to_owned()]
        );
    }

    // -----------------------------------------------------------------------
    // `has_presentation_linkbase = false` (regression fix): a bare iXBRL
    // instance has no `*_pre.xml` to attach roles from, so `roles` is empty on
    // every one of its facts. Strict mode would then classify every fact as
    // `non_primary_statement_skipped` and the document would silently project
    // zero facts — measured on the maintainer's DB: 13 real facts from one
    // bare-instance document. The fallback recovers the pre-epic selection
    // (dimensionless + crosswalk-resolved, no role filter) for exactly this
    // evidence-free case; a package that DOES carry a linkbase never takes
    // this path (`true` in every test above).
    // -----------------------------------------------------------------------

    /// A dimensionless fact with NO roles attached — the honest shape Layer 1
    /// produces for a bare instance (`roles_by_concept` is an empty map, so
    /// nothing is ever attached; see `esef::layer1` module docs).
    fn roleless_fact(concept: &str, identity: &str, value: &str) -> NewTaggedFact {
        NewTaggedFact {
            fact_identity: identity.to_owned(),
            concept_local_name: concept.to_owned(),
            period_type: "instant".to_owned(),
            period_end: "2025-12-31".to_owned(),
            unit_measure: Some("PLN".to_owned()),
            value_numeric: Some(value.to_owned()),
            roles: Vec::new(),
            ..Default::default()
        }
    }

    #[test]
    fn no_linkbase_projects_dimensionless_crosswalked_facts_without_a_role_filter() {
        let facts = vec![
            roleless_fact("Assets", "assets", "45000000"),
            roleless_fact("Liabilities", "liabilities", "20000000"),
            roleless_fact("Equity", "equity", "25000000"),
        ];
        let projected = project_period(&facts, "2025-12-31", false);

        assert_eq!(
            projected.facts.len(),
            3,
            "every dimensionless, crosswalked fact must project — a realistic \
             count > 0, matching pre-epic behaviour"
        );
        let mut keys: Vec<&str> = projected
            .facts
            .iter()
            .map(|f| f.fact.metric_key.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["total_assets", "total_equity", "total_liabilities"]
        );
        assert_eq!(
            projected.non_primary_statement_skipped, 0,
            "the role filter must not run at all when there is no linkbase evidence"
        );
    }

    #[test]
    fn no_linkbase_still_skips_dimensional_and_uncrosswalked_rows() {
        // The fallback widens only the role requirement — the dimensional
        // filter and the crosswalk requirement are unconditional (decision
        // 1/3), never loosened by a missing linkbase.
        let mut dimensional = roleless_fact("Assets", "dim1", "999");
        dimensional.is_dimensional = true;
        let facts = vec![
            dimensional,
            roleless_fact("SomeUncrosswalkedExtension", "unk1", "1"),
        ];
        let projected = project_period(&facts, "2025-12-31", false);

        assert!(projected.facts.is_empty());
        assert_eq!(projected.dimensional_skipped, 1);
        assert!(projected
            .uncrosswalked_concepts
            .contains("SomeUncrosswalkedExtension"));
    }

    #[test]
    fn a_linkbase_bearing_package_still_applies_the_strict_role_filter() {
        // Same roleless fixture as above, but `has_presentation_linkbase =
        // true`: the document DOES carry a linkbase, so an unclassified fact
        // is a genuine "not a primary statement line", never a fallback
        // candidate — the strict path is unchanged.
        let facts = vec![roleless_fact("Assets", "assets", "45000000")];
        let projected = project_period(&facts, "2025-12-31", true);

        assert!(projected.facts.is_empty());
        assert_eq!(projected.non_primary_statement_skipped, 1);
    }

    // -----------------------------------------------------------------------
    // Data-transform invariants (ADR 0049): `project_period` is a dedup/merge
    // transform over Layer 1 rows, so it ships the same class of property
    // coverage `esef::dedup_longest_duration` (the collapse it replaces)
    // never had — the core claim of ADR 0100 decision 4 IS order-independence,
    // so that is the invariant worth proving over arbitrary input, not just
    // the hand-picked cases above.
    mod properties {
        use super::*;
        use proptest::prelude::*;

        /// `(metric_key, value, sorted contributing fact identities)`.
        type CanonicalFact = (String, String, Vec<String>);
        /// `(metric_key, sorted contributing fact identities)`.
        type CanonicalConflict = (String, Vec<String>);

        /// A canonicalized, `Eq`-comparable view of a [`ProjectionResult`]:
        /// sorted facts (by metric_key, with sorted contributing ids) and
        /// sorted conflicts (by metric_key, with sorted contributing ids) —
        /// order-independence must hold on CONTENT, never on `Vec` order.
        fn canonical(result: &ProjectionResult) -> (Vec<CanonicalFact>, Vec<CanonicalConflict>) {
            let mut facts: Vec<CanonicalFact> = result
                .facts
                .iter()
                .map(|pf| {
                    let mut ids = pf.contributing_fact_identities.clone();
                    ids.sort();
                    (pf.fact.metric_key.clone(), pf.fact.value.to_string(), ids)
                })
                .collect();
            facts.sort();
            let mut conflicts: Vec<CanonicalConflict> = result
                .conflicts
                .iter()
                .map(|c| {
                    let mut ids = c.contributing_fact_identities.clone();
                    ids.sort();
                    (c.metric_key.clone(), ids)
                })
                .collect();
            conflicts.sort();
            (facts, conflicts)
        }

        /// A small, deliberately overlapping catalog: two crosswalked concepts
        /// (one balance, one profit-and-loss with all four role families
        /// contending) plus one uncrosswalked concept — enough to exercise
        /// every step (dimensional skip, role precedence, crosswalk miss,
        /// repeat vs conflict) under arbitrary shuffling.
        fn arbitrary_fact() -> impl Strategy<Value = NewTaggedFact> {
            (
                prop::sample::select(vec!["Assets", "ProfitLoss", "NotInTheCrosswalk"]),
                prop::sample::select(vec![
                    "income",
                    "comprehensive_income",
                    "cash_flow",
                    "balance",
                    "other",
                ]),
                prop::sample::select(vec!["2025-12-31", "2024-12-31"]),
                any::<bool>(),
                0i64..3, // a small range so equal/differing values both occur often
            )
                .prop_map(|(concept, role_kind, period_end, is_dimensional, value)| {
                    NewTaggedFact {
                        concept_local_name: concept.to_owned(),
                        period_type: "instant".to_owned(),
                        period_end: period_end.to_owned(),
                        unit_measure: Some("PLN".to_owned()),
                        value_numeric: Some(value.to_string()),
                        is_dimensional,
                        roles: vec![NewTaggedFactRole {
                            role_uri: format!("http://x/role/{role_kind}"),
                            role_kind: role_kind.to_owned(),
                        }],
                        ..Default::default()
                    }
                })
        }

        proptest! {
            // `project_period` must never panic on arbitrary (even
            // self-contradictory) Layer 1 rows — a hostile/malformed
            // extraction feeds it, it does not crash the pipeline.
            #[test]
            fn never_panics(mut facts in prop::collection::vec(arbitrary_fact(), 0..12)) {
                for (i, f) in facts.iter_mut().enumerate() {
                    f.fact_identity = format!("f{i}");
                }
                let _ = project_period(&facts, "2025-12-31", true);
            }

            // Decision 4's core claim: the result never depends on document
            // (Vec) order — shuffling the SAME rows must yield the SAME
            // canonicalized projection.
            #[test]
            fn order_independent(
                mut facts in prop::collection::vec(arbitrary_fact(), 0..12),
                seed in any::<u64>(),
            ) {
                for (i, f) in facts.iter_mut().enumerate() {
                    f.fact_identity = format!("f{i}");
                }
                let original = project_period(&facts, "2025-12-31", true);

                // A cheap deterministic shuffle: rotate by a seed-derived amount
                // and reverse every other element — no external RNG dependency.
                let mut shuffled = facts.clone();
                if !shuffled.is_empty() {
                    let rotate_by = (seed as usize) % shuffled.len();
                    shuffled.rotate_left(rotate_by);
                    shuffled.reverse();
                }
                let reordered = project_period(&shuffled, "2025-12-31", true);

                prop_assert_eq!(canonical(&original), canonical(&reordered));
            }
        }
    }
}
