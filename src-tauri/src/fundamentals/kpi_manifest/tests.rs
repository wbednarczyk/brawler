//! Tests for the deterministic KPI ingest manifest (#361). Bez DB — pure
//! rule-core + `SealedManifest` tests only; the atomic storage/service
//! layers are exercised in `storage::kpi_ingest_staging` and
//! `jobs::kpi_ingest_validation`.

use super::*;
use rust_decimal::Decimal;
use std::str::FromStr;

fn amt(s: &str) -> Decimal {
    Decimal::from_str(s).expect("valid decimal literal")
}

fn base_run() -> ManifestRunInput {
    ManifestRunInput {
        run_id: "run1".to_owned(),
        revision: 1,
        company_id: "c1".to_owned(),
        report_document_id: "doc1".to_owned(),
        source_content_hash: Some("hash-src".to_owned()),
        scope: "consolidated".to_owned(),
        data_quality: "final".to_owned(),
        period_id: None,
        fiscal_year: 2025,
        period_type: "FY".to_owned(),
        missing_reasons: MissingReasons::None,
        expected: None,
    }
}

/// A plausible-by-default history (median ~1250) close enough to every test
/// fixture's value (100s-10,000s range) that the plausibility gate stays
/// `Plausible` unless a test deliberately overrides `history`.
fn resolved_def(metric_key: &str, value_kind: &str) -> ResolvedDefinitionInput {
    ResolvedDefinitionInput {
        definition_id: format!("kpidef_{metric_key}"),
        metric_key: metric_key.to_owned(),
        value_kind: value_kind.to_owned(),
        history: SlotHistoryInput {
            values: vec![amt("1000"), amt("1500")],
        },
    }
}

/// A clean, fully-passing monetary observation — every test mutates from here.
fn base_obs(id: &str, ordinal: i64) -> ManifestObservationInput {
    ManifestObservationInput {
        observation_id: id.to_owned(),
        ordinal,
        raw_value: "1 234,5".to_owned(),
        metric_key_candidate: Some("revenue".to_owned()),
        mapping_status: "mapped".to_owned(),
        normalized_value: Some("1234.5".to_owned()),
        currency: Some("PLN".to_owned()),
        unit_scale: Some("ones".to_owned()),
        measure_window: Some("flow".to_owned()),
        attribution: None,
        scope: None,
        citation_page: Some(3),
        citation_table: None,
        citation_row: None,
        citation_quote: Some("Przychody 1 234,5".to_owned()),
        definition: Some(resolved_def("revenue", "monetary")),
    }
}

fn codes_for<'a>(manifest: &'a KpiIngestManifest, id: &str) -> &'a [Code] {
    &manifest
        .observations
        .iter()
        .find(|o| o.observation_id == id)
        .expect("observation present")
        .codes
}

fn has_code(codes: &[Code], code: DiagnosticCode) -> bool {
    codes.iter().any(|c| c.code == code)
}

// ---------------------------------------------------------------------------
// Group 1: per-code-family core rules
// ---------------------------------------------------------------------------

#[test]
fn clean_observation_passes_with_no_codes() {
    let manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    assert_eq!(codes_for(&manifest, "o1"), &[] as &[Code]);
    assert_eq!(
        manifest.observations[0].validation_state,
        ValidationState::Passed
    );
    assert_eq!(manifest.outcome, Outcome::Ready);
}

#[test]
fn unmapped_observation_gets_mapping_unresolved_and_skips_dependent_checks() {
    let mut obs = base_obs("o1", 0);
    obs.mapping_status = "unmapped".to_owned();
    obs.definition = None;
    // Also broken currency/scale — must NOT fire since mapping is unresolved.
    obs.currency = None;
    let manifest = build_manifest(&base_run(), &[obs]);
    let codes = codes_for(&manifest, "o1");
    assert!(has_code(codes, DiagnosticCode::MappingUnresolved));
    assert!(!has_code(codes, DiagnosticCode::UnitCurrencyMissing));
    assert!(!has_code(codes, DiagnosticCode::UnitScaleIncoherent));
    assert!(!has_code(codes, DiagnosticCode::PeriodWindowKindMismatch));
    assert_eq!(
        manifest.observations[0].validation_state,
        ValidationState::Flagged
    );
}

#[test]
fn value_unparseable_skips_identity_plausibility_duplicate_but_not_unit_checks() {
    let mut a = base_obs("a", 0);
    a.normalized_value = None;
    a.currency = None; // monetary + no currency -> unit.currency_missing must still fire
    let manifest = build_manifest(&base_run(), &[a]);
    let codes = codes_for(&manifest, "a");
    assert!(has_code(codes, DiagnosticCode::ValueUnparseable));
    assert!(has_code(codes, DiagnosticCode::UnitCurrencyMissing));
}

#[test]
fn attribution_none_defaults_to_total_in_output() {
    let manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    assert_eq!(manifest.observations[0].attribution, "total");
}

#[test]
fn duplicate_slot_with_different_values_is_conflict_on_all_members() {
    let a = base_obs("a", 0);
    let mut b = base_obs("b", 1);
    b.normalized_value = Some("999".to_owned());
    let manifest = build_manifest(&base_run(), &[a, b]);
    for id in ["a", "b"] {
        let codes = codes_for(&manifest, id);
        assert!(has_code(codes, DiagnosticCode::DuplicateConflict), "{id}");
        assert!(!has_code(codes, DiagnosticCode::DuplicateRepeat), "{id}");
    }
}

#[test]
fn duplicate_slot_with_equal_values_is_repeat_on_all_members() {
    let a = base_obs("a", 0);
    let b = base_obs("b", 1); // same slot, same normalized_value
    let manifest = build_manifest(&base_run(), &[a, b]);
    for id in ["a", "b"] {
        let codes = codes_for(&manifest, id);
        assert!(has_code(codes, DiagnosticCode::DuplicateRepeat), "{id}");
        assert!(!has_code(codes, DiagnosticCode::DuplicateConflict), "{id}");
    }
}

#[test]
fn identity_violation_only_attributed_to_total_attribution_observations() {
    // Balance sheet identity: assets = liabilities + equity. Break it.
    let mut assets = base_obs("assets", 0);
    assets.metric_key_candidate = Some("total_assets".to_owned());
    assets.definition = Some(resolved_def("total_assets", "monetary"));
    assets.normalized_value = Some("1000".to_owned());
    assets.raw_value = "1000".to_owned();

    let mut liabilities = base_obs("liabilities", 1);
    liabilities.metric_key_candidate = Some("total_liabilities".to_owned());
    liabilities.definition = Some(resolved_def("total_liabilities", "monetary"));
    liabilities.normalized_value = Some("100".to_owned());
    liabilities.raw_value = "100".to_owned();

    let mut equity = base_obs("equity", 2);
    equity.metric_key_candidate = Some("total_equity".to_owned());
    equity.definition = Some(resolved_def("total_equity", "monetary"));
    equity.normalized_value = Some("100".to_owned());
    equity.raw_value = "100".to_owned();
    // NCI-attributed sibling with a DIFFERENT (irrelevant) value: must never
    // be pulled into the total-only FactSet nor flagged.
    let mut equity_nci = base_obs("equity_nci", 3);
    equity_nci.metric_key_candidate = Some("total_equity".to_owned());
    equity_nci.definition = Some(resolved_def("total_equity", "monetary"));
    equity_nci.attribution = Some("nci".to_owned());
    equity_nci.normalized_value = Some("9999".to_owned());
    equity_nci.raw_value = "9999".to_owned();

    let manifest = build_manifest(&base_run(), &[assets, liabilities, equity, equity_nci]);
    assert!(has_code(
        codes_for(&manifest, "assets"),
        DiagnosticCode::IdentityViolation
    ));
    assert!(has_code(
        codes_for(&manifest, "liabilities"),
        DiagnosticCode::IdentityViolation
    ));
    assert!(has_code(
        codes_for(&manifest, "equity"),
        DiagnosticCode::IdentityViolation
    ));
    assert!(!has_code(
        codes_for(&manifest, "equity_nci"),
        DiagnosticCode::IdentityViolation
    ));
}

#[test]
fn identity_not_applicable_never_produces_a_code() {
    // Only total_assets present -> balance_sheet_identity is NotApplicable
    // (missing inputs), never a violation.
    let mut assets = base_obs("assets", 0);
    assets.metric_key_candidate = Some("total_assets".to_owned());
    assets.definition = Some(resolved_def("total_assets", "monetary"));
    let manifest = build_manifest(&base_run(), &[assets]);
    assert!(!has_code(
        codes_for(&manifest, "assets"),
        DiagnosticCode::IdentityViolation
    ));
}

#[test]
fn scale_eff_defaults_to_ones_when_unit_scale_is_none() {
    let mut obs = base_obs("o1", 0);
    obs.unit_scale = None;
    obs.raw_value = "1234,5".to_owned();
    obs.normalized_value = Some("1234.5".to_owned());
    let manifest = build_manifest(&base_run(), &[obs]);
    assert!(!has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::UnitScaleIncoherent
    ));
}

#[test]
fn scale_incoherent_when_parsed_times_factor_disagrees_with_normalized() {
    let mut obs = base_obs("o1", 0);
    obs.unit_scale = Some("thousands".to_owned());
    obs.raw_value = "1234,5".to_owned(); // parses to 1234.5 (comma decimal)
    obs.normalized_value = Some("1234.5".to_owned()); // should have been 1234500
    let manifest = build_manifest(&base_run(), &[obs]);
    assert!(has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::UnitScaleIncoherent
    ));
}

#[test]
fn scale_coherent_thousands_passes() {
    let mut obs = base_obs("o1", 0);
    obs.unit_scale = Some("thousands".to_owned());
    obs.raw_value = "1234,5".to_owned();
    obs.normalized_value = Some("1234500".to_owned());
    let manifest = build_manifest(&base_run(), &[obs]);
    assert!(!has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::UnitScaleIncoherent
    ));
}

#[test]
fn raw_value_unparseable_is_scale_unverifiable_not_incoherent() {
    let mut obs = base_obs("o1", 0);
    obs.raw_value = "patrz przypis 12".to_owned();
    let manifest = build_manifest(&base_run(), &[obs]);
    let codes = codes_for(&manifest, "o1");
    assert!(has_code(codes, DiagnosticCode::UnitScaleUnverifiable));
    assert!(!has_code(codes, DiagnosticCode::UnitScaleIncoherent));
    assert_eq!(
        manifest.observations[0].validation_state,
        ValidationState::Unreviewed
    );
}

#[test]
fn currency_missing_and_unexpected() {
    let mut monetary_no_currency = base_obs("m", 0);
    monetary_no_currency.currency = None;
    let mut ratio_with_currency = base_obs("r", 1);
    ratio_with_currency.metric_key_candidate = Some("roe".to_owned());
    ratio_with_currency.definition = Some(resolved_def("roe", "ratio"));
    ratio_with_currency.currency = Some("PLN".to_owned());
    let manifest = build_manifest(&base_run(), &[monetary_no_currency, ratio_with_currency]);
    assert!(has_code(
        codes_for(&manifest, "m"),
        DiagnosticCode::UnitCurrencyMissing
    ));
    assert!(has_code(
        codes_for(&manifest, "r"),
        DiagnosticCode::UnitCurrencyUnexpected
    ));
}

#[test]
fn window_missing_and_scope_mismatch_and_citation_missing() {
    let mut no_window = base_obs("w", 0);
    no_window.measure_window = None;
    let mut wrong_scope = base_obs("s", 1);
    wrong_scope.scope = Some("standalone".to_owned()); // run is consolidated
    let mut no_citation = base_obs("c", 2);
    no_citation.citation_page = None;
    no_citation.citation_quote = None;
    let manifest = build_manifest(&base_run(), &[no_window, wrong_scope, no_citation]);
    assert!(has_code(
        codes_for(&manifest, "w"),
        DiagnosticCode::PeriodWindowMissing
    ));
    assert!(has_code(
        codes_for(&manifest, "s"),
        DiagnosticCode::ScopeRunMismatch
    ));
    assert!(has_code(
        codes_for(&manifest, "c"),
        DiagnosticCode::CitationMissing
    ));
}

#[test]
fn citation_with_table_and_row_is_sufficient_without_page_or_quote() {
    let mut obs = base_obs("o1", 0);
    obs.citation_page = None;
    obs.citation_quote = None;
    obs.citation_table = Some("Bilans".to_owned());
    obs.citation_row = Some("Aktywa razem".to_owned());
    let manifest = build_manifest(&base_run(), &[obs]);
    assert!(!has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::CitationMissing
    ));
}

#[test]
fn window_kind_mismatch_stock_with_flow_window() {
    let mut obs = base_obs("o1", 0);
    obs.metric_key_candidate = Some("total_assets".to_owned());
    obs.definition = Some(resolved_def("total_assets", "monetary"));
    obs.measure_window = Some("flow".to_owned());
    let manifest = build_manifest(&base_run(), &[obs]);
    assert!(has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::PeriodWindowKindMismatch
    ));
}

#[test]
fn window_kind_mismatch_flow_with_point_in_time() {
    let mut obs = base_obs("o1", 0);
    obs.measure_window = Some("point_in_time".to_owned());
    let manifest = build_manifest(&base_run(), &[obs]);
    assert!(has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::PeriodWindowKindMismatch
    ));
}

#[test]
fn plausibility_implausible_flags_with_history_median_detail() {
    let mut obs = base_obs("o1", 0);
    obs.definition = Some(ResolvedDefinitionInput {
        history: SlotHistoryInput {
            // `history_median` takes the upper-middle element of the sorted
            // magnitudes (fundamentals::validation), so [10, 12] -> "12".
            values: vec![amt("10"), amt("12")],
        },
        ..resolved_def("revenue", "monetary")
    });
    let manifest = build_manifest(&base_run(), &[obs]);
    let codes = codes_for(&manifest, "o1");
    assert!(has_code(codes, DiagnosticCode::PlausibilityImplausible));
    let detail = codes
        .iter()
        .find(|c| c.code == DiagnosticCode::PlausibilityImplausible)
        .and_then(|c| c.detail.clone());
    assert_eq!(
        detail,
        Some(DiagnosticDetail::HistoryMedian {
            history_median: "12".to_owned()
        })
    );
    assert_eq!(
        manifest.observations[0].validation_state,
        ValidationState::Flagged
    );
}

#[test]
fn plausibility_abstained_thin_history_is_unreviewed_with_reason() {
    let mut obs = base_obs("o1", 0);
    obs.definition = Some(ResolvedDefinitionInput {
        history: SlotHistoryInput { values: Vec::new() },
        ..resolved_def("revenue", "monetary")
    });
    let manifest = build_manifest(&base_run(), &[obs]);
    let codes = codes_for(&manifest, "o1");
    assert!(has_code(codes, DiagnosticCode::PlausibilityAbstained));
    let detail = codes
        .iter()
        .find(|c| c.code == DiagnosticCode::PlausibilityAbstained)
        .and_then(|c| c.detail.clone());
    assert_eq!(
        detail,
        Some(DiagnosticDetail::Reason {
            reason: "thin_history".to_owned()
        })
    );
    assert_eq!(
        manifest.observations[0].validation_state,
        ValidationState::Unreviewed
    );
}

// ---------------------------------------------------------------------------
// Group 2: period grammar
// ---------------------------------------------------------------------------

#[test]
fn supported_period_grammar_never_fires_the_code() {
    for pt in ["Q1", "H1", "9M", "FY"] {
        let mut run = base_run();
        run.period_type = pt.to_owned();
        let manifest = build_manifest(&run, &[base_obs("o1", 0)]);
        assert!(
            manifest.run_diagnostics.is_empty(),
            "{pt} must not raise run.unsupported_period_grammar"
        );
        assert_eq!(manifest.outcome, Outcome::Ready, "{pt}");
    }
}

#[test]
fn unsupported_period_grammar_fails_the_run() {
    for pt in ["Q2", "Q3", "Q4", "H2", "M01"] {
        let mut run = base_run();
        run.period_type = pt.to_owned();
        let manifest = build_manifest(&run, &[base_obs("o1", 0)]);
        assert!(
            has_code(
                &manifest.run_diagnostics,
                DiagnosticCode::RunUnsupportedPeriodGrammar
            ),
            "{pt}"
        );
        assert_eq!(manifest.outcome, Outcome::Failed, "{pt}");
    }
}

#[test]
fn flow_metric_on_interim_period_passes_without_a_code_adr_0093() {
    let mut run = base_run();
    run.period_type = "H1".to_owned();
    let manifest = build_manifest(&run, &[base_obs("o1", 0)]); // revenue, flow window
    assert!(!has_code(
        codes_for(&manifest, "o1"),
        DiagnosticCode::PeriodWindowKindMismatch
    ));
}

// ---------------------------------------------------------------------------
// Group 3: completeness + missing_reasons
// ---------------------------------------------------------------------------

fn expected(keys: &[&str]) -> ExpectedSnapshot {
    ExpectedSnapshot {
        schema_version: 1,
        source: "kpi_relevance".to_owned(),
        pack_version: None,
        keys: keys.iter().map(|s| s.to_string()).collect(),
    }
}

#[test]
fn missing_expected_kpi_without_reason_fails_the_run() {
    let mut run = base_run();
    run.expected = Some(expected(&["revenue", "net_profit"]));
    let manifest = build_manifest(&run, &[base_obs("o1", 0)]); // only revenue present
    assert!(has_code(
        &manifest.run_diagnostics,
        DiagnosticCode::CompletenessMissingWithoutReason
    ));
    assert_eq!(manifest.outcome, Outcome::Failed);
    let completeness = manifest.completeness.expect("completeness computed");
    assert_eq!(completeness.missing.len(), 1);
    assert_eq!(completeness.missing[0].metric_key, "net_profit");
    assert!(completeness.missing[0].reason.is_none());
}

#[test]
fn missing_expected_kpi_with_reason_passes() {
    let mut run = base_run();
    run.expected = Some(expected(&["revenue", "net_profit"]));
    run.missing_reasons = MissingReasons::Valid(
        [(
            "net_profit".to_owned(),
            "not disclosed this period".to_owned(),
        )]
        .into_iter()
        .collect(),
    );
    let manifest = build_manifest(&run, &[base_obs("o1", 0)]);
    assert!(!has_code(
        &manifest.run_diagnostics,
        DiagnosticCode::CompletenessMissingWithoutReason
    ));
    assert_eq!(manifest.outcome, Outcome::Ready);
    let completeness = manifest.completeness.expect("completeness computed");
    assert_eq!(
        completeness.missing[0].reason.as_deref(),
        Some("not disclosed this period")
    );
}

#[test]
fn malformed_missing_reasons_raises_run_invalid_missing_reasons() {
    let mut run = base_run();
    run.missing_reasons = MissingReasons::Invalid;
    let manifest = build_manifest(&run, &[base_obs("o1", 0)]);
    assert!(has_code(
        &manifest.run_diagnostics,
        DiagnosticCode::RunInvalidMissingReasons
    ));
    assert_eq!(manifest.outcome, Outcome::Failed);
}

#[test]
fn no_expected_configuration_yields_no_completeness_gate() {
    let run = base_run(); // expected: None
    let manifest = build_manifest(&run, &[base_obs("o1", 0)]);
    assert!(manifest.completeness.is_none());
    assert!(!has_code(
        &manifest.run_diagnostics,
        DiagnosticCode::CompletenessMissingWithoutReason
    ));
}

#[test]
fn expected_snapshot_with_empty_keys_yields_no_completeness_gate() {
    let mut run = base_run();
    run.expected = Some(expected(&[]));
    let manifest = build_manifest(&run, &[base_obs("o1", 0)]);
    assert!(manifest.completeness.is_none());
}

// ---------------------------------------------------------------------------
// Group 4 (pure slice): seal() consistency re-verification
// ---------------------------------------------------------------------------

#[test]
fn seal_accepts_a_builder_produced_manifest() {
    let manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    let sealed = SealedManifest::seal(manifest).expect("consistent manifest seals");
    assert_eq!(sealed.outcome(), Outcome::Ready);
    assert_eq!(sealed.manifest_hash().len(), 64);
    assert!(sealed
        .manifest_hash()
        .chars()
        .all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn seal_rejects_passed_state_with_a_flagged_code() {
    let mut manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    manifest.observations[0]
        .codes
        .push(Code::bare(DiagnosticCode::MappingUnresolved));
    // stated validationState left at Passed -- inconsistent with the pushed code.
    let err = SealedManifest::seal(manifest).expect_err("flagged code under passed state");
    assert!(matches!(
        err,
        ManifestSealError::ObservationStateMismatch { .. }
    ));
}

#[test]
fn seal_rejects_passed_state_with_an_abstained_code() {
    let mut manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    manifest.observations[0].codes.push(Code {
        code: DiagnosticCode::PlausibilityAbstained,
        detail: Some(DiagnosticDetail::Reason {
            reason: "thin_history".to_owned(),
        }),
    });
    let err = SealedManifest::seal(manifest).expect_err("unreviewed code under passed state");
    assert!(matches!(
        err,
        ManifestSealError::ObservationStateMismatch { .. }
    ));
}

#[test]
fn seal_rejects_ready_outcome_over_a_flagged_observation() {
    let mut obs = base_obs("o1", 0);
    obs.currency = None; // flags: unit.currency_missing
    let mut manifest = build_manifest(&base_run(), &[obs]);
    assert_eq!(
        manifest.outcome,
        Outcome::Failed,
        "sanity: builder derives failed"
    );
    manifest.outcome = Outcome::Ready; // hand-tamper
    let err = SealedManifest::seal(manifest).expect_err("flagged obs cannot be ready");
    assert!(matches!(err, ManifestSealError::OutcomeMismatch { .. }));
}

#[test]
fn seal_rejects_mismatched_counts() {
    let mut manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    manifest.counts.passed = 0;
    manifest.counts.flagged = 1;
    let err = SealedManifest::seal(manifest).expect_err("counts must match derived");
    assert!(matches!(err, ManifestSealError::CountsMismatch { .. }));
}

// ---------------------------------------------------------------------------
// Group 6: determinism / golden / proptest
// ---------------------------------------------------------------------------

#[test]
fn two_builds_of_the_same_input_are_byte_identical() {
    let run = base_run();
    let observations = vec![base_obs("o1", 0), base_obs("o2", 1)];
    let a = SealedManifest::seal(build_manifest(&run, &observations)).expect("seals");
    let b = SealedManifest::seal(build_manifest(&run, &observations)).expect("seals");
    assert_eq!(a.manifest_json(), b.manifest_json());
    assert_eq!(a.manifest_hash(), b.manifest_hash());
}

#[test]
fn permuting_observation_input_order_does_not_change_the_hash() {
    let run = base_run();
    let observations = vec![base_obs("o1", 0), base_obs("o2", 1), base_obs("o3", 2)];
    let mut reversed = observations.clone();
    reversed.reverse();
    let a = SealedManifest::seal(build_manifest(&run, &observations)).expect("seals");
    let b = SealedManifest::seal(build_manifest(&run, &reversed)).expect("seals");
    assert_eq!(a.manifest_hash(), b.manifest_hash());
}

#[test]
fn changing_a_value_changes_the_hash() {
    let run = base_run();
    let a = SealedManifest::seal(build_manifest(&run, &[base_obs("o1", 0)])).expect("seals");
    let mut changed = base_obs("o1", 0);
    changed.normalized_value = Some("1234.6".to_owned());
    let b = SealedManifest::seal(build_manifest(&run, &[changed])).expect("seals");
    assert_ne!(a.manifest_hash(), b.manifest_hash());
}

#[test]
fn manifest_json_round_trips_through_serde() {
    let manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    let bytes = serde_json::to_string(&manifest).expect("serialize");
    let back: KpiIngestManifest = serde_json::from_str(&bytes).expect("deserialize");
    let bytes_again = serde_json::to_string(&back).expect("re-serialize");
    assert_eq!(bytes, bytes_again);
}

#[test]
fn golden_ready_manifest_bytes() {
    let manifest = build_manifest(&base_run(), &[base_obs("o1", 0)]);
    let sealed = SealedManifest::seal(manifest).expect("seals");
    insta::assert_snapshot!("kpi_manifest_ready_bytes", sealed.manifest_json());
}

#[test]
fn golden_failed_manifest_bytes() {
    let mut obs = base_obs("o1", 0);
    obs.currency = None;
    let manifest = build_manifest(&base_run(), &[obs]);
    let sealed = SealedManifest::seal(manifest).expect("seals");
    insta::assert_snapshot!("kpi_manifest_failed_bytes", sealed.manifest_json());
}

proptest::proptest! {
    #[test]
    fn evaluate_never_panics_on_arbitrary_strings(
        raw in ".{0,20}",
        normalized in proptest::option::of(".{0,20}"),
        currency in proptest::option::of(".{0,5}"),
        scale in proptest::option::of("ones|thousands|millions|bogus"),
        window in proptest::option::of("flow|point_in_time|trailing|cumulative|duration|bogus"),
        mapping_status in "unmapped|mapped|no_definition",
    ) {
        let mut obs = base_obs("o1", 0);
        obs.raw_value = raw;
        obs.normalized_value = normalized;
        obs.currency = currency;
        obs.unit_scale = scale;
        obs.measure_window = window;
        obs.mapping_status = mapping_status;
        // evaluate() must be total: no panics for any of these combinations.
        let _ = build_manifest(&base_run(), &[obs]);
    }
}

// ---------------------------------------------------------------------------
// Group 10: registry guard + module boundary guard
// ---------------------------------------------------------------------------

#[test]
fn registry_is_exhaustive_and_every_literal_is_unique() {
    assert_eq!(
        REGISTRY.len(),
        18,
        "update this count deliberately with the registry"
    );
    let mut seen = std::collections::HashSet::new();
    for code in REGISTRY {
        assert!(
            seen.insert(code.as_str()),
            "duplicate literal: {}",
            code.as_str()
        );
        assert_eq!(DiagnosticCode::parse(code.as_str()), Ok(*code));
    }
}

#[test]
fn registry_golden_list() {
    let literals: Vec<&str> = REGISTRY.iter().map(|c| c.as_str()).collect();
    insta::assert_debug_snapshot!("kpi_diagnostic_code_registry", literals);
}

#[test]
fn unknown_code_token_is_a_typed_error() {
    assert!(DiagnosticCode::parse("not.a.real.code").is_err());
}

#[test]
fn module_boundary_never_imports_crate_storage() {
    let source = include_str!("../kpi_manifest.rs");
    assert!(
        !source.contains("crate::storage"),
        "fundamentals::kpi_manifest must stay storage-free (sol r5-1)"
    );
}
