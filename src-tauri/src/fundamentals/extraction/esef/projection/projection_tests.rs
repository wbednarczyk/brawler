//! Unit + property tests for [`super::project_period`] (ADR 0100 decisions 1,
//! 3, 4, 7; #509). A child of the sibling of the module it tests — declared
//! externally (the `structured_extraction/{role_families_tests,
//! unit_refusal_tests}.rs` precedent: the coverage summary forbids a nested
//! `mod x;` inside an inline `#[cfg(test)] mod tests`), so the pinned parent
//! stays under its file-size ratchet.

use super::*;
use crate::storage::NewTaggedFactRole;

fn role(kind: &str) -> NewTaggedFactRole {
    NewTaggedFactRole {
        role_uri: format!("http://x/role/{kind}"),
        role_kind: kind.to_owned(),
    }
}

const IFRS_NS: &str = "https://xbrl.ifrs.org/taxonomy/2024-03-27/ifrs-full";
/// Every fixture below (bar the mixed-basis ones) defaults to an empty
/// `package_entry_path`, which `basis_of` classifies `Consolidated` — the
/// short alias every pre-#508 `project_period` call passes explicitly now
/// that `basis` is a required argument (#508 decision 2).
const CONSOLIDATED: Option<StatementBasis> = Some(StatementBasis::Consolidated);

/// A minimal balance-sheet fact builder: instant, dimensionless, PLN,
/// standard IFRS namespace, classified `balance` unless overridden.
fn balance_fact(concept: &str, identity: &str, value: &str) -> NewTaggedFact {
    NewTaggedFact {
        fact_identity: identity.to_owned(),
        concept_namespace_uri: IFRS_NS.to_owned(),
        concept_local_name: concept.to_owned(),
        period_type: "instant".to_owned(),
        period_end: "2025-12-31".to_owned(),
        unit_measure: Some("PLN".to_owned()),
        value_numeric: Some(value.to_owned()),
        roles: vec![role("balance")],
        ..Default::default()
    }
}

/// A minimal duration (P&L) fact builder. `roles` carries the CONCEPT'S
/// whole role set — the parser attaches one identical set to every
/// occurrence of a concept, so a test giving two occurrences different
/// role vectors would fabricate evidence the parser cannot produce.
fn duration_fact(concept: &str, identity: &str, value: &str, role_kinds: &[&str]) -> NewTaggedFact {
    NewTaggedFact {
        fact_identity: identity.to_owned(),
        concept_namespace_uri: IFRS_NS.to_owned(),
        concept_local_name: concept.to_owned(),
        period_type: "duration".to_owned(),
        period_start: Some("2025-01-01".to_owned()),
        period_end: "2025-12-31".to_owned(),
        unit_measure: Some("PLN".to_owned()),
        value_numeric: Some(value.to_owned()),
        roles: role_kinds.iter().map(|k| role(k)).collect(),
        ..Default::default()
    }
}

fn d(v: &str) -> Decimal {
    v.parse().unwrap()
}

/// The real corpus shape (XTB tags `ProfitLoss` 3x per filing): every
/// occurrence of a concept carries the concept's whole role set — the
/// income-statement line and the cash-flow reconciliation's opening line
/// are indistinguishable by role. Equal normalized values resolve as ONE
/// re-observed fact tracing back to every occurrence.
#[test]
fn same_concept_tagged_across_statements_with_equal_values_is_one_reobserved_fact() {
    let facts = vec![
        duration_fact("ProfitLoss", "income_occ", "100", &["income", "cash_flow"]),
        duration_fact("ProfitLoss", "cfo_occ", "100", &["income", "cash_flow"]),
    ];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert_eq!(projected.facts.len(), 1);
    let net_profit = &projected.facts[0];
    assert_eq!(net_profit.fact.metric_key, "net_profit");
    assert_eq!(net_profit.fact.value, d("100"));
    let mut ids = net_profit.contributing_fact_identities.clone();
    ids.sort();
    assert_eq!(ids, vec!["cfo_occ".to_owned(), "income_occ".to_owned()]);
    assert!(projected.conflicts.is_empty());
}

/// DIVERGENT normalized values of one concept in one period are a typed
/// conflict — the linkbase is concept-level, so no statement ranking can
/// choose between them (ADR 0100 decision 3 as amended; sol finding 2).
/// A conflict is the honest outcome: the filing itself is inconsistent.
#[test]
fn same_concept_divergent_values_across_statements_is_a_typed_conflict_never_a_pick() {
    let facts = vec![
        duration_fact("ProfitLoss", "income_occ", "100", &["income", "cash_flow"]),
        duration_fact("ProfitLoss", "cfo_occ", "-100", &["income", "cash_flow"]),
    ];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert!(projected.facts.is_empty());
    assert_eq!(projected.conflicts.len(), 1);
    assert_eq!(projected.conflicts[0].metric_key, "net_profit");
}

#[test]
fn a_single_statement_filing_with_no_income_role_projects_from_comprehensive_income() {
    // LPP-shaped: only the ias_1_role-410000 comprehensive-income role
    // exists in the package; no 310000/320000 income role at all.
    let facts = vec![duration_fact(
        "ProfitLoss",
        "ci_occ",
        "250",
        &["comprehensive_income"],
    )];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert_eq!(projected.facts.len(), 1);
    assert_eq!(projected.facts[0].fact.metric_key, "net_profit");
    assert_eq!(projected.facts[0].fact.value, d("250"));
}

/// A Q3 filing tags the 3-month quarter AND the cumulative 9-month
/// figure with the same period end. The longest window is the reported
/// figure (GPW cumulative convention, `dedup_longest_duration` parity);
/// the shorter one is counted, never a conflict (sol finding 3).
#[test]
fn a_shorter_duration_window_sharing_the_period_end_never_conflicts_with_the_ytd_figure() {
    let ytd = NewTaggedFact {
        period_start: Some("2025-01-01".to_owned()),
        ..duration_fact("Revenue", "ytd_occ", "90", &["income"])
    };
    let q3_only = NewTaggedFact {
        period_start: Some("2025-07-01".to_owned()),
        ..duration_fact("Revenue", "q3_occ", "30", &["income"])
    };
    let projected = project_period(&[ytd, q3_only], "2025-12-31", true, CONSOLIDATED);

    assert_eq!(projected.facts.len(), 1);
    assert_eq!(projected.facts[0].fact.value, d("90"));
    assert_eq!(
        projected.facts[0].contributing_fact_identities,
        vec!["ytd_occ".to_owned()]
    );
    assert!(projected.conflicts.is_empty());
    assert_eq!(projected.shorter_window_skipped, 1);
}

/// A package carrying a standalone AND a consolidated filing (the TXT corpus
/// shape) projects ONLY the document's selected primary basis (ADR 0100
/// decision 2, #508 amendment) — the other instance's occurrence is dropped
/// before slotting and counted, never silently merged into the same slot
/// (the pre-#508 bug this replaces) and never projected as a second fact
/// (this test's own pre-#508 behavior, superseded: "projects BOTH, on
/// separate statement bases").
#[test]
fn only_the_selected_basis_projects_the_other_instance_is_dropped_and_counted() {
    let consolidated = NewTaggedFact {
        package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "cons_occ", "150")
    };
    let standalone = NewTaggedFact {
        package_entry_path: "Jednostkowe Sprawozdanie finansowe/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "solo_occ", "100")
    };
    let facts = [consolidated, standalone];
    let basis = select_primary_basis(&facts, true);
    assert_eq!(basis, CONSOLIDATED);
    let projected = project_period(&facts, "2025-12-31", true, basis);

    assert!(projected.conflicts.is_empty());
    assert_eq!(projected.facts.len(), 1);
    assert_eq!(projected.facts[0].fact.basis, CONSOLIDATED);
    assert_eq!(projected.facts[0].fact.value, d("150"));
    assert_eq!(
        projected.non_primary_basis_skipped, 1,
        "the standalone occurrence is dropped, never merged into the consolidated slot"
    );
    assert_eq!(projected.ambiguous_basis_skipped, 0);
}

/// Test A (#508 decision 1): `basis_of`'s three-valued classification table
/// — every standalone/consolidated token, `unconsolidated` classifying
/// STANDALONE (never letting its embedded "consolidated" substring win),
/// both tokens present -> `Ambiguous`, and no token -> `Consolidated` (the
/// documented ESEF-is-consolidated-by-default rule).
#[test]
fn basis_of_classifies_every_token_the_ambiguous_and_the_default_case() {
    let cases: &[(&str, InstanceBasis)] = &[
        (
            "reports/jednostkowe/raport.xhtml",
            InstanceBasis::Standalone,
        ),
        (
            "reports/separate-financial-statements.xhtml",
            InstanceBasis::Standalone,
        ),
        ("reports/standalone.xhtml", InstanceBasis::Standalone),
        (
            "reports/unconsolidated-statement.xhtml",
            InstanceBasis::Standalone,
        ),
        (
            "reports/skonsolidowane/raport.xhtml",
            InstanceBasis::Consolidated,
        ),
        (
            "reports/consolidated-statement.xhtml",
            InstanceBasis::Consolidated,
        ),
        (
            "reports/consolidated-and-standalone.xhtml",
            InstanceBasis::Ambiguous,
        ),
        ("reports/instance.xhtml", InstanceBasis::Consolidated),
        ("", InstanceBasis::Consolidated),
    ];
    for (path, expected) in cases {
        assert_eq!(basis_of(path), *expected, "path {path:?}");
    }
}

/// Test B (#508 decision 2): mixed instances -> `Consolidated` wins.
#[test]
fn select_primary_basis_mixed_prefers_consolidated() {
    let consolidated = NewTaggedFact {
        package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "cons_occ", "150")
    };
    let standalone = NewTaggedFact {
        package_entry_path: "reports/jednostkowe/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "solo_occ", "100")
    };
    assert_eq!(
        select_primary_basis(&[consolidated, standalone], true),
        CONSOLIDATED
    );
}

/// Test B: standalone-only -> `Standalone`.
#[test]
fn select_primary_basis_standalone_only_selects_standalone() {
    let standalone = NewTaggedFact {
        package_entry_path: "reports/jednostkowe/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "solo_occ", "100")
    };
    assert_eq!(
        select_primary_basis(&[standalone], true),
        Some(StatementBasis::Standalone)
    );
}

/// Test B: an ambiguous-only document has no eligible primary basis.
#[test]
fn select_primary_basis_ambiguous_only_selects_none() {
    let ambiguous = NewTaggedFact {
        package_entry_path: "reports/consolidated-and-standalone.xhtml".to_owned(),
        ..balance_fact("Assets", "amb_occ", "100")
    };
    assert_eq!(select_primary_basis(&[ambiguous], true), None);
}

/// Test B: a consolidated instance carrying ONLY facts that never reach
/// crosswalk resolution (an uncrosswalked extension) or the primary-statement
/// role filter (a note-level role) must never suppress a mapped, eligible
/// standalone set — the selector's evidence is "eligible primary-statement,
/// crosswalk-resolved facts", never mere instance presence.
#[test]
fn select_primary_basis_an_ineligible_consolidated_instance_never_suppresses_a_standalone_set() {
    let uncrosswalked_consolidated = NewTaggedFact {
        package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
        ..balance_fact("SomeUncrosswalkedExtension", "cons_occ", "1")
    };
    let note_level_consolidated = NewTaggedFact {
        package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
        ..duration_fact("Revenue", "cons_note_occ", "1", &["other"])
    };
    let standalone = NewTaggedFact {
        package_entry_path: "reports/jednostkowe/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "solo_occ", "100")
    };
    let basis = select_primary_basis(
        &[
            uncrosswalked_consolidated,
            note_level_consolidated,
            standalone,
        ],
        true,
    );
    assert_eq!(basis, Some(StatementBasis::Standalone));
}

/// Astra r1 #2 (spec amended, code kept as-is): a consolidated instance
/// whose MAPPED primary facts are all UNPARSEABLE (`has_usable_value`
/// rejects them first) must never suppress a usable standalone set — those
/// are stored under their true standalone label.
#[test]
fn select_primary_basis_a_consolidated_instance_with_only_unparseable_values_selects_standalone() {
    let unparseable_consolidated = [
        NewTaggedFact {
            package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
            value_numeric: None,
            ..balance_fact("Assets", "cons_assets", "0")
        },
        NewTaggedFact {
            package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
            value_numeric: None,
            ..balance_fact("Liabilities", "cons_liabilities", "0")
        },
    ];
    let standalone = [
        NewTaggedFact {
            package_entry_path: "reports/jednostkowe/raport.xhtml".to_owned(),
            ..balance_fact("Assets", "solo_assets", "50")
        },
        NewTaggedFact {
            package_entry_path: "reports/jednostkowe/raport.xhtml".to_owned(),
            ..balance_fact("Liabilities", "solo_liabilities", "20")
        },
    ];
    let mut facts = Vec::new();
    facts.extend(unparseable_consolidated);
    facts.extend(standalone);

    let basis = select_primary_basis(&facts, true);
    assert_eq!(
        basis,
        Some(StatementBasis::Standalone),
        "an unparseable consolidated value is never eligible evidence — it \
         must not suppress the usable standalone set"
    );

    // The projection stores the standalone facts under their TRUE label.
    let projected = project_period(&facts, "2025-12-31", true, basis);
    let mut got: Vec<(&str, Option<StatementBasis>)> = projected
        .facts
        .iter()
        .map(|pf| (pf.fact.metric_key.as_str(), pf.fact.basis))
        .collect();
    got.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(
        got,
        vec![
            ("total_assets", Some(StatementBasis::Standalone)),
            ("total_liabilities", Some(StatementBasis::Standalone)),
        ]
    );
}

/// Test B: instance order must never change the outcome — a pure OR over
/// evidence, never a first-wins pick.
#[test]
fn select_primary_basis_is_independent_of_instance_order() {
    let consolidated = NewTaggedFact {
        package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "cons_occ", "150")
    };
    let standalone = NewTaggedFact {
        package_entry_path: "reports/jednostkowe/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "solo_occ", "100")
    };
    let forward = select_primary_basis(&[consolidated.clone(), standalone.clone()], true);
    let reversed = select_primary_basis(&[standalone, consolidated], true);
    assert_eq!(forward, reversed);
    assert_eq!(forward, CONSOLIDATED);
}

/// Test C (#508 decision 2, the contract's asymmetric fixture): a document
/// mixing a consolidated instance (the balanced identity `Assets 100 =
/// Liabilities 60 + Equity 40`) with a standalone instance (a duplicate
/// `Assets 50` PLUS its own exclusive `CurrentLiabilities 30`) projects
/// EXACTLY the three consolidated facts, all on `Consolidated` basis; the
/// standalone occurrences are dropped and counted
/// (`non_primary_basis_skipped == 2`) — master projected the standalone
/// slots too, `current_liabilities` included.
#[test]
fn projection_on_the_asymmetric_fixture_keeps_only_the_consolidated_slots() {
    let facts = vec![
        NewTaggedFact {
            package_entry_path: "pkg/reports/skonsolidowane/instance.xhtml".to_owned(),
            ..balance_fact("Assets", "cons_assets", "100")
        },
        NewTaggedFact {
            package_entry_path: "pkg/reports/skonsolidowane/instance.xhtml".to_owned(),
            ..balance_fact("Liabilities", "cons_liabilities", "60")
        },
        NewTaggedFact {
            package_entry_path: "pkg/reports/skonsolidowane/instance.xhtml".to_owned(),
            ..balance_fact("Equity", "cons_equity", "40")
        },
        NewTaggedFact {
            package_entry_path: "pkg/reports/jednostkowe/instance.xhtml".to_owned(),
            ..balance_fact("Assets", "solo_assets", "50")
        },
        NewTaggedFact {
            package_entry_path: "pkg/reports/jednostkowe/instance.xhtml".to_owned(),
            ..balance_fact("CurrentLiabilities", "solo_current_liabilities", "30")
        },
    ];
    let basis = select_primary_basis(&facts, true);
    assert_eq!(basis, CONSOLIDATED);
    let projected = project_period(&facts, "2025-12-31", true, basis);

    let mut by_metric: Vec<(&str, Decimal, Option<StatementBasis>)> = projected
        .facts
        .iter()
        .map(|pf| (pf.fact.metric_key.as_str(), pf.fact.value, pf.fact.basis))
        .collect();
    by_metric.sort_by(|a, b| a.0.cmp(b.0));
    assert_eq!(
        by_metric,
        vec![
            ("total_assets", d("100"), CONSOLIDATED),
            ("total_equity", d("40"), CONSOLIDATED),
            ("total_liabilities", d("60"), CONSOLIDATED),
        ]
    );
    assert!(projected.conflicts.is_empty());
    assert_eq!(projected.non_primary_basis_skipped, 2);
    assert_eq!(projected.ambiguous_basis_skipped, 0);
}

/// Astra r1 #3: a MAPPED, genuinely ambiguous occurrence (both tokens in its
/// entry path) is excluded and counted (`ambiguous_basis_skipped`), separate
/// from a non-primary-basis drop; an UNMAPPED occurrence from the SAME
/// ambiguous instance is never counted there too — crosswalk resolution
/// runs before the ambiguity check, so it stays purely `uncrosswalked`.
#[test]
fn an_ambiguous_instance_counts_only_its_mapped_occurrence() {
    let selected = NewTaggedFact {
        package_entry_path: "reports/skonsolidowane/raport.xhtml".to_owned(),
        ..balance_fact("Assets", "cons_occ", "150")
    };
    let mapped_ambiguous = NewTaggedFact {
        package_entry_path: "reports/consolidated-and-standalone/raport.xhtml".to_owned(),
        ..balance_fact("Liabilities", "amb_occ", "60")
    };
    let unmapped_ambiguous = NewTaggedFact {
        package_entry_path: "reports/consolidated-and-standalone/raport.xhtml".to_owned(),
        ..balance_fact("SomeUncrosswalkedExtension", "amb_unk_occ", "1")
    };
    let facts = [selected, mapped_ambiguous, unmapped_ambiguous];
    let basis = select_primary_basis(&facts, true);
    assert_eq!(basis, CONSOLIDATED);
    let projected = project_period(&facts, "2025-12-31", true, basis);

    assert_eq!(
        projected.facts.len(),
        1,
        "only the consolidated Assets projects"
    );
    assert_eq!(projected.facts[0].fact.metric_key, "total_assets");
    assert_eq!(
        projected.ambiguous_basis_skipped, 1,
        "only the MAPPED ambiguous occurrence counts here"
    );
    assert_eq!(projected.non_primary_basis_skipped, 0);
    assert!(projected
        .uncrosswalked_concepts
        .contains("SomeUncrosswalkedExtension"));
    assert_eq!(projected.uncrosswalked_fact_count, 1);
}

/// An issuer-extension concept that reuses a STANDARD local name must
/// never resolve to the global key (sol finding 1): the namespace, not
/// the local name, is the concept's identity.
#[test]
fn an_issuer_extension_reusing_a_standard_local_name_never_resolves_to_the_global_key() {
    let extension = NewTaggedFact {
        concept_namespace_uri: "http://www.example-issuer.com/xbrl/2025-12-31".to_owned(),
        ..duration_fact("Revenue", "ext_occ", "120", &["income"])
    };
    let projected = project_period(&[extension], "2025-12-31", true, CONSOLIDATED);

    assert!(
        projected.facts.is_empty(),
        "an extension must never align with the standard `revenue` series"
    );
    assert!(projected.uncrosswalked_concepts.contains("Revenue"));
    assert_eq!(projected.uncrosswalked_fact_count, 1);
}

#[test]
fn two_rows_one_slot_identical_values_project_one_fact_marked_a_repeat() {
    let facts = vec![
        balance_fact("Assets", "occ1", "1000"),
        balance_fact("Assets", "occ2", "1000"),
    ];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

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
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

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
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

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
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert!(projected.facts.is_empty());
    assert!(projected
        .uncrosswalked_concepts
        .contains("SomeCompanyExtensionConceptNotInTheCrosswalk"));
}

/// Test B4 (#511, preservation): `equity_changes` is not one of
/// `is_primary_statement`'s four matched kinds, so an `equity_changes`-only
/// fact is excluded exactly like an unrecognised role — already true on
/// master, pinned here as part of the ADR 0100 dec. 3 amendment's test set.
#[test]
fn a_row_with_no_primary_statement_role_is_never_projected_and_is_counted() {
    let facts = vec![duration_fact("Assets", "eq1", "10", &["equity_changes"])];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert!(projected.facts.is_empty());
    assert_eq!(projected.non_primary_statement_skipped, 1);
}

/// Test B3 (#511, preservation): a role that parses but classifies
/// `other` (ADR 0100 decision 3 amendment) must never be treated as a
/// primary-statement role — `is_primary_statement` only matches the four
/// primary kinds, so this is the same rejection as no role data at all,
/// already true on master.
#[test]
fn a_row_whose_only_role_classifies_other_is_never_projected_and_is_counted() {
    let facts = vec![duration_fact("Assets", "note1", "10", &["other"])];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert!(projected.facts.is_empty());
    assert_eq!(projected.non_primary_statement_skipped, 1);
}

#[test]
fn a_row_with_no_role_at_all_is_never_projected_and_is_counted() {
    let mut no_role = balance_fact("Assets", "noroleocc", "10");
    no_role.roles = Vec::new();
    let facts = vec![no_role];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

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
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

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
        concept_namespace_uri: IFRS_NS.to_owned(),
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
    let projected = project_period(&facts, "2025-12-31", false, CONSOLIDATED);

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
    let projected = project_period(&facts, "2025-12-31", false, CONSOLIDATED);

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
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert!(projected.facts.is_empty());
    assert_eq!(projected.non_primary_statement_skipped, 1);
}

/// Test A (#509, decision 1): currency is set by the crosswalk's
/// `value_kind` at the projection, never copied verbatim from the Layer 1
/// unit measure — a `count`-kind fact (`WeightedAverageShares`, unit
/// `shares`) must never reach `currency: Some("shares")` (master: it
/// does, and the store's #93 guard then refuses the whole write).
#[test]
fn currency_is_set_by_value_kind_not_copied_from_the_unit_measure() {
    let shares = NewTaggedFact {
        unit_measure: Some("shares".to_owned()),
        ..duration_fact(
            "WeightedAverageShares",
            "shares_occ",
            "1000000",
            &["income"],
        )
    };
    let projected = project_period(&[shares], "2025-12-31", true, CONSOLIDATED);
    assert_eq!(projected.facts.len(), 1);
    assert_eq!(
        projected.facts[0].fact.currency, None,
        "a count-kind fact must never carry a unit as its currency (red \
         on master: Some(\"shares\"))"
    );

    let eps = duration_fact("BasicEarningsLossPerShare", "eps_occ", "1.23", &["income"]);
    let projected_eps = project_period(&[eps], "2025-12-31", true, CONSOLIDATED);
    assert_eq!(projected_eps.facts.len(), 1);
    assert_eq!(
        projected_eps.facts[0].fact.currency,
        Some("PLN".to_owned()),
        "preservation: a monetary fact still carries its unit as currency"
    );

    let revenue = duration_fact("Revenue", "rev_occ", "500", &["income"]);
    let projected_revenue = project_period(&[revenue], "2025-12-31", true, CONSOLIDATED);
    assert_eq!(projected_revenue.facts.len(), 1);
    assert_eq!(
        projected_revenue.facts[0].fact.currency,
        Some("PLN".to_owned()),
        "preservation: Revenue/PLN still projects Some(\"PLN\")"
    );
}

/// Test B (#509, decision 5, ADR 0049 invariant): every crosswalk entry,
/// given a synthetic primary-role fact with a unit valid for its kind,
/// projects exactly one fact whose `currency` is `Some("PLN")` iff
/// `value_kind == "monetary"`, and every projected currency passes the
/// store's ISO shape guard (`None`, or exactly three ASCII letters). Red
/// on master for the two `count` entries (`WeightedAverageShares`,
/// `AdjustedWeightedAverageShares`): they project `Some("shares")`, 6
/// letters, not 3.
#[test]
fn every_crosswalk_entry_projects_currency_by_its_own_value_kind() {
    for entry in ifrs_crosswalk::entries() {
        let unit = if entry.value_kind == "monetary" {
            "PLN"
        } else {
            "shares"
        };
        let fact = NewTaggedFact {
            unit_measure: Some(unit.to_owned()),
            concept_namespace_uri: IFRS_NS.to_owned(),
            concept_local_name: entry.concept.to_owned(),
            period_type: "duration".to_owned(),
            period_start: Some("2025-01-01".to_owned()),
            period_end: "2025-12-31".to_owned(),
            value_numeric: Some("100".to_owned()),
            roles: vec![role("income")],
            fact_identity: format!("{}_occ", entry.concept),
            ..Default::default()
        };
        let projected = project_period(&[fact], "2025-12-31", true, CONSOLIDATED);
        assert_eq!(
            projected.facts.len(),
            1,
            "{} must project exactly one fact",
            entry.concept
        );
        let currency = &projected.facts[0].fact.currency;
        if entry.value_kind == "monetary" {
            assert_eq!(
                currency,
                &Some("PLN".to_owned()),
                "{} (monetary) must carry its unit as currency",
                entry.concept
            );
        } else {
            assert_eq!(
                currency, &None,
                "{} ({}) must never carry a currency",
                entry.concept, entry.value_kind
            );
        }
        if let Some(c) = currency {
            assert!(
                c.len() == 3 && c.chars().all(|ch| ch.is_ascii_alphabetic()),
                "{}: projected currency {c:?} must pass the store's ISO shape guard",
                entry.concept
            );
        }
    }
}

/// Test B (#509, decision 5): entries sharing a `metric_key` agree on
/// `value_kind` — the slot key stays `(basis, metric_key)`, never
/// `(basis, entry)` (ADR 0100 decision 2), so a per-slot currency-by-kind
/// rule would be ambiguous if aliases disagreed.
#[test]
fn crosswalk_entries_sharing_a_metric_key_agree_on_value_kind() {
    let mut by_metric: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for entry in ifrs_crosswalk::entries() {
        match by_metric.get(entry.metric_key) {
            Some(kind) => assert_eq!(
                *kind, entry.value_kind,
                "metric_key {} carries conflicting value_kind across aliases",
                entry.metric_key
            ),
            None => {
                by_metric.insert(entry.metric_key, entry.value_kind);
            }
        }
    }
}

/// The plan's promised alias case: `Revenue` and
/// `RevenueFromContractsWithCustomers` both resolve to the `revenue`
/// `metric_key` (`ifrs_crosswalk.rs`) — real crosswalk resolution through
/// `project_period`, not a fabricated slot. Equal values -> one
/// re-observed fact tracing back to both occurrences.
#[test]
fn revenue_and_revenue_from_contracts_alias_with_equal_values_project_one_fact() {
    let facts = vec![
        duration_fact("Revenue", "rev_occ", "500", &["income"]),
        duration_fact(
            "RevenueFromContractsWithCustomers",
            "rfc_occ",
            "500",
            &["income"],
        ),
    ];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert_eq!(projected.facts.len(), 1);
    assert_eq!(projected.facts[0].fact.metric_key, "revenue");
    assert_eq!(projected.facts[0].fact.value, d("500"));
    let mut ids = projected.facts[0].contributing_fact_identities.clone();
    ids.sort();
    assert_eq!(ids, vec!["rev_occ".to_owned(), "rfc_occ".to_owned()]);
    assert!(projected.conflicts.is_empty());
}

/// The same alias pair with DIVERGENT values: a typed conflict, nothing
/// emitted — never a document-order pick between two differently-tagged
/// alias concepts.
#[test]
fn revenue_and_revenue_from_contracts_alias_with_divergent_values_is_a_typed_conflict() {
    let facts = vec![
        duration_fact("Revenue", "rev_occ", "500", &["income"]),
        duration_fact(
            "RevenueFromContractsWithCustomers",
            "rfc_occ",
            "600",
            &["income"],
        ),
    ];
    let projected = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

    assert!(
        projected.facts.is_empty(),
        "a genuine alias value disagreement must never resolve by a pick"
    );
    assert_eq!(projected.conflicts.len(), 1);
    assert_eq!(projected.conflicts[0].metric_key, "revenue");
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
        // no-assert-ok: no-panic invariant (ADR 0049) — not panicking IS the test.
        #[test]
        fn never_panics(mut facts in prop::collection::vec(arbitrary_fact(), 0..12)) {
            for (i, f) in facts.iter_mut().enumerate() {
                f.fact_identity = format!("f{i}");
            }
            let _ = project_period(&facts, "2025-12-31", true, CONSOLIDATED);
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
            let original = project_period(&facts, "2025-12-31", true, CONSOLIDATED);

            // A cheap deterministic shuffle: rotate by a seed-derived amount
            // and reverse every other element — no external RNG dependency.
            let mut shuffled = facts.clone();
            if !shuffled.is_empty() {
                let rotate_by = (seed as usize) % shuffled.len();
                shuffled.rotate_left(rotate_by);
                shuffled.reverse();
            }
            let reordered = project_period(&shuffled, "2025-12-31", true, CONSOLIDATED);

            prop_assert_eq!(canonical(&original), canonical(&reordered));
        }
    }
}
