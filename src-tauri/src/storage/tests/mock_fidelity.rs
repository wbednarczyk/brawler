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
use crate::mcp::lifecycle::McpLifecycle;
use crate::storage::{
    NewCockpitLayout, NewCompany, NewFrameworkCriterion, NewQualityFramework, NewWatchlist,
    WatchlistUpdate,
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
fn dispatch(state: &AppState, lifecycle: &McpLifecycle, command: &str, input: &Value) -> Value {
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
        // Price context read model (v0.53 T5, ADR 0067/0082). Same computed-model
        // helper the command wrapper offloads, so the corpus can never diverge
        // from real assembly.
        "get_price_context" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::market_data::compute_price_context(state, company_id)
                    .expect("get_price_context"),
            )
            .unwrap()
        }
        // Basic info read model (v0.53 follow-up): same computed-model helper
        // the command wrapper uses.
        "get_company_basic_info" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::companies::compute_company_basic_info(state, company_id)
                    .expect("get_company_basic_info"),
            )
            .unwrap()
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
        // Decision journal (ADR 0071, J2). Immutable per-company judgments —
        // the store methods the thin command wrappers delegate to.
        "create_decision_entry" => {
            let new: crate::storage::NewDecisionEntry =
                serde_json::from_value(inner).expect("NewDecisionEntry");
            serde_json::to_value(
                state
                    .decision_journal()
                    .create_decision_entry(new)
                    .expect("create_decision_entry"),
            )
            .unwrap()
        }
        "list_decision_entries" => {
            let list_input: crate::storage::DecisionEntryListInput =
                serde_json::from_value(inner).unwrap_or_default();
            serde_json::to_value(
                state
                    .decision_journal()
                    .list_decision_entries(list_input)
                    .expect("list_decision_entries"),
            )
            .unwrap()
        }
        // Pre-report expectations (ADR 0071, J2), keyed by (company, event_key).
        "create_report_expectation" => {
            let new: crate::storage::NewReportExpectation =
                serde_json::from_value(inner).expect("NewReportExpectation");
            serde_json::to_value(
                state
                    .report_expectations()
                    .create_report_expectation(new)
                    .expect("create_report_expectation"),
            )
            .unwrap()
        }
        "update_report_expectation" => {
            let upd: crate::storage::UpdateReportExpectation =
                serde_json::from_value(inner).expect("UpdateReportExpectation");
            serde_json::to_value(
                state
                    .report_expectations()
                    .update_report_expectation(upd)
                    .expect("update_report_expectation"),
            )
            .unwrap()
        }
        "list_report_expectations" => {
            let list_input: crate::storage::ListReportExpectationsInput =
                serde_json::from_value(inner).unwrap_or_default();
            serde_json::to_value(
                state
                    .report_expectations()
                    .list_report_expectations(list_input)
                    .expect("list_report_expectations"),
            )
            .unwrap()
        }
        // Composed read model (ADR 0071): expectation vs confirmed facts. Same
        // store method the command wrapper offloads, so the corpus can never
        // diverge from real composition.
        "expectation_review" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let event_key = inner["eventKey"].as_str().expect("eventKey");
            serde_json::to_value(
                state
                    .report_expectations()
                    .expectation_review(company_id, event_key)
                    .expect("expectation_review"),
            )
            .unwrap()
        }
        "record_expectation_resolution" => {
            let res: crate::storage::RecordExpectationResolutionInput =
                serde_json::from_value(inner).expect("RecordExpectationResolutionInput");
            serde_json::to_value(
                state
                    .report_expectations()
                    .record_expectation_resolution(res)
                    .expect("record_expectation_resolution"),
            )
            .unwrap()
        }
        // MCP token lifecycle (ADR 0078 M1). Same command helpers production
        // uses, pointed at the TEST keychain slot so a run on a machine with a
        // persistent keychain backend never touches the real
        // `brawler/mcp/auth_token` entry.
        "regenerate_mcp_token" => serde_json::to_value(
            crate::commands::mcp::regenerate_mcp_token_impl(
                &crate::providers::credentials::test_mcp_auth_token_descriptor(),
            )
            .expect("regenerate_mcp_token"),
        )
        .unwrap(),
        "revoke_mcp_token" => serde_json::to_value(
            crate::commands::mcp::revoke_mcp_token_impl(
                &crate::providers::credentials::test_mcp_auth_token_descriptor(),
            )
            .expect("revoke_mcp_token"),
        )
        .unwrap(),
        "mcp_token_status" => serde_json::to_value(
            crate::commands::mcp::mcp_token_status_impl(
                &crate::providers::credentials::test_mcp_auth_token_descriptor(),
            )
            .expect("mcp_token_status"),
        )
        .unwrap(),
        // MCP server lifecycle (ADR 0078 M3). Drives the real `McpLifecycle`
        // (persisting `mcp.enabled` + flipping the live server) against the same
        // command impl production uses, pointed at the TEST keychain descriptor.
        // With BRAWLER_MCP_TOKEN scrubbed (see the journey `_comment`) there is
        // no token, so the enabled path refuses cleanly (`running:false`) and no
        // socket is bound — deterministic and portable across keychain backends.
        "set_mcp_enabled" => serde_json::to_value(
            crate::commands::mcp::set_mcp_enabled_impl(
                input["enabled"].as_bool().expect("enabled"),
                state,
                lifecycle,
                &crate::providers::credentials::test_mcp_auth_token_descriptor(),
            )
            .expect("set_mcp_enabled"),
        )
        .unwrap(),
        // Attention routing: alert rules + fired events (ADR 0068 T3). Thin
        // command wrappers delegate to these `AttentionStore` methods, so the
        // corpus exercises the same CRUD/list/mutate behavior the mock replays.
        "create_alert_rule" => {
            let new: crate::storage::NewAlertRule =
                serde_json::from_value(inner).expect("NewAlertRule");
            serde_json::to_value(
                state
                    .attention()
                    .create_alert_rule(new)
                    .expect("create_alert_rule"),
            )
            .unwrap()
        }
        "list_alert_rules" => serde_json::to_value(
            state
                .attention()
                .list_alert_rules()
                .expect("list_alert_rules"),
        )
        .unwrap(),
        "update_alert_rule" => {
            let update: crate::storage::AlertRuleUpdate =
                serde_json::from_value(inner).expect("AlertRuleUpdate");
            serde_json::to_value(
                state
                    .attention()
                    .update_alert_rule(update)
                    .expect("update_alert_rule"),
            )
            .unwrap()
        }
        "set_alert_rule_enabled" => {
            let id = inner["id"].as_str().expect("id");
            let enabled = inner["enabled"].as_bool().expect("enabled");
            serde_json::to_value(
                state
                    .attention()
                    .set_alert_rule_enabled(id, enabled)
                    .expect("set_alert_rule_enabled"),
            )
            .unwrap()
        }
        "delete_alert_rule" => {
            let id = inner["id"].as_str().expect("id");
            state
                .attention()
                .delete_alert_rule(id)
                .expect("delete_alert_rule");
            Value::Null
        }
        "list_attention_events" => {
            let list_input: crate::storage::AttentionEventListInput =
                serde_json::from_value(inner).unwrap_or_default();
            serde_json::to_value(
                state
                    .attention()
                    .list_attention_events(list_input)
                    .expect("list_attention_events"),
            )
            .unwrap()
        }
        "mark_attention_event_seen" => {
            let id = inner["id"].as_str().expect("id");
            state
                .attention()
                .mark_attention_event_seen(id)
                .expect("mark_attention_event_seen");
            Value::Null
        }
        "dismiss_attention_event" => {
            let id = inner["id"].as_str().expect("id");
            state
                .attention()
                .dismiss_attention_event(id)
                .expect("dismiss_attention_event");
            Value::Null
        }
        // Corpus-only setup bridge: the real inline autopilot-completion hook
        // (ADR 0068 §T2) fires events; there is no user command to mint one. Fire
        // it, then return the resulting event so the seen/dismiss steps have a
        // real id (the mock runtime seeds an equivalent event in its handler).
        "evaluate_autopilot_completion" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let run_id = inner["runId"].as_str().expect("runId");
            state
                .attention()
                .evaluate_autopilot_completion(company_id, run_id)
                .expect("evaluate_autopilot_completion");
            let events = state
                .attention()
                .list_attention_events(crate::storage::AttentionEventListInput {
                    company_id: Some(company_id.to_owned()),
                    include_dismissed: true,
                })
                .expect("list_attention_events");
            serde_json::to_value(events.into_iter().next()).unwrap()
        }
        // Morning briefing (ADR 0068 decision 4, §T5). `generate` enqueues an
        // async compose job (no synchronous result); `get_latest` returns the most
        // recent briefing or null. The replayer does not drain the worker, so no
        // briefing exists mid-corpus — both sides agree on `null`.
        "generate_morning_briefing" => {
            crate::jobs::morning_briefing::enqueue_on_demand_briefing(state);
            Value::Null
        }
        "get_latest_morning_briefing" => serde_json::to_value(
            state
                .morning_briefings()
                .latest_morning_briefing()
                .expect("get_latest_morning_briefing"),
        )
        .unwrap(),
        "mcp_status" => serde_json::to_value(lifecycle.status()).unwrap(),
        other => {
            panic!("fidelity corpus uses '{other}', which the Rust replayer does not dispatch")
        }
    }
}

#[test]
fn rust_backend_satisfies_the_fidelity_corpus() {
    // Scrub-hermeticity (docs/testing.md): the MCP journey asserts
    // missing-token behavior, which a BRAWLER_MCP_TOKEN exported in the
    // developer's shell would silently flip to configured.
    crate::providers::credentials::scrub_provider_env_fallbacks();

    let corpus: Value = serde_json::from_str(CORPUS).expect("corpus json parses");
    let journeys = corpus["journeys"].as_array().expect("journeys array");
    assert!(!journeys.is_empty(), "corpus has no journeys");

    for journey in journeys {
        let name = journey["name"].as_str().unwrap_or("<unnamed>");
        // Fresh, isolated backend per journey.
        let state = AppState::new(open_in_memory_database().expect("in-memory db"));
        // Per-journey MCP lifecycle so `set_mcp_enabled`/`mcp_status` share one
        // controller across the journey's steps (ADR 0078 M3).
        let lifecycle = McpLifecycle::new();
        let mut caps: Map<String, Value> = Map::new();

        for step in journey["steps"].as_array().expect("steps array") {
            let command = step["command"].as_str().expect("command");
            let input = substitute(step.get("input").unwrap_or(&json!({})), &caps);
            let result = dispatch(&state, &lifecycle, command, &input);

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
