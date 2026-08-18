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

// Path resolved by build.rs into BRAWLER_FIDELITY_CORPUS (derived from
// BRAWLER_SCENARIOS_DIR): the normal relative location by default, or an
// absolute override so `cargo-mutants`' scratch-tree copy (which excludes
// anything above the workspace root) still finds the file.
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
        // Cross-company comparison read model (v0.61 §A2, ADR 0089 dec. 1). Same
        // computed helper the command wrapper offloads, so the corpus can never
        // diverge from real assembly. A fresh company has no confirmed facts, so
        // the axis is empty and each requested (company, metric) is an empty
        // series — the deterministic empty comparison both sides must agree on.
        "get_kpi_comparison" => {
            let parsed: crate::commands::comparison::KpiComparisonInput =
                serde_json::from_value(input.clone()).expect("KpiComparisonInput");
            serde_json::to_value(
                crate::commands::comparison::compute_kpi_comparison(state, &parsed)
                    .expect("get_kpi_comparison"),
            )
            .unwrap()
        }
        // Sector percentiles read model (v0.61 §B1, ADR 0089 dec. 3). Same
        // computed helper the command wrapper offloads, so the corpus can never
        // diverge from real assembly. A freshly created company has no classified
        // sector, so the deterministic result both sides must agree on is the
        // typed `no_sector` empty state (peerCount 0, thin true).
        "get_sector_percentiles" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::sector_percentiles::compute_company_sector_percentiles(
                    state, company_id,
                )
                .expect("get_sector_percentiles"),
            )
            .unwrap()
        }
        // Comparative valuation L1 (v0.61 §B2, ADR 0089 dec. 4-5). Same
        // compute-and-persist helper the command wrapper offloads. A freshly
        // created company has no sector, so both sides agree on the typed
        // `no_sector` empty state and persist no run.
        "compute_comparative_valuation" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::valuation::compute_and_persist_comparative_valuation(
                    state, company_id,
                )
                .expect("compute_comparative_valuation"),
            )
            .unwrap()
        }
        "list_valuation_runs" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                state
                    .valuation_runs()
                    .list_runs(company_id)
                    .expect("list_valuation_runs"),
            )
            .unwrap()
        }
        // Company health scores (v0.57 T2, ADR 0083). Same computed helper the
        // command wrapper offloads, so the corpus can never diverge from real
        // score assembly.
        "get_company_health" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::company_health::compute_company_health(state, company_id)
                    .expect("get_company_health"),
            )
            .unwrap()
        }
        // Flagged/failed extraction outcomes (v0.59 A2, ADR 0061 dec. 2). Same
        // store method the command wrapper delegates to, so the mock can never
        // claim a review surface the real pipeline would not produce. A company
        // no extraction ever ran over reads back empty — absence means "never
        // attempted", which is exactly the distinction the table exists for.
        // Flagged FACTS (epic #229 T5): same store method the command wrapper
        // delegates to, with the same optional company scope, so the mock can
        // never claim a review surface the real join would not produce.
        "list_flagged_fact_provenance" => {
            let company_id = inner["companyId"].as_str();
            serde_json::to_value(
                state
                    .fundamentals_provenance()
                    .list_flagged_facts(company_id)
                    .expect("list_flagged_fact_provenance"),
            )
            .unwrap()
        }
        "list_flagged_extraction_outcomes" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                state
                    .fundamentals_provenance()
                    .list_flagged_extraction_outcomes(company_id)
                    .expect("list_flagged_extraction_outcomes"),
            )
            .unwrap()
        }
        // Red-flags panel state (v0.57 T7, ADR 0083 D8). Same store method the
        // command wrapper offloads, so the corpus can never diverge from real
        // assembly. Detections are adapter/extraction-triggered, so an untouched
        // company reads back the empty view (no active flags, empty history).
        "get_red_flags" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                state
                    .red_flags()
                    .red_flags_view(company_id)
                    .expect("get_red_flags"),
            )
            .unwrap()
        }
        "acknowledge_red_flag" => {
            let flag_id = inner["flagId"].as_str().expect("flagId");
            serde_json::to_value(
                state
                    .red_flags()
                    .acknowledge(flag_id)
                    .expect("acknowledge_red_flag"),
            )
            .unwrap()
        }
        // Analyst recommendations (v0.58 A3, ADR 0073). Same computed helper the
        // command wrapper offloads, so the corpus can never diverge from real
        // assembly. The register is adapter-populated, so an untouched company
        // reads back the empty view (no entries, no latest target, no refresh).
        "get_analyst_recommendations" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::analyst_recommendations::compute_analyst_recommendations(
                    state, company_id,
                )
                .expect("get_analyst_recommendations"),
            )
            .unwrap()
        }
        // Insider overview (v0.57 T6, ADR 0083 D7). Same computed helper the command
        // wrapper offloads, so the corpus can never diverge from real assembly. An
        // untouched company has no parsed substrate → the empty overview (no
        // transactions/holdings, both windows below the 2-transaction minimum). No
        // wall-clock field is serialized, so the dual execution stays deterministic.
        "get_insider_overview" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::insider::compute_insider_overview(state, company_id)
                    .expect("get_insider_overview"),
            )
            .unwrap()
        }
        // Ownership overview + review (v0.56 T6, ADR 0072). Same computed helper /
        // store method the command wrappers delegate to, so the corpus can never
        // diverge from real assembly.
        "get_ownership_overview" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::ownership::compute_ownership_overview(state, company_id)
                    .expect("get_ownership_overview"),
            )
            .unwrap()
        }
        "set_ownership_holder_type" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            let holder_key = input["holderKey"].as_str().expect("holderKey");
            let holder_type = input["holderType"].as_str();
            state
                .ownership()
                .set_holder_type(company_id, holder_key, holder_type)
                .expect("set_ownership_holder_type");
            serde_json::to_value(
                crate::commands::ownership::compute_ownership_overview(state, company_id)
                    .expect("get_ownership_overview"),
            )
            .unwrap()
        }
        // Tier-4 OCR proposal confirm/reject (v0.57 T8, ADR 0077): the same store
        // methods + recompute the command wrappers use.
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
        "write_export_file" => {
            // The command body minus the tauri async runtime: same path policy,
            // same write, same scalar return (commands/import_export.rs).
            let request: crate::commands::import_export::WriteExportFileInput =
                serde_json::from_value(inner).expect("WriteExportFileInput");
            let target = crate::commands::import_export::resolve_export_path(&request)
                .expect("resolve_export_path");
            std::fs::write(&target, request.contents.as_bytes()).expect("write_export_file");
            Value::String(target.to_string_lossy().into_owned())
        }
        "rename_cockpit_layout" => {
            let layout_id = inner["layoutId"].as_str().expect("layoutId");
            let name = inner["name"].as_str().expect("name");
            serde_json::to_value(
                state
                    .cockpit_layouts()
                    .rename_cockpit_layout(layout_id, name)
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
        // ADR 0084: the `get_qualitative_assessment_status` command is retired
        // with the assessment-generation layer, but the shared fidelity corpus
        // still exercises the read until the orchestrator reconciles it. Compute
        // the same status the removed command did (no queue row + no stored
        // assessment ⇒ idle) inline so the corpus parity holds mid-flight.
        "get_qualitative_assessment_status" => {
            let company_id = inner["companyId"].as_str().expect("companyId");
            let framework_id = inner["frameworkId"].as_str().expect("frameworkId");
            let stored = !state
                .get_qualitative_assessment(framework_id, company_id)
                .expect("get_qualitative_assessment")
                .is_empty();
            serde_json::json!({
                "status": if stored { "succeeded" } else { "idle" },
                "attempts": 0,
                "lastError": Value::Null,
            })
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
        "update_financial_fact" => {
            let update: crate::storage::UpdateFinancialFact =
                serde_json::from_value(inner).expect("UpdateFinancialFact");
            serde_json::to_value(
                state
                    .financials()
                    .update_financial_fact(update)
                    .expect("update_financial_fact"),
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
        // Version-aware re-extraction (epic #398 Item B). Same gated core the
        // command wrapper offloads, so the corpus can never diverge from real
        // behavior.
        "run_pipeline_reextraction" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::pipeline_reextraction::start_pipeline_reextraction(
                    state, company_id,
                )
                .expect("run_pipeline_reextraction"),
            )
            .unwrap()
        }
        "get_pipeline_reextraction_progress" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::pipeline_reextraction::compute_pipeline_reextraction_progress(
                    state, company_id,
                )
                .expect("get_pipeline_reextraction_progress"),
            )
            .unwrap()
        }
        // Layer 1 raw-tagged-fact read model (ADR 0100, epic #398 final
        // slice). Same core the command wrapper offloads, so the corpus can
        // never diverge from real assembly.
        "get_report_tagged_fact_coverage" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::tagged_fact_promotion::compute_tagged_fact_coverage(
                    state, company_id,
                )
                .expect("get_report_tagged_fact_coverage"),
            )
            .unwrap()
        }
        "list_uncrosswalked_concepts" => {
            let company_id = input["companyId"].as_str().expect("companyId");
            serde_json::to_value(
                crate::commands::tagged_fact_promotion::compute_uncrosswalked_concepts(
                    state, company_id,
                )
                .expect("list_uncrosswalked_concepts"),
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
        // KNF short-selling read model (v0.55 T4b). A normal company-scoped user
        // read — joins the corpus; the register is adapter-populated, so an
        // untouched company reads back the empty view (no active positions).
        "list_short_positions" => {
            let list_input: crate::storage::ShortPositionsInput =
                serde_json::from_value(inner).expect("ShortPositionsInput");
            serde_json::to_value(
                state
                    .short_positions()
                    .short_positions_view(&list_input.company_id)
                    .expect("list_short_positions"),
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
        // MCP token lifecycle (ADR 0078 M1 + ADR 0099 restart-on-rotation).
        // The SAME composed wrappers production uses (rotate/revoke restart
        // the listener from stored-credential truth), pointed at the
        // rotation-test descriptors: their in-memory backend gives the
        // persistent read-back the production read-back rule requires, which
        // the EntryOnly Linux keyring cannot (EntryOnly truthfulness itself
        // is pinned by the credentials.rs unit tests on the real-keyring
        // scratch slot).
        "regenerate_mcp_token" => serde_json::to_value(
            crate::commands::mcp::regenerate_token_with_restart_impl(
                state,
                lifecycle,
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
            )
            .expect("regenerate_mcp_token"),
        )
        .unwrap(),
        "revoke_mcp_token" => serde_json::to_value(
            crate::commands::mcp::revoke_token_with_restart_impl(
                state,
                lifecycle,
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
            )
            .expect("revoke_mcp_token"),
        )
        .unwrap(),
        "mcp_token_status" => serde_json::to_value(
            crate::commands::mcp::mcp_token_status_impl(
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
            )
            .expect("mcp_token_status"),
        )
        .unwrap(),
        "regenerate_kpi_acquisition_token" => serde_json::to_value(
            crate::commands::mcp::regenerate_token_with_restart_impl(
                state,
                lifecycle,
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
            )
            .expect("regenerate_kpi_acquisition_token"),
        )
        .unwrap(),
        "revoke_kpi_acquisition_token" => serde_json::to_value(
            crate::commands::mcp::revoke_token_with_restart_impl(
                state,
                lifecycle,
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_rotation_descriptor(),
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
            )
            .expect("revoke_kpi_acquisition_token"),
        )
        .unwrap(),
        "kpi_acquisition_token_status" => serde_json::to_value(
            crate::commands::mcp::mcp_token_status_impl(
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
            )
            .expect("kpi_acquisition_token_status"),
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
                &crate::providers::credentials::test_mcp_kpi_rotation_descriptor(),
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
        "mark_attention_events_seen" => {
            let ids: Vec<String> = inner["ids"]
                .as_array()
                .expect("ids")
                .iter()
                .map(|value| value.as_str().expect("id").to_owned())
                .collect();
            state
                .attention()
                .mark_attention_events_seen(&ids)
                .expect("mark_attention_events_seen");
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
        // Corpus-only setup bridge (epic #40 S3, ADR 0091 dec. 1): a background job
        // that exhausted its retries raises a SYSTEM `job_failed` event. Drives the
        // REAL queue store + the real `record_job_failure` write, so the read model
        // resolves the job's kind + last_error exactly as it does in production
        // (the mock runtime seeds an equivalent event in its handler).
        "record_job_failure" => {
            let job_id = inner["jobId"].as_str().expect("jobId");
            let kind = inner["kind"].as_str().expect("kind");
            let error = inner["error"].as_str().unwrap_or("corpus failure");
            let jobs = state.jobs();
            jobs.enqueue(job_id, kind, "{}", 1).expect("enqueue");
            jobs.claim_next_for_kinds(&[kind]).expect("claim");
            jobs.mark_failed(job_id, error, 0).expect("mark_failed");
            state
                .attention()
                .record_job_failure(
                    job_id,
                    inner["companyId"].as_str(),
                    inner["subject"].as_str(),
                )
                .expect("record_job_failure");
            let events = state
                .attention()
                .list_attention_events(crate::storage::AttentionEventListInput {
                    company_id: None,
                    include_dismissed: true,
                })
                .expect("list_attention_events");
            serde_json::to_value(
                events
                    .into_iter()
                    .find(|event| event.evidence_ref == job_id),
            )
            .unwrap()
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
        // Retired AI-generation commands (ADR 0084): the corpus and both replayers
        // are reconciled together by the orchestrator after the Rust + frontend
        // retirement slices both land. Until then the shared corpus may still list
        // these steps; the command no longer exists in the Rust registry, so the
        // replayer skips them rather than panicking mid-flight. Remove this arm and
        // the corresponding corpus steps in the reconciliation pass.
        "confirm_ownership_ocr_proposal"
        | "reject_ownership_ocr_proposal"
        | "list_pending_kpi_proposals"
        | "confirm_kpi_proposal"
        | "reject_kpi_proposal"
        | "list_ai_analysis"
        | "list_research_briefs"
        | "list_research_digests"
        | "confirm_ownership_holder_type_proposal"
        | "reject_ownership_holder_type_proposal"
        | "run_qualitative_assessment"
        | "rerun_qualitative_criterion"
        | "start_ai_analysis"
        | "retry_ai_analysis"
        | "start_kpi_extraction"
        | "retry_kpi_extraction"
        | "list_kpi_extraction"
        | "start_claim_extraction"
        | "retry_claim_extraction"
        | "list_claim_extraction"
        | "confirm_claim_proposal"
        | "reject_claim_proposal"
        | "start_research_brief"
        | "start_research_digest"
        | "run_ai_signal_classification"
        | "run_ai_event_derivation"
        | "run_ownership_classification"
        | "run_ownership_ocr_extraction"
        | "run_company_ownership_ocr"
        | "list_ai_provider_catalog" => Value::Null,
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
