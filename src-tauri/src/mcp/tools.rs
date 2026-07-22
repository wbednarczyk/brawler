//! The four read-only MCP tools (ADR 0078 decision 5).
//!
//! Every tool binds directly to existing `AppState` read models / extracted
//! pure command helpers — never SQL, files, or Tauri internals (ADR 0039).
//! Inputs are strict `#[serde(deny_unknown_fields)]` structs; the JSON Schemas
//! returned by `tools/list` are hand-written constants, and the insta snapshot
//! of that response is the **frozen tool contract** (ADR 0078 G-1). Tool
//! failures map onto the ADR 0070 `CommandError` code set. Outputs are sourced
//! facts, coverage, scorecards, and the user's own research — decision support
//! only; no buy/sell/hold phrasing (ADR 0042).

use std::collections::BTreeMap;

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
// Tool inputs (strict: unknown fields are rejected)
// ============================================================================

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GetCompanyDossierInput {
    /// Qualified ticker, e.g. `GPW:CDR` (a bare ticker is accepted when unambiguous).
    pub company: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SearchResearchInput {
    pub query: String,
    #[serde(default)]
    pub company: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ListClaimsDueInput {
    #[serde(default)]
    pub company: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct GetQualityAssessmentInput {
    pub company: String,
}

// ============================================================================
// Hand-written JSON Schemas (the tools/list contract, ADR 0078 G-1)
// ============================================================================

const GET_COMPANY_DOSSIER_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "company": {
      "type": "string",
      "description": "Qualified ticker, e.g. \"GPW:CDR\". A bare ticker is accepted when unambiguous."
    }
  },
  "required": ["company"],
  "additionalProperties": false
}"#;

const SEARCH_RESEARCH_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "query": {
      "type": "string",
      "description": "Full-text query over the user's research: notes, reports, transcripts, claims, and facts."
    },
    "company": {
      "type": "string",
      "description": "Optional qualified ticker to scope the search to one company."
    },
    "limit": {
      "type": "integer",
      "minimum": 1,
      "maximum": 200,
      "description": "Maximum number of matches to return (default 50)."
    }
  },
  "required": ["query"],
  "additionalProperties": false
}"#;

const LIST_CLAIMS_DUE_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "company": {
      "type": "string",
      "description": "Optional qualified ticker; when omitted, every tracked company with open claims is included."
    }
  },
  "additionalProperties": false
}"#;

const GET_QUALITY_ASSESSMENT_SCHEMA: &str = r#"{
  "type": "object",
  "properties": {
    "company": {
      "type": "string",
      "description": "Qualified ticker, e.g. \"GPW:CDR\". A bare ticker is accepted when unambiguous."
    }
  },
  "required": ["company"],
  "additionalProperties": false
}"#;

/// The `tools` array of the `tools/list` response (hand-written schemas). The
/// insta snapshot of the full response freezes this contract — changing the
/// tool set or any shape is a reviewed spec change (ADR 0078 G-1).
pub fn descriptors() -> Value {
    let schema = |raw: &str| -> Value {
        serde_json::from_str(raw).expect("hand-written tool schema is valid JSON")
    };
    Value::Array(vec![
        serde_json::json!({
            "name": "get_company_dossier",
            "description": "One company's research dossier: identity, fundamentals coverage per fiscal period, confirmed financial facts, and quality-scorecard summaries. Sourced from the user's own research; decision support only.",
            "inputSchema": schema(GET_COMPANY_DOSSIER_SCHEMA),
        }),
        serde_json::json!({
            "name": "search_research",
            "description": "Full-text search across the user's research workspace (notes, report documents, transcripts, claims, facts). Returns ranked matches with snippets.",
            "inputSchema": schema(SEARCH_RESEARCH_SCHEMA),
        }),
        serde_json::json!({
            "name": "list_claims_due",
            "description": "Management claims whose verification period has arrived (due), passed (overdue), or is approaching (upcoming), per company.",
            "inputSchema": schema(LIST_CLAIMS_DUE_SCHEMA),
        }),
        serde_json::json!({
            "name": "get_quality_assessment",
            "description": "Quality-framework state for one company: the latest stored scorecard evaluation per framework, plus previously-stored qualitative verdicts. The in-app qualitative-assessment writer was retired (ADR 0084) — qualitative criteria are recorded manually now, and this tool reads only stored verdicts (new criteria stay empty until the planned MCP write-tools). Decision support only — never an investment recommendation.",
            "inputSchema": schema(GET_QUALITY_ASSESSMENT_SCHEMA),
        }),
    ])
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

/// Run one tool by name over the domain state (read paths only — ADR 0078 G-2).
pub fn call(state: &AppState, name: &str, arguments: &Value) -> Result<ToolOutcome, ToolCallError> {
    match name {
        "get_company_dossier" => run(arguments, |input| get_company_dossier(state, input)),
        "search_research" => run(arguments, |input| search_research(state, input)),
        "list_claims_due" => run(arguments, |input| list_claims_due(state, input)),
        "get_quality_assessment" => run(arguments, |input| get_quality_assessment(state, input)),
        other => Err(ToolCallError::UnknownTool(other.to_owned())),
    }
}

/// Deserialize the strict input (unknown fields ⇒ −32602 via
/// `InvalidArguments`), run the tool, serialize the payload.
fn run<I, T>(
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
            Ok(value) => ToolOutcome::Success(value),
            Err(error) => ToolOutcome::Failure(CommandError::new(
                CommandErrorCode::Internal,
                format!("tool output serialization failed: {error}"),
            )),
        },
        Err(error) => ToolOutcome::Failure(error),
    })
}

// ============================================================================
// Company resolution (shared by every tool taking a `company` param)
// ============================================================================

/// Resolve a qualified ticker (`GPW:CDR`) — or a bare ticker when unambiguous —
/// to a tracked company. Unknown ⇒ `not_found`; ambiguous ⇒ `invalid_input`.
fn resolve_company(state: &AppState, reference: &str) -> Result<Company, CommandError> {
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

    #[test]
    fn unknown_company_maps_to_not_found() {
        use crate::commands::error::CommandErrorCode;
        use crate::storage::open_in_memory_database;

        let state = crate::storage::AppState::new(open_in_memory_database().expect("db"));
        let outcome = call(
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
