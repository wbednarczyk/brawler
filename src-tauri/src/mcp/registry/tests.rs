use super::*;
use crate::storage;
use serde_json::json;
use std::collections::BTreeSet;

/// ADR 0088 dec. 2 classification gate. The command inventory is derived
/// from the REAL registration point — the `invoke_handler` block in
/// `lib.rs` — never a hand-copied list, so a new command that is neither
/// exposed nor denylisted fails this test until it is classified.
#[test]
fn every_registered_command_is_classified() {
    let source = include_str!("../../lib.rs");
    let start = source
        .find("generate_handler![")
        .expect("invoke_handler block present in lib.rs");
    let block = &source[start..];
    let end = block
        .find("])")
        .expect("closing of the generate_handler macro");
    let block = &block[..end];

    let pattern = regex::Regex::new(r"commands::\w+::(\w+)").expect("valid regex");
    let registered: BTreeSet<&str> = pattern
        .captures_iter(block)
        .map(|capture| capture.get(1).expect("group 1").as_str())
        .collect();
    assert!(
        registered.len() > 100,
        "sanity: expected the full command inventory, got {}",
        registered.len()
    );

    let entries = entries();
    let classified: BTreeSet<&str> = entries.iter().map(|entry| entry.command_name).collect();

    let unclassified: Vec<&str> = registered
        .iter()
        .filter(|command| !classified.contains(*command))
        .copied()
        .collect();
    assert!(
        unclassified.is_empty(),
        "{} command(s) have no MCP registry classification (ADR 0088 dec. 2): {:?}",
        unclassified.len(),
        unclassified
    );

    // Each command is classified exactly once (no ambiguous double rows).
    let mut seen = BTreeSet::new();
    for entry in &entries {
        if registered.contains(entry.command_name) {
            assert!(
                seen.insert(entry.command_name),
                "command {} is classified more than once",
                entry.command_name
            );
        }
    }
}

/// The MVP tools' wire contract survives the switch to schemars-generated
/// schemas AND the read wave preserves it for representative new tools:
/// names, required input fields, `additionalProperties: false` (deny
/// unknown), and camelCase property names all as expected. Covers the four
/// MVP tools plus three new read-wave shapes — one no-arg (`list_companies`),
/// one filtered (`list_attention_events`), and the facts+provenance tool
/// (`list_financial_facts`).
#[test]
fn mvp_tools_wire_contract_is_preserved() {
    // (tool name, required fields, all property names)
    let expected: &[(&str, &[&str], &[&str])] = &[
        ("get_company_dossier", &["company"], &["company"]),
        (
            "search_research",
            &["query"],
            &["query", "company", "limit"],
        ),
        ("list_claims_due", &[], &["company"]),
        ("get_quality_assessment", &["company"], &["company"]),
        // Read wave: no-arg, filtered, and facts+provenance.
        ("list_companies", &[], &[]),
        (
            "list_attention_events",
            &[],
            &["company", "includeDismissed"],
        ),
        ("list_financial_facts", &["company"], &["company"]),
    ];

    let tools = descriptors(McpScope::Full);
    let tools = tools.as_array().expect("tools array");
    let exposed = entries().iter().filter(|entry| entry.exposed).count();
    assert_eq!(
        tools.len(),
        exposed,
        "tools/list advertises exactly the exposed registry entries"
    );
    assert_eq!(
        tools.len(),
        FROZEN_EXPOSED_TOOL_COUNT,
        "the frozen exposed-tool count (itemized at FROZEN_EXPOSED_TOOL_COUNT)"
    );

    for (name, required, properties) in expected {
        let tool = tools
            .iter()
            .find(|tool| tool["name"] == json!(name))
            .unwrap_or_else(|| panic!("tool {name} present in tools/list"));
        let schema = &tool["inputSchema"];

        assert_eq!(
            schema["additionalProperties"],
            json!(false),
            "{name}: unknown fields must be denied"
        );
        assert_eq!(schema["type"], json!("object"), "{name}: object schema");

        let mut actual_required: Vec<&str> = schema["required"]
            .as_array()
            .map(|array| array.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        actual_required.sort_unstable();
        let mut want_required = required.to_vec();
        want_required.sort_unstable();
        assert_eq!(
            actual_required, want_required,
            "{name}: required input fields unchanged"
        );

        // A no-arg tool (empty input struct) has no `properties` key at all.
        let empty = serde_json::Map::new();
        let object = schema["properties"].as_object().unwrap_or(&empty);
        let mut actual_props: Vec<&str> = object.keys().map(String::as_str).collect();
        actual_props.sort_unstable();
        let mut want_props = properties.to_vec();
        want_props.sort_unstable();
        assert_eq!(actual_props, want_props, "{name}: property names unchanged");

        for key in object.keys() {
            assert!(
                !key.contains('_') && !key.chars().next().unwrap().is_uppercase(),
                "{name}.{key} must be camelCase on the wire"
            );
        }
    }
}

/// The registry-truth capability manifest the docs-drift gate consumes
/// (#387, ADR 0065): the ordered exposed surface with each tool's
/// authoritative `tier` and acquisition-scope membership. The wire
/// `tools/list` shape deliberately omits `tier` ([`descriptors`]), so this
/// committed snapshot is the ONLY registry-truth artifact the JS catalog
/// gate can read — it splits read/act from `tier` here instead of
/// re-deriving it from a tool-name regex.
fn capability_manifest() -> Value {
    let tools: Vec<Value> = entries()
        .iter()
        .filter(|entry| entry.exposed)
        .map(|entry| {
            let tier = match entry.tier {
                CapabilityTier::Read => "read",
                CapabilityTier::Act => "act",
                CapabilityTier::Excluded => {
                    unreachable!("excluded commands are never exposed as tools")
                }
            };
            json!({
                "name": entry.tool_name,
                "tier": tier,
                "acquisition": KPI_ACQUISITION_TOOLS.contains(&entry.tool_name),
            })
        })
        .collect();
    json!({ "schemaVersion": 1, "tools": tools })
}

/// The capability manifest is a frozen, committed snapshot (`cargo insta
/// accept` to regenerate — CI runs without `INSTA_UPDATE`, so a tier change
/// or a newly-exposed tool reddens here). The docs-drift gate reads this
/// file to classify read/act by registry truth, not a name regex (#387).
#[test]
fn capability_manifest_is_the_frozen_contract() {
    let manifest = capability_manifest();
    // Coherence the JS gate relies on: the manifest's acquisition tools are
    // exactly `KPI_ACQUISITION_TOOLS`, in the frozen contract order.
    let acquisition: Vec<&str> = manifest["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter(|tool| tool["acquisition"] == json!(true))
        .map(|tool| tool["name"].as_str().expect("tool name"))
        .collect();
    assert_eq!(
        acquisition, KPI_ACQUISITION_TOOLS,
        "manifest acquisition set must equal KPI_ACQUISITION_TOOLS in contract order"
    );
    insta::assert_snapshot!(
        "mcp_registry_manifest",
        serde_json::to_string_pretty(&manifest).expect("serializable")
    );
}

/// The read wave's umbrella proof (ADR 0088 dec. 2): every exposed `read`
/// tool is listed in `tools/list` AND callable against a seeded DB with a
/// minimal valid input — none may `500`/panic or reject its own minimal
/// arguments. The per-tool minimal-input map must cover the exposed set
/// exactly, so a newly-exposed tool with no test input reddens here.
#[test]
fn every_exposed_read_tool_is_listed_and_callable() {
    use crate::storage::{open_in_memory_database, AppState, NewCompany};

    let state = AppState::new(open_in_memory_database().expect("db"));
    state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "TST".to_owned(),
            display_name: "Test S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");

    let company = json!({ "company": "GPW:TST" });
    // Minimal valid input per exposed tool. `may_fail` tools take an id we do
    // not seed, so a `not_found` Failure is the acceptable answer (still a
    // domain outcome, not a panic / protocol error).
    let inputs: &[(&str, Value, bool)] = &[
        // MVP.
        ("get_company_dossier", company.clone(), false),
        ("search_research", json!({ "query": "profit" }), false),
        ("list_claims_due", json!({}), false),
        ("get_quality_assessment", company.clone(), false),
        // Read wave.
        ("list_companies", json!({}), false),
        ("get_company_basic_info", company.clone(), false),
        ("list_watchlists", json!({}), false),
        ("list_watchlist_memberships", json!({}), false),
        ("list_feed_items", json!({}), false),
        ("list_company_signals", company.clone(), false),
        ("list_company_events", company.clone(), false),
        ("list_financial_facts", company.clone(), false),
        ("list_financial_periods", company.clone(), false),
        ("list_kpi_definitions", json!({}), false),
        ("list_flagged_fact_provenance", json!({}), false),
        ("get_price_context", company.clone(), false),
        (
            "get_kpi_comparison",
            json!({ "companies": ["GPW:TST"], "metricKeys": ["revenue"], "granularity": "annual" }),
            false,
        ),
        ("get_sector_percentiles", company.clone(), false),
        ("list_valuation_runs", company.clone(), false),
        ("get_ownership_overview", company.clone(), false),
        ("get_insider_overview", company.clone(), false),
        ("list_short_positions", company.clone(), false),
        ("get_analyst_recommendations", company.clone(), false),
        ("get_company_health", company.clone(), false),
        ("get_red_flags", company.clone(), false),
        ("get_report_documents_view", company.clone(), false),
        ("list_report_diff_candidates", company.clone(), false),
        (
            "get_report_diff",
            json!({ "olderReportDocumentId": "x", "newerReportDocumentId": "y" }),
            true,
        ),
        ("list_video_transcript_jobs", json!({}), false),
        (
            "list_transcript_segments",
            json!({ "transcriptJobId": "x" }),
            false,
        ),
        ("list_notebook_entries", company.clone(), false),
        ("list_management_claims", company.clone(), false),
        ("list_report_expectations", json!({}), false),
        ("list_decision_entries", json!({}), false),
        ("list_research_questions", json!({}), false),
        ("list_report_season", json!({}), false),
        ("list_attention_events", json!({}), false),
        ("get_latest_morning_briefing", json!({}), false),
        ("list_autopilot_runs", json!({}), false),
        ("get_autopilot_run", json!({ "runId": "x" }), true),
        ("list_quality_frameworks", json!({}), false),
        ("list_alert_rules", json!({}), false),
        // Triage (ADR 0088 dec. 4).
        ("list_flagged_extraction_outcomes", company.clone(), false),
        ("list_unclassified_filings", json!({}), false),
        // Raw tagged-fact capture (ADR 0100 decision 11).
        ("get_report_tagged_fact_coverage", company.clone(), false),
        ("get_pipeline_reextraction_progress", company.clone(), false),
        // Acquisition lifecycle reads (ADR 0099, #384/#385).
        ("list_pending_kpi_ingests", json!({}), false),
        (
            "get_kpi_ingest_status",
            json!({ "runId": "kpiing_missing" }),
            true,
        ),
        (
            "get_kpi_ingest_context",
            json!({ "runId": "kpiing_missing" }),
            true,
        ),
        (
            "get_kpi_ingest_document",
            json!({ "runId": "kpiing_missing", "offset": 0, "length": 1 }),
            true,
        ),
    ];

    // The map covers exactly the exposed `read` set.
    let exposed: BTreeSet<&str> = entries()
        .iter()
        .filter(|entry| entry.exposed && entry.tier == CapabilityTier::Read)
        .map(|entry| entry.tool_name)
        .collect();
    let covered: BTreeSet<&str> = inputs.iter().map(|(name, _, _)| *name).collect();
    assert_eq!(
        exposed, covered,
        "the umbrella input map must cover exactly the exposed read tools"
    );

    // `tools/list` advertises exactly the exposed tools.
    let listed: BTreeSet<String> = descriptors(McpScope::Full)
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();
    for name in &exposed {
        assert!(listed.contains(*name), "{name} is listed in tools/list");
    }

    // Each is callable and returns a domain outcome (never a protocol
    // error), and every success payload is a JSON OBJECT — MCP requires
    // `structuredContent` to be a record, so a bare array/scalar must have
    // been wrapped by the `tools::run` envelope (issue #249).
    for (name, arguments, may_fail) in inputs {
        let outcome = call(&state, McpScope::Full, name, arguments)
            .unwrap_or_else(|error| panic!("{name} rejected minimal input: {error:?}"));
        match outcome {
            ToolOutcome::Success(value) => assert!(
                value.is_object(),
                "{name}: structuredContent must be a JSON object (MCP spec), got: {value}"
            ),
            ToolOutcome::Failure(error) => {
                if !may_fail {
                    panic!("{name} failed on seeded minimal input: {error:?}")
                }
            }
        }
    }
}

// ---- Act tier (ADR 0088 M3) --------------------------------------------

use crate::storage::{
    open_in_memory_database, AppState, NewCompany, NewFrameworkCriterion, NewQualityFramework,
    SettingsUpdate,
};

fn act_state() -> AppState {
    AppState::new(open_in_memory_database().expect("db"))
}

/// Flip the live `mcpWritesEnabled` setting (the ONLY toggle — `update_settings`
/// is itself MCP-excluded, so no agent can reach it).
fn set_writes_enabled(state: &AppState, enabled: bool) {
    state
        .update_settings(SettingsUpdate {
            mcp_writes_enabled: Some(enabled),
            ..Default::default()
        })
        .expect("update settings");
}

/// Seed a company + a framework with one criterion; return their ids.
fn seed_company_framework(state: &AppState) -> (String, String, String) {
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "TST".to_owned(),
            display_name: "Test S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let framework: NewQualityFramework =
        serde_json::from_value(json!({ "name": "Framework" })).expect("framework input");
    let framework = state
        .create_quality_framework(framework)
        .expect("framework");
    let criterion: NewFrameworkCriterion = serde_json::from_value(json!({
        "frameworkId": framework.id,
        "label": "Moat",
        "kind": "qualitative",
        "assessmentGuidance": "Assess the durability of the competitive moat.",
    }))
    .expect("criterion input");
    let criterion = state
        .create_framework_criterion(criterion)
        .expect("criterion");
    (company.id, framework.id, criterion.id)
}

fn failure_code(outcome: ToolOutcome) -> CommandErrorCode {
    match outcome {
        ToolOutcome::Failure(error) => error.code,
        ToolOutcome::Success(value) => panic!("expected a Failure, got Success: {value}"),
    }
}

/// Toggle OFF: every act tool call is rejected with `writes_disabled` and the
/// handler never runs (nothing is written).
#[test]
fn act_tool_call_with_writes_disabled_is_rejected() {
    let state = act_state();
    let (company_id, _, _) = seed_company_framework(&state);
    // A fully valid create note — but writes are off.
    let outcome = call(
        &state,
        McpScope::Full,
        "create_notebook_entry",
        &json!({
            "companyId": company_id,
            "title": "t",
            "body": "b",
            "kind": "note",
            "tags": [],
            "origins": [{ "sourceType": "report" }],
        }),
    )
    .expect("a domain outcome, not a protocol error");
    assert_eq!(failure_code(outcome), CommandErrorCode::WritesDisabled);
    // Nothing was written.
    assert!(state
        .list_notebook_entries(&company_id)
        .expect("notes")
        .is_empty());
}

/// Toggle ON but the mandatory provenance carrier is empty ⇒ `provenance_required`
/// (naming the missing field), and the handler never runs. Covers one of each
/// carrier shape: Origins, SourceEvidence, FactCitation, and the batch
/// CitationsJson used by set_qualitative_verdicts.
#[test]
fn act_provenance_required_rejects_empty_carriers() {
    let state = act_state();
    let (company_id, framework_id, criterion_id) = seed_company_framework(&state);
    set_writes_enabled(&state, true);

    // Origins — create note with an empty origins array.
    let outcome = call(
        &state,
        McpScope::Full,
        "create_notebook_entry",
        &json!({
            "companyId": company_id,
            "title": "t",
            "body": "b",
            "kind": "note",
            "tags": [],
            "origins": [],
        }),
    )
    .expect("domain outcome");
    match outcome {
        ToolOutcome::Failure(error) => {
            assert_eq!(error.code, CommandErrorCode::ProvenanceRequired);
            assert!(
                error.message.contains("origins"),
                "names origins: {error:?}"
            );
        }
        ToolOutcome::Success(value) => panic!("empty origins must be rejected: {value}"),
    }
    assert!(state
        .list_notebook_entries(&company_id)
        .expect("notes")
        .is_empty());

    // SourceEvidence — create claim without sourceEvidenceId.
    let outcome = call(
        &state,
        McpScope::Full,
        "create_management_claim",
        &json!({ "companyId": company_id, "statement": "guidance" }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::ProvenanceRequired);

    // FactCitation — create fact without a citation.
    let outcome = call(
        &state,
        McpScope::Full,
        "create_financial_fact",
        &json!({
            "companyId": company_id,
            "periodId": "p",
            "definitionId": "kpidef_net_profit",
            "valueNumeric": "1",
        }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::ProvenanceRequired);

    // FactCitation — `attribution` alone (the slot dimension, not a
    // citation) must NOT satisfy the gate (epic #285 T9 defect closure).
    let outcome = call(
        &state,
        McpScope::Full,
        "create_financial_fact",
        &json!({
            "companyId": company_id,
            "periodId": "p",
            "definitionId": "kpidef_net_profit",
            "valueNumeric": "1",
            "attribution": "total",
        }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::ProvenanceRequired);

    // CitationsJson (batch) — set verdicts where a result carries no citations.
    let outcome = call(
        &state,
        McpScope::Full,
        "set_qualitative_verdicts",
        &json!({
            "frameworkId": framework_id,
            "companyId": company_id,
            "results": [{
                "criterionId": criterion_id,
                "verdict": "pass",
                "reasoning": "r",
                "citationsJson": "[]",
                "confidence": "low",
            }],
        }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::ProvenanceRequired);
}

/// Freezes the error-layer distinction (#343): an EMPTY citations array is
/// stopped at the port gate (`provenance_required`, test above); a
/// non-empty array of garbage passes the gate and is refused by the persist
/// chokepoint's integrity check as `invalid_input`.
#[test]
fn garbage_citations_pass_the_gate_and_fail_integrity_as_invalid_input() {
    let state = act_state();
    let (company_id, framework_id, criterion_id) = seed_company_framework(&state);
    set_writes_enabled(&state, true);
    let outcome = call(
        &state,
        McpScope::Full,
        "set_qualitative_verdicts",
        &json!({
            "frameworkId": framework_id,
            "companyId": company_id,
            "results": [{
                "criterionId": criterion_id,
                "verdict": "pass",
                "reasoning": "r",
                "citationsJson": "[\"freeform prose\"]",
                "confidence": "low",
            }],
        }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::InvalidInput);
}

/// Seed a company + fiscal period a financial-fact write can slot into.
/// `kpidef_net_profit` is a canonical seeded definition's deterministic id
/// (no lookup needed — the same convention `every_exposed_act_tool_is_
/// listed_and_gated`'s umbrella input map uses).
fn seed_fact_slot(state: &AppState) -> (String, String) {
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "AGT".to_owned(),
            display_name: "Agent Test S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let period = state
        .create_financial_period(storage::NewFinancialPeriod {
            company_id: company.id.clone(),
            fiscal_year: 2026,
            period_type: "FY".to_owned(),
            period_end_date: Some("2026-12-31".to_owned()),
            report_evidence_ref: None,
        })
        .expect("financial period");
    (company.id, period.id)
}

/// ADR 0093 decision 1 honesty rule (epic #285 T9): an MCP-authored
/// `create_financial_fact` write must stamp a `financial_fact_provenance`
/// row and `extraction_method` != `'manual'` — never masquerading as the
/// owner's own entry at the untouchable top of the trust ladder.
#[test]
fn create_financial_fact_over_mcp_stamps_agent_provenance() {
    let state = act_state();
    let (company_id, period_id) = seed_fact_slot(&state);
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_financial_fact",
        &json!({
            "companyId": company_id,
            "periodId": period_id,
            "definitionId": "kpidef_net_profit",
            "valueNumeric": "1000000",
            "sourceDocumentRef": "doc_xtb_rb18",
        }),
    )
    .expect("domain outcome");
    assert!(
        matches!(outcome, ToolOutcome::Success(_)),
        "expected a Success outcome: {outcome:?}"
    );

    let facts = state
        .list_financial_facts(storage::ListFinancialFactsInput {
            company_id: Some(company_id),
            period_id: Some(period_id),
            definition_id: None,
        })
        .expect("facts should list");
    assert_eq!(facts.len(), 1);
    assert_eq!(
        facts[0].extraction_method, "mcp_agent",
        "an MCP write must never claim `manual`"
    );

    let provenance = state
        .fundamentals_provenance()
        .get_fact_provenance(&facts[0].id)
        .expect("provenance read")
        .expect("an MCP write must leave a provenance row (closes the untouchable-slot hole)");
    assert_eq!(provenance.source_tier, "agent");
    assert_eq!(provenance.validation_status, "unreviewed");
    assert_eq!(provenance.citation.as_deref(), Some("doc_xtb_rb18"));
}

/// ADR 0093 decision 1 (epic #285 T9): `update_financial_fact` over MCP
/// stamps `source_tier='agent'` provenance even on a fact that started as
/// a plain UI `manual` entry (no provenance row at all) — the owner's own
/// agent, acting through the `mcpWritesEnabled`-gated interactive act
/// path, may take the slot over, but the takeover is recorded honestly
/// rather than silently preserving the `manual` label. RED (pre-fix): no
/// provenance row existed after the update (the handler called storage
/// verbatim).
#[test]
fn update_financial_fact_over_mcp_stamps_agent_provenance_on_a_manual_fact() {
    let state = act_state();
    let (company_id, period_id) = seed_fact_slot(&state);

    // A plain UI/manual create — the exact shape `create_financial_fact`
    // (Tauri command) produces: no provenance row, extraction_method
    // defaults 'manual'.
    let fact = state
        .create_financial_fact(storage::NewFinancialFact {
            company_id,
            period_id,
            definition_id: "kpidef_net_profit".to_owned(),
            value_numeric: "500000".to_owned(),
            currency: None,
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: None,
            confidence: None,
            confirmation_state: None,
            supersedes_id: None,
            source_document_ref: None,
            annotation: None,
        })
        .expect("manual fact should create");
    assert_eq!(fact.extraction_method, "manual");
    assert!(
        state
            .fundamentals_provenance()
            .get_fact_provenance(&fact.id)
            .expect("provenance read")
            .is_none(),
        "a fresh manual fact has no provenance row"
    );

    set_writes_enabled(&state, true);
    let outcome = call(
        &state,
        McpScope::Full,
        "update_financial_fact",
        &json!({
            "id": fact.id,
            "valueNumeric": "600000",
            "sourceDocumentRef": "doc_correction",
        }),
    )
    .expect("domain outcome");
    assert!(
        matches!(outcome, ToolOutcome::Success(_)),
        "an agent update on a manual fact succeeds (interactive, owner-gated write): {outcome:?}"
    );

    let provenance = state
        .fundamentals_provenance()
        .get_fact_provenance(&fact.id)
        .expect("provenance read")
        .expect("the MCP update must stamp a provenance row, even taking over a manual fact");
    assert_eq!(provenance.source_tier, "agent");
    assert_eq!(provenance.validation_status, "unreviewed");
    assert_eq!(provenance.citation.as_deref(), Some("doc_correction"));
}

/// Ladder test (epic #285 T9): an MCP-written fact (agent tier) is
/// subsequently UPGRADEABLE by an issuer-tier `record_structured_fact`
/// write — impossible before this fix (no provenance = manual = the top
/// of the ladder, untouchable by every automatic path). Mirrors
/// `an_issuer_reobservation_upgrades_an_agent_slot_label`
/// (`storage/kpi_extraction.rs`), but starting from the REAL MCP act path
/// rather than a raw `StructuredFactInput`.
#[test]
fn an_mcp_written_fact_is_upgradeable_by_a_later_issuer_tier_write() {
    let state = act_state();
    let (company_id, period_id) = seed_fact_slot(&state);
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_financial_fact",
        &json!({
            "companyId": company_id,
            "periodId": period_id,
            "definitionId": "kpidef_net_profit",
            "valueNumeric": "1000000",
            "sourceDocumentRef": "doc_xtb_rb18",
        }),
    )
    .expect("domain outcome");
    assert!(matches!(outcome, ToolOutcome::Success(_)), "agent write");

    let company_for_structured = company_id.clone();
    let commit = state
        .kpi_extraction()
        .record_structured_fact(storage::StructuredFactInput {
            company_id: &company_for_structured,
            fiscal_year: 2026,
            period_type: "FY",
            period_end: Some("2026-12-31"),
            report_document_id: "doc_esef",
            metric_key: "net_profit",
            value_numeric: "1000000",
            currency: None,
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Net profit"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        })
        .expect("issuer re-observation");
    assert!(
        matches!(commit, storage::StructuredFactCommit::Upgraded { .. }),
        "an issuer tier must upgrade the agent-held slot, not skip/divergence it: {commit:?}"
    );

    let facts = state
        .list_financial_facts(storage::ListFinancialFactsInput {
            company_id: Some(company_id),
            period_id: Some(period_id),
            definition_id: None,
        })
        .expect("facts should list");
    assert_eq!(
        facts.len(),
        1,
        "the upgrade rewrites the same slot in place"
    );
    let provenance = state
        .fundamentals_provenance()
        .get_fact_provenance(&facts[0].id)
        .expect("provenance read")
        .expect("provenance row");
    assert_eq!(
        provenance.source_tier, "esef",
        "the issuer tier now owns the slot's label"
    );
}

/// #111 closure (epic #285 T9): a notebook note origin can cite a stored
/// `report_documents` row directly (`source_type: "report_document"`),
/// not just a bare `external_url` link — the document the agent actually
/// read and registered via `capture_report_document`. Round-trips through
/// the REAL MCP `create_notebook_entry` act path. RED (pre-fix): the
/// storage-layer allow-list rejected `"report_document"` with a typed
/// `InvalidNotebookValue` error, surfaced as an MCP `invalid_input`
/// Failure.
#[test]
fn create_notebook_entry_over_mcp_accepts_a_report_document_origin() {
    let state = act_state();
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "RDO".to_owned(),
            display_name: "Report Doc Origin S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    let document = state
        .create_or_find_pending_report_document(storage::CaptureReportDocumentInput {
            company_id: company.id.clone(),
            source_type: "user_url".to_owned(),
            url: "https://example.com/xtb-rb-18-2026.pdf".to_owned(),
            period_id: None,
            origin_ref: None,
            title: Some("XTB RB 18/2026".to_owned()),
            attribution: None,
        })
        .expect("report document should register");
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_notebook_entry",
        &json!({
            "companyId": company.id,
            "title": "Preliminary H1 results",
            "body": "Net profit ~1.0bn PLN per the preliminary release.",
            "kind": "observation",
            "tags": [],
            "origins": [{
                "sourceType": "report_document",
                "sourceId": document.id,
                "label": "XTB RB 18/2026",
            }],
        }),
    )
    .expect("domain outcome");
    assert!(
        matches!(outcome, ToolOutcome::Success(_)),
        "a report_document origin must be accepted: {outcome:?}"
    );

    let entries = state
        .list_notebook_entries(&company.id)
        .expect("notes should list");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].origins.len(), 1);
    assert_eq!(entries[0].origins[0].source_type, "report_document");
    assert_eq!(
        entries[0].origins[0].source_id.as_deref(),
        Some(document.id.as_str())
    );
}

/// ADR 0093 decision 4 (epic #285 T9): `create_kpi_definition` over MCP
/// always stamps `origin='agent'`, regardless of what the caller sends
/// (even an explicit `"origin": "user"` is overridden — the field is not
/// a caller's to set). RED (pre-fix): the handler called storage verbatim
/// and the definition's `origin` stayed the DEFAULT `user`.
#[test]
fn create_kpi_definition_over_mcp_stamps_agent_origin() {
    let state = act_state();
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "KDO".to_owned(),
            display_name: "KPI Definition Origin S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_kpi_definition",
        &json!({
            "scope": "company",
            "companyId": company.id,
            "metricKey": "broker_client_count",
            "label": "Broker Client Count",
            "valueKind": "count",
            "computation": "reported",
            "origin": "user",
        }),
    )
    .expect("domain outcome");
    assert!(
        matches!(outcome, ToolOutcome::Success(_)),
        "expected a Success outcome: {outcome:?}"
    );

    let definitions = state
        .list_kpi_definitions(storage::ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions should list");
    let definition = definitions
        .iter()
        .find(|d| d.metric_key == "broker_client_count")
        .expect("the definition should exist");
    assert_eq!(
        definition.origin, "agent",
        "MCP-minted definitions are always agent-origin, never caller-controlled"
    );
}

/// ADR 0093 decision 4 (epic #285 T9): a non-snake_case `metricKey` is a
/// typed refusal, not a silently-accepted catalog pollution. RED (pre-fix):
/// `create_kpi_definition` accepted ANY metric_key string verbatim.
#[test]
fn create_kpi_definition_over_mcp_rejects_a_non_snake_case_metric_key() {
    let state = act_state();
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_kpi_definition",
        &json!({
            "scope": "company",
            "companyId": "irrelevant",
            "metricKey": "Broker Client Count!",
            "label": "Broker Client Count",
            "valueKind": "count",
            "computation": "reported",
        }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::InvalidInput);

    assert!(
        state
            .list_kpi_definitions(storage::ListKpiDefinitionsInput {
                scope: None,
                sector: None,
                company_id: None,
            })
            .expect("definitions should list")
            .iter()
            .all(|d| d.metric_key != "Broker Client Count!"),
        "a rejected metricKey must never be written"
    );
}

/// Card #307: `create_kpi_definition` over MCP accepts an explicit
/// `statementGroup` from the caller (unlike `origin`, nothing forces this
/// field) and stores it verbatim when it is a valid vocabulary token.
#[test]
fn create_kpi_definition_over_mcp_accepts_a_valid_statement_group() {
    let state = act_state();
    let company = state
        .create_company(NewCompany {
            exchange: "GPW".to_owned(),
            ticker: "SGD".to_owned(),
            display_name: "Statement Group S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        })
        .expect("company");
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_kpi_definition",
        &json!({
            "scope": "company",
            "companyId": company.id,
            "metricKey": "cfd_lots_traded",
            "label": "CFD lots traded",
            "valueKind": "count",
            "computation": "reported",
            "statementGroup": "cash_flow",
        }),
    )
    .expect("domain outcome");
    assert!(
        matches!(outcome, ToolOutcome::Success(_)),
        "expected a Success outcome: {outcome:?}"
    );

    let definitions = state
        .list_kpi_definitions(storage::ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        })
        .expect("definitions should list");
    let definition = definitions
        .iter()
        .find(|d| d.metric_key == "cfd_lots_traded")
        .expect("the definition should exist");
    assert_eq!(definition.statement_group, "cash_flow");
}

/// Card #307: an unknown `statementGroup` token is a typed refusal, not a
/// silently-accepted catalog pollution — same shape as the metricKey guard.
#[test]
fn create_kpi_definition_over_mcp_rejects_an_unknown_statement_group() {
    let state = act_state();
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_kpi_definition",
        &json!({
            "scope": "company",
            "companyId": "irrelevant",
            "metricKey": "garbage_group_metric",
            "label": "Garbage Group Metric",
            "valueKind": "count",
            "computation": "reported",
            "statementGroup": "nonsense",
        }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(outcome), CommandErrorCode::InvalidInput);

    assert!(
        state
            .list_kpi_definitions(storage::ListKpiDefinitionsInput {
                scope: None,
                sector: None,
                company_id: None,
            })
            .expect("definitions should list")
            .iter()
            .all(|d| d.metric_key != "garbage_group_metric"),
        "a rejected statementGroup must never be written"
    );
}

/// End-to-end: with writes on and citations present, set_qualitative_verdicts
/// persists, and the get_quality_assessment READ tool returns the verdict.
#[test]
fn set_qualitative_verdicts_round_trips_through_mcp() {
    let state = act_state();
    let (company_id, framework_id, criterion_id) = seed_company_framework(&state);
    set_writes_enabled(&state, true);
    // Citation integrity (#343): the cited evidence must exist, so the
    // round-trip cites a real notebook entry.
    let note = state
        .create_notebook_entry(storage::NewNotebookEntry {
            company_id: company_id.clone(),
            title: "Evidence".to_owned(),
            body: "Cited by the verdict.".to_owned(),
            body_format: Some("markdown".to_owned()),
            tags: vec![],
            kind: "manual".to_owned(),
            claim_status: None,
            event_date: None,
            follow_up_after: None,
            follow_up_date: None,
            origins: vec![],
        })
        .expect("evidence note");

    let write = call(
            &state,
            McpScope::Full,
            "set_qualitative_verdicts",
            &json!({
                "frameworkId": framework_id,
                "companyId": company_id,
                "results": [{
                    "criterionId": criterion_id,
                    "verdict": "pass",
                    "reasoning": "wide moat",
                    "citationsJson": format!(r#"[{{"evidenceType":"notebook_entry","evidenceId":"{}"}}]"#, note.id),
                    "confidence": "high",
                }],
            }),
        )
        .expect("domain outcome");
    assert!(
        matches!(write, ToolOutcome::Success(_)),
        "verdict write should succeed: {write:?}"
    );

    // The read tool speaks qualified tickers.
    let read = call(
        &state,
        McpScope::Full,
        "get_quality_assessment",
        &json!({ "company": "GPW:TST" }),
    )
    .expect("domain outcome");
    let payload = match read {
        ToolOutcome::Success(value) => value,
        other => panic!("assessment read failed: {other:?}"),
    };
    let text = payload.to_string();
    assert!(
        text.contains("wide moat"),
        "verdict reasoning present: {text}"
    );
    assert!(
        text.contains("\"verdict\":\"pass\""),
        "verdict present: {text}"
    );
}

/// MCP parity (issue #250): `create_company` routes through the same
/// command impl as the UI, so a GPW company created over MCP gets its
/// quote-backfill job enqueued (the tool description promises it).
#[test]
fn create_company_over_mcp_enqueues_the_gpw_quote_backfill() {
    let state = act_state();
    set_writes_enabled(&state, true);

    let outcome = call(
        &state,
        McpScope::Full,
        "create_company",
        &json!({
            "exchange": "GPW",
            "ticker": "ZZZ",
            "displayName": "Zzz S.A.",
        }),
    )
    .expect("domain outcome");
    let company = match outcome {
        ToolOutcome::Success(value) => value,
        ToolOutcome::Failure(error) => panic!("create_company failed: {error:?}"),
    };
    let company_id = company["id"].as_str().expect("company id");

    let job = state
        .jobs()
        .status(&format!("quote_backfill:{company_id}"))
        .expect("job status query")
        .expect("backfill job enqueued for an MCP-created GPW company");
    assert_eq!(job.status, "pending");
}

/// Guardrail (issue #250): a command that wraps extra logic in an
/// extracted `<command>_impl` helper (e.g. `create_company_impl`'s GPW
/// quote-backfill enqueue) must have its exposed MCP handler route through
/// that helper — dispatching the bare storage write silently drops the
/// command's extra behavior. Source-scan: for every exposed tool whose
/// backing command has a `<command>_impl` in `src/commands/`, the `mcp`
/// module must reference that helper.
#[test]
fn exposed_handlers_route_through_command_impl_helpers() {
    use std::fs;
    use std::path::{Path, PathBuf};

    fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).expect("source dir") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                rust_sources(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let impl_pattern = regex::Regex::new(r"\bfn ([a-z0-9_]+)_impl\b").expect("valid regex");

    let mut command_sources = Vec::new();
    rust_sources(&manifest.join("src/commands"), &mut command_sources);
    let mut impl_names = BTreeSet::new();
    for path in command_sources {
        let source = fs::read_to_string(&path).expect("command source");
        for capture in impl_pattern.captures_iter(&source) {
            impl_names.insert(capture.get(1).expect("group 1").as_str().to_owned());
        }
    }
    assert!(
        impl_names.contains("create_company"),
        "sanity: create_company_impl should be discovered in src/commands/"
    );

    let mut mcp_sources = Vec::new();
    rust_sources(&manifest.join("src/mcp"), &mut mcp_sources);
    let mcp_source: String = mcp_sources
        .iter()
        .map(|path| fs::read_to_string(path).expect("mcp source"))
        .collect();

    for entry in entries() {
        if !entry.exposed || !impl_names.contains(entry.command_name) {
            continue;
        }
        assert!(
            mcp_source.contains(&format!("{}_impl", entry.command_name)),
            "{}: the backing command extracts `{}_impl`, but the MCP handler never \
                 references it — route the handler through the impl (UI parity, issue #250)",
            entry.tool_name,
            entry.command_name
        );
    }
}

/// The act wave's umbrella proof: every exposed `act` tool is listed in
/// `tools/list`, rejected with `writes_disabled` when the toggle is OFF, and —
/// with the toggle ON and a minimal valid input — dispatches without panic and
/// without a protocol error (a domain Success/Failure is fine). The per-tool
/// input map must cover the exposed act set exactly, so a newly-exposed act
/// tool with no test input reddens here.
#[test]
fn every_exposed_act_tool_is_listed_and_gated() {
    let state = act_state();
    let (company_id, framework_id, criterion_id) = seed_company_framework(&state);

    // Minimal valid input per exposed act tool: required fields satisfied and,
    // for provenance carriers, the carrier present.
    let inputs: Vec<(&str, Value)> = vec![
        (
            "create_notebook_entry",
            json!({ "companyId": company_id, "title": "t", "body": "b", "kind": "note", "tags": [], "origins": [{ "sourceType": "report" }] }),
        ),
        (
            "create_note_from_transcript_selection",
            json!({ "transcriptJobId": "x", "transcriptSegmentIds": ["s"], "noteDraft": { "title": "t", "body": "b", "tags": [], "kind": "note" } }),
        ),
        (
            "update_notebook_entry",
            json!({ "id": "x", "title": "t", "body": "b", "kind": "note", "tags": [] }),
        ),
        (
            "create_management_claim",
            json!({ "companyId": company_id, "statement": "s", "sourceEvidenceId": "ev" }),
        ),
        (
            "update_management_claim",
            json!({ "id": "x", "sourceEvidenceId": "ev" }),
        ),
        (
            "set_claim_verdict",
            json!({ "claimId": "x", "status": "verified_true" }),
        ),
        (
            "create_financial_fact",
            json!({ "companyId": company_id, "periodId": "p", "definitionId": "kpidef_net_profit", "valueNumeric": "1", "sourceDocumentRef": "doc" }),
        ),
        (
            "update_financial_fact",
            json!({ "id": "x", "sourceDocumentRef": "doc" }),
        ),
        (
            // https-only gate refuses before any network call (ADR 0093
            // dec. 5) — a domain Success carrying success:false, never a
            // live fetch; the umbrella stays hermetic.
            "capture_report_document",
            json!({ "companyId": company_id, "url": "http://example.com/doc.pdf" }),
        ),
        (
            "record_financial_facts",
            // reportDocumentId "x" is unseeded here (seed_company_framework
            // creates no report_documents row) — a typed `not_found` Failure
            // is the expected, acceptable domain outcome (comment above).
            json!({
                "companyId": company_id,
                "reportDocumentId": "x",
                "period": { "fiscalYear": 2025, "periodType": "FY" },
                "facts": [{ "metricKey": "net_profit", "valueNumeric": "1", "citation": "p.1" }]
            }),
        ),
        (
            "set_qualitative_verdicts",
            json!({ "frameworkId": framework_id, "companyId": company_id, "results": [{ "criterionId": criterion_id, "verdict": "pass", "reasoning": "r", "citationsJson": "[{\"k\":1}]", "confidence": "low" }] }),
        ),
        (
            "create_research_question",
            json!({ "scopeType": "company", "scopeId": company_id, "title": "q" }),
        ),
        ("update_research_question", json!({ "id": "x" })),
        (
            "create_evidence_link",
            json!({ "fromType": "note", "fromId": "a", "toType": "fact", "toId": "b", "relationType": "supports" }),
        ),
        (
            "create_research_reminder",
            json!({ "scopeType": "company", "scopeId": company_id, "reminderKind": "follow_up", "title": "r" }),
        ),
        ("update_research_reminder", json!({ "id": "x" })),
        (
            "create_decision_entry",
            json!({ "companyId": company_id, "kind": "buy", "rationaleMd": "r", "decidedAt": "2026-01-01" }),
        ),
        (
            "create_report_expectation",
            json!({ "companyId": company_id, "eventKey": "e", "fiscalYear": 2025, "periodType": "FY", "stanceMd": "s" }),
        ),
        (
            "update_report_expectation",
            json!({ "companyId": company_id, "eventKey": "e" }),
        ),
        (
            "record_expectation_resolution",
            json!({ "companyId": company_id, "eventKey": "e", "resolutionNoteMd": "n" }),
        ),
        (
            "create_company_event",
            json!({ "companyId": company_id, "eventType": "dividend", "title": "t", "eventDate": "2026-01-01" }),
        ),
        (
            "create_kpi_definition",
            json!({ "scope": "global", "metricKey": "mk", "label": "L", "valueKind": "currency", "computation": "reported" }),
        ),
        (
            "create_kpi_relevance",
            json!({ "companyId": company_id, "definitionId": "kpidef_net_profit", "source": "manual" }),
        ),
        ("update_kpi_relevance", json!({ "id": "x" })),
        ("create_quality_framework", json!({ "name": "F2" })),
        ("update_quality_framework", json!({ "id": framework_id })),
        (
            "create_framework_criterion",
            json!({ "frameworkId": framework_id, "label": "C2" }),
        ),
        ("update_framework_criterion", json!({ "id": criterion_id })),
        (
            "create_alert_rule",
            json!({ "triggerType": "autopilot_run_completed", "scopeType": "company", "scopeRef": company_id }),
        ),
        ("update_alert_rule", json!({ "id": "x" })),
        (
            "create_company",
            json!({ "exchange": "GPW", "ticker": "NEW", "displayName": "New S.A." }),
        ),
        ("create_watchlist", json!({ "name": "W" })),
        (
            "add_company_to_watchlist",
            json!({ "watchlistId": "x", "companyId": company_id }),
        ),
        (
            "remove_company_from_watchlist",
            json!({ "watchlistId": "x", "companyId": company_id }),
        ),
        ("update_feed_item_state", json!({ "id": "x" })),
        (
            "mark_report_prepared",
            json!({ "companyId": company_id, "eventKey": "e" }),
        ),
        (
            "mark_report_processed",
            json!({ "companyId": company_id, "eventKey": "e" }),
        ),
        (
            "mark_research_scope_reviewed",
            json!({ "scopeType": "company", "scopeId": company_id }),
        ),
        ("confirm_company_signal", json!({ "id": "x" })),
        ("reject_company_signal", json!({ "id": "x" })),
        (
            "classify_filing",
            json!({ "feedItemId": "x", "category": "dividend" }),
        ),
        (
            "confirm_derived_event",
            json!({ "eventId": "x", "action": "confirm" }),
        ),
        ("acknowledge_red_flag", json!({ "flagId": "x" })),
        (
            "set_ownership_holder_type",
            json!({ "companyId": company_id, "holderKey": "k" }),
        ),
        ("mark_attention_event_seen", json!({ "id": "x" })),
        ("dismiss_attention_event", json!({ "id": "x" })),
        (
            "set_autopilot_run_notification_state",
            json!({ "runId": "x", "notificationState": "read" }),
        ),
        (
            "evaluate_framework",
            json!({ "frameworkId": framework_id, "companyId": company_id }),
        ),
        (
            "compute_comparative_valuation",
            json!({ "companyId": company_id }),
        ),
        (
            "set_alert_rule_enabled",
            json!({ "id": "x", "enabled": true }),
        ),
        (
            "trigger_autopilot_run",
            json!({ "companyId": company_id, "reportDocumentId": "x" }),
        ),
        ("generate_morning_briefing", json!({})),
        // Networked / heavy triggers — listed + gated here, but NOT invoked
        // on ON (see NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS). Inputs present
        // only to keep the map coverage complete.
        ("refresh_sources", json!({})),
        ("refresh_source", json!({ "adapterId": "x" })),
        ("run_aggregator_fundamentals_pull", json!({})),
        (
            "backfill_company_history",
            json!({ "companyId": company_id }),
        ),
        (
            "run_structured_extraction",
            json!({ "companyId": company_id, "reportDocumentId": "x", "fiscalYear": 2025, "periodType": "FY", "periodEnd": "2025-12-31" }),
        ),
        ("rerun_extraction_outcome", json!({ "outcomeId": "x" })),
        (
            "run_pipeline_reextraction",
            json!({ "companyId": company_id }),
        ),
        // Acquisition lifecycle acts (ADR 0099, #384/#386) — domain
        // failures on the minimal inputs (unknown run), which the
        // umbrella accepts.
        ("start_kpi_ingest", json!({ "runId": "kpiing_missing" })),
        ("cancel_kpi_ingest", json!({ "runId": "kpiing_missing" })),
        (
            "stage_kpi_observations",
            json!({
                "runId": "kpiing_missing",
                "observations": [{ "rawLabel": "x", "rawValue": "1" }],
                "missingReasons": {}
            }),
        ),
        (
            "propose_kpi_definition",
            json!({
                "runId": "kpiing_missing",
                "metricKey": "broker_client_count",
                "label": "Broker client count",
                "statementGroup": "other"
            }),
        ),
        (
            "validate_kpi_ingest",
            json!({ "runId": "kpiing_missing", "revision": 1 }),
        ),
        (
            "commit_kpi_ingest",
            json!({
                "runId": "kpiing_missing",
                "manifestHash": "0000000000000000000000000000000000000000000000000000000000000000",
                "revision": 1
            }),
        ),
    ];

    // Networked / heavy triggers: the toggle-ON minimal-input invocation is
    // skipped for exactly these — they run live source ingestion / extraction
    // / backfill work with no hermetic seam, and are exercised by the M6 live
    // dogfooding ritual instead. They are still listed and still
    // writes_disabled-gated below.
    const NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS: &[&str] = &[
        "refresh_sources",
        "refresh_source",
        "run_aggregator_fundamentals_pull",
        "backfill_company_history",
        "run_structured_extraction",
        "rerun_extraction_outcome",
    ];

    let exposed: BTreeSet<&str> = entries()
        .iter()
        .filter(|entry| entry.exposed && entry.tier == CapabilityTier::Act)
        .map(|entry| entry.tool_name)
        .collect();
    let covered: BTreeSet<&str> = inputs.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        exposed, covered,
        "the umbrella input map must cover exactly the exposed act tools"
    );
    // A rename can't silently orphan the skip-allowlist.
    assert!(
        NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS
            .iter()
            .all(|name| exposed.contains(name)),
        "every NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS entry must be an exposed act tool"
    );

    let listed: BTreeSet<String> = descriptors(McpScope::Full)
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();

    for (name, arguments) in &inputs {
        // Listed in tools/list ALWAYS (discoverability).
        assert!(listed.contains(*name), "{name} is listed in tools/list");

        // Toggle OFF ⇒ writes_disabled, before the handler runs.
        set_writes_enabled(&state, false);
        let off = call(&state, McpScope::Full, name, arguments).expect("domain outcome");
        assert_eq!(
            failure_code(off),
            CommandErrorCode::WritesDisabled,
            "{name} must be gated off when writes are disabled"
        );

        // Toggle ON ⇒ dispatches without panic / protocol error (a domain
        // Success or Failure is acceptable — many use unseeded ids). The
        // networked / heavy triggers are gate-only here (invocation would run
        // live work) — exercised by the M6 dogfooding ritual instead.
        if NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS.contains(name) {
            continue;
        }
        set_writes_enabled(&state, true);
        let outcome = call(&state, McpScope::Full, name, arguments)
            .unwrap_or_else(|error| panic!("{name} raised a protocol error on ON: {error:?}"));
        // Every success payload is a JSON OBJECT — MCP requires
        // `structuredContent` to be a record (issue #249).
        if let ToolOutcome::Success(value) = outcome {
            assert!(
                value.is_object(),
                "{name}: structuredContent must be a JSON object (MCP spec), got: {value}"
            );
        }
    }
}

#[test]
fn validate_provenance_rejects_empty_and_accepts_present() {
    use ProvenanceRequirement::*;

    // Origins — a non-empty array.
    assert_eq!(
        validate_provenance(Origins, &json!({})).unwrap_err().code,
        CommandErrorCode::ProvenanceRequired
    );
    assert!(validate_provenance(Origins, &json!({ "origins": [] })).is_err());
    assert!(
        validate_provenance(Origins, &json!({ "origins": [{ "sourceType": "report" }] })).is_ok()
    );

    // SourceEvidence — a non-blank sourceEvidenceId.
    assert!(validate_provenance(SourceEvidence, &json!({})).is_err());
    assert!(validate_provenance(SourceEvidence, &json!({ "sourceEvidenceId": "  " })).is_err());
    assert!(validate_provenance(SourceEvidence, &json!({ "sourceEvidenceId": "ev_1" })).is_ok());

    // CitationsJson — a serialized, non-empty array.
    assert!(validate_provenance(CitationsJson, &json!({})).is_err());
    assert!(validate_provenance(CitationsJson, &json!({ "citationsJson": "[]" })).is_err());
    assert!(validate_provenance(
        CitationsJson,
        &json!({ "citationsJson": "[{\"kind\":\"note\"}]" })
    )
    .is_ok());

    // CitationsJson batch shape (set_qualitative_verdicts): every entry in a
    // non-empty `results` array must carry a non-empty citationsJson.
    assert!(validate_provenance(CitationsJson, &json!({ "results": [] })).is_err());
    assert!(validate_provenance(
        CitationsJson,
        &json!({ "results": [{ "citationsJson": "[]" }] })
    )
    .is_err());
    assert!(validate_provenance(
        CitationsJson,
        &json!({ "results": [{ "citationsJson": "[{\"kind\":\"note\"}]" }] })
    )
    .is_ok());

    // FactCitation — a non-blank sourceDocumentRef ONLY (epic #285 T9):
    // `attribution` is the fact's slot dimension, never an alternate
    // citation carrier — prose there must be refused, not accepted.
    assert!(validate_provenance(FactCitation, &json!({})).is_err());
    assert!(validate_provenance(FactCitation, &json!({ "sourceDocumentRef": "doc_1" })).is_ok());
    assert!(validate_provenance(
        FactCitation,
        &json!({ "attribution": "Skonsolidowany 2025" })
    )
    .is_err());
    // A blank sourceDocumentRef alongside attribution still refuses —
    // attribution never compensates.
    assert!(validate_provenance(
        FactCitation,
        &json!({ "sourceDocumentRef": "  ", "attribution": "total" })
    )
    .is_err());
    // `UpdateFinancialFact` carries no `attribution` field at all (it is
    // immutable post-create — a slot dimension, not editable); after this
    // fix both create and update gate on sourceDocumentRef ONLY, so the
    // same carrier shape covers both — parity, not a special case.

    // DocumentAndPerFactCitations — reportDocumentId AND every fact cited.
    assert!(validate_provenance(DocumentAndPerFactCitations, &json!({})).is_err());
    assert!(validate_provenance(
        DocumentAndPerFactCitations,
        &json!({ "reportDocumentId": "doc_1", "facts": [] })
    )
    .is_err());
    assert!(validate_provenance(
        DocumentAndPerFactCitations,
        &json!({
            "reportDocumentId": "doc_1",
            "facts": [{ "citation": "p.1" }, { "citation": "  " }]
        })
    )
    .is_err());
    assert!(validate_provenance(
        DocumentAndPerFactCitations,
        &json!({
            "reportDocumentId": "doc_1",
            "facts": [{ "citation": "p.1" }, { "citation": "p.2" }]
        })
    )
    .is_ok());
}

/// ADR 0093 dec. 5 (epic #285 T8): `capture_report_document` always
/// writes `source_type = "user_url"` — an agent trying to set `sourceType`
/// itself is refused before the handler runs (the field is not in the
/// exposed schema at all, so `deny_unknown_fields` produces the same
/// typed protocol refusal T7 proved for `totallyMadeUpField`) — and the
/// write is idempotent on `(companyId, url)`. Exercised via an `http://`
/// URL so the https-only gate refuses BEFORE any network call — hermetic.
#[test]
fn capture_report_document_forces_user_url_and_is_idempotent_without_network() {
    let state = act_state();
    let (company_id, _framework_id, _criterion_id) = seed_company_framework(&state);
    set_writes_enabled(&state, true);

    let unknown_field = call(
        &state,
        McpScope::Full,
        "capture_report_document",
        &json!({
            "companyId": company_id,
            "url": "https://example.com/doc.pdf",
            "sourceType": "espi_attachment",
        }),
    );
    match unknown_field {
        Err(ToolCallError::InvalidArguments(message)) => {
            assert!(
                message.contains("sourceType"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected InvalidArguments, got {other:?}"),
    }

    let url = "http://example.com/doc.pdf";
    let first = call(
        &state,
        McpScope::Full,
        "capture_report_document",
        &json!({ "companyId": company_id, "url": url }),
    )
    .expect("domain outcome, not a protocol error");
    let first_id = match first {
        ToolOutcome::Success(value) => value["documentId"]
            .as_str()
            .expect("documentId present")
            .to_owned(),
        ToolOutcome::Failure(error) => panic!("capture_report_document failed: {error:?}"),
    };
    assert!(
        !first_id.is_empty(),
        "a document row is created even though the fetch itself is refused"
    );

    let document = state
        .get_report_document(&first_id)
        .expect("report document");
    assert_eq!(
        document.source_type, "user_url",
        "an agent capture always registers source_type=user_url, never an ingest type"
    );

    // Idempotent: same company_id+url returns the SAME row (existing
    // UNIQUE(company_id, url) behavior, unaffected by the new gates).
    let second = call(
        &state,
        McpScope::Full,
        "capture_report_document",
        &json!({ "companyId": company_id, "url": url }),
    )
    .expect("domain outcome");
    let second_id = match second {
        ToolOutcome::Success(value) => value["documentId"].as_str().expect("documentId").to_owned(),
        ToolOutcome::Failure(error) => panic!("capture_report_document failed: {error:?}"),
    };
    assert_eq!(
        first_id, second_id,
        "same company+url must return the same row"
    );
}

/// Two ADR 0093 dec. 6 guardrails for `record_financial_facts` that only
/// the real dispatch path (`registry::call`) can exercise: (1) an unknown
/// input field is rejected before the handler runs (schemars
/// `deny_unknown_fields` ⇒ a protocol `InvalidArguments`, never a silently
/// dropped agent typo); (2) ONE blank citation among several facts refuses
/// the WHOLE batch atomically — the provenance gate runs BEFORE the
/// handler, so nothing is written even though the other facts in the same
/// call are individually well-formed.
#[test]
fn record_financial_facts_rejects_unknown_field_and_a_blank_citation_atomically() {
    let state = act_state();
    let (company_id, _framework_id, _criterion_id) = seed_company_framework(&state);
    set_writes_enabled(&state, true);

    let unknown_field = call(
        &state,
        McpScope::Full,
        "record_financial_facts",
        &json!({
            "companyId": company_id,
            "reportDocumentId": "x",
            "period": { "fiscalYear": 2025, "periodType": "FY" },
            "facts": [{ "metricKey": "net_profit", "valueNumeric": "1", "citation": "p.1" }],
            "totallyMadeUpField": true
        }),
    );
    match unknown_field {
        Err(ToolCallError::InvalidArguments(message)) => {
            assert!(
                message.contains("totallyMadeUpField"),
                "unexpected message: {message}"
            );
        }
        other => panic!("expected InvalidArguments, got {other:?}"),
    }

    let one_blank_citation = call(
        &state,
        McpScope::Full,
        "record_financial_facts",
        &json!({
            "companyId": company_id,
            "reportDocumentId": "x",
            "period": { "fiscalYear": 2025, "periodType": "FY" },
            "facts": [
                { "metricKey": "net_profit", "valueNumeric": "1", "citation": "p.1" },
                { "metricKey": "revenue", "valueNumeric": "2", "citation": "  " }
            ]
        }),
    )
    .expect("domain outcome, not a protocol error");
    assert_eq!(
        failure_code(one_blank_citation),
        CommandErrorCode::ProvenanceRequired,
        "one blank citation must refuse the whole batch before any write"
    );
}

// ---- M4 unclassified-filings triage pair (ADR 0088 dec. 4) -------------

/// Seed a matched 'Official report' feed item with NO `company_signals` row —
/// an unclassified filing — and return its id.
fn seed_unclassified_official_filing(state: &AppState, company_id: &str) -> String {
    let connection = state.checkout_for_tests().expect("connection");
    let feed_item_id = "feed_unclassified_1";
    connection
        .execute(
            "
                INSERT INTO feed_items (
                    id, type, source_adapter_id, source_name, source_url, title,
                    summary, language, published_at, fetched_at, dedupe_key,
                    attribution, display_company
                ) VALUES (?1, 'Official report', 'bankier-company-komunikaty',
                          'Bankier ESPI', 'https://example.test/espi/1',
                          'Zawiadomienie o zmianie adresu Spółki', '', 'pl',
                          '2026-05-30T09:00:00Z', '2026-05-30T09:30:00Z',
                          'espi:unclassified:1', 'Bankier', 'GPW:TST')
                ",
            rusqlite::params![feed_item_id],
        )
        .expect("official filing inserts");
    connection
        .execute(
            "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                 VALUES (?1, ?2, 'ticker')",
            rusqlite::params![feed_item_id, company_id],
        )
        .expect("company match inserts");
    feed_item_id.to_owned()
}

/// Both triage tools are listed; the read tool returns the seeded
/// unclassified filing; classify_filing is rejected `writes_disabled` when
/// the toggle is OFF (default), before any signal is written.
#[test]
fn unclassified_triage_pair_is_exposed_read_and_gated_act() {
    let state = act_state();
    let (company_id, _, _) = seed_company_framework(&state);
    let feed_item_id = seed_unclassified_official_filing(&state, &company_id);

    let listed: BTreeSet<String> = descriptors(McpScope::Full)
        .as_array()
        .expect("tools array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name").to_owned())
        .collect();
    assert!(listed.contains("list_unclassified_filings"));
    assert!(listed.contains("classify_filing"));

    // The read tool returns the seeded filing.
    let read = call(
        &state,
        McpScope::Full,
        "list_unclassified_filings",
        &json!({}),
    )
    .expect("domain outcome");
    let payload = match read {
        ToolOutcome::Success(value) => value,
        other => panic!("read failed: {other:?}"),
    };
    let text = payload.to_string();
    assert!(
        text.contains(&feed_item_id) && text.contains("zmianie adresu"),
        "seeded unclassified filing present: {text}"
    );

    // classify_filing is writes-gated OFF (the default), before any write.
    let off = call(
        &state,
        McpScope::Full,
        "classify_filing",
        &json!({ "feedItemId": feed_item_id, "category": "significant_contract" }),
    )
    .expect("domain outcome");
    assert_eq!(failure_code(off), CommandErrorCode::WritesDisabled);
    // Nothing was written.
    assert!(state
        .list_company_signals(crate::storage::CompanySignalListInput {
            company_id: Some(company_id),
            watchlist_id: None,
            category: None,
            status: None,
        })
        .expect("signals")
        .is_empty());
}

#[test]
fn the_acquisition_scope_lists_exactly_its_allowlist() {
    // ADR 0099 dec. 3 — both directions: the scoped surface IS the
    // allowlist (complete at nine since #386, ten since #399 S4 / ADR
    // 0101), and every allowlisted name must be an exposed tool (a
    // typo'd entry would silently vanish).
    assert_eq!(
        KPI_ACQUISITION_TOOLS.len(),
        10,
        "the acquisition allowlist grew to ten with propose_kpi_definition (ADR 0101)"
    );
    assert!(KPI_ACQUISITION_TOOLS.contains(&"propose_kpi_definition"));
    let scoped = descriptors(McpScope::KpiAcquisition);
    let names: Vec<&str> = scoped
        .as_array()
        .expect("array")
        .iter()
        .map(|tool| tool["name"].as_str().expect("name"))
        .collect();
    assert_eq!(names, KPI_ACQUISITION_TOOLS.to_vec());

    let exposed: Vec<&str> = entries()
        .into_iter()
        .filter(|entry| entry.exposed)
        .map(|entry| entry.tool_name)
        .collect();
    for name in KPI_ACQUISITION_TOOLS {
        assert!(
            exposed.contains(name),
            "allowlisted tool {name} is not an exposed registry entry"
        );
    }
}

#[test]
fn a_full_only_tool_is_unknown_to_the_acquisition_scope() {
    // ADR 0099 dec. 3: outside the allowlist the surface does not exist
    // for that identity — the same UnknownTool/-32602 as a typo, never a
    // permission-flavored error that confirms the tool exists.
    let state =
        crate::storage::AppState::new(crate::storage::open_in_memory_database().expect("db"));
    let error = call(
        &state,
        McpScope::KpiAcquisition,
        "list_companies",
        &json!({}),
    )
    .expect_err("out-of-scope must be unknown");
    assert!(matches!(error, ToolCallError::UnknownTool(name) if name == "list_companies"));
}
