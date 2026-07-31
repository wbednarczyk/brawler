//! The MCP **act** (write) tier (ADR 0088 dec. 2/3, M3): provenance-mandatory
//! research writes, workspace actions, and job triggers a connected agent can
//! perform — every one gated at `tools/call` dispatch by the `mcpWritesEnabled`
//! setting, and the provenance-carrying writes additionally by
//! [`super::registry::validate_provenance`] BEFORE the handler runs (see
//! [`super::registry::call`]).
//!
//! Unlike the read wave, act tools reference entities by their **internal ids**
//! (the same ids the read tools return in every payload — e.g. `list_companies`
//! → `Company.id`, `list_financial_periods` → period id): an agent reads the
//! workspace, then writes against the ids it saw. So each handler deserializes
//! the SAME strict typed input the Tauri command uses and calls the SAME
//! `AppState`/sub-facade method the command delegates to (ADR 0039) — the MCP
//! surface and the UI can never diverge on a write path. The async in the Tauri
//! wrappers is only UI-thread offload (DoD §C); the underlying storage calls are
//! synchronous and run here on the MCP server's worker thread.
//!
//! Outputs are the domain write's own return value verbatim. Deletes, undo,
//! settings, credentials, and MCP self-management are permanently `Excluded`
//! (registry) and never reachable here.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::tools::{run, ToolCallError, ToolOutcome};
use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage;

/// Bind an act tool handler over one backing `AppState` write. The closure
/// receives the deserialized strict input and returns a `Result<T: Serialize,
/// CommandError>`; `run` maps deserialization failures to `-32602` and domain
/// failures to an `isError: true` result (ADR 0070 envelope).
macro_rules! act_handler {
    ($handler:ident, $input:ty, |$state:ident, $in:ident| $body:expr) => {
        pub fn $handler(state: &AppState, arguments: &Value) -> Result<ToolOutcome, ToolCallError> {
            run(arguments, |$in: $input| {
                let $state = state;
                $body
            })
        }
    };
}

/// No-argument act input (e.g. `generate_morning_briefing`). Distinct from
/// `reads::NoInput` only to keep the module self-contained.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct NoInput {}

// ============================================================================
// Research writes — provenance-carrying (Origins / SourceEvidence / FactCitation)
// ============================================================================

act_handler!(
    create_notebook_entry_handler,
    storage::NewNotebookEntry,
    |state, input| state
        .create_notebook_entry(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_note_from_transcript_selection_handler,
    storage::CreateNoteFromTranscriptSelectionInput,
    |state, input| state
        .create_note_from_transcript_selection(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_notebook_entry_handler,
    storage::NotebookEntryUpdate,
    |state, input| state
        .update_notebook_entry(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_management_claim_handler,
    storage::NewManagementClaim,
    |state, input| state
        .create_management_claim(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_management_claim_handler,
    storage::ManagementClaimUpdate,
    |state, input| state
        .update_management_claim(input)
        .map_err(CommandError::from)
);

act_handler!(
    set_claim_verdict_handler,
    storage::SetClaimVerdictInput,
    |state, input| state.set_claim_verdict(input).map_err(CommandError::from)
);

act_handler!(
    create_financial_fact_handler,
    storage::NewFinancialFact,
    |state, input| state
        .create_financial_fact(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_financial_fact_handler,
    storage::UpdateFinancialFact,
    |state, input| state
        .update_financial_fact(input)
        .map_err(CommandError::from)
);

/// `record_financial_facts` batch input (ADR 0093 dec. 6, epic #285 T7):
/// `companyId`/`reportDocumentId` are internal ids (act tier convention).
/// `facts` is capped 1..=100 (`jobs::record_financial_facts::MAX_BATCH_FACTS`
/// — a typed refusal beyond that, checked in the handler since schemars
/// `minItems`/`maxItems` are schema metadata only, never enforced at
/// deserialization). Every fact's `citation` is mandatory and non-blank,
/// checked BEFORE this handler runs
/// (`ProvenanceRequirement::DocumentAndPerFactCitations`,
/// `mcp::registry::act_gate`) — never the broken `FactCitation` shape
/// (`attribution` is a slot dimension, not a citation carrier).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecordFinancialFactsInput {
    pub company_id: String,
    pub report_document_id: String,
    pub period: RecordFinancialFactsPeriod,
    /// `final` (default when absent) | `preliminary` | `estimated` (ADR 0093
    /// dec. 2). Applies to every fact in the batch.
    #[serde(default)]
    pub data_quality: Option<String>,
    #[schemars(length(min = 1, max = 100))]
    pub facts: Vec<RecordFinancialFactEntry>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecordFinancialFactsPeriod {
    pub fiscal_year: i64,
    /// GPW interim reporting is cumulative — record H1/9M/FY only (ADR 0093
    /// dec. 3); a discrete-quarter column is derivable by span arithmetic,
    /// never submitted here.
    pub period_type: String,
    #[serde(default)]
    pub period_end: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RecordFinancialFactEntry {
    pub metric_key: String,
    pub value_numeric: String,
    #[serde(default)]
    pub currency: Option<String>,
    /// Slot dimension `total` (default) | `owners_of_parent` | `nci` — a
    /// typed refusal rejects any other token. NEVER a citation.
    #[serde(default)]
    pub attribution: Option<String>,
    #[serde(default)]
    pub measure_window: Option<String>,
    /// REQUIRED, non-blank — page/quote free text pointing at where this
    /// value was read in the cited document.
    pub citation: String,
}

act_handler!(
    record_financial_facts_handler,
    RecordFinancialFactsInput,
    |state, input| {
        let facts: Vec<crate::jobs::record_financial_facts::BatchFactInput<'_>> = input
            .facts
            .iter()
            .map(|fact| crate::jobs::record_financial_facts::BatchFactInput {
                metric_key: &fact.metric_key,
                value_numeric: &fact.value_numeric,
                currency: fact.currency.as_deref(),
                attribution: fact.attribution.as_deref(),
                measure_window: fact.measure_window.as_deref(),
                citation: &fact.citation,
            })
            .collect();
        crate::jobs::record_financial_facts::record_financial_facts(
            state,
            crate::jobs::record_financial_facts::RecordFinancialFactsInput {
                company_id: &input.company_id,
                report_document_id: &input.report_document_id,
                fiscal_year: input.period.fiscal_year,
                period_type: &input.period.period_type,
                period_end: input.period.period_end.as_deref(),
                data_quality: input.data_quality.as_deref(),
                facts: &facts,
            },
        )
    }
);

/// `capture_report_document` MCP input (ADR 0093 dec. 5, epic #285 T8):
/// mirrors `storage::CaptureReportDocumentInput` but deliberately EXCLUDES
/// `sourceType` — an agent registers a URL it read, it must never mint an
/// `espi_attachment`/other ingest-only row. `deny_unknown_fields` makes that
/// the honest refusal: an agent that sends `sourceType` gets the same typed
/// `-32602` "unknown field" protocol error `record_financial_facts` proves
/// for `totallyMadeUpField` (T7 precedent) — never a silent override of a
/// field the schema doesn't expose. The handler always writes
/// `source_type = "user_url"`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AgentCaptureReportDocumentInput {
    pub company_id: String,
    pub url: String,
    #[serde(default)]
    pub period_id: Option<String>,
    #[serde(default)]
    pub origin_ref: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub attribution: Option<String>,
}

// Fetches and stores the document at `url`, always under
// `source_type = "user_url"` (ADR 0093 dec. 5). Runs the SAME production
// path (`report_documents_capture::capture_report_document`) the
// `capture_report_document` Tauri command uses, but through the gated
// `HttpDocumentFetcher::agent_capture()` fetcher (https-only, SSRF-guarded
// on every redirect hop, content-type allowlisted, 30 MiB streaming cap) —
// never the ingest fetcher's unrestricted behavior. Idempotent: a repeat
// call with the same `companyId`+`url` returns the existing row
// (`UNIQUE(company_id, url)`).
act_handler!(
    capture_report_document_handler,
    AgentCaptureReportDocumentInput,
    |state, input| {
        let fetcher = crate::document_fetcher::HttpDocumentFetcher::agent_capture();
        crate::report_documents_capture::capture_report_document(
            state,
            &fetcher,
            storage::CaptureReportDocumentInput {
                company_id: input.company_id,
                source_type: "user_url".to_owned(),
                url: input.url,
                period_id: input.period_id,
                origin_ref: input.origin_ref,
                title: input.title,
                attribution: input.attribution,
            },
        )
        .map_err(CommandError::from)
    }
);

// ---- Qualitative verdicts (CitationsJson, batch shape) ---------------------

act_handler!(
    set_qualitative_verdicts_handler,
    crate::commands::quality_frameworks::SetQualitativeVerdictsInput,
    |state, input| {
        let persist =
            crate::commands::quality_frameworks::build_persist_qualitative_input(state, input)?;
        state
            .persist_qualitative_assessment(persist)
            .map_err(CommandError::from)
    }
);

// ============================================================================
// Research writes — no provenance carrier
// ============================================================================

act_handler!(
    create_research_question_handler,
    storage::NewResearchQuestion,
    |state, input| state
        .create_research_question(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_research_question_handler,
    storage::ResearchQuestionUpdate,
    |state, input| state
        .update_research_question(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_evidence_link_handler,
    storage::NewEvidenceLink,
    |state, input| state
        .create_evidence_link(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_research_reminder_handler,
    storage::NewResearchReminder,
    |state, input| state
        .create_research_reminder(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_research_reminder_handler,
    storage::ResearchReminderUpdate,
    |state, input| state
        .update_research_reminder(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_decision_entry_handler,
    storage::NewDecisionEntry,
    |state, input| state
        .decision_journal()
        .create_decision_entry(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_report_expectation_handler,
    storage::NewReportExpectation,
    |state, input| state
        .report_expectations()
        .create_report_expectation(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_report_expectation_handler,
    storage::UpdateReportExpectation,
    |state, input| state
        .report_expectations()
        .update_report_expectation(input)
        .map_err(CommandError::from)
);

act_handler!(
    record_expectation_resolution_handler,
    storage::RecordExpectationResolutionInput,
    |state, input| state
        .report_expectations()
        .record_expectation_resolution(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_company_event_handler,
    storage::NewCompanyEvent,
    |state, input| state
        .create_company_event(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_kpi_definition_handler,
    storage::NewKpiDefinition,
    |state, input| state
        .create_kpi_definition(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_kpi_relevance_handler,
    storage::NewKpiRelevance,
    |state, input| state
        .create_kpi_relevance(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_kpi_relevance_handler,
    storage::UpdateKpiRelevance,
    |state, input| state
        .update_kpi_relevance(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_quality_framework_handler,
    storage::NewQualityFramework,
    |state, input| state
        .create_quality_framework(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_quality_framework_handler,
    storage::UpdateQualityFramework,
    |state, input| state
        .update_quality_framework(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_framework_criterion_handler,
    storage::NewFrameworkCriterion,
    |state, input| state
        .create_framework_criterion(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_framework_criterion_handler,
    storage::UpdateFrameworkCriterion,
    |state, input| state
        .update_framework_criterion(input)
        .map_err(CommandError::from)
);

act_handler!(
    create_alert_rule_handler,
    storage::NewAlertRule,
    |state, input| state
        .attention()
        .create_alert_rule(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_alert_rule_handler,
    storage::AlertRuleUpdate,
    |state, input| state
        .attention()
        .update_alert_rule(input)
        .map_err(CommandError::from)
);

// ============================================================================
// Workspace actions
// ============================================================================

act_handler!(
    create_company_handler,
    storage::NewCompany,
    // Routes through the COMMAND impl, not the bare storage write, so the GPW
    // quote-backfill enqueue fires for MCP-created companies too (issue #250).
    |state, input| crate::commands::companies::create_company_impl(input, state)
        .map_err(CommandError::from)
);

act_handler!(
    create_watchlist_handler,
    storage::NewWatchlist,
    |state, input| state
        .watchlists()
        .create_watchlist(input)
        .map_err(CommandError::from)
);

act_handler!(
    add_company_to_watchlist_handler,
    storage::WatchlistCompanyInput,
    |state, input| state
        .watchlists()
        .add_company_to_watchlist(input)
        .map_err(CommandError::from)
);

act_handler!(
    remove_company_from_watchlist_handler,
    storage::WatchlistCompanyInput,
    |state, input| state
        .watchlists()
        .remove_company_from_watchlist(input)
        .map_err(CommandError::from)
);

act_handler!(
    update_feed_item_state_handler,
    storage::FeedItemStateInput,
    |state, input| state
        .update_feed_item_state(input)
        .map_err(CommandError::from)
);

act_handler!(
    mark_report_prepared_handler,
    storage::MarkReportPreparedInput,
    |state, input| state
        .mark_report_prepared(input)
        .map_err(CommandError::from)
);

act_handler!(
    mark_report_processed_handler,
    storage::MarkReportProcessedInput,
    |state, input| state
        .mark_report_processed(input)
        .map_err(CommandError::from)
);

act_handler!(
    mark_research_scope_reviewed_handler,
    storage::ResearchReviewCheckpointInput,
    |state, input| state
        .mark_research_scope_reviewed(input)
        .map_err(CommandError::from)
);

act_handler!(
    confirm_company_signal_handler,
    storage::CompanySignalActionInput,
    |state, input| state
        .confirm_company_signal(&input.id)
        .map_err(CommandError::from)
);

act_handler!(
    reject_company_signal_handler,
    storage::CompanySignalActionInput,
    |state, input| state
        .reject_company_signal(&input.id)
        .map_err(CommandError::from)
);

// Classify one unclassified official filing into a `confirmed` signal (ADR 0088
// dec. 4). No provenance carrier: the mandatory `feedItemId` IS the evidence
// anchor. Shares `classify_filing_inner` with the typed command (single source
// of truth) — the category/official/conflict checks return typed errors.
act_handler!(
    classify_filing_handler,
    crate::commands::signals::ClassifyFilingInput,
    |state, input| crate::commands::signals::classify_filing_inner(state, input)
);

act_handler!(
    acknowledge_red_flag_handler,
    storage::AcknowledgeRedFlagInput,
    |state, input| state
        .red_flags()
        .acknowledge(&input.flag_id)
        .map_err(CommandError::from)
);

/// Confirm/reject a proposed derived calendar event. Mirrors the loose
/// `confirm_derived_event` command args as one strict object.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ConfirmDerivedEventInput {
    pub event_id: String,
    /// `confirm` places the proposed event on the calendar; `reject` discards it.
    pub action: String,
}

act_handler!(
    confirm_derived_event_handler,
    ConfirmDerivedEventInput,
    |state, input| {
        let confirm = match input.action.as_str() {
            "confirm" => true,
            "reject" => false,
            other => {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    format!("unknown derived-event action: {other}"),
                ))
            }
        };
        state
            .confirm_derived_event(&input.event_id, confirm)
            .map_err(CommandError::from)
    }
);

/// The one write whose command takes loose scalar Tauri args rather than a
/// wrapped input; mirrored here as a strict object.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SetOwnershipHolderTypeInput {
    pub company_id: String,
    pub holder_key: String,
    #[serde(default)]
    pub holder_type: Option<String>,
}

act_handler!(
    set_ownership_holder_type_handler,
    SetOwnershipHolderTypeInput,
    |state, input| {
        state
            .ownership()
            .set_holder_type(
                &input.company_id,
                &input.holder_key,
                input.holder_type.as_deref(),
            )
            .map_err(CommandError::from)?;
        crate::commands::ownership::compute_ownership_overview(state, &input.company_id)
            .map_err(|message| CommandError::new(CommandErrorCode::Internal, message))
    }
);

/// `{ id }` action input shared by the two attention-event state writes.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct AttentionEventActionInput {
    pub id: String,
}

act_handler!(
    mark_attention_event_seen_handler,
    AttentionEventActionInput,
    |state, input| state
        .attention()
        .mark_attention_event_seen(&input.id)
        .map_err(CommandError::from)
);

act_handler!(
    dismiss_attention_event_handler,
    AttentionEventActionInput,
    |state, input| state
        .attention()
        .dismiss_attention_event(&input.id)
        .map_err(CommandError::from)
);

/// `set_autopilot_run_notification_state` input (mirrors the command's local struct).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SetRunNotificationStateInput {
    pub run_id: String,
    pub notification_state: String,
}

act_handler!(
    set_autopilot_run_notification_state_handler,
    SetRunNotificationStateInput,
    |state, input| {
        match input.notification_state.as_str() {
            "unread" | "read" | "dismissed" => {}
            other => {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    format!("unknown notification state: {other}"),
                ))
            }
        }
        state
            .autopilot()
            .set_notification_state(&input.run_id, &input.notification_state)
            .map_err(CommandError::from)
    }
);

// ============================================================================
// Job triggers
// ============================================================================

act_handler!(
    evaluate_framework_handler,
    storage::EvaluateFrameworkInput,
    |state, input| state.evaluate_framework(input).map_err(CommandError::from)
);

/// `compute_comparative_valuation` input — the internal company id (act tier
/// references entities by internal id, per the module contract).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ComputeComparativeValuationInput {
    pub company_id: String,
}

act_handler!(
    compute_comparative_valuation_handler,
    ComputeComparativeValuationInput,
    |state, input| crate::commands::valuation::compute_and_persist_comparative_valuation(
        state,
        &input.company_id,
    )
    .map_err(|message| CommandError::new(CommandErrorCode::Internal, message))
);

/// `set_alert_rule_enabled` input (mirrors the command's local struct).
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SetAlertRuleEnabledInput {
    pub id: String,
    pub enabled: bool,
}

act_handler!(
    set_alert_rule_enabled_handler,
    SetAlertRuleEnabledInput,
    |state, input| state
        .attention()
        .set_alert_rule_enabled(&input.id, input.enabled)
        .map_err(CommandError::from)
);

/// `trigger_autopilot_run` input: fail-fast — an unknown report document /
/// company id is rejected before any job is enqueued.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TriggerAutopilotRunInput {
    pub company_id: String,
    pub report_document_id: String,
}

act_handler!(
    trigger_autopilot_run_handler,
    TriggerAutopilotRunInput,
    |state, input| {
        // Fail-fast id validation before touching the queue (mirrors the command).
        let document = state
            .get_report_document(&input.report_document_id)
            .map_err(|_| {
                CommandError::new(CommandErrorCode::NotFound, "report_document_not_found")
            })?;
        if document.company_id != input.company_id {
            return Err(CommandError::new(
                CommandErrorCode::InvalidInput,
                "report document does not belong to the given company",
            ));
        }
        let mode = state
            .autopilot()
            .get_mode(&input.company_id)
            .map_err(CommandError::from)?;
        let mode = if mode == storage::MODE_AUTOPILOT {
            storage::MODE_AUTOPILOT
        } else {
            storage::MODE_ASSIST
        };
        let run_id = format!(
            "autopilot_run:{}:{}",
            input.company_id, input.report_document_id
        );
        match state
            .autopilot()
            .create_run_if_absent(
                &run_id,
                &input.company_id,
                &input.report_document_id,
                "manual",
                mode,
                None,
            )
            .map_err(CommandError::from)?
        {
            Some(run) => {
                crate::jobs::autopilot::enqueue_first_stage(state, &run.id);
                Ok::<_, CommandError>(run)
            }
            None => Err(CommandError::new(
                CommandErrorCode::Conflict,
                "autopilot_run_in_progress",
            )),
        }
    }
);

act_handler!(
    generate_morning_briefing_handler,
    NoInput,
    |state, _input| {
        crate::jobs::morning_briefing::enqueue_on_demand_briefing(state);
        Ok::<Value, CommandError>(Value::Null)
    }
);

// ---- Job triggers — networked / heavy (ADR 0088 dec. 2) --------------------
//
// These run live source ingestion / deterministic extraction / backfill work,
// with no id guard that fails fast on minimal input. They are fully gated like
// every other act tool, but the hermetic umbrella test does NOT invoke them
// (`NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS`) — they are exercised by the M6 live
// dogfooding ritual. Each binds to the same job fn its Tauri command runs on the
// scheduler pool; here they run synchronously on the MCP worker thread.

/// Map a job-level `String` error onto the typed envelope (`internal` — a source
/// refresh / extraction failure carries no finer machine code at this seam).
fn job_error(message: String) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, message)
}

/// Normalize a source-refresh trigger (`manual` | `scheduler`, default `manual`)
/// — mirrors the `sources` command's private helper.
fn refresh_trigger(trigger: Option<String>) -> String {
    trigger
        .filter(|trigger| trigger == "manual" || trigger == "scheduler")
        .unwrap_or_else(|| "manual".to_owned())
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RefreshSourcesInput {
    /// `manual` (default) | `scheduler`.
    #[serde(default)]
    pub trigger: Option<String>,
}

act_handler!(
    refresh_sources_handler,
    RefreshSourcesInput,
    |state, input| {
        crate::jobs::source_refresh::refresh_sources_for_trigger(
            state,
            &refresh_trigger(input.trigger),
        )
        .map_err(job_error)
    }
);

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RefreshSourceInput {
    pub adapter_id: String,
    /// `manual` (default) | `scheduler`.
    #[serde(default)]
    pub trigger: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
}

act_handler!(
    refresh_source_handler,
    RefreshSourceInput,
    |state, input| {
        crate::jobs::source_refresh::refresh_source_for_trigger(
            state,
            &input.adapter_id,
            &refresh_trigger(input.trigger),
            input.date.as_deref(),
        )
        .map_err(job_error)
    }
);

act_handler!(
    run_aggregator_fundamentals_pull_handler,
    NoInput,
    |state, _input| {
        crate::jobs::aggregator_fundamentals_pull::run_aggregator_fundamentals_pull(state)
            .map_err(job_error)
    }
);

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct BackfillCompanyHistoryInput {
    pub company_id: String,
}

act_handler!(
    backfill_company_history_handler,
    BackfillCompanyHistoryInput,
    |state, input| {
        Ok::<_, CommandError>(crate::jobs::backfill::backfill_company_history(
            state,
            &input.company_id,
        ))
    }
);

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RunStructuredExtractionInput {
    pub company_id: String,
    pub report_document_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end: String,
    /// Trust-ladder mode `autopilot` (default) | `assist`.
    #[serde(default)]
    pub mode: Option<String>,
}

act_handler!(
    run_structured_extraction_handler,
    RunStructuredExtractionInput,
    |state, input| {
        use crate::commands::fundamentals_extraction as fx;
        let mode = fx::normalize_mode(input.mode.as_deref())
            .map_err(|message| CommandError::new(CommandErrorCode::InvalidInput, message))?;
        let result = crate::jobs::structured_extraction::run_structured_extraction(
            state,
            &input.company_id,
            &input.report_document_id,
            input.fiscal_year,
            &input.period_type,
            &input.period_end,
            &mode,
        )
        .map_err(job_error)?;
        fx::detect_score_deterioration_after_extraction(state, &input.company_id, result.emitted);
        Ok::<_, CommandError>(fx::summarize(result))
    }
);

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct RerunExtractionOutcomeInput {
    pub outcome_id: String,
    /// Trust-ladder mode `autopilot` (default) | `assist`.
    #[serde(default)]
    pub mode: Option<String>,
}

act_handler!(
    rerun_extraction_outcome_handler,
    RerunExtractionOutcomeInput,
    |state, input| {
        use crate::commands::fundamentals_extraction as fx;
        let mode = fx::normalize_mode(input.mode.as_deref())
            .map_err(|message| CommandError::new(CommandErrorCode::InvalidInput, message))?;
        let company_id = state
            .fundamentals_provenance()
            .get_extraction_outcome(&input.outcome_id)
            .map_err(CommandError::from)?
            .map(|outcome| outcome.company_id);
        let result = crate::jobs::structured_extraction::rerun_extraction_outcome(
            state,
            &input.outcome_id,
            &mode,
        )
        .map_err(job_error)?;
        if let Some(company_id) = company_id.as_deref() {
            fx::detect_score_deterioration_after_extraction(state, company_id, result.emitted);
        }
        Ok::<_, CommandError>(fx::summarize(result))
    }
);
