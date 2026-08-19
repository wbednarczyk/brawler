//! The broad UI-parity read wave — wired into [`super::entries`].

use super::{exposed_read, RegistryEntry};
use crate::mcp::{reads, tools};

/// The broad UI-parity **read wave** (ADR 0088 dec. 2 `read` tier): one exposed
/// tool per domain read so a connected agent can READ every domain. Handlers +
/// strict inputs live in [`super::reads`]; each backing command's classification
/// row is carried HERE (removed from [`classifications`]) so the gate still
/// classifies every command exactly once. Order fixes `tools/list` after the
/// four MVP tools — the frozen snapshot contract.
pub(super) fn read_wave_tools() -> Vec<RegistryEntry> {
    vec![
        // ---- Companies / watchlists ----------------------------------------
        exposed_read(
            "list_companies",
            "list_companies",
            "Every company tracked in the user's workspace (identity, exchange, qualified ticker).",
            tools::tool_schema::<reads::NoInput>,
            reads::list_companies_handler,
        ),
        exposed_read(
            "get_company_basic_info",
            "get_company_basic_info",
            "One company's identity card: name, exchange, ticker, ISIN, sector (with its provenance), and latest reported shares outstanding.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_company_basic_info_handler,
        ),
        exposed_read(
            "list_watchlists",
            "list_watchlists",
            "The user's watchlists (id, name, ordering).",
            tools::tool_schema::<reads::NoInput>,
            reads::list_watchlists_handler,
        ),
        exposed_read(
            "list_watchlist_memberships",
            "list_watchlist_memberships",
            "Which companies belong to which watchlist (the membership edges).",
            tools::tool_schema::<reads::NoInput>,
            reads::list_watchlist_memberships_handler,
        ),
        // ---- Feed / signals / events ---------------------------------------
        exposed_read(
            "list_feed_items",
            "list_feed_items",
            "The unified newsfeed: official filings (ESPI/EBI) and allowed media items with their read/saved state.",
            tools::tool_schema::<reads::NoInput>,
            reads::list_feed_items_handler,
        ),
        exposed_read(
            "list_company_signals",
            "list_company_signals",
            "Typed filing classifications (ESPI/EBI signals) for one company, with their confirmation status.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_company_signals_handler,
        ),
        exposed_read(
            "list_company_events",
            "list_company_events",
            "One company's calendar events (dividends, general meetings, report dates) and their status.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_company_events_handler,
        ),
        // ---- Financial facts (with provenance) / periods / KPI catalog -----
        exposed_read(
            "list_financial_facts",
            "list_financial_facts",
            "One company's stored financial facts, each carrying its trust-ladder provenance: sourceTier, validationStatus, and citation. Decision support only.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_financial_facts_with_provenance,
        ),
        exposed_read(
            "list_financial_periods",
            "list_financial_periods",
            "One company's fiscal periods (year + period type + period-end date).",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_financial_periods_handler,
        ),
        exposed_read(
            "list_kpi_definitions",
            "list_kpi_definitions",
            "The metric catalog: every KPI/financial-concept definition (id, label, unit) facts are keyed by.",
            tools::tool_schema::<reads::NoInput>,
            reads::list_kpi_definitions_handler,
        ),
        exposed_read(
            "list_flagged_fact_provenance",
            "list_flagged_fact_provenance",
            "Every fact the extraction pipeline flagged for review (a drift or contradiction against another source) — the data-quality review surface.",
            tools::tool_schema::<reads::NoInput>,
            reads::list_flagged_fact_provenance_handler,
        ),
        // ---- Quotes / ownership / insiders / analyst recommendations -------
        exposed_read(
            "get_price_context",
            "get_price_context",
            "One company's price context: latest quote and the recent range, plus derived valuation ratios where computable.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_price_context_handler,
        ),
        exposed_read(
            "get_kpi_comparison",
            "get_kpi_comparison",
            "Compare one or more canonical KPIs across companies on a shared, aligned period axis (annual or quarterly). Each cell carries the native + PLN-converted value with its FX basis, the evidence link (fact id + validation status), and server-computed QoQ/YoY deltas; gaps and unconvertible currencies are typed flags, never silent. Works for a single company too (the periods×deltas view). Decision support only.",
            tools::tool_schema::<reads::KpiComparisonRef>,
            reads::get_kpi_comparison_handler,
        ),
        exposed_read(
            "get_sector_percentiles",
            "get_sector_percentiles",
            "Where one company stands against its tracked sector peers: rank-based percentiles for the level-0 market ratios (P/E, P/BV, EV/EBITDA, dividend yield, FCF yield) and selected canonical KPIs, computed from confirmed data only. Always returns the peer count N and flags thin sets (N < 4); a company with no sector returns a typed empty reason. Decision support only.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_sector_percentiles_handler,
        ),
        exposed_read(
            "list_valuation_runs",
            "list_valuation_runs",
            "One company's append-only comparative-valuation run history (ADR 0089): each stored run's method (P/E, EV/EBITDA, or P/BV multiple), per-share fair-value range (low/base/high), input signature, confidence grade, and data-as-of date, newest first. The compute-and-persist path is the act-tier compute_comparative_valuation. Decision support only.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_valuation_runs_handler,
        ),
        exposed_read(
            "get_ownership_overview",
            "get_ownership_overview",
            "One company's shareholder structure: significant holders, holder types, and free float, with change history.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_ownership_overview_handler,
        ),
        exposed_read(
            "get_insider_overview",
            "get_insider_overview",
            "One company's insider-transaction timeline, management holdings, and rolling net-direction aggregates.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_insider_overview_handler,
        ),
        exposed_read(
            "list_short_positions",
            "list_short_positions",
            "One company's KNF short-selling register: active positions, change history, aggregate net short %, and the 30-day change.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_short_positions_handler,
        ),
        exposed_read(
            "get_analyst_recommendations",
            "get_analyst_recommendations",
            "One company's recorded analyst recommendations and price targets over time. Decision support only — never an investment recommendation.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_analyst_recommendations_handler,
        ),
        // ---- Health / red flags --------------------------------------------
        exposed_read(
            "get_company_health",
            "get_company_health",
            "One company's deterministic financial-health scores (Piotroski F, Altman Z\") per fiscal period, computed from confirmed facts.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_company_health_handler,
        ),
        exposed_read(
            "get_red_flags",
            "get_red_flags",
            "One company's active red flags (auditor concerns, short spikes, contradictions) plus the acknowledged history.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_red_flags_handler,
        ),
        // ---- Report documents + diffs --------------------------------------
        exposed_read(
            "get_report_documents_view",
            "get_report_documents_view",
            "One company's stored report documents, each tagged with its fiscal period and whether it is that period's canonical report.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_report_documents_view_handler,
        ),
        exposed_read(
            "list_report_diff_candidates",
            "list_report_diff_candidates",
            "Comparable pairs of successive financial statements for one company — the (older, newer) document pairs get_report_diff can diff.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_report_diff_candidates_handler,
        ),
        exposed_read(
            "get_report_diff",
            "get_report_diff",
            "The section-level text diff between two report documents (discover the pair via list_report_diff_candidates).",
            tools::tool_schema::<reads::ReportDiffRef>,
            reads::get_report_diff_handler,
        ),
        // ---- Transcripts ---------------------------------------------------
        exposed_read(
            "list_video_transcript_jobs",
            "list_video_transcript_jobs",
            "Video-transcript jobs (optionally scoped to one company): their source, status, and resolved company.",
            tools::tool_schema::<reads::OptionalCompanyRef>,
            reads::list_video_transcript_jobs_handler,
        ),
        exposed_read(
            "list_transcript_segments",
            "list_transcript_segments",
            "One transcript job's ordered segments (timestamped text) — the transcript body itself.",
            tools::tool_schema::<reads::TranscriptSegmentsRef>,
            reads::list_transcript_segments_handler,
        ),
        // ---- Notes / claims ------------------------------------------------
        exposed_read(
            "list_notebook_entries",
            "list_notebook_entries",
            "One company's research notes, each preserving the origin (report/article/transcript) it traces back to.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_notebook_entries_handler,
        ),
        exposed_read(
            "list_management_claims",
            "list_management_claims",
            "One company's tracked management claims (guidance/promises), with their verification period and verdict.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_management_claims_handler,
        ),
        // ---- Expectations / journal / research questions -------------------
        exposed_read(
            "list_report_expectations",
            "list_report_expectations",
            "Pre-report expectations the user recorded (optionally scoped to one company), with any resolution outcome.",
            tools::tool_schema::<reads::OptionalCompanyRef>,
            reads::list_report_expectations_handler,
        ),
        exposed_read(
            "list_decision_entries",
            "list_decision_entries",
            "The decision journal (optionally scoped to one company): recorded decisions and their rationale.",
            tools::tool_schema::<reads::OptionalCompanyRef>,
            reads::list_decision_entries_handler,
        ),
        exposed_read(
            "list_research_questions",
            "list_research_questions",
            "Open and answered research questions across the workspace, with their scope and status.",
            tools::tool_schema::<reads::NoInput>,
            reads::list_research_questions_handler,
        ),
        // ---- Calendar: report season / attention / briefing / autopilot ----
        exposed_read(
            "list_report_season",
            "list_report_season",
            "The upcoming report-season calendar: which tracked companies report when, with preparation state.",
            tools::tool_schema::<reads::NoInput>,
            reads::list_report_season_handler,
        ),
        exposed_read(
            "list_attention_events",
            "list_attention_events",
            "Fired attention events (newest first), optionally scoped to one company and optionally including dismissed ones.",
            tools::tool_schema::<reads::AttentionEventsRef>,
            reads::list_attention_events_handler,
        ),
        exposed_read(
            "get_latest_morning_briefing",
            "get_latest_morning_briefing",
            "The most recently composed morning briefing (its structured item list plus any narrative), or null when none exists yet.",
            tools::tool_schema::<reads::NoInput>,
            reads::get_latest_morning_briefing_handler,
        ),
        exposed_read(
            "list_autopilot_runs",
            "list_autopilot_runs",
            "Recent autopilot runs (optionally scoped to one company): what the autonomous pipeline produced and its notification state.",
            tools::tool_schema::<reads::AutopilotRunsRef>,
            reads::list_autopilot_runs_handler,
        ),
        exposed_read(
            "get_autopilot_run",
            "get_autopilot_run",
            "One autopilot run's full composed result (discover run ids via list_autopilot_runs).",
            tools::tool_schema::<reads::AutopilotRunRef>,
            reads::get_autopilot_run_handler,
        ),
        // ---- Quality frameworks catalog ------------------------------------
        exposed_read(
            "list_quality_frameworks",
            "list_quality_frameworks",
            "The quality-scorecard framework catalog: every framework and its criteria (the rubric get_quality_assessment scores against).",
            tools::tool_schema::<reads::NoInput>,
            reads::list_quality_frameworks_handler,
        ),
        // Alert rules: an agent with the trigger tools (set_alert_rule_enabled,
        // create/update_alert_rule) reads its own rule set (orchestrator ruling,
        // M2 review). Fired events stay on list_attention_events.
        exposed_read(
            "list_alert_rules",
            "list_alert_rules",
            "The alert-rule catalog: every configured rule (trigger, scope, enabled state). Fired events are read via list_attention_events.",
            tools::tool_schema::<reads::NoInput>,
            reads::list_alert_rules_handler,
        ),
        // ---- Triage: coverage gaps / unclassified filings (ADR 0088 dec. 4) --
        exposed_read(
            "list_flagged_extraction_outcomes",
            "list_flagged_extraction_outcomes",
            "One company's extraction-coverage gaps: the fiscal periods where the deterministic pipeline emitted nothing (a flagged/failed outcome). Complements list_flagged_fact_provenance (flagged facts that DID emit) — the coverage-gap review surface.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::list_flagged_extraction_outcomes_handler,
        ),
        exposed_read(
            "list_unclassified_filings",
            "list_unclassified_filings",
            "Official filings (ESPI/EBI) the deterministic rule classifier could not place — the explicit unclassified bucket, never guessed at. Optionally scoped to one company. Classify one with classify_filing.",
            tools::tool_schema::<reads::UnclassifiedFilingsRef>,
            reads::list_unclassified_filings_handler,
        ),
        // ---- Raw tagged-fact capture (ADR 0100 decision 11) ----------------
        exposed_read(
            "get_report_tagged_fact_coverage",
            "get_report_tagged_fact_coverage",
            "How much of one company's tagged filings reached Fundamentals, and where the rest went: comparatives, dimensional breakdowns, note-level figures, positions awaiting a name, and conflicts. Every captured number is either projected or has a stated reason it is not.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_report_tagged_fact_coverage_handler,
        ),
        exposed_read(
            "get_pipeline_reextraction_progress",
            "get_pipeline_reextraction_progress",
            "Progress of one company's latest re-extraction batch (re-armed runs, how many have terminated, how many failed). A null batch means the company never ran one.",
            tools::tool_schema::<reads::CompanyRef>,
            reads::get_pipeline_reextraction_progress_handler,
        ),
    ]
}
