//! Dual-execution mock-fidelity contract — Rust side (ADR 0049, T6).
//!
//! Replays the SHARED journey corpus (`src/test/scenarios/fidelity-corpus.json`)
//! against the REAL `AppState`/storage layer on a fresh in-memory database. The TS
//! side (`src/test/scenarios/fidelity.test.ts`) replays the SAME corpus against
//! the hand-written mock runtime. Both must satisfy every assertion, so the mock
//! cannot silently drift from real backend behavior — `ts-rs` already guarantees
//! the DTO *shapes* match Rust; this guarantees the *behavior* does.
//!
//! The corpus targets the `AppState`/storage layer the thin `#[tauri::command]`
//! wrappers delegate to (the `tauri::State` wrapper itself is not unit-constructible).

use serde_json::{json, Map, Value};

use super::*;
use crate::storage::{
    NewCockpitLayout, NewCompany, NewFrameworkCriterion, NewQualityFramework, NewWatchlist,
    RenameCockpitLayoutInput, WatchlistUpdate,
};

// Path resolved by build.rs into BRAWLER_FIDELITY_CORPUS: the normal relative
// location by default, or an absolute override so `cargo-mutants`' scratch-tree
// copy (which excludes anything above the workspace root) still finds the file.
const CORPUS: &str = include_str!(env!("BRAWLER_FIDELITY_CORPUS"));

/// Replace any `"$name"` leaf with the captured value of `name`.
fn substitute(value: &Value, caps: &Map<String, Value>) -> Value {
    match value {
        Value::String(s) if s.starts_with('$') => caps.get(&s[1..]).cloned().unwrap_or(Value::Null),
        Value::Array(items) => Value::Array(items.iter().map(|v| substitute(v, caps)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, caps)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// True when `actual` is an object containing every key/value pair in `subset`.
fn is_superset(actual: &Value, subset: &Value) -> bool {
    match (actual, subset) {
        (Value::Object(a), Value::Object(s)) => s.iter().all(|(k, v)| a.get(k) == Some(v)),
        _ => false,
    }
}

/// Run one corpus command through the real storage layer; return its serialized
/// result (camelCase, matching the IPC contract the mock also returns).
fn dispatch(state: &AppState, command: &str, input: &Value) -> Value {
    let inner = input.get("input").cloned().unwrap_or(Value::Null);
    match command {
        "create_watchlist" => {
            let new: NewWatchlist = serde_json::from_value(inner).expect("NewWatchlist");
            serde_json::to_value(state.create_watchlist(new).expect("create_watchlist")).unwrap()
        }
        "rename_watchlist" => {
            let update: WatchlistUpdate = serde_json::from_value(inner).expect("WatchlistUpdate");
            serde_json::to_value(state.rename_watchlist(update).expect("rename_watchlist")).unwrap()
        }
        "delete_watchlist" => {
            let id = input["watchlistId"].as_str().expect("watchlistId");
            state.delete_watchlist(id).expect("delete_watchlist");
            Value::Null
        }
        "list_watchlists" => {
            serde_json::to_value(state.list_watchlists().expect("list_watchlists")).unwrap()
        }
        "create_company" => {
            let new: NewCompany = serde_json::from_value(inner).expect("NewCompany");
            serde_json::to_value(state.create_company(new).expect("create_company")).unwrap()
        }
        "delete_company" => {
            let id = input["companyId"].as_str().expect("companyId");
            state.delete_company(id).expect("delete_company");
            Value::Null
        }
        "list_companies" => {
            serde_json::to_value(state.list_companies().expect("list_companies")).unwrap()
        }
        "save_cockpit_layout" => {
            let new: NewCockpitLayout = serde_json::from_value(inner).expect("NewCockpitLayout");
            serde_json::to_value(
                state
                    .cockpit_layouts()
                    .save_cockpit_layout(new)
                    .expect("save_cockpit_layout"),
            )
            .unwrap()
        }
        "rename_cockpit_layout" => {
            let rename: RenameCockpitLayoutInput =
                serde_json::from_value(inner).expect("RenameCockpitLayoutInput");
            serde_json::to_value(
                state
                    .cockpit_layouts()
                    .rename_cockpit_layout(rename)
                    .expect("rename_cockpit_layout"),
            )
            .unwrap()
        }
        "delete_cockpit_layout" => {
            let id = input["layoutId"].as_str().expect("layoutId");
            state
                .cockpit_layouts()
                .delete_cockpit_layout(id)
                .expect("delete_cockpit_layout");
            Value::Null
        }
        "list_cockpit_layouts" => serde_json::to_value(
            state
                .cockpit_layouts()
                .list_cockpit_layouts()
                .expect("list_cockpit_layouts"),
        )
        .unwrap(),
        "set_company_autopilot" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let mode = inner["mode"].as_str().expect("mode");
            serde_json::to_value(
                state
                    .autopilot()
                    .set_mode(company_id, mode)
                    .expect("set_company_autopilot"),
            )
            .unwrap()
        }
        "get_company_autopilot" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let mode = state
                .autopilot()
                .get_mode(company_id)
                .expect("get_company_autopilot");
            json!({ "companyId": company_id, "mode": mode })
        }
        "list_company_autopilot_modes" => serde_json::to_value(
            state
                .autopilot()
                .list_modes()
                .expect("list_company_autopilot_modes"),
        )
        .unwrap(),
        "set_companies_autopilot" => {
            let company_ids: Vec<String> = inner["companyIds"]
                .as_array()
                .map(|values| {
                    values
                        .iter()
                        .filter_map(|value| value.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let mode = inner["mode"].as_str().expect("mode");
            let count = state
                .autopilot()
                .set_modes(&company_ids, mode)
                .expect("set_companies_autopilot");
            serde_json::to_value(count).unwrap()
        }
        "list_autopilot_runs" => {
            let list_input: crate::storage::ListAutopilotRunsInput =
                serde_json::from_value(inner).unwrap_or_default();
            serde_json::to_value(
                state
                    .autopilot()
                    .list_runs(&list_input)
                    .expect("list_autopilot_runs"),
            )
            .unwrap()
        }
        "create_quality_framework" => {
            let new: NewQualityFramework =
                serde_json::from_value(inner).expect("NewQualityFramework");
            serde_json::to_value(
                state
                    .create_quality_framework(new)
                    .expect("create_quality_framework"),
            )
            .unwrap()
        }
        "create_framework_criterion" => {
            let new: NewFrameworkCriterion =
                serde_json::from_value(inner).expect("NewFrameworkCriterion");
            serde_json::to_value(
                state
                    .create_framework_criterion(new)
                    .expect("create_framework_criterion"),
            )
            .unwrap()
        }
        "get_qualitative_assessment" => {
            let framework_id = inner["frameworkId"].as_str().expect("frameworkId");
            let company_id = inner["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                state
                    .get_qualitative_assessment(framework_id, company_id)
                    .expect("get_qualitative_assessment"),
            )
            .unwrap()
        }
        "get_qualitative_assessment_status" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let framework_id = inner["frameworkId"].as_str().expect("frameworkId");
            serde_json::to_value(
                crate::commands::quality_frameworks::qualitative_assessment_status(
                    state,
                    company_id,
                    framework_id,
                )
                .expect("get_qualitative_assessment_status"),
            )
            .unwrap()
        }
        // run/rerun enqueue the durable job (no per-job status table; the
        // job_queue row IS the status). Call the SAME command helper production
        // uses so the corpus can never diverge from the real enqueue id/payload.
        "run_qualitative_assessment" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let framework_id = inner["frameworkId"].as_str().expect("frameworkId");
            crate::commands::quality_frameworks::enqueue_assessment(
                state,
                company_id,
                framework_id,
                None,
            )
            .expect("run_qualitative_assessment");
            Value::Null
        }
        "rerun_qualitative_criterion" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let framework_id = inner["frameworkId"].as_str().expect("frameworkId");
            let criterion_id = inner["criterionId"].as_str().expect("criterionId");
            crate::commands::quality_frameworks::enqueue_assessment(
                state,
                company_id,
                framework_id,
                Some(vec![criterion_id.to_owned()]),
            )
            .expect("rerun_qualitative_criterion");
            Value::Null
        }
        "reclassify_report_documents" => serde_json::to_value(
            state
                .reclassify_report_documents()
                .expect("reclassify_report_documents"),
        )
        .unwrap(),
        "create_financial_period" => {
            let new: crate::storage::NewFinancialPeriod =
                serde_json::from_value(inner).expect("NewFinancialPeriod");
            serde_json::to_value(
                state
                    .financials()
                    .create_financial_period(new)
                    .expect("create_financial_period"),
            )
            .unwrap()
        }
        "create_financial_fact" => {
            let new: crate::storage::NewFinancialFact =
                serde_json::from_value(inner).expect("NewFinancialFact");
            serde_json::to_value(
                state
                    .financials()
                    .create_financial_fact(new)
                    .expect("create_financial_fact"),
            )
            .unwrap()
        }
        // Computed read model (ADR 0077 §2). Call the same helper the command
        // wrapper delegates to so the corpus can never diverge from real assembly.
        "get_fundamentals_coverage" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::fundamentals_coverage::compute_fundamentals_coverage(
                    state, company_id,
                )
                .expect("get_fundamentals_coverage"),
            )
            .unwrap()
        }
        // History sweep (ADR 0077 §3, T3.2). Same gated core the command wrapper
        // offloads, so the corpus can never diverge from real behavior.
        "run_history_sweep" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::history_sweep::start_history_sweep(state, company_id)
                    .expect("run_history_sweep"),
            )
            .unwrap()
        }
        "get_history_sweep_progress" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::history_sweep::compute_history_sweep_progress(state, company_id)
                    .expect("get_history_sweep_progress"),
            )
            .unwrap()
        }
        // Computed read model (ADR 0077 §1/§2, Panel B). Same helper the command
        // wrapper delegates to, so the corpus can never diverge from real assembly.
        "get_report_documents_view" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::report_documents_view::compute_report_documents_view(
                    state, company_id,
                )
                .expect("get_report_documents_view"),
            )
            .unwrap()
        }
        // F5 review-queue read model (ADR 0077 §4/§5, T5.3b). Same store method
        // the command wrapper offloads, so the corpus can never diverge.
        "list_pending_kpi_proposals" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                state
                    .kpi_extraction()
                    .list_pending_kpi_proposals(company_id)
                    .expect("list_pending_kpi_proposals"),
            )
            .unwrap()
        }
        other => {
            panic!("fidelity corpus uses '{other}', which the Rust replayer does not dispatch")
        }
    }
}

#[test]
fn rust_backend_satisfies_the_fidelity_corpus() {
    let corpus: Value = serde_json::from_str(CORPUS).expect("corpus json parses");
    let journeys = corpus["journeys"].as_array().expect("journeys array");
    assert!(!journeys.is_empty(), "corpus has no journeys");

    for journey in journeys {
        let name = journey["name"].as_str().unwrap_or("<unnamed>");
        // Fresh, isolated backend per journey.
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        let mut caps: Map<String, Value> = Map::new();

        for step in journey["steps"].as_array().expect("steps array") {
            let command = step["command"].as_str().expect("command");
            let input = substitute(step.get("input").unwrap_or(&json!({})), &caps);
            let result = dispatch(&state, command, &input);

            if let Some(cap) = step.get("capture").and_then(Value::as_str) {
                caps.insert(
                    cap.to_string(),
                    result.get("id").cloned().unwrap_or(Value::Null),
                );
            }
            if let Some(field) = step.get("expectField") {
                assert!(
                    is_superset(&result, field),
                    "[{name}] {command}: result {result} is missing {field}"
                );
            }
            if let Some(subset) = step.get("expectContains") {
                let array = result.as_array().unwrap_or_else(|| {
                    panic!("[{name}] {command}: expected an array, got {result}")
                });
                assert!(
                    array.iter().any(|item| is_superset(item, subset)),
                    "[{name}] {command}: none of {result} contains {subset}"
                );
            }
            if let Some(subset) = step.get("expectAbsent") {
                let array = result.as_array().unwrap_or_else(|| {
                    panic!("[{name}] {command}: expected an array, got {result}")
                });
                assert!(
                    array.iter().all(|item| !is_superset(item, subset)),
                    "[{name}] {command}: {result} still contains {subset}"
                );
            }
        }
    }
}
