//! The exposed act (write) wave — wired into [`super::entries`].

use super::{exposed_act, ProvenanceRequirement, RegistryEntry};
use crate::mcp::{acts, tools};
use crate::storage;

/// The exposed **act** (write) wave (ADR 0088 dec. 2/3, M3): the meaningful
/// agent write surface — every provenance-carrying research write, the broad
/// no-provenance research writes, workspace actions, and the light/fail-fast job
/// triggers. Handlers + input types live in [`super::acts`]; each backing
/// command's classification row is carried HERE (removed from [`classifications`])
/// so the gate still classifies every command exactly once. Every row is listed
/// in `tools/list` ALWAYS — the write gate (`mcpWritesEnabled` + provenance) is
/// enforced at call time ([`call`]), never by hiding the tool.
pub(super) fn act_wave_tools() -> Vec<RegistryEntry> {
    use ProvenanceRequirement::*;
    vec![
        // ---- Research writes — provenance-carrying ------------------------
        exposed_act(
            "create_notebook_entry",
            Some(Origins),
            "Create a research note for a company. Every note must carry a non-empty `origins` array tracing it to a report/article/transcript (provenance). References the company by its internal id (from list_companies).",
            tools::tool_schema::<storage::NewNotebookEntry>,
            acts::create_notebook_entry_handler,
        ),
        exposed_act(
            "create_note_from_transcript_selection",
            Some(Origins),
            "Create a research note anchored to selected transcript segments (the selection is the note's origin/provenance).",
            tools::tool_schema::<storage::CreateNoteFromTranscriptSelectionInput>,
            acts::create_note_from_transcript_selection_handler,
        ),
        // No provenance carrier: a note's origins are immutable from creation
        // (ADR 0088 dec. 3 is satisfied at create) and the update input carries
        // no origins field — an update can only edit title/body/tags/kind, never
        // remove provenance.
        exposed_act(
            "update_notebook_entry",
            None,
            "Update an existing research note (by id): title/body/tags/kind. The note keeps its recorded origins.",
            tools::tool_schema::<storage::NotebookEntryUpdate>,
            acts::update_notebook_entry_handler,
        ),
        exposed_act(
            "create_management_claim",
            Some(SourceEvidence),
            "Record a tracked management claim (guidance/promise). Must anchor to a `sourceEvidenceId` (the report/transcript it was made in).",
            tools::tool_schema::<storage::NewManagementClaim>,
            acts::create_management_claim_handler,
        ),
        exposed_act(
            "update_management_claim",
            Some(SourceEvidence),
            "Update a tracked management claim (by id). Must carry its `sourceEvidenceId` provenance.",
            tools::tool_schema::<storage::ManagementClaimUpdate>,
            acts::update_management_claim_handler,
        ),
        // No provenance carrier: the claim's source evidence is fixed at
        // creation; a verdict's own evidence is the optional `verifyingFactId`
        // (absent for a qualitative claim), so it cannot be a mandatory carrier.
        exposed_act(
            "set_claim_verdict",
            None,
            "Record a verification verdict on a management claim (optionally linking the verifying fact).",
            tools::tool_schema::<storage::SetClaimVerdictInput>,
            acts::set_claim_verdict_handler,
        ),
        exposed_act(
            "create_financial_fact",
            Some(FactCitation),
            "Low-level single-fact repair write (ADR 0098) — never report ingestion; once the KPI ingest run-workflow tools are present on this server, use them for ingesting reports. Records a financial fact for a company/period/metric. Must carry a non-blank `sourceDocumentRef` citation (`attribution` is the total/owners_of_parent/nci slot dimension, never a citation carrier). Decision support only. MCP writes are stamped honestly: `source_tier='agent'` provenance, `extraction_method='mcp_agent'`, `validation_status='unreviewed'` — never masquerading as a manual entry.",
            tools::tool_schema::<storage::NewFinancialFact>,
            acts::create_financial_fact_handler,
        ),
        exposed_act(
            "update_financial_fact",
            Some(FactCitation),
            "Low-level single-fact repair write (ADR 0098) — never report ingestion. Updates a stored financial fact (by id). Must carry its non-blank `sourceDocumentRef` citation. Stamps `source_tier='agent'` provenance (honest takeover — never masquerading as manual), even on a previously-manual fact.",
            tools::tool_schema::<storage::UpdateFinancialFact>,
            acts::update_financial_fact_handler,
        ),
        // The document an agent read, registered so record_financial_facts can
        // cite it (ADR 0093 dec. 5, epic #285 T8). Fetch is gated:
        // https-only + SSRF guard (every resolved address, re-checked on
        // every redirect hop), content-type allowlist, 30 MiB streaming cap
        // (`document_fetcher::HttpDocumentFetcher::agent_capture()`) — never
        // the unrestricted ingest fetcher every other caller keeps.
        exposed_act(
            "capture_report_document",
            None,
            "Register and fetch a report document by URL for a company — the document an agent read before citing facts from it (the fact-write tools need its returned documentId). Always registers under source_type \"user_url\"; passing a sourceType is refused (unknown field). Gated: https only, private/loopback/link-local network addresses refused (including via redirect), content-type restricted to application/pdf | text/html | application/xhtml+xml, 30 MiB size cap. Idempotent on (companyId, url). Returns the document's id, local path, and fetch success/error.",
            tools::tool_schema::<acts::AgentCaptureReportDocumentInput>,
            acts::capture_report_document_handler,
        ),
        // Batch write over `record_structured_fact` (ADR 0093 dec. 6, epic
        // #285 T7), demoted to a low-level repair tool by ADR 0098 (#365) —
        // the normal ingestion path is the KPI ingest run workflow (its MCP
        // tools land with #353). No Tauri command twin (MCP-only, like the
        // four MVP read composites) — `tool_name` == `command_name` (the
        // [`exposed_act`] convention for such tools).
        exposed_act(
            "record_financial_facts",
            Some(DocumentAndPerFactCitations),
            "Low-level batch fact write (ADR 0098). If `start_kpi_ingest` is absent from this server's tools, this is the only supported temporary report-ingest route; once the run-workflow tools are present, use them for ingestion and this tool ONLY for manual repair. Records a batch (1-100) of financial facts for one company/period from a document an agent read, with per-fact citations. Ensures the fiscal period, resolves each metricKey against the KPI catalog, judges the set against stored history and same-period accounting identities, and commits every plausible fact under the `agent` source tier (ADR 0093) — never overwriting an issuer-held or manual fact; a disagreement is reported as `divergent`, never silently resolved. Use `dataQuality: \"preliminary\"` for issuer pre-report releases (e.g. GPW wstępne wyniki) — record CUMULATIVE columns only (H1/9M/FY), never discrete-quarter columns. Decision support only.",
            tools::tool_schema::<acts::RecordFinancialFactsInput>,
            acts::record_financial_facts_handler,
        ),
        // The qualitative-verdict write path (ADR 0084 dec. 5 successor). Batch
        // shape: every result carries its own `citationsJson` provenance.
        exposed_act(
            "set_qualitative_verdicts",
            Some(CitationsJson),
            "Record agent-authored qualitative criterion verdicts for one framework+company as one immutable snapshot. Every result must carry `citationsJson`: a serialized non-empty array of typed evidence refs `[{\"evidenceType\":\"notebook_entry\",\"evidenceId\":\"<id>\"}]` (types: feed_item | notebook_entry | claim | transcript_segment | company_event | research_question | company_signal | decision_entry); every ref must resolve to an existing row or the whole batch is refused. Decision support only — never an investment recommendation.",
            tools::tool_schema::<crate::commands::quality_frameworks::SetQualitativeVerdictsInput>,
            acts::set_qualitative_verdicts_handler,
        ),
        // ---- Research writes — no provenance carrier ----------------------
        exposed_act(
            "create_research_question",
            None,
            "Open a research question scoped to a company/watchlist/sector.",
            tools::tool_schema::<storage::NewResearchQuestion>,
            acts::create_research_question_handler,
        ),
        exposed_act(
            "update_research_question",
            None,
            "Update a research question (title/body/status) by id.",
            tools::tool_schema::<storage::ResearchQuestionUpdate>,
            acts::update_research_question_handler,
        ),
        exposed_act(
            "create_evidence_link",
            None,
            "Link two research-graph entities (note/claim/fact/document…) with a typed relation.",
            tools::tool_schema::<storage::NewEvidenceLink>,
            acts::create_evidence_link_handler,
        ),
        exposed_act(
            "create_research_reminder",
            None,
            "Create a personal follow-up reminder scoped to a company/watchlist/sector.",
            tools::tool_schema::<storage::NewResearchReminder>,
            acts::create_research_reminder_handler,
        ),
        exposed_act(
            "update_research_reminder",
            None,
            "Update a research reminder (status/due/snooze) by id.",
            tools::tool_schema::<storage::ResearchReminderUpdate>,
            acts::update_research_reminder_handler,
        ),
        exposed_act(
            "create_decision_entry",
            None,
            "Append an immutable decision-journal entry for a company (rationale + decided-at).",
            tools::tool_schema::<storage::NewDecisionEntry>,
            acts::create_decision_entry_handler,
        ),
        exposed_act(
            "create_report_expectation",
            None,
            "Record pre-report expectations for a company's upcoming report event (stance + optional metrics).",
            tools::tool_schema::<storage::NewReportExpectation>,
            acts::create_report_expectation_handler,
        ),
        exposed_act(
            "update_report_expectation",
            None,
            "Update a company's report expectation (stance/metrics) by its event key.",
            tools::tool_schema::<storage::UpdateReportExpectation>,
            acts::update_report_expectation_handler,
        ),
        exposed_act(
            "record_expectation_resolution",
            None,
            "Resolve a report expectation after the report lands (resolution note).",
            tools::tool_schema::<storage::RecordExpectationResolutionInput>,
            acts::record_expectation_resolution_handler,
        ),
        exposed_act(
            "create_company_event",
            None,
            "Add a calendar event (dividend/meeting/report date) for a company.",
            tools::tool_schema::<storage::NewCompanyEvent>,
            acts::create_company_event_handler,
        ),
        exposed_act(
            "create_kpi_definition",
            None,
            "Add a KPI/financial-concept definition to the metric catalog.",
            tools::tool_schema::<storage::NewKpiDefinition>,
            acts::create_kpi_definition_handler,
        ),
        exposed_act(
            "create_kpi_relevance",
            None,
            "Mark a KPI definition relevant to a company (scorecard editor).",
            tools::tool_schema::<storage::NewKpiRelevance>,
            acts::create_kpi_relevance_handler,
        ),
        exposed_act(
            "update_kpi_relevance",
            None,
            "Update a company's KPI-relevance row (status/rank) by id.",
            tools::tool_schema::<storage::UpdateKpiRelevance>,
            acts::update_kpi_relevance_handler,
        ),
        exposed_act(
            "create_quality_framework",
            None,
            "Create a quality-scorecard framework.",
            tools::tool_schema::<storage::NewQualityFramework>,
            acts::create_quality_framework_handler,
        ),
        exposed_act(
            "update_quality_framework",
            None,
            "Update a quality framework (name/description) by id.",
            tools::tool_schema::<storage::UpdateQualityFramework>,
            acts::update_quality_framework_handler,
        ),
        exposed_act(
            "create_framework_criterion",
            None,
            "Add a criterion to a quality framework.",
            tools::tool_schema::<storage::NewFrameworkCriterion>,
            acts::create_framework_criterion_handler,
        ),
        exposed_act(
            "update_framework_criterion",
            None,
            "Update a framework criterion by id.",
            tools::tool_schema::<storage::UpdateFrameworkCriterion>,
            acts::update_framework_criterion_handler,
        ),
        exposed_act(
            "create_alert_rule",
            None,
            "Create an alert rule (trigger + scope). Fired events surface via list_attention_events.",
            tools::tool_schema::<storage::NewAlertRule>,
            acts::create_alert_rule_handler,
        ),
        exposed_act(
            "update_alert_rule",
            None,
            "Update an alert rule (trigger/scope/enabled) by id.",
            tools::tool_schema::<storage::AlertRuleUpdate>,
            acts::update_alert_rule_handler,
        ),
        // ---- Workspace actions --------------------------------------------
        exposed_act(
            "create_company",
            None,
            "Track a new company (exchange + ticker + display name). On GPW this also enqueues a quote backfill.",
            tools::tool_schema::<storage::NewCompany>,
            acts::create_company_handler,
        ),
        exposed_act(
            "create_watchlist",
            None,
            "Create a watchlist.",
            tools::tool_schema::<storage::NewWatchlist>,
            acts::create_watchlist_handler,
        ),
        exposed_act(
            "add_company_to_watchlist",
            None,
            "Add a company to a watchlist (by their internal ids).",
            tools::tool_schema::<storage::WatchlistCompanyInput>,
            acts::add_company_to_watchlist_handler,
        ),
        exposed_act(
            "remove_company_from_watchlist",
            None,
            "Remove a company from a watchlist (by their internal ids).",
            tools::tool_schema::<storage::WatchlistCompanyInput>,
            acts::remove_company_from_watchlist_handler,
        ),
        exposed_act(
            "update_feed_item_state",
            None,
            "Set a feed item's read/saved flags (by id).",
            tools::tool_schema::<storage::FeedItemStateInput>,
            acts::update_feed_item_state_handler,
        ),
        exposed_act(
            "mark_report_prepared",
            None,
            "Mark a company's upcoming report event as prepared.",
            tools::tool_schema::<storage::MarkReportPreparedInput>,
            acts::mark_report_prepared_handler,
        ),
        exposed_act(
            "mark_report_processed",
            None,
            "Mark a company's report event as processed (optionally linking the report document).",
            tools::tool_schema::<storage::MarkReportProcessedInput>,
            acts::mark_report_processed_handler,
        ),
        exposed_act(
            "mark_research_scope_reviewed",
            None,
            "Set a 'reviewed' checkpoint for a research scope (optionally cascading to its companies).",
            tools::tool_schema::<storage::ResearchReviewCheckpointInput>,
            acts::mark_research_scope_reviewed_handler,
        ),
        exposed_act(
            "confirm_company_signal",
            None,
            "Confirm a proposed filing signal (by id).",
            tools::tool_schema::<storage::CompanySignalActionInput>,
            acts::confirm_company_signal_handler,
        ),
        exposed_act(
            "reject_company_signal",
            None,
            "Reject a proposed filing signal (by id).",
            tools::tool_schema::<storage::CompanySignalActionInput>,
            acts::reject_company_signal_handler,
        ),
        // Provenance None: the mandatory `feedItemId` IS the evidence anchor
        // (ADR 0088 dec. 4) — the created signal cites the filing it classifies.
        exposed_act(
            "classify_filing",
            None,
            "Classify an unclassified official filing (from list_unclassified_filings) into a confirmed signal. Takes `feedItemId` (the evidence anchor) and a `category` key from the seeded taxonomy. Rejects an unknown category, a non-official item, or an already-classified filing.",
            tools::tool_schema::<crate::commands::signals::ClassifyFilingInput>,
            acts::classify_filing_handler,
        ),
        exposed_act(
            "confirm_derived_event",
            None,
            "Confirm or reject a proposed derived calendar event (`action`: confirm|reject).",
            tools::tool_schema::<acts::ConfirmDerivedEventInput>,
            acts::confirm_derived_event_handler,
        ),
        exposed_act(
            "acknowledge_red_flag",
            None,
            "Acknowledge an active red flag (by id).",
            tools::tool_schema::<storage::AcknowledgeRedFlagInput>,
            acts::acknowledge_red_flag_handler,
        ),
        exposed_act(
            "set_ownership_holder_type",
            None,
            "Relabel a shareholder's holder type for a company; returns the recomputed ownership overview.",
            tools::tool_schema::<acts::SetOwnershipHolderTypeInput>,
            acts::set_ownership_holder_type_handler,
        ),
        exposed_act(
            "mark_attention_event_seen",
            None,
            "Mark an attention event as seen (by id).",
            tools::tool_schema::<acts::AttentionEventActionInput>,
            acts::mark_attention_event_seen_handler,
        ),
        exposed_act(
            "dismiss_attention_event",
            None,
            "Dismiss an attention event (by id).",
            tools::tool_schema::<acts::AttentionEventActionInput>,
            acts::dismiss_attention_event_handler,
        ),
        exposed_act(
            "set_autopilot_run_notification_state",
            None,
            "Set an autopilot run's notification state (unread|read|dismissed).",
            tools::tool_schema::<acts::SetRunNotificationStateInput>,
            acts::set_autopilot_run_notification_state_handler,
        ),
        // ---- Job triggers — light / fail-fast -----------------------------
        exposed_act(
            "evaluate_framework",
            None,
            "Run the deterministic quantitative scorecard engine for a framework+company and persist the evaluation.",
            tools::tool_schema::<storage::EvaluateFrameworkInput>,
            acts::evaluate_framework_handler,
        ),
        exposed_act(
            "compute_comparative_valuation",
            None,
            "Compute the level-1 comparative valuation for one company (peer-multiple implied fair-value ranges for P/E, EV/EBITDA, and P/BV, method-convergence spread, and a deterministic confidence grade) and append a valuation_runs row per method whose input signature changed. Read the history via list_valuation_runs. Decision support only — never buy/sell/hold language.",
            tools::tool_schema::<acts::ComputeComparativeValuationInput>,
            acts::compute_comparative_valuation_handler,
        ),
        exposed_act(
            "set_alert_rule_enabled",
            None,
            "Enable/disable an alert rule (by id).",
            tools::tool_schema::<acts::SetAlertRuleEnabledInput>,
            acts::set_alert_rule_enabled_handler,
        ),
        exposed_act(
            "trigger_autopilot_run",
            None,
            "Trigger an autopilot run over one company's report document (fail-fast on unknown ids); enqueues the durable pipeline.",
            tools::tool_schema::<acts::TriggerAutopilotRunInput>,
            acts::trigger_autopilot_run_handler,
        ),
        exposed_act(
            "generate_morning_briefing",
            None,
            "Enqueue composition of a fresh morning briefing (read the result via get_latest_morning_briefing).",
            tools::tool_schema::<acts::NoInput>,
            acts::generate_morning_briefing_handler,
        ),
        // ---- Job triggers — networked / heavy (ADR 0088 dec. 2) -----------
        // Gated identically; the hermetic umbrella test lists + gates them but
        // does NOT invoke them (they run live source/extraction/backfill work) —
        // see `NETWORK_TRIGGERS_NOT_INVOKED_IN_TESTS`. Exercised by M6 dogfooding.
        exposed_act(
            "refresh_sources",
            None,
            "Run a source-refresh sweep across all enabled adapters (`trigger`: manual | scheduler).",
            tools::tool_schema::<acts::RefreshSourcesInput>,
            acts::refresh_sources_handler,
        ),
        exposed_act(
            "refresh_source",
            None,
            "Run a refresh for one source adapter (by `adapterId`; optional `trigger`, `date`).",
            tools::tool_schema::<acts::RefreshSourceInput>,
            acts::refresh_source_handler,
        ),
        exposed_act(
            "run_aggregator_fundamentals_pull",
            None,
            "Run the aggregator fundamentals pull across tracked companies.",
            tools::tool_schema::<acts::NoInput>,
            acts::run_aggregator_fundamentals_pull_handler,
        ),
        exposed_act(
            "backfill_company_history",
            None,
            "Run an on-track history backfill for one company (`companyId`); progress via get_backfill_progress.",
            tools::tool_schema::<acts::BackfillCompanyHistoryInput>,
            acts::backfill_company_history_handler,
        ),
        exposed_act(
            "run_structured_extraction",
            None,
            "Run the deterministic structured-first extraction pipeline over one company report+period (`mode`: autopilot | assist).",
            tools::tool_schema::<acts::RunStructuredExtractionInput>,
            acts::run_structured_extraction_handler,
        ),
        exposed_act(
            "rerun_extraction_outcome",
            None,
            "Re-run the deterministic pipeline for a recorded extraction outcome slot (`outcomeId`).",
            tools::tool_schema::<acts::RerunExtractionOutcomeInput>,
            acts::rerun_extraction_outcome_handler,
        ),
        exposed_act(
            "run_pipeline_reextraction",
            None,
            "Re-arm one company's landed ESEF runs whose stored pipeline version is stale, so the current extractor reads their filings again. Queues a durable batch; poll `get_pipeline_reextraction_progress`.",
            tools::tool_schema::<acts::RunPipelineReextractionInput>,
            acts::run_pipeline_reextraction_handler,
        ),
    ]
}
