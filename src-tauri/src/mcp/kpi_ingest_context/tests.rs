//! Behavior tests for [`super`]'s two acquisition-workflow read tools.

use super::super::kpi_ingest::test_support::*;
use super::catalog::candidate_window;
use super::*;
use crate::report_documents_capture::content_hash_hex;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::json;

fn doc_hash() -> String {
    content_hash_hex(DOC_BYTES)
}

/// Start the shared `doc1` run with full context → `extracting`, hash
/// frozen, blob pinned.
fn started_run(state: &AppState) -> String {
    let payload = success(acquisition_call(
        state,
        "start_kpi_ingest",
        &full_start_args(),
    ));
    payload["runId"].as_str().expect("runId").to_owned()
}

fn context(state: &AppState, args: serde_json::Value) -> ToolOutcome {
    acquisition_call(state, "get_kpi_ingest_context", &args)
}

fn document_chunk(state: &AppState, args: serde_json::Value) -> ToolOutcome {
    acquisition_call(state, "get_kpi_ingest_document", &args)
}

#[allow(clippy::too_many_arguments)]
fn seed_definition_raw(
    state: &AppState,
    id: &str,
    scope: &str,
    company_id: Option<&str>,
    metric_key: &str,
    label: &str,
    value_kind: &str,
    origin: &str,
) {
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "INSERT INTO kpi_definitions
                (id, scope, company_id, metric_key, label, value_kind, origin)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![id, scope, company_id, metric_key, label, value_kind, origin],
        )
        .expect("definition row");
}

fn seed_attempt_raw(
    state: &AppState,
    id: &str,
    run_id: &str,
    revision: i64,
    attempt: i64,
    outcome: &str,
    observation_count: usize,
) {
    let observations: Vec<Value> = (0..observation_count)
        .map(|ordinal| json!({ "ordinal": ordinal, "metricKey": format!("m{ordinal}") }))
        .collect();
    let manifest = json!({
        "manifestSchemaVersion": 1,
        "runId": run_id,
        "revision": revision,
        "outcome": outcome,
        "runDiagnostics": [],
        "completeness": { "expected": [], "present": [], "missing": [] },
        "observations": observations,
    });
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "INSERT INTO kpi_ingest_validation_attempts
                (id, run_id, revision, attempt, outcome, manifest_hash, manifest_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                run_id,
                revision,
                attempt,
                outcome,
                format!("hash-{id}"),
                manifest.to_string(),
            ],
        )
        .expect("attempt row");
}

// ------------------------------------------------------------------
// Default call
// ------------------------------------------------------------------

#[test]
fn default_context_golden_shape() {
    let state = test_state();
    let run_id = started_run(&state);
    let payload = success(context(&state, json!({ "runId": run_id })));
    let pretty = serde_json::to_string_pretty(&payload).expect("serializable");
    let redacted = regex::Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z")
        .expect("regex")
        .replace_all(&pretty, "[timestamp]")
        .into_owned();
    let redacted = regex::Regex::new(r"kpiing_[0-9a-f]+")
        .expect("regex")
        .replace_all(&redacted, "kpiing_[uid]")
        .into_owned();
    insta::assert_snapshot!("context_default_wire_shape", redacted);
}

#[test]
fn cursor_or_limit_without_section_is_invalid_input() {
    let state = test_state();
    let run_id = started_run(&state);
    for args in [
        json!({ "runId": run_id, "cursor": "abc" }),
        json!({ "runId": run_id, "limit": 5 }),
    ] {
        assert_eq!(
            failure_code(context(&state, args)),
            CommandErrorCode::InvalidInput
        );
    }
}

#[test]
fn unknown_run_is_not_found_and_control_chars_are_invalid_input() {
    let state = test_state();
    assert_eq!(
        failure_code(context(
            &state,
            json!({ "runId": "kpiing_ffffffffffffffffffffffffffffffff" })
        )),
        CommandErrorCode::NotFound
    );
    assert_eq!(
        failure_code(context(&state, json!({ "runId": "bad\u{0001}id" }))),
        CommandErrorCode::InvalidInput
    );
    assert_eq!(
        failure_code(document_chunk(
            &state,
            json!({ "runId": "bad\u{0001}id", "offset": 0, "length": 1 })
        )),
        CommandErrorCode::InvalidInput
    );
}

// ------------------------------------------------------------------
// Catalog
// ------------------------------------------------------------------

#[test]
fn catalog_carries_expected_keys_plus_minted_extras_only() {
    let state = test_state();
    seed_definition_raw(
        &state,
        "kdmint",
        "company",
        Some("c1"),
        "custom_pipeline_yield",
        "Custom pipeline yield",
        "currency",
        "agent",
    );
    seed_definition_raw(
        &state,
        "kduser",
        "company",
        Some("c1"),
        "user_only_metric",
        "User-created",
        "currency",
        "user",
    );
    let run_id = started_run(&state);
    let payload = success(context(&state, json!({ "runId": run_id })));
    let keys: Vec<&str> = payload["catalog"]
        .as_array()
        .expect("catalog")
        .iter()
        .map(|entry| entry["metricKey"].as_str().expect("key"))
        .collect();

    assert!(keys.contains(&"net_profit"), "expected floor key present");
    assert!(
        keys.contains(&"custom_pipeline_yield"),
        "agent-minted company extra present"
    );
    assert!(
        !keys.contains(&"user_only_metric"),
        "a user-origin company definition is not a minted extra"
    );
    // Full entries (expected + agent-minted) sort ahead of every compact
    // canonical entry (ADR 0101 dec. 7/9 tier order), not plain alphabetical
    // — the two Full keys here stay internally sorted.
    let full_keys: Vec<&str> = keys
        .iter()
        .copied()
        .filter(|key| *key == "net_profit" || *key == "custom_pipeline_yield")
        .collect();
    let mut sorted_full = full_keys.clone();
    sorted_full.sort_unstable();
    assert_eq!(
        full_keys, sorted_full,
        "full-tier entries sort by metric key"
    );
}

/// §G harvest (epic #399 S8): the compact tier served SECTOR-scoped rows
/// to companies of OTHER sectors — the entry hides its scope, the agent
/// maps to the key, and resolution then refuses it (`mapping.unresolved`).
/// A sector row is visible only to companies of that statement type.
#[test]
fn catalog_compact_tier_excludes_foreign_sector_keys() {
    let state = test_state();
    let run_id = started_run(&state);

    let mut keys: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = json!({ "runId": run_id, "section": "catalog", "limit": 64 });
        if let Some(cursor) = &cursor {
            args["cursor"] = json!(cursor);
        }
        let page = success(context(&state, args));
        for entry in page["catalog"].as_array().expect("page") {
            keys.push(entry["metricKey"].as_str().expect("key").to_owned());
        }
        match page["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }

    // `operating_expenses` is seeded sector='banking'; the test company is
    // not a bank, so the key must NOT be offered (it would never resolve).
    // Its canonical sibling `operating_expense` stays visible.
    assert!(
        !keys.iter().any(|k| k == "operating_expenses"),
        "foreign-sector key offered to a non-banking company"
    );
    assert!(
        keys.iter().any(|k| k == "operating_expense"),
        "canonical sibling must stay visible"
    );
}

/// ADR 0101 dec. 7/9: `build_catalog` widens to the full canon visible to
/// this company — crossing the `CATALOG_PAGE_MAX` boundary, so the default
/// call's `catalog` is always a truncated prefix; walking the `catalog`
/// section cursor to exhaustion must reach every visible row exactly once.
#[test]
fn catalog_section_pagination_reaches_full_canon_without_duplicates() {
    let state = test_state();
    let run_id = started_run(&state);

    let canonical_count: i64 = {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .query_row(
                // Mirrors build_catalog's compact-tier predicate: a
                // sector row counts only for its own statement type
                // (§G harvest, epic #399 S8).
                "SELECT COUNT(*) FROM kpi_definitions
                 WHERE company_id IS NULL
                   AND (sector IS NULL OR sector = (
                        SELECT statement_type FROM companies
                        WHERE id = (SELECT company_id FROM kpi_ingest_runs WHERE id = ?1)))",
                [&run_id],
                |row| row.get(0),
            )
            .expect("count")
    };
    assert!(
        canonical_count as usize > CATALOG_PAGE_MAX,
        "the widened canon crosses the {CATALOG_PAGE_MAX}-entry page boundary: \
         {canonical_count}"
    );

    let mut walked: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = json!({ "runId": run_id, "section": "catalog", "limit": 64 });
        if let Some(cursor) = &cursor {
            args["cursor"] = json!(cursor);
        }
        let page = success(context(&state, args));
        assert_eq!(page["section"], "catalog");
        for entry in page["catalog"].as_array().expect("page") {
            walked.push(entry["metricKey"].as_str().expect("key").to_owned());
        }
        match page["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    assert_eq!(
        walked.len(),
        canonical_count as usize,
        "every canonical row reached exactly once"
    );
    let mut deduped = walked.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(deduped.len(), walked.len(), "no duplicates across pages");

    // The default call's own catalog is a prefix of the full walk, with a
    // cursor signalling truncation (the canon now always exceeds one page).
    let full = success(context(&state, json!({ "runId": run_id })));
    let default_page: Vec<String> = full["catalog"]
        .as_array()
        .expect("catalog")
        .iter()
        .map(|entry| entry["metricKey"].as_str().expect("key").to_owned())
        .collect();
    assert_eq!(
        &walked[..default_page.len()],
        default_page.as_slice(),
        "the default call's catalog page is a prefix of the full walk"
    );
    assert!(
        full["truncated"]["catalog"].is_string(),
        "the default call's catalog is always truncated once the canon exceeds one page"
    );
}

/// ADR 0101 dec. 7: the catalog now carries every canonical definition,
/// not only what this run expected — an agent can check "does this
/// already exist" before proposing.
#[test]
fn catalog_includes_compact_canonical_beyond_expected() {
    let state = test_state();
    let run_id = started_run(&state);
    let payload = success(context(&state, json!({ "runId": run_id })));
    let expected: BTreeSet<String> = payload["run"]["expectedKpis"]["keys"]
        .as_array()
        .expect("expected keys")
        .iter()
        .map(|key| key.as_str().expect("key").to_owned())
        .collect();
    let catalog = payload["catalog"].as_array().expect("catalog");
    assert!(
        catalog.len() > expected.len(),
        "the widened catalog carries canon beyond the expected floor: {} vs {}",
        catalog.len(),
        expected.len()
    );
    let compact = catalog
        .iter()
        .find(|entry| !expected.contains(entry["metricKey"].as_str().expect("key")))
        .expect("a canonical entry beyond expected");
    let mut keys: Vec<&str> = compact
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["label", "metricKey"],
        "a canon entry beyond expected is the compact shape: {compact}"
    );
}

/// ADR 0101 dec. 7/9: full metadata is reserved for the run's expected
/// keys and the company's agent-minted extras; every other canonical row
/// is the compact `{metricKey, label}` projection.
#[test]
fn expected_and_agent_entries_carry_full_metadata_compact_rest() {
    let state = test_state();
    seed_definition_raw(
        &state,
        "kdmint2",
        "company",
        Some("c1"),
        "custom_pipeline_yield_2",
        "Custom pipeline yield 2",
        "currency",
        "agent",
    );
    let run_id = started_run(&state);
    let payload = success(context(&state, json!({ "runId": run_id })));
    let catalog = payload["catalog"].as_array().expect("catalog");

    for key in ["net_profit", "custom_pipeline_yield_2"] {
        let entry = catalog
            .iter()
            .find(|entry| entry["metricKey"] == key)
            .unwrap_or_else(|| panic!("{key} present in catalog"));
        assert!(
            entry["definitionId"].is_string(),
            "{key} carries definitionId: {entry}"
        );
        assert!(
            entry["statementGroup"].is_string(),
            "{key} carries statementGroup: {entry}"
        );
        assert!(
            entry["valueKind"].is_string(),
            "{key} carries valueKind: {entry}"
        );
        assert!(entry["origin"].is_string(), "{key} carries origin: {entry}");
    }

    let expected: BTreeSet<String> = payload["run"]["expectedKpis"]["keys"]
        .as_array()
        .expect("expected keys")
        .iter()
        .map(|key| key.as_str().expect("key").to_owned())
        .collect();
    let compact = catalog
        .iter()
        .find(|entry| {
            let key = entry["metricKey"].as_str().expect("key");
            key != "custom_pipeline_yield_2" && !expected.contains(key)
        })
        .expect("a compact canonical entry");
    assert!(
        compact["definitionId"].is_null(),
        "compact entry omits definitionId: {compact}"
    );
}

#[test]
fn cursors_round_trip_pipes_unicode_and_max_length_legacy_keys() {
    let state = test_state();
    // Pre-guard legacy identities seeded raw: a pipe+unicode key and a
    // 300-byte key (the write bound is 256 B — these rows predate it).
    seed_definition_raw(
        &state,
        "kdpipe",
        "company",
        Some("c1"),
        "weird|key_π",
        "Pipe key",
        "currency",
        "agent",
    );
    let long_key = "x".repeat(300);
    seed_definition_raw(
        &state,
        "kdlong",
        "company",
        Some("c1"),
        &long_key,
        "Long key",
        "currency",
        "agent",
    );
    let run_id = started_run(&state);

    let mut seen: Vec<String> = Vec::new();
    let mut cursor: Option<String> = None;
    loop {
        let mut args = json!({ "runId": run_id, "section": "catalog", "limit": 1 });
        if let Some(cursor) = &cursor {
            args["cursor"] = json!(cursor);
        }
        let page = success(context(&state, args));
        for entry in page["catalog"].as_array().expect("page") {
            seen.push(entry["metricKey"].as_str().expect("key").to_owned());
        }
        match page["nextCursor"].as_str() {
            Some(next) => cursor = Some(next.to_owned()),
            None => break,
        }
    }
    assert!(seen.contains(&"weird|key_π".to_owned()));
    assert!(seen.contains(&long_key));
    let mut deduped = seen.clone();
    deduped.dedup();
    assert_eq!(seen, deduped, "no entry repeats across limit=1 pages");
}

#[test]
fn malformed_cursors_are_invalid_input() {
    let state = test_state();
    let run_id = started_run(&state);
    for cursor in [
        "not-base64url-!!!",
        &URL_SAFE_NO_PAD.encode(b"not json"),
        &URL_SAFE_NO_PAD.encode(br#"{"wrong":"fields"}"#),
    ] {
        assert_eq!(
            failure_code(context(
                &state,
                json!({ "runId": run_id, "section": "catalog", "cursor": cursor })
            )),
            CommandErrorCode::InvalidInput,
            "cursor {cursor:?}"
        );
    }
}

#[test]
fn the_start_sentinel_reads_from_the_beginning_and_is_refused_for_manifest() {
    let state = test_state();
    let run_id = started_run(&state);
    let sentinel = start_sentinel_cursor();
    let page = success(context(
        &state,
        json!({ "runId": run_id, "section": "catalog", "cursor": sentinel, "limit": 2 }),
    ));
    assert_eq!(
        page["catalog"].as_array().expect("page").len(),
        2,
        "the {{}} sentinel starts from the beginning"
    );
    assert_eq!(
        failure_code(context(
            &state,
            json!({ "runId": run_id, "section": "manifest", "cursor": sentinel })
        )),
        CommandErrorCode::InvalidInput,
        "a manifest continuation must pin its attempt"
    );
}

#[test]
fn section_limits_outside_the_cap_are_budget_refusals() {
    let state = test_state();
    let run_id = started_run(&state);
    for (section, limit) in [
        ("catalog", 0),
        ("catalog", 65),
        ("plausibility", 65),
        ("manifest", 51),
    ] {
        assert_eq!(
            failure_code(context(
                &state,
                json!({ "runId": run_id, "section": section, "limit": limit })
            )),
            CommandErrorCode::ResponseBudgetExceeded,
            "{section} limit {limit}"
        );
    }
}

#[test]
fn overlong_stored_labels_are_byte_truncated_with_a_marker() {
    let state = test_state();
    // 150 two-byte chars = 300 bytes — over the 256-byte label cap.
    let label = "ł".repeat(150);
    seed_definition_raw(
        &state,
        "kdlab",
        "company",
        Some("c1"),
        "labelled_metric",
        &label,
        "currency",
        "agent",
    );
    let run_id = started_run(&state);
    let payload = success(context(&state, json!({ "runId": run_id })));
    let entry = payload["catalog"]
        .as_array()
        .expect("catalog")
        .iter()
        .find(|entry| entry["metricKey"] == "labelled_metric")
        .expect("entry");
    let label = entry["label"].as_str().expect("label");
    assert!(
        label.len() <= 256,
        "label stays ≤256 bytes: {}",
        label.len()
    );
    assert!(label.ends_with('…'), "truncation carries the marker");
}

// ------------------------------------------------------------------
// Plausibility
// ------------------------------------------------------------------

fn seed_period_and_fact(
    state: &AppState,
    period_id: &str,
    fiscal_year: i64,
    period_type: &str,
    definition_id: &str,
    value: &str,
) {
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "INSERT OR IGNORE INTO financial_periods
                (id, company_id, fiscal_year, period_type, period_end_date)
             VALUES (?1, 'c1', ?2, ?3, ?4)",
            rusqlite::params![
                period_id,
                fiscal_year,
                period_type,
                format!("{fiscal_year}-12-31"),
            ],
        )
        .expect("period");
    connection
        .execute(
            "INSERT INTO financial_facts
                (id, company_id, period_id, definition_id, value_numeric, statement_basis,
                 attribution, variant, measure_window, data_quality)
             VALUES (?1, 'c1', ?2, ?3, ?4, 'consolidated', 'total', 'reported', 'flow',
                     'final')",
            rusqlite::params![
                format!("f-{period_id}-{definition_id}"),
                period_id,
                definition_id,
                value,
            ],
        )
        .expect("fact");
}

fn canonical_definition_id(state: &AppState, metric_key: &str) -> String {
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .query_row(
            "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'canonical'",
            [metric_key],
            |row| row.get(0),
        )
        .expect("canonical definition")
}

#[test]
fn plausibility_observed_slot_matches_the_validator_history() {
    let state = test_state();
    let revenue = canonical_definition_id(&state, "revenue");
    seed_period_and_fact(&state, "p2022", 2022, "FY", &revenue, "100");
    seed_period_and_fact(&state, "p2023", 2023, "FY", &revenue, "300");
    seed_period_and_fact(&state, "p2024", 2024, "FY", &revenue, "500");
    // Start with consolidated scope so the seeded slots are in-basis.
    let payload = success(acquisition_call(
        &state,
        "start_kpi_ingest",
        &json!({
            "documentId": "doc1",
            "profileId": "gpw_ifrs_annual",
            "scope": "consolidated",
            "dataQuality": "final",
            "period": { "fiscalYear": 2026, "periodType": "FY" }
        }),
    ));
    let run_id = payload["runId"].as_str().expect("runId").to_owned();

    let full = success(context(&state, json!({ "runId": run_id })));
    let entries = full["plausibility"].as_array().expect("plausibility");
    let observed = entries
        .iter()
        .find(|entry| entry["metricKey"] == "revenue" && entry["slotOrigin"] == "observed")
        .expect("observed revenue slot");

    // history_median of [100, 300, 500] = 300 (upper middle); every point
    // is non-zero; chronological recent points.
    assert_eq!(observed["median"], "300");
    assert_eq!(observed["nonZeroCount"], 3);
    assert_eq!(observed["abstentionReason"], Value::Null);
    assert_eq!(observed["slot"]["scope"], "consolidated");
    assert_eq!(observed["slot"]["attribution"], "total");
    assert_eq!(observed["slot"]["measureWindow"], "flow");
    let years: Vec<i64> = observed["recentPoints"]
        .as_array()
        .expect("points")
        .iter()
        .map(|point| point["fiscalYear"].as_i64().expect("year"))
        .collect();
    assert_eq!(years, vec![2022, 2023, 2024], "chronological order");

    // The observed slot equals the candidate default slot here, so no
    // duplicate candidate entry exists for revenue/consolidated.
    let revenue_entries: Vec<_> = entries
        .iter()
        .filter(|entry| entry["metricKey"] == "revenue")
        .collect();
    assert_eq!(
        revenue_entries.len(),
        1,
        "an observed default slot suppresses its candidate twin: {revenue_entries:?}"
    );

    // A history-less expected definition still gets candidate evidence.
    let candidate = entries
        .iter()
        .find(|entry| entry["metricKey"] == "net_profit")
        .expect("net_profit candidate");
    assert_eq!(candidate["slotOrigin"], "candidate");
    assert_eq!(candidate["median"], Value::Null);
    assert_eq!(candidate["nonZeroCount"], 0);
    assert_eq!(candidate["abstentionReason"], "thin_history");
    assert_eq!(
        candidate["recentPoints"].as_array().expect("points").len(),
        0
    );

    // Stocks classify point_in_time via the validator's own classifier.
    let stock = entries
        .iter()
        .find(|entry| entry["metricKey"] == "total_assets")
        .expect("total_assets candidate");
    assert_eq!(stock["slot"]["measureWindow"], "point_in_time");
}

#[test]
fn unattached_scope_serves_candidates_for_both_bases() {
    let state = test_state();
    // Two-phase fresh start: no scope/quality/period → source_captured.
    let payload = success(acquisition_call(
        &state,
        "start_kpi_ingest",
        &json!({ "documentId": "doc1", "profileId": "gpw_ifrs_annual" }),
    ));
    assert_eq!(payload["status"], "source_captured");
    let run_id = payload["runId"].as_str().expect("runId").to_owned();

    let full = success(context(&state, json!({ "runId": run_id })));
    let scopes: std::collections::BTreeSet<&str> = full["plausibility"]
        .as_array()
        .expect("plausibility")
        .iter()
        .filter(|entry| entry["metricKey"] == "revenue")
        .map(|entry| entry["slot"]["scope"].as_str().expect("scope"))
        .collect();
    assert_eq!(
        scopes.into_iter().collect::<Vec<_>>(),
        vec!["consolidated", "standalone"],
        "no attached scope → evidence for both bases"
    );
}

/// ADR 0101 dec. 8: plausibility stays computed only from `expected`
/// (cost unchanged — `build_plausibility` is untouched by this slice);
/// a catalog key outside `expected` is `notRequested`, an explicit signal
/// on the response rather than a silent absence a caller could misread
/// as "no history exists".
#[test]
fn plausibility_not_requested_for_unexpected_key() {
    let state = test_state();
    let run_id = started_run(&state);
    let payload = success(context(&state, json!({ "runId": run_id })));
    let expected: BTreeSet<String> = payload["run"]["expectedKpis"]["keys"]
        .as_array()
        .expect("expected keys")
        .iter()
        .map(|key| key.as_str().expect("key").to_owned())
        .collect();
    let unexpected_key = payload["catalog"]
        .as_array()
        .expect("catalog")
        .iter()
        .map(|entry| entry["metricKey"].as_str().expect("key").to_owned())
        .find(|key| !expected.contains(key))
        .expect("a catalog key outside expected");

    assert!(
        payload["notRequested"]
            .as_array()
            .expect("notRequested")
            .iter()
            .any(|key| key.as_str() == Some(unexpected_key.as_str())),
        "the unexpected key reads as notRequested: {:?}",
        payload["notRequested"]
    );
    assert!(
        !payload["plausibility"]
            .as_array()
            .expect("plausibility")
            .iter()
            .any(|entry| entry["metricKey"] == unexpected_key),
        "an unrequested key never gets plausibility evidence — absence, not a computed \
         abstention"
    );
}

#[test]
fn candidate_window_is_profile_aware_and_classifier_driven() {
    // `period_nature` decides instant/duration; the profile decides the
    // duration window (interim = cumulative, ADR 0098 dec. 3). `instant`
    // (e.g. `total_assets`, `wdf_book_value_per_share`,
    // `shares_outstanding`) always short-circuits to `point_in_time`,
    // whatever the profile.
    assert_eq!(candidate_window("gpw_ifrs_annual@v1", "duration"), "flow");
    assert_eq!(candidate_window("gpw_interim@v1", "duration"), "cumulative");
    assert_eq!(
        candidate_window("gpw_interim@v1", "instant"),
        "point_in_time"
    );
    assert_eq!(
        candidate_window("gpw_ifrs_annual@v1", "instant"),
        "point_in_time"
    );
    // ADR 0100 decision 6 fix: `roe` is a ratio, never TTM-eligible, but
    // it is duration-REPORTED (not in STOCK_METRIC_KEYS) -- so its
    // candidate window is `flow`/`cumulative`, never `point_in_time` as
    // the old `is_flow_key`-based classifier (which conflated the
    // TTM-eligibility and window-kind axes) produced for it. TTM
    // eligibility is the separate question `is_ttm_eligible` answers.
    assert_eq!(
        candidate_window("gpw_ifrs_annual@v1", "duration"),
        "flow",
        "a duration ratio like roe gets a flow window, not point_in_time"
    );
}

// ------------------------------------------------------------------
// derivedPeriod + document meta
// ------------------------------------------------------------------

#[test]
fn derived_period_hint_requires_matching_provenance() {
    let state = test_state();
    let run_id = started_run(&state);
    let hash = doc_hash();
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE report_documents SET content_hash = ?1 WHERE id = 'doc1'",
                [&hash],
            )
            .expect("stamp doc hash");
    }
    // Cache bound to the SAME bytes the run pinned → the hint serves.
    state
        .financials()
        .store_derived_period("doc1", Some((2025, "FY", "2025-12-31")), 2, Some(&hash))
        .expect("cache");
    let full = success(context(&state, json!({ "runId": run_id })));
    assert_eq!(full["derivedPeriod"]["fiscalYear"], 2025);
    assert_eq!(full["derivedPeriod"]["periodType"], "FY");
    assert_eq!(full["derivedPeriod"]["periodEnd"], "2025-12-31");

    // A→B→A: the cache now describes OTHER bytes — the hint must go null
    // even though the document row's hash still matches the run's.
    state
        .financials()
        .store_derived_period("doc1", Some((1999, "FY", "1999-12-31")), 2, Some("bbbb"))
        .expect("cache for other bytes");
    let full = success(context(&state, json!({ "runId": run_id })));
    assert_eq!(full["derivedPeriod"], Value::Null);

    // Legacy NULL-provenance row → null too.
    state
        .financials()
        .store_derived_period("doc1", Some((1999, "FY", "1999-12-31")), 2, None)
        .expect("legacy row");
    let full = success(context(&state, json!({ "runId": run_id })));
    assert_eq!(full["derivedPeriod"], Value::Null);
}

#[test]
fn document_meta_reports_the_pinned_blob_not_the_recaptured_row() {
    let state = test_state();
    let run_id = started_run(&state);

    // Simulate a recapture: bigger file at local_path, bigger byte_size on
    // the row. The context must keep describing the run's frozen blob.
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE report_documents SET byte_size = 999999 WHERE id = 'doc1'",
                [],
            )
            .expect("update row");
    }
    std::fs::write(
        state.data_dir().join("report_documents/doc1.pdf"),
        vec![0u8; 4096],
    )
    .expect("recapture bytes");

    let full = success(context(&state, json!({ "runId": run_id })));
    assert_eq!(
        full["document"]["byteSize"],
        DOC_BYTES.len() as i64,
        "byteSize is the pinned blob's size"
    );
    assert_eq!(full["document"]["url"], "https://x/doc1.pdf");
}

// ------------------------------------------------------------------
// Manifest section
// ------------------------------------------------------------------

#[test]
fn manifest_availability_flips_and_the_section_pins_its_attempt() {
    let state = test_state();
    let run_id = started_run(&state);

    let before = success(context(&state, json!({ "runId": run_id })));
    assert_eq!(before["manifestAvailable"], false);
    assert_eq!(before["truncated"].get("manifest"), None);
    assert_eq!(
        failure_code(context(
            &state,
            json!({ "runId": run_id, "section": "manifest" })
        )),
        CommandErrorCode::Conflict,
        "no attempt yet → conflict"
    );

    seed_attempt_raw(&state, "att1", &run_id, 1, 1, "failed", 7);
    let after = success(context(&state, json!({ "runId": run_id })));
    assert_eq!(after["manifestAvailable"], true);
    assert_eq!(after["truncated"]["manifest"], true);

    // Page 1: header + first observations; the FAILED attempt serves (the
    // run row's manifest_hash is NULL — this is the repair context).
    let page1 = success(context(
        &state,
        json!({ "runId": run_id, "section": "manifest", "limit": 5 }),
    ));
    let manifest = &page1["manifest"];
    assert_eq!(manifest["manifestSchemaVersion"], 1, "header on page 1");
    assert_eq!(manifest["outcome"], "failed");
    assert_eq!(manifest["observations"].as_array().expect("obs").len(), 5);
    let cursor = page1["nextCursor"].as_str().expect("cursor").to_owned();

    // A NEWER attempt lands between pages — the pinned cursor must keep
    // serving the ORIGINAL manifest, never splice two together.
    seed_attempt_raw(&state, "att2", &run_id, 2, 1, "ready", 1);
    let page2 = success(context(
        &state,
        json!({ "runId": run_id, "section": "manifest", "cursor": cursor, "limit": 5 }),
    ));
    let manifest2 = &page2["manifest"];
    assert_eq!(
        manifest2.get("manifestSchemaVersion"),
        None,
        "continuation pages carry observations only"
    );
    let ordinals: Vec<i64> = manifest2["observations"]
        .as_array()
        .expect("obs")
        .iter()
        .map(|observation| observation["ordinal"].as_i64().expect("ordinal"))
        .collect();
    assert_eq!(
        ordinals,
        vec![5, 6],
        "the pinned attempt's tail, not att2's"
    );
    assert_eq!(page2["nextCursor"], Value::Null);

    // A FRESH section call (no cursor) now serves the newer attempt.
    let fresh = success(context(
        &state,
        json!({ "runId": run_id, "section": "manifest" }),
    ));
    assert_eq!(fresh["manifest"]["outcome"], "ready");
}

// ------------------------------------------------------------------
// Receipt section (ADR 0102 dec. 12, epic #399 S6)
// ------------------------------------------------------------------

fn seed_receipt_raw(state: &AppState, run_id: &str, outcomes_json: &str) {
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "INSERT INTO kpi_ingest_commit_receipts
                (id, run_id, manifest_hash, manifest_revision, terminal_status,
                 period_id, accepted_count, outcomes_schema_version, outcomes_json)
             VALUES (?1, ?2, ?3, 1, 'complete', NULL, 1, 2, ?4)",
            rusqlite::params![
                format!("rcpt-{run_id}"),
                run_id,
                "0".repeat(64),
                outcomes_json,
            ],
        )
        .expect("receipt row");
}

/// No receipt yet → `conflict` (mirrors `manifestAvailable`'s "no attempt
/// yet" gate); `commit_kpi_ingest`'s response stopped carrying the full
/// outcomes ledger (ADR 0102 dec. 12) — this section is the paged
/// substitute, reading the SAME stored `outcomes_json` the bounded
/// `CommitReceiptDto` only counts (`receipt_v2_outcome_excluded_and_ledger`,
/// `kpi_ingest_submit.rs`, covers the internal parse/validate step this
/// section's OWN read shares).
#[test]
fn receipt_section_serves_the_full_excluded_ledger() {
    let state = test_state();
    let run_id = started_run(&state);

    assert_eq!(
        failure_code(context(
            &state,
            json!({ "runId": run_id, "section": "receipt" })
        )),
        CommandErrorCode::Conflict,
        "no receipt yet → conflict"
    );

    let outcomes = json!([
        {
            "observationId": "kpiobs_1", "revision": 1, "ordinal": 0,
            "metricKey": "revenue", "factId": "fact_1", "outcome": "created"
        },
        {
            "observationId": "kpiobs_2", "revision": 1, "ordinal": 1,
            "metricKey": "", "factId": Value::Null, "outcome": "excluded",
            "detail": { "label": "Liczba pracowników", "reason": "not a KPI" }
        }
    ])
    .to_string();
    seed_receipt_raw(&state, &run_id, &outcomes);

    let page = success(context(
        &state,
        json!({ "runId": run_id, "section": "receipt" }),
    ));
    assert_eq!(page["receipt"]["terminalStatus"], "complete");
    assert_eq!(page["receipt"]["outcomesSchemaVersion"], 2);
    let served: Vec<Value> = page["receipt"]["outcomes"]
        .as_array()
        .expect("outcomes")
        .clone();
    assert_eq!(served.len(), 2);
    assert_eq!(served[1]["outcome"], "excluded");
    assert_eq!(served[1]["detail"]["label"], "Liczba pracowników");
    assert_eq!(served[1]["detail"]["reason"], "not a KPI");
    assert_eq!(page["nextCursor"], Value::Null);
}

/// A draft never validates or commits (ADR 0102 dec. 11): while a draft
/// is open and has an appended chunk on a run that has never been staged,
/// `validate_kpi_ingest`/`commit_kpi_ingest` see EXACTLY the run's real
/// state (never `staged`) — the draft is structurally invisible to both,
/// same as it is to `list_staged_observations`
/// (`append_never_bumps_revision_invisible_to_validation`,
/// `kpi_ingest_drafts.rs`).
#[test]
fn draft_cannot_validate_or_commit() {
    let state = test_state();
    let run_id = started_run(&state);
    success(acquisition_call(
        &state,
        "stage_kpi_observations",
        &json!({
            "runId": run_id,
            "draft": { "open": true, "expectedObservations": 1 },
        }),
    ));

    let validated = success(acquisition_call(
        &state,
        "validate_kpi_ingest",
        &json!({ "runId": run_id, "revision": 1 }),
    ));
    assert_eq!(
        validated["outcome"], "superseded",
        "the run was never staged — an open draft is not a staged revision"
    );

    assert_eq!(
        failure_code(acquisition_call(
            &state,
            "commit_kpi_ingest",
            &json!({
                "runId": run_id,
                "manifestHash": "0".repeat(64),
                "revision": 1,
            })
        )),
        CommandErrorCode::Conflict,
        "no ready_to_commit generation exists — a draft never produces one"
    );
}

// ------------------------------------------------------------------
// Budget: dynamic shrink + defensive gate
// ------------------------------------------------------------------

fn plant_last_error(state: &AppState, run_id: &str, bytes: usize) {
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "UPDATE kpi_ingest_runs SET last_error = ?1 WHERE id = ?2",
            rusqlite::params!["e".repeat(bytes), run_id],
        )
        .expect("plant last_error");
}

#[test]
fn an_adversarial_baseline_shrinks_sections_instead_of_refusing() {
    let state = test_state();
    let run_id = started_run(&state);
    // A pre-bound legacy row: 259 KB of last_error — baseline + sections
    // would exceed the budget, sections alone fit easily.
    plant_last_error(&state, &run_id, 259_000);

    let full = success(context(&state, json!({ "runId": run_id })));
    let serialized = serde_json::to_vec(&full).expect("serialize");
    assert!(
        serialized.len() <= 262_144,
        "the shrunk response fits: {} bytes",
        serialized.len()
    );
    let truncated = &full["truncated"];
    assert!(
        truncated.get("catalog").is_some() || truncated.get("plausibility").is_some(),
        "at least one section was shrunk with a cursor: {truncated:?}"
    );

    // Every shrunk row is reachable via its section call (no dead end).
    let full_catalog_via_section = success(context(
        &state,
        json!({ "runId": run_id, "section": "catalog" }),
    ));
    assert!(
        !full_catalog_via_section["catalog"]
            .as_array()
            .expect("catalog")
            .is_empty(),
        "section calls have a small baseline and serve the entries"
    );
}

#[test]
fn a_baseline_beyond_the_budget_is_a_defensive_typed_refusal() {
    let state = test_state();
    let run_id = started_run(&state);
    plant_last_error(&state, &run_id, 300_000);
    assert_eq!(
        failure_code(context(&state, json!({ "runId": run_id }))),
        CommandErrorCode::ResponseBudgetExceeded
    );
}

/// Mark a definition as a company's PRIMARY relevant KPI — the real source
/// `expected_primary_metric_keys` (financials.rs) reads, so `start_kpi_ingest`
/// stamps it into the run's expected set at creation exactly as production
/// does (no hand-written stamp).
fn seed_primary_relevance(state: &AppState, definition_id: &str) {
    let connection = state.checkout_for_tests().expect("raw");
    connection
        .execute(
            "INSERT INTO kpi_relevance (id, company_id, definition_id, status, source, rank)
             VALUES (?1, 'c1', ?2, 'active', 'manual', 'primary')",
            rusqlite::params![format!("krel-{definition_id}"), definition_id],
        )
        .expect("relevance row");
}

/// A REALISTIC populated default context (sol #387 B1): unlike the
/// adversarial baseline (which plants a producer-invalid 259 KB `last_error`
/// to force shrinking), this seeds a producer-valid run through the real
/// read path — both pageable sections full to their caps, each slot carrying
/// the maximum recent history, output strings near their byte caps — and
/// proves the ≤256 KiB budget holds WITHOUT any shrink. Cardinalities are
/// asserted first so a future fixture shrinkage cannot leave a vacuous green.
#[test]
fn a_realistic_full_context_fits_the_budget_without_shrinking() {
    let state = test_state();
    // 64 CANONICAL definitions, each marked a PRIMARY relevant KPI for the
    // company and given eight consolidated FY facts (the run's FY2025 is
    // excluded from history). `start_kpi_ingest` then stamps them into the
    // run's expected set through the real producer path — agent-minted
    // definitions never enter that set (ADR 0093 dec. 4). Labels near
    // LABEL_MAX; every slot carries RECENT_POINTS_MAX points.
    for idx in 0..CATALOG_PAGE_MAX {
        let id = format!("kdpop{idx:02}");
        let key = format!("mkey_{idx:02}");
        let label = format!(
            "Skonsolidowany wskaźnik operacyjny numer {idx:02} — pozycja sprawozdania z \
             całkowitych dochodów grupy kapitałowej w ujęciu narastającym, wyrażona w tysiącach \
             złotych, wraz z komentarzem zarządu o czynnikach zmiany rok do roku i sezonowości"
        );
        seed_definition_raw(
            &state,
            &id,
            "canonical",
            None,
            &key,
            &label,
            "currency",
            "system",
        );
        seed_primary_relevance(&state, &id);
        for year in 2016..2024 {
            seed_period_and_fact(
                &state,
                &format!("finper_c1_{year}_fy"),
                year,
                "FY",
                &id,
                &format!("{year}000000"),
            );
        }
    }

    // Start consolidated (matching the seeded facts' basis) with full context.
    // The expected-KPI stamp is now the natural union of the company's primary
    // relevance and the profile pack — no hand-written stamp.
    let run_id = success(acquisition_call(
        &state,
        "start_kpi_ingest",
        &json!({
            "documentId": "doc1",
            "profileId": "gpw_ifrs_annual",
            "scope": "consolidated",
            "dataQuality": "final",
            "period": { "fiscalYear": 2025, "periodType": "FY" }
        }),
    ))["runId"]
        .as_str()
        .expect("runId")
        .to_owned();

    let payload = success(context(&state, json!({ "runId": run_id })));
    let catalog = payload["catalog"].as_array().expect("catalog");
    let plausibility = payload["plausibility"].as_array().expect("plausibility");

    // Pinned cardinalities FIRST — both sections full to their page caps, and
    // at least one slot carries the maximum recent history.
    assert_eq!(
        catalog.len(),
        CATALOG_PAGE_MAX,
        "catalog full to its page cap"
    );
    assert_eq!(
        plausibility.len(),
        PLAUSIBILITY_PAGE_MAX,
        "plausibility full to its page cap (not byte-shrunk below it)"
    );
    let max_points = plausibility
        .iter()
        .filter_map(|entry| entry["recentPoints"].as_array().map(Vec::len))
        .max()
        .expect("a slot with history");
    assert_eq!(
        max_points, RECENT_POINTS_MAX,
        "a slot carries the max recent history"
    );

    // The realistic full-page response fits the budget with no shrink.
    let serialized = serde_json::to_vec(&payload).expect("serialize");
    assert!(
        serialized.len() <= RESPONSE_BUDGET_BYTES,
        "a realistic full context fits the budget: {} bytes",
        serialized.len()
    );
}

// ------------------------------------------------------------------
// get_kpi_ingest_document
// ------------------------------------------------------------------

#[test]
fn document_chunks_slice_verify_and_reassemble() {
    let state = test_state();
    let run_id = started_run(&state);
    let before = BLOB_HASH_COUNT.with(std::cell::Cell::get);

    let first = success(document_chunk(
        &state,
        json!({ "runId": run_id, "offset": 0, "length": 10 }),
    ));
    assert_eq!(first["totalBytes"], DOC_BYTES.len() as u64);
    assert_eq!(first["sha256"], doc_hash());
    assert_eq!(first["eof"], false);
    let second = success(document_chunk(
        &state,
        json!({ "runId": run_id, "offset": 10, "length": 262144 }),
    ));
    assert_eq!(second["eof"], true);
    assert_eq!(second["length"], (DOC_BYTES.len() - 10) as u64);

    let mut reassembled = STANDARD
        .decode(first["bytesBase64"].as_str().expect("b64"))
        .expect("decode");
    reassembled.extend(
        STANDARD
            .decode(second["bytesBase64"].as_str().expect("b64"))
            .expect("decode"),
    );
    assert_eq!(reassembled, DOC_BYTES, "chunks reassemble the exact bytes");

    let after = BLOB_HASH_COUNT.with(std::cell::Cell::get);
    assert_eq!(
        after - before,
        1,
        "verification hashes the blob ONCE; later chunks seek"
    );

    // Read at/past EOF: empty + eof, offset echoed (u64::MAX saturates).
    let at_end = success(document_chunk(
        &state,
        json!({ "runId": run_id, "offset": DOC_BYTES.len() as u64, "length": 1 }),
    ));
    assert_eq!(at_end["bytesBase64"], "");
    assert_eq!(at_end["length"], 0);
    assert_eq!(at_end["eof"], true);
    let far = success(document_chunk(
        &state,
        json!({ "runId": run_id, "offset": u64::MAX, "length": 1 }),
    ));
    assert_eq!(far["eof"], true);
    assert_eq!(far["length"], 0);
}

#[test]
fn document_length_outside_the_cap_is_a_budget_refusal() {
    let state = test_state();
    let run_id = started_run(&state);
    for length in [0u64, 262_145] {
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": run_id, "offset": 0, "length": length })
            )),
            CommandErrorCode::ResponseBudgetExceeded,
            "length {length}"
        );
    }
}

#[test]
fn document_availability_follows_the_source_not_the_status() {
    let state = test_state();
    // A run that never captured: created raw in `discovered`, then walked
    // to cancelled/failed — the document is a conflict in all three.
    {
        let connection = state.checkout_for_tests().expect("raw");
        for (id, status) in [
            ("kpiing_00000000000000000000000000000001", "discovered"),
            ("kpiing_00000000000000000000000000000002", "cancelled"),
            ("kpiing_00000000000000000000000000000003", "failed"),
        ] {
            connection
                .execute(
                    "INSERT INTO kpi_ingest_runs
                        (id, report_document_id, company_id, profile_version, status)
                     VALUES (?1, 'doc1', 'c1', 'gpw_ifrs_annual@v1', ?2)",
                    rusqlite::params![id, status],
                )
                .expect("run row");
        }
    }
    for id in [
        "kpiing_00000000000000000000000000000001",
        "kpiing_00000000000000000000000000000002",
        "kpiing_00000000000000000000000000000003",
    ] {
        assert_eq!(
            failure_code(document_chunk(
                &state,
                json!({ "runId": id, "offset": 0, "length": 1 })
            )),
            CommandErrorCode::Conflict,
            "{id}: no captured source → conflict"
        );
    }

    // A terminal run WITH a pinned source stays readable.
    let run_id = started_run(&state);
    success(acquisition_call(
        &state,
        "cancel_kpi_ingest",
        &json!({ "runId": run_id }),
    ));
    success(document_chunk(
        &state,
        json!({ "runId": run_id, "offset": 0, "length": 8 }),
    ));

    // A `discovered` run that somehow carries a hash is a broken invariant.
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET status = 'discovered' WHERE id = ?1",
                [&run_id],
            )
            .expect("force discovered");
    }
    assert_eq!(
        failure_code(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 0, "length": 1 })
        )),
        CommandErrorCode::Internal
    );
}

#[test]
fn corrupt_missing_or_malformed_blobs_are_internal() {
    let state = test_state();
    let run_id = started_run(&state);
    let hash = doc_hash();
    let blob_path = state.data_dir().join(SNAPSHOT_DIR).join(&hash);

    // Corrupt the blob (content no longer matches the frozen hash).
    std::fs::write(&blob_path, b"tampered bytes").expect("corrupt");
    assert_eq!(
        failure_code(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 0, "length": 1 })
        )),
        CommandErrorCode::Internal,
        "hash mismatch"
    );

    // Remove it entirely.
    std::fs::remove_file(&blob_path).expect("remove");
    assert_eq!(
        failure_code(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 0, "length": 1 })
        )),
        CommandErrorCode::Internal,
        "missing blob"
    );

    // Malformed stored hash: validated BEFORE any path is built.
    {
        let connection = state.checkout_for_tests().expect("raw");
        connection
            .execute(
                "UPDATE kpi_ingest_runs SET source_content_hash = 'XYZ' WHERE id = ?1",
                [&run_id],
            )
            .expect("malform hash");
    }
    assert_eq!(
        failure_code(document_chunk(
            &state,
            json!({ "runId": run_id, "offset": 0, "length": 1 })
        )),
        CommandErrorCode::Internal,
        "malformed stored hash"
    );
}

#[test]
fn a_second_data_dir_never_borrows_anothers_verification() {
    // Two states, same document bytes → same blob NAME in two data dirs.
    // Verify in the first; corrupt the second's blob to the SAME SIZE —
    // the canonical-path cache key forces a fresh verification → internal.
    let state_a = test_state();
    let run_a = started_run(&state_a);
    success(document_chunk(
        &state_a,
        json!({ "runId": run_a, "offset": 0, "length": 4 }),
    ));

    let state_b = test_state();
    let run_b = started_run(&state_b);
    let hash = doc_hash();
    let blob_b = state_b.data_dir().join(SNAPSHOT_DIR).join(&hash);
    let mut corrupt = DOC_BYTES.to_vec();
    corrupt[0] ^= 0xFF; // same size, different content
    std::fs::write(&blob_b, &corrupt).expect("corrupt same-size");
    assert_eq!(
        failure_code(document_chunk(
            &state_b,
            json!({ "runId": run_b, "offset": 0, "length": 4 })
        )),
        CommandErrorCode::Internal,
        "a different data dir's blob is verified on its own"
    );
}
