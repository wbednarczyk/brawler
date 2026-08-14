//! The broad UI-parity **read wave** (ADR 0088 dec. 2 `read` tier): one exposed
//! MCP tool per domain read, so a connected agent can READ every domain the user
//! sees in the UI — companies/watchlists, feed, signals, facts (with
//! provenance), periods, quotes, ownership, insiders, analyst recommendations,
//! health/red flags, report documents + diffs, transcripts, notes, claims,
//! expectations, journal, research questions, report season, calendar events,
//! attention, briefing, autopilot runs, and quality frameworks.
//!
//! Every handler binds to an existing `AppState` read model or an extracted pure
//! command helper (`compute_*`) — never SQL, files, or Tauri internals (ADR
//! 0039). Company-scoped tools take a qualified ticker (`GPW:CDR`), resolved
//! through [`super::tools::resolve_company`], instead of the internal id, so the
//! MCP surface speaks the user's vocabulary. Strict inputs
//! (`deny_unknown_fields`, camelCase) drive schemars-generated schemas frozen by
//! the `tools/list` snapshot. Outputs are the domain read models verbatim —
//! decision support only, no buy/sell/hold phrasing (ADR 0042).
//!
//! The single composition allowed in this layer (ADR 0088 dec. 2, item 4) is
//! [`list_financial_facts_with_provenance`]: each fact carries its trust-ladder
//! `sourceTier`, `validationStatus`, and `citation`, joined from
//! `fundamentals_provenance().get_many`.

use std::collections::HashMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::tools::{resolve_company, run, ToolCallError, ToolOutcome};
use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::commands::report_diff::GetReportDiffInput as CmdGetReportDiffInput;
use crate::storage::{
    AttentionEventListInput, CompanyEventListInput, CompanySignalListInput, DecisionEntryListInput,
    FinancialFact, ListAutopilotRunsInput, ListFinancialFactsInput, ListFinancialPeriodsInput,
    ListKpiDefinitionsInput, ListReportExpectationsInput, ReportSeasonInput,
    TranscriptJobListInput,
};

// ============================================================================
// Shared inputs
// ============================================================================

/// No arguments — a global domain list. `deny_unknown_fields` still rejects junk.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NoInput {}

/// A single required company (qualified ticker, e.g. `GPW:CDR`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct CompanyRef {
    /// Qualified ticker, e.g. "GPW:CDR". A bare ticker is accepted when unambiguous.
    pub company: String,
}

/// An optional company scope; absent ⇒ every tracked company.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OptionalCompanyRef {
    /// Optional qualified ticker to scope the result to one company.
    #[serde(default)]
    pub company: Option<String>,
}

fn internal(message: impl std::fmt::Display) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, message.to_string())
}

/// Resolve an optional qualified ticker to its internal id (`None` stays `None`).
fn optional_company_id(
    state: &AppState,
    company: &Option<String>,
) -> Result<Option<String>, CommandError> {
    company
        .as_deref()
        .map(|reference| resolve_company(state, reference).map(|company| company.id))
        .transpose()
}

// ============================================================================
// Companies / watchlists
// ============================================================================

pub fn list_companies_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state.list_companies().map_err(CommandError::from)
    })
}

pub fn get_company_basic_info_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::companies::compute_company_basic_info(state, &company.id).map_err(internal)
    })
}

pub fn list_alert_rules_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .attention()
            .list_alert_rules()
            .map_err(CommandError::from)
    })
}

pub fn list_watchlists_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .watchlists()
            .list_watchlists()
            .map_err(CommandError::from)
    })
}

pub fn list_watchlist_memberships_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .watchlists()
            .list_watchlist_memberships()
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Feed / signals / events
// ============================================================================

pub fn list_feed_items_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state.list_feed_items().map_err(CommandError::from)
    })
}

pub fn list_company_signals_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .list_company_signals(CompanySignalListInput {
                company_id: Some(company.id),
                watchlist_id: None,
                category: None,
                status: None,
            })
            .map_err(CommandError::from)
    })
}

pub fn list_company_events_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .list_company_events(CompanyEventListInput {
                mode: None,
                company_id: Some(company.id),
                watchlist_id: None,
                event_type: None,
                status: None,
                date_from: None,
                date_to: None,
            })
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Financial facts (with provenance) / periods / KPI catalog
// ============================================================================

/// A confirmed-or-otherwise financial fact carrying its trust-ladder provenance
/// (ADR 0088 dec. 2, item 4): where the number came from, whether the pipeline
/// flagged it, and its citation. `null` provenance fields mean the fact has no
/// provenance row (e.g. a legacy or manually-entered fact without one).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FactWithProvenance {
    #[serde(flatten)]
    fact: FinancialFact,
    /// Trust-ladder tier, e.g. `deterministic_esef` / `aggregator` / `manual`.
    source_tier: Option<String>,
    /// `ok` | `flagged` — whether the pipeline flagged a drift/contradiction.
    validation_status: Option<String>,
    /// Free-form source citation, when recorded.
    citation: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompanyFactsWithProvenance {
    company: String,
    facts: Vec<FactWithProvenance>,
}

pub fn list_financial_facts_with_provenance(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        let facts = state
            .list_financial_facts(ListFinancialFactsInput {
                company_id: Some(company.id.clone()),
                period_id: None,
                definition_id: None,
            })
            .map_err(CommandError::from)?;
        let ids: Vec<String> = facts.iter().map(|fact| fact.id.clone()).collect();
        let provenance: HashMap<String, _> = state
            .fundamentals_provenance()
            .get_many(&ids)
            .map_err(CommandError::from)?
            .into_iter()
            .map(|entry| (entry.fact_id.clone(), entry))
            .collect();
        let facts = facts
            .into_iter()
            .map(|fact| {
                let entry = provenance.get(&fact.id);
                FactWithProvenance {
                    source_tier: entry.map(|entry| entry.source_tier.clone()),
                    validation_status: entry.map(|entry| entry.validation_status.clone()),
                    citation: entry.and_then(|entry| entry.citation.clone()),
                    fact,
                }
            })
            .collect();
        Ok::<_, CommandError>(CompanyFactsWithProvenance {
            company: company.qualified_ticker,
            facts,
        })
    })
}

pub fn list_financial_periods_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .list_financial_periods(ListFinancialPeriodsInput {
                company_id: company.id,
                fiscal_year: None,
            })
            .map_err(CommandError::from)
    })
}

pub fn list_kpi_definitions_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .list_kpi_definitions(ListKpiDefinitionsInput {
                scope: None,
                sector: None,
                company_id: None,
            })
            .map_err(CommandError::from)
    })
}

pub fn list_flagged_fact_provenance_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        // App-wide by design: this is the data-quality surface across every
        // tracked company (the per-company scoping lives on the IPC command the
        // Coverage panel calls).
        state
            .fundamentals_provenance()
            .list_flagged_facts(None)
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Quotes / ownership / insiders / analyst recommendations
// ============================================================================

pub fn get_price_context_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::market_data::compute_price_context(state, &company.id).map_err(internal)
    })
}

/// Cross-company KPI comparison over qualified tickers (ADR 0089 dec. 1). Speaks
/// the user's vocabulary (`GPW:CDR`) and resolves to internal ids before the
/// shared read model runs. `granularity` defaults to `annual`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct KpiComparisonRef {
    /// Qualified tickers to compare, e.g. `["GPW:CDR", "GPW:PKN"]` (1..N).
    pub companies: Vec<String>,
    /// Canonical metric keys to align, e.g. `["revenue", "net_profit"]` (1..N).
    pub metric_keys: Vec<String>,
    /// `"annual"` (default) or `"quarterly"`.
    #[serde(default)]
    pub granularity: Option<String>,
}

pub fn get_kpi_comparison_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: KpiComparisonRef| {
        let company_ids = input
            .companies
            .iter()
            .map(|reference| resolve_company(state, reference).map(|company| company.id))
            .collect::<Result<Vec<_>, _>>()?;
        crate::commands::comparison::compute_kpi_comparison(
            state,
            &crate::commands::comparison::KpiComparisonInput {
                company_ids,
                metric_keys: input.metric_keys,
                granularity: input.granularity.unwrap_or_else(|| "annual".to_owned()),
            },
        )
        .map_err(internal)
    })
}

/// Sector percentiles for one company (ADR 0089 dec. 3): where it stands against
/// its tracked sector peers on the level-0 market ratios and selected canonical
/// KPIs. Speaks the user's vocabulary (`GPW:CDR`) and resolves to the internal id.
pub fn get_sector_percentiles_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::sector_percentiles::compute_company_sector_percentiles(state, &company.id)
            .map_err(internal)
    })
}

/// Append-only comparative-valuation run history for one company (ADR 0089
/// dec. 5). Read-tier; the compute-and-persist path is the `act`-tier
/// `compute_comparative_valuation`.
pub fn list_valuation_runs_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .valuation_runs()
            .list_runs(&company.id)
            .map_err(internal)
    })
}

pub fn get_ownership_overview_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::ownership::compute_ownership_overview(state, &company.id).map_err(internal)
    })
}

pub fn get_insider_overview_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::insider::compute_insider_overview(state, &company.id).map_err(internal)
    })
}

pub fn list_short_positions_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .short_positions()
            .short_positions_view(&company.id)
            .map_err(CommandError::from)
    })
}

pub fn get_analyst_recommendations_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::analyst_recommendations::compute_analyst_recommendations(
            state,
            &company.id,
        )
        .map_err(internal)
    })
}

// ============================================================================
// Health / red flags
// ============================================================================

pub fn get_company_health_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::company_health::compute_company_health(state, &company.id)
            .map_err(internal)
    })
}

pub fn get_red_flags_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .red_flags()
            .red_flags_view(&company.id)
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Report documents + diffs
// ============================================================================

pub fn get_report_documents_view_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        crate::commands::report_documents_view::compute_report_documents_view(state, &company.id)
            .map_err(internal)
    })
}

pub fn list_report_diff_candidates_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        let documents = state
            .list_report_documents_by_company(&company.id)
            .map_err(CommandError::from)?;
        crate::commands::report_diff::build_candidates(state, documents).map_err(internal)
    })
}

/// The two report documents to compare (canonical financial statements of
/// successive periods; discover pairs via `list_report_diff_candidates`).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReportDiffRef {
    pub older_report_document_id: String,
    pub newer_report_document_id: String,
}

pub fn get_report_diff_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: ReportDiffRef| {
        crate::commands::report_diff::build_diff(
            state,
            &CmdGetReportDiffInput {
                older_report_document_id: input.older_report_document_id,
                newer_report_document_id: input.newer_report_document_id,
            },
        )
        .map_err(internal)
    })
}

// ============================================================================
// Transcripts
// ============================================================================

pub fn list_video_transcript_jobs_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: OptionalCompanyRef| {
        let company_id = optional_company_id(state, &input.company)?;
        state
            .list_transcript_jobs(TranscriptJobListInput { company_id })
            .map_err(CommandError::from)
    })
}

/// One transcript job's ordered segments.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TranscriptSegmentsRef {
    /// The transcript job id (from `list_video_transcript_jobs`).
    pub transcript_job_id: String,
}

pub fn list_transcript_segments_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: TranscriptSegmentsRef| {
        state
            .list_transcript_segments(&input.transcript_job_id)
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Notes / claims
// ============================================================================

pub fn list_notebook_entries_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .list_notebook_entries(&company.id)
            .map_err(CommandError::from)
    })
}

pub fn list_management_claims_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .list_management_claims(&company.id)
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Expectations / journal / research questions
// ============================================================================

pub fn list_report_expectations_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: OptionalCompanyRef| {
        let company_id = optional_company_id(state, &input.company)?;
        state
            .report_expectations()
            .list_report_expectations(ListReportExpectationsInput { company_id })
            .map_err(CommandError::from)
    })
}

pub fn list_decision_entries_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: OptionalCompanyRef| {
        let company_id = optional_company_id(state, &input.company)?;
        state
            .decision_journal()
            .list_decision_entries(DecisionEntryListInput {
                company_id,
                kind: None,
            })
            .map_err(CommandError::from)
    })
}

pub fn list_research_questions_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .list_research_questions(crate::storage::ResearchQuestionListInput {
                scope_type: None,
                scope_id: None,
                status: None,
            })
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Calendar: report season / attention / briefing / autopilot runs
// ============================================================================

pub fn list_report_season_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .list_report_season(ReportSeasonInput { watchlist_id: None })
            .map_err(CommandError::from)
    })
}

/// Attention events, optionally scoped and optionally including dismissed ones.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AttentionEventsRef {
    #[serde(default)]
    pub company: Option<String>,
    /// Include dismissed events (default: only non-dismissed).
    #[serde(default)]
    pub include_dismissed: bool,
}

pub fn list_attention_events_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: AttentionEventsRef| {
        let company_id = optional_company_id(state, &input.company)?;
        state
            .attention()
            .list_attention_events(AttentionEventListInput {
                company_id,
                include_dismissed: input.include_dismissed,
            })
            .map_err(CommandError::from)
    })
}

pub fn get_latest_morning_briefing_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state
            .morning_briefings()
            .latest_morning_briefing()
            .map_err(CommandError::from)
    })
}

/// Recent autopilot runs, optionally scoped to one company.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AutopilotRunsRef {
    #[serde(default)]
    pub company: Option<String>,
    /// Maximum number of runs to return.
    #[serde(default)]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<i64>,
}

pub fn list_autopilot_runs_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: AutopilotRunsRef| {
        let company_id = optional_company_id(state, &input.company)?;
        state
            .autopilot()
            .list_runs(&ListAutopilotRunsInput {
                company_id,
                notification_state: None,
                limit: input.limit,
            })
            .map_err(CommandError::from)
    })
}

/// One autopilot run's full composed result.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AutopilotRunRef {
    /// The run id (from `list_autopilot_runs`).
    pub run_id: String,
}

pub fn get_autopilot_run_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: AutopilotRunRef| {
        state
            .autopilot()
            .get_run(&input.run_id)
            .map_err(CommandError::from)
    })
}

// ============================================================================
// Quality frameworks catalog
// ============================================================================

pub fn list_quality_frameworks_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |_: NoInput| {
        state.list_quality_frameworks().map_err(CommandError::from)
    })
}

// ============================================================================
// Triage: coverage gaps / unclassified filings (ADR 0088 dec. 4)
// ============================================================================

/// Optional company scope + page size for the unclassified-filings triage read.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UnclassifiedFilingsRef {
    /// Optional qualified ticker (`GPW:CDR`) to scope the bucket to one company.
    #[serde(default)]
    pub company: Option<String>,
    /// Maximum filings to return (default 50, max 200).
    #[serde(default)]
    #[schemars(range(min = 1, max = 200))]
    pub limit: Option<i64>,
}

/// Per-period extraction-coverage gaps for one company — the periods where the
/// deterministic pipeline emitted nothing (the review surface complementing
/// `list_flagged_fact_provenance`, which lists flagged facts that DID emit).
pub fn list_flagged_extraction_outcomes_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: CompanyRef| {
        let company = resolve_company(state, &input.company)?;
        state
            .fundamentals_provenance()
            .list_flagged_extraction_outcomes(&company.id)
            .map_err(CommandError::from)
    })
}

/// Official filings the deterministic ESPI rule classifier could not place — the
/// explicit unclassified bucket (ADR 0088 dec. 4). Never guessed at; classify one
/// with `classify_filing`.
pub fn list_unclassified_filings_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input: UnclassifiedFilingsRef| {
        let company_id = optional_company_id(state, &input.company)?;
        let limit = input.limit.unwrap_or(50);
        state
            .list_unclassified_filings(company_id.as_deref(), limit)
            .map_err(CommandError::from)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::registry;
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewFactProvenance, NewFinancialFact,
        NewFinancialPeriod, NewNotebookEntry,
    };
    use serde_json::json;

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    /// Redact non-deterministic values so snapshots stay stable: SQLite
    /// CURRENT_TIMESTAMP (as in `protocol::tests::dossier_output_snapshot`) and
    /// the random `ulid_suffix()` embedded in period/fact ids (an 8-hex segment
    /// between `_` delimiters).
    fn redact(value: &Value) -> String {
        let pretty = serde_json::to_string_pretty(value).expect("serializable");
        let no_ts =
            regex::Regex::new(r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?(Z|\+\d{2}:\d{2})?")
                .expect("valid redaction regex")
                .replace_all(&pretty, "[timestamp]")
                .into_owned();
        regex::Regex::new(r"_[0-9a-f]{8}([_\x22])")
            .expect("valid ulid regex")
            .replace_all(&no_ts, "_[uid]$1")
            .into_owned()
    }

    /// Seed one company with a fiscal period, a confirmed fact, that fact's
    /// trust-ladder provenance, and a note — the substrate for the facts +
    /// provenance and note read tools.
    fn seed_company(state: &AppState) -> String {
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "TST".to_owned(),
                display_name: "Test S.A.".to_owned(),
                isin: Some("PLTEST0000011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company");
        let period = state
            .create_financial_period(NewFinancialPeriod {
                company_id: company.id.clone(),
                fiscal_year: 2025,
                period_type: "FY".to_owned(),
                period_end_date: Some("2025-12-31".to_owned()),
                report_evidence_ref: None,
            })
            .expect("period");
        let fact = state
            .create_financial_fact(NewFinancialFact {
                company_id: company.id.clone(),
                period_id: period.id.clone(),
                definition_id: "kpidef_net_profit".to_owned(),
                value_numeric: "1250000".to_owned(),
                currency: Some("PLN".to_owned()),
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
                confirmation_state: Some("confirmed".to_owned()),
                supersedes_id: None,
                source_document_ref: None,
                annotation: None,
            })
            .expect("fact");
        state
            .fundamentals_provenance()
            .set_fact_provenance(NewFactProvenance {
                fact_id: &fact.id,
                source_tier: "deterministic_esef",
                validation_status: "ok",
                drift_json: None,
                citation: Some("Skonsolidowany raport roczny 2025 SSF"),
            })
            .expect("provenance");
        state
            .create_notebook_entry(NewNotebookEntry {
                company_id: company.id.clone(),
                title: "Q4 read".to_owned(),
                body: "Net profit up.".to_owned(),
                body_format: None,
                tags: vec![],
                kind: "observation".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
                origins: vec![],
            })
            .expect("note");
        company.id
    }

    /// The facts + provenance composition tool (ADR 0088 dec. 2, item 4): each
    /// fact carries its trust-ladder `sourceTier`, `validationStatus`, and
    /// `citation`. Written before `list_financial_facts_with_provenance` existed.
    #[test]
    fn facts_tool_joins_provenance_onto_each_fact() {
        let state = state();
        seed_company(&state);
        let outcome = registry::call(
            &state,
            registry::McpScope::Full,
            "list_financial_facts",
            &json!({"company": "GPW:TST"}),
        )
        .expect("domain outcome");
        let payload = match outcome {
            ToolOutcome::Success(value) => value,
            ToolOutcome::Failure(error) => panic!("facts tool failed: {error:?}"),
        };
        let facts = payload["facts"].as_array().expect("facts array");
        assert_eq!(facts.len(), 1, "one seeded fact");
        let fact = &facts[0];
        assert_eq!(fact["definitionId"], json!("kpidef_net_profit"));
        assert_eq!(fact["sourceTier"], json!("deterministic_esef"));
        assert_eq!(fact["validationStatus"], json!("ok"));
        assert_eq!(
            fact["citation"],
            json!("Skonsolidowany raport roczny 2025 SSF"),
            "provenance citation travels with the fact"
        );
        insta::assert_snapshot!("facts_with_provenance", redact(&payload));
    }

    /// Golden output for the simplest domain (companies) over a seeded DB.
    #[test]
    fn companies_domain_snapshot() {
        let state = state();
        seed_company(&state);
        let outcome = registry::call(
            &state,
            registry::McpScope::Full,
            "list_companies",
            &json!({}),
        )
        .expect("domain outcome");
        let payload = match outcome {
            ToolOutcome::Success(value) => value,
            ToolOutcome::Failure(error) => panic!("list_companies failed: {error:?}"),
        };
        insta::assert_snapshot!("companies_output", redact(&payload));
    }

    /// Golden output for the research (notes) domain over a seeded DB.
    #[test]
    fn notes_domain_snapshot() {
        let state = state();
        seed_company(&state);
        let outcome = registry::call(
            &state,
            registry::McpScope::Full,
            "list_notebook_entries",
            &json!({"company": "GPW:TST"}),
        )
        .expect("domain outcome");
        let payload = match outcome {
            ToolOutcome::Success(value) => value,
            ToolOutcome::Failure(error) => panic!("list_notebook_entries failed: {error:?}"),
        };
        insta::assert_snapshot!("notes_output", redact(&payload));
    }

    /// A company-scoped read tool rejects an unknown ticker with `not_found`
    /// (never an empty success), like the MVP tools.
    #[test]
    fn company_scoped_tool_maps_unknown_to_not_found() {
        let state = state();
        let outcome = registry::call(
            &state,
            registry::McpScope::Full,
            "get_company_health",
            &json!({"company": "GPW:NOPE"}),
        )
        .expect("domain outcome");
        match outcome {
            ToolOutcome::Failure(error) => assert_eq!(error.code, CommandErrorCode::NotFound),
            ToolOutcome::Success(value) => panic!("unknown company must not succeed: {value}"),
        }
    }
}
