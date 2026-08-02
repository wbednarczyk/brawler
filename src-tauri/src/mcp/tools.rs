//! The four read MCP tools' domain logic, strict input types, and
//! schemars-generated JSON Schemas (ADR 0078 decision 5, ADR 0088 decision 1).
//!
//! Every tool binds directly to existing `AppState` read models / extracted
//! pure command helpers — never SQL, files, or Tauri internals (ADR 0039).
//! Inputs are strict `#[serde(deny_unknown_fields)]` structs; the JSON Schemas
//! returned by `tools/list` are **generated from these serde types** by
//! `schemars` ([`tool_schema`]) — never hand-written (ADR 0088 dec. 1) — and the
//! insta snapshot of that response is the **frozen tool contract** (ADR 0078
//! G-1). The registry ([`super::registry`]) owns tool naming, tier, and
//! dispatch; this module owns the schemas, inputs, and handlers. Tool failures
//! map onto the ADR 0070 `CommandError` code set. Outputs are sourced facts,
//! coverage, scorecards, and the user's own research — decision support only;
//! no buy/sell/hold phrasing (ADR 0042).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::commands::fundamentals_coverage::{compute_fundamentals_coverage, FundamentalsCoverage};
use crate::storage::{
    ClaimsToVerify, Company, CompanyLookupInput, CriterionResult, FrameworkEvaluation,
    ListFinancialFactsInput, ListFinancialPeriodsInput, ListFrameworkEvaluationsInput, SearchMatch,
};

// ============================================================================
// Tool inputs (strict: unknown fields are rejected; camelCase on the wire)
// ============================================================================

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GetCompanyDossierInput {
    /// Qualified ticker, e.g. "GPW:CDR". A bare ticker is accepted when unambiguous.
    pub company: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchResearchInput {
    /// Full-text query over the user's research: notes, reports, transcripts, claims, and facts.
    pub query: String,
    /// Optional qualified ticker to scope the search to one company.
    #[serde(default)]
    pub company: Option<String>,
    /// Maximum number of matches to return (default 50).
    #[serde(default)]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ListClaimsDueInput {
    /// Optional qualified ticker; when omitted, every tracked company with open claims is included.
    #[serde(default)]
    pub company: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GetQualityAssessmentInput {
    /// Qualified ticker, e.g. "GPW:CDR". A bare ticker is accepted when unambiguous.
    pub company: String,
}

// ============================================================================
// Generated JSON Schemas (the tools/list contract, ADR 0088 dec. 1 / 0078 G-1)
// ============================================================================

/// Generate a tool's `inputSchema` from its serde type via `schemars`. The
/// draft-07 root schema is trimmed of its `$schema`/`title` meta keys (the MCP
/// `inputSchema` slot wants a bare object schema), and `Option<T>` fields keep
/// `T`'s schema (optional-by-absence) rather than gaining a `null` type —
/// `#[serde(deny_unknown_fields)]` still yields `additionalProperties: false`.
/// The insta snapshot of `tools/list` freezes whatever this produces (ADR 0078
/// G-1): regeneration is a reviewed spec change.
pub fn tool_schema<T: JsonSchema>() -> Value {
    let settings = schemars::generate::SchemaSettings::draft07();
    let root = settings.into_generator().into_root_schema_for::<T>();
    let mut value = serde_json::to_value(root).expect("schema serializes to JSON");
    if let Some(object) = value.as_object_mut() {
        object.remove("$schema");
        object.remove("title");
        // A struct-level doc comment lands as a root `description`; the tool
        // already carries a top-level description in `tools/list`, so drop the
        // duplicate at the schema root (nested field descriptions are kept).
        object.remove("description");
    }
    normalize_generated_schema(&mut value);
    value
}

/// Recursively bring schemars' raw output onto the shape the tool contract
/// promises. Four normalizations, each one a contract property rather than a
/// cosmetic preference:
///
/// - **`"default": null` is dropped.** `#[serde(default)]` on an `Option` field
///   makes schemars advertise a null default — pure noise on an external wire
///   contract.
/// - **`"null"` is removed from `type` unions.** An optional input is
///   optional-by-absence: schemars 1.x makes every `Option<T>` nullable, which
///   would invite agents to send `"company": null` instead of omitting the key.
///   (0.8 expressed this as the `option_add_null_type` setting, which 1.x
///   dropped — hence the explicit pass.)
/// - **`required` is sorted.** JSON Schema treats it as a set, and schemars 1.x
///   emits declaration order, so reordering two struct fields would otherwise
///   churn the frozen snapshot for no wire-visible reason.
/// - **`description` is unwrapped to one line.** schemars 1.x keeps the doc
///   comment's newlines, which are rustfmt's wrapping, not authored structure —
///   without this, re-wrapping a comment would edit the external contract.
fn normalize_generated_schema(value: &mut Value) {
    match value {
        Value::Object(map) => {
            if map.get("default") == Some(&Value::Null) {
                map.remove("default");
            }
            if let Some(Value::String(description)) = map.get_mut("description") {
                if description.contains('\n') {
                    *description = description.split_whitespace().collect::<Vec<_>>().join(" ");
                }
            }
            if let Some(Value::Array(types)) = map.get_mut("type") {
                // Guard the union case only: a lone `"type": "null"` stays as-is
                // rather than collapsing into an empty, meaningless array.
                if types.len() > 1 {
                    types.retain(|entry| entry != "null");
                    if let [single] = types.as_slice() {
                        let single = single.clone();
                        map.insert("type".into(), single);
                    }
                }
            }
            if let Some(Value::Array(required)) = map.get_mut("required") {
                required.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
            }
            for child in map.values_mut() {
                normalize_generated_schema(child);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(normalize_generated_schema),
        _ => {}
    }
}

// ============================================================================
// Tool call surface
// ============================================================================

/// Protocol-level failures of a `tools/call` — both map to JSON-RPC −32602.
#[derive(Debug, PartialEq, Eq)]
pub enum ToolCallError {
    UnknownTool(String),
    InvalidArguments(String),
}

/// Domain-level outcome of a tool run. `Failure` becomes an MCP tool result
/// with `isError: true`, carrying the ADR 0070 error envelope as text.
#[derive(Debug)]
pub enum ToolOutcome {
    Success(Value),
    Failure(CommandError),
}

// ---- Per-tool handlers (wired into the registry, ADR 0088 dec. 1) ----------
//
// Each handler deserializes the strict input, runs the read tool, and serializes
// the payload. The registry ([`super::registry`]) maps a tool name to one of
// these; `protocol.rs` never matches on tool names directly. Read paths only —
// a mutating tool requires the `act` tier + provenance enforcement (ADR 0088).

pub fn get_company_dossier_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_company_dossier(state, input))
}

pub fn search_research_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| search_research(state, input))
}

pub fn list_claims_due_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| list_claims_due(state, input))
}

pub fn get_quality_assessment_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_quality_assessment(state, input))
}

/// Deserialize the strict input (unknown fields ⇒ −32602 via
/// `InvalidArguments`), run the tool, serialize the payload. Shared with the
/// broad read-wave handlers in [`super::reads`] (ADR 0088 dec. 2).
pub(super) fn run<I, T>(
    arguments: &Value,
    tool: impl FnOnce(I) -> Result<T, CommandError>,
) -> Result<ToolOutcome, ToolCallError>
where
    I: DeserializeOwned,
    T: Serialize,
{
    let input: I = serde_json::from_value(arguments.clone())
        .map_err(|error| ToolCallError::InvalidArguments(format!("invalid arguments: {error}")))?;
    Ok(match tool(input) {
        Ok(payload) => match serde_json::to_value(payload) {
            Ok(value) => ToolOutcome::Success(envelope(value)),
            Err(error) => ToolOutcome::Failure(CommandError::new(
                CommandErrorCode::Internal,
                format!("tool output serialization failed: {error}"),
            )),
        },
        Err(error) => ToolOutcome::Failure(error),
    })
}

/// MCP requires `structuredContent` to be a JSON OBJECT; a bare array (every
/// command returning `Vec<T>`) fails strict client-side schema validation
/// (issue #249 — a deliberate ADR 0088 contract amendment, this is the single
/// choke point every handler serializes through). Arrays are wrapped as
/// `{ "items": [...] }`; scalars (a bare unit/bool/number/string result) as
/// `{ "result": ... }`; objects pass through verbatim.
fn envelope(value: Value) -> Value {
    match value {
        Value::Array(items) => serde_json::json!({ "items": items }),
        Value::Object(_) => value,
        scalar => serde_json::json!({ "result": scalar }),
    }
}

// ============================================================================
// Company resolution (shared by every tool taking a `company` param)
// ============================================================================

/// Resolve a qualified ticker (`GPW:CDR`) — or a bare ticker when unambiguous —
/// to a tracked company. Unknown ⇒ `not_found`; ambiguous ⇒ `invalid_input`.
/// Shared with the broad read-wave handlers in [`super::reads`].
pub(super) fn resolve_company(state: &AppState, reference: &str) -> Result<Company, CommandError> {
    let trimmed = reference.trim();
    if trimmed.is_empty() {
        return Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            "company must be a non-empty qualified ticker, e.g. \"GPW:CDR\"",
        ));
    }
    let needle = trimmed.to_uppercase();
    let companies = state.list_companies().map_err(CommandError::from)?;
    let matches: Vec<&Company> = if needle.contains(':') {
        companies
            .iter()
            .filter(|company| company.qualified_ticker == needle)
            .collect()
    } else {
        companies
            .iter()
            .filter(|company| company.ticker == needle)
            .collect()
    };
    match matches.as_slice() {
        [company] => Ok((*company).clone()),
        [] => Err(company_not_found(state, &needle)),
        _ => Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            format!("ticker {needle} is ambiguous — use a qualified ticker like \"GPW:{needle}\""),
        )),
    }
}

/// Build the `not_found` envelope, consulting the `lookup_company` registry to
/// distinguish "known company, just not tracked here" from "never heard of it".
fn company_not_found(state: &AppState, needle: &str) -> CommandError {
    let (exchange, ticker) = needle.split_once(':').unwrap_or(("GPW", needle));
    let registry_hit = state
        .lookup_company(CompanyLookupInput {
            exchange: exchange.to_owned(),
            ticker: Some(ticker.to_owned()),
            display_name: None,
            isin: None,
        })
        .ok()
        .flatten();
    let message = match registry_hit {
        Some(hit) => format!(
            "company {} ({}) is known but not tracked in this workspace",
            hit.qualified_ticker, hit.display_name
        ),
        None => format!("unknown company: {needle}"),
    };
    CommandError::new(CommandErrorCode::NotFound, message)
}

// ============================================================================
// get_company_dossier
// ============================================================================

/// Cap on the confirmed-facts slice so a long-tracked company cannot blow the
/// response up; newest periods dominate because facts list period-grouped.
const CONFIRMED_FACTS_LIMIT: usize = 100;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyDossier {
    company: Company,
    fundamentals_coverage: FundamentalsCoverage,
    /// Slice of facts with `confirmation_state = confirmed` (max
    /// [`CONFIRMED_FACTS_LIMIT`]), each tagged with its fiscal period.
    confirmed_facts: Vec<ConfirmedFact>,
    /// Latest quality-framework evaluation per framework (verdict counts from
    /// the `fundamentals/scorecard.rs` engine, read from the stored snapshot).
    scorecards: Vec<ScorecardSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ConfirmedFact {
    metric: String,
    fiscal_year: Option<i64>,
    period_type: Option<String>,
    value: String,
    currency: Option<String>,
    measure_window: String,
    data_quality: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScorecardSummary {
    framework_id: String,
    framework_name: String,
    evaluated_at: String,
    pass: i64,
    partial: i64,
    fail: i64,
    unavailable: i64,
    engine_version: String,
}

fn get_company_dossier(
    state: &AppState,
    input: GetCompanyDossierInput,
) -> Result<CompanyDossier, CommandError> {
    let company = resolve_company(state, &input.company)?;
    let fundamentals_coverage = compute_fundamentals_coverage(state, &company.id)
        .map_err(|message| CommandError::new(CommandErrorCode::Internal, message))?;

    let periods: BTreeMap<String, (i64, String)> = state
        .list_financial_periods(ListFinancialPeriodsInput {
            company_id: company.id.clone(),
            fiscal_year: None,
        })
        .map_err(CommandError::from)?
        .into_iter()
        .map(|period| (period.id, (period.fiscal_year, period.period_type)))
        .collect();

    let confirmed_facts = state
        .list_financial_facts(ListFinancialFactsInput {
            company_id: Some(company.id.clone()),
            period_id: None,
            definition_id: None,
        })
        .map_err(CommandError::from)?
        .into_iter()
        .filter(|fact| fact.confirmation_state == "confirmed")
        .take(CONFIRMED_FACTS_LIMIT)
        .map(|fact| {
            let period = periods.get(&fact.period_id);
            ConfirmedFact {
                metric: fact.definition_id,
                fiscal_year: period.map(|(year, _)| *year),
                period_type: period.map(|(_, period_type)| period_type.clone()),
                value: fact.value_numeric,
                currency: fact.currency,
                measure_window: fact.measure_window,
                data_quality: fact.data_quality,
            }
        })
        .collect();

    let scorecards = scorecard_summaries(state, &company.id)?;

    Ok(CompanyDossier {
        company,
        fundamentals_coverage,
        confirmed_facts,
        scorecards,
    })
}

/// The latest stored evaluation per framework — the persisted output of the
/// `fundamentals/scorecard.rs` engine (its `VerdictCounts` tally), read rather
/// than recomputed.
fn scorecard_summaries(
    state: &AppState,
    company_id: &str,
) -> Result<Vec<ScorecardSummary>, CommandError> {
    let mut summaries = Vec::new();
    for framework in state
        .list_quality_frameworks()
        .map_err(CommandError::from)?
    {
        let evaluations = state
            .list_framework_evaluations(ListFrameworkEvaluationsInput {
                framework_id: framework.id.clone(),
                company_id: company_id.to_owned(),
            })
            .map_err(CommandError::from)?;
        // The store orders newest-first.
        if let Some(latest) = evaluations.into_iter().next() {
            summaries.push(ScorecardSummary {
                framework_id: framework.id,
                framework_name: framework.name,
                evaluated_at: latest.created_at,
                pass: latest.pass_count,
                partial: latest.partial_count,
                fail: latest.fail_count,
                unavailable: latest.unavailable_count,
                engine_version: latest.engine_version,
            });
        }
    }
    Ok(summaries)
}

// ============================================================================
// search_research
// ============================================================================

const DEFAULT_SEARCH_LIMIT: i64 = 50;
const MAX_SEARCH_LIMIT: i64 = 200;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResearchOutput {
    query: String,
    /// The qualified ticker the search was scoped to, when a scope was given.
    company: Option<String>,
    matches: Vec<SearchMatch>,
}

fn search_research(
    state: &AppState,
    input: SearchResearchInput,
) -> Result<SearchResearchOutput, CommandError> {
    if input.query.trim().is_empty() {
        return Err(CommandError::new(
            CommandErrorCode::InvalidInput,
            "query must not be empty",
        ));
    }
    let company = input
        .company
        .as_deref()
        .map(|reference| resolve_company(state, reference))
        .transpose()?;
    let limit = input
        .limit
        .unwrap_or(DEFAULT_SEARCH_LIMIT)
        .clamp(1, MAX_SEARCH_LIMIT);
    let matches = state
        .search(
            &input.query,
            &[],
            company.as_ref().map(|company| company.id.as_str()),
            limit,
        )
        .map_err(CommandError::from)?;
    Ok(SearchResearchOutput {
        query: input.query,
        company: company.map(|company| company.qualified_ticker),
        matches,
    })
}

// ============================================================================
// list_claims_due
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ListClaimsDueOutput {
    companies: Vec<CompanyClaimsDue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyClaimsDue {
    company: String,
    display_name: String,
    claims: ClaimsToVerify,
}

fn list_claims_due(
    state: &AppState,
    input: ListClaimsDueInput,
) -> Result<ListClaimsDueOutput, CommandError> {
    let (companies, scoped) = match input.company.as_deref() {
        Some(reference) => (vec![resolve_company(state, reference)?], true),
        None => (state.list_companies().map_err(CommandError::from)?, false),
    };
    let mut out = Vec::new();
    for company in companies {
        let claims = state
            .list_claims_to_verify(&company.id)
            .map_err(CommandError::from)?;
        let has_claims =
            !(claims.due.is_empty() && claims.overdue.is_empty() && claims.upcoming.is_empty());
        // A scoped call always answers (empty buckets are the answer); the
        // global sweep only lists companies that actually have open claims.
        if scoped || has_claims {
            out.push(CompanyClaimsDue {
                company: company.qualified_ticker,
                display_name: company.display_name,
                claims,
            });
        }
    }
    Ok(ListClaimsDueOutput { companies: out })
}

// ============================================================================
// get_quality_assessment
// ============================================================================

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct QualityAssessmentOutput {
    company: String,
    display_name: String,
    frameworks: Vec<FrameworkQuality>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameworkQuality {
    framework_id: String,
    framework_name: String,
    /// The newest stored scorecard evaluation (quantitative engine run), if any.
    latest_evaluation: Option<FrameworkEvaluation>,
    /// Current-state qualitative read (ADR 0075): per criterion, the
    /// most-recent agent-assessed result across all snapshots.
    qualitative: Vec<CriterionResult>,
}

fn get_quality_assessment(
    state: &AppState,
    input: GetQualityAssessmentInput,
) -> Result<QualityAssessmentOutput, CommandError> {
    let company = resolve_company(state, &input.company)?;
    let mut frameworks = Vec::new();
    for framework in state
        .list_quality_frameworks()
        .map_err(CommandError::from)?
    {
        let latest_evaluation = state
            .list_framework_evaluations(ListFrameworkEvaluationsInput {
                framework_id: framework.id.clone(),
                company_id: company.id.clone(),
            })
            .map_err(CommandError::from)?
            .into_iter()
            .next();
        let qualitative = state
            .get_qualitative_assessment(&framework.id, &company.id)
            .map_err(CommandError::from)?;
        // Only frameworks with any recorded state for this company answer;
        // never-evaluated frameworks would be noise.
        if latest_evaluation.is_some() || !qualitative.is_empty() {
            frameworks.push(FrameworkQuality {
                framework_id: framework.id,
                framework_name: framework.name,
                latest_evaluation,
                qualitative,
            });
        }
    }
    Ok(QualityAssessmentOutput {
        company: company.qualified_ticker,
        display_name: company.display_name,
        frameworks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    /// Base valid argument object per tool, used to prove that ONE injected
    /// unknown field flips deserialization to an error.
    const KNOWN_FIELDS: &[&str] = &["company", "query", "limit"];

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(64))]

        /// deny_unknown_fields on every tool input: a valid payload plus one
        /// arbitrary unknown key must be rejected (ADR 0078 decision 5).
        #[test]
        fn tool_inputs_reject_unknown_fields(
            key in "[a-zA-Z][a-zA-Z0-9_]{0,16}",
            value in any::<i64>(),
        ) {
            prop_assume!(!KNOWN_FIELDS.contains(&key.as_str()));

            let mut dossier = json!({"company": "GPW:TST"});
            dossier[&key] = json!(value);
            prop_assert!(serde_json::from_value::<GetCompanyDossierInput>(dossier).is_err());

            let mut search = json!({"query": "esef", "company": "GPW:TST", "limit": 5});
            search[&key] = json!(value);
            prop_assert!(serde_json::from_value::<SearchResearchInput>(search).is_err());

            let mut claims = json!({"company": "GPW:TST"});
            claims[&key] = json!(value);
            prop_assert!(serde_json::from_value::<ListClaimsDueInput>(claims).is_err());

            let mut quality = json!({"company": "GPW:TST"});
            quality[&key] = json!(value);
            prop_assert!(serde_json::from_value::<GetQualityAssessmentInput>(quality).is_err());
        }
    }

    #[test]
    fn valid_inputs_deserialize() {
        assert!(
            serde_json::from_value::<GetCompanyDossierInput>(json!({"company":"GPW:TST"})).is_ok()
        );
        assert!(
            serde_json::from_value::<SearchResearchInput>(json!({"query":"backlog"})).is_ok(),
            "company and limit are optional"
        );
        assert!(serde_json::from_value::<ListClaimsDueInput>(json!({})).is_ok());
        assert!(
            serde_json::from_value::<GetQualityAssessmentInput>(json!({"company":"GPW:TST"}))
                .is_ok()
        );
    }

    /// The four normalizations `tool_schema` applies to schemars' raw output.
    /// The `tools/list` snapshot (ADR 0078 G-1) would also redden if one of
    /// these regressed, but only as an opaque 77 KB diff — this names the
    /// property that broke. Each was a real difference in schemars 1.x.
    #[test]
    fn generated_schemas_are_normalized_onto_the_wire_contract() {
        // An optional input is optional-by-absence: no `null` in the union, and
        // no `"default": null` riding along with `#[serde(default)]`.
        let search = tool_schema::<SearchResearchInput>();
        assert_eq!(
            search["properties"]["company"]["type"],
            json!("string"),
            "Option<String> must stay a plain string, not become nullable: {search}"
        );
        assert!(
            search["properties"]["company"].get("default").is_none(),
            "a null default must not reach the wire contract: {search}"
        );

        // `required` is a set — emit it sorted so reordering struct fields does
        // not churn the frozen contract.
        let dossier = tool_schema::<GetCompanyDossierInput>();
        let required = dossier["required"].as_array().expect("required array");
        let mut sorted = required.clone();
        sorted.sort_by(|left, right| left.as_str().cmp(&right.as_str()));
        assert_eq!(
            required, &sorted,
            "required must be emitted sorted: {dossier}"
        );

        // Descriptions carry no rustfmt line wrapping — otherwise re-wrapping a
        // doc comment would edit the external contract.
        let mut descriptions = Vec::new();
        collect_descriptions(&search, &mut descriptions);
        assert!(
            !descriptions.is_empty(),
            "the fixture must carry descriptions"
        );
        for description in descriptions {
            assert!(
                !description.contains('\n'),
                "description must be unwrapped to one line: {description:?}"
            );
        }
    }

    fn collect_descriptions(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(Value::String(description)) = map.get("description") {
                    out.push(description.clone());
                }
                map.values()
                    .for_each(|child| collect_descriptions(child, out));
            }
            Value::Array(items) => items
                .iter()
                .for_each(|item| collect_descriptions(item, out)),
            _ => {}
        }
    }

    #[test]
    fn unknown_company_maps_to_not_found() {
        use crate::commands::error::CommandErrorCode;
        use crate::storage::open_in_memory_database;

        let state = crate::storage::AppState::new(open_in_memory_database().expect("db"));
        let outcome = super::super::registry::call(
            &state,
            "get_company_dossier",
            &json!({"company": "GPW:NOPE"}),
        )
        .expect("a domain outcome, not a protocol error");
        match outcome {
            ToolOutcome::Failure(error) => assert_eq!(error.code, CommandErrorCode::NotFound),
            ToolOutcome::Success(v) => panic!("unknown company must not succeed: {v}"),
        }
    }
}
