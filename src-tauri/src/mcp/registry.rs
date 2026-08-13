//! The MCP tool registry: the single classification + dispatch table over the
//! typed command layer (ADR 0088 decisions 1–3).
//!
//! Every Tauri command has exactly one **capability tier** here — `read`, `act`,
//! or `excluded` — so a command can never silently leak into (or stay out of)
//! the MCP surface undecided; the classification gate ([`tests`]) derives the
//! full command inventory from the real `invoke_handler` registration point and
//! fails on any unclassified command.
//!
//! A subset of `read` entries is **exposed**: dispatchable now over the
//! `tools/list` and `tools/call` methods, with a schemars-generated
//! `inputSchema` and a handler. This
//! M1 slice exposes exactly the four MVP read tools; the broad UI-parity read
//! wave (M2) and the provenance-gated `act` writes (M3) only **add** exposed
//! entries / flip [`RegistryEntry::exposed`] — the shape below is designed so
//! neither later slice restructures the table.
//!
//! `act` writes must carry provenance (ADR 0088 dec. 3): the enum +
//! [`validate_provenance`] scaffold is landed here now and enforced at `act`
//! dispatch in M3.

use serde_json::Value;

use super::tools::{
    self, GetCompanyDossierInput, GetQualityAssessmentInput, ListClaimsDueInput,
    SearchResearchInput, ToolCallError, ToolOutcome,
};
use super::{acts, reads};
use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage;

// ============================================================================
// Capability tiers + provenance
// ============================================================================

/// The MCP capability tier of a command (ADR 0088 dec. 2). Every command is
/// classified into exactly one; only `read` entries are ever exposed as tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilityTier {
    /// Domain reads — active whenever the MCP server is enabled.
    Read,
    /// Research writes, workspace actions, and job triggers — gated by the
    /// `mcpWritesEnabled` setting (M3), provenance mandatory on writes.
    Act,
    /// Permanent denylist: deletes, undo, settings/credentials mutations, MCP
    /// self-management, dev/diagnostic mutating commands. UI-only, never over MCP.
    Excluded,
}

/// Which provenance carrier an `act` write must present, non-empty, before the
/// boundary accepts it (ADR 0088 dec. 3). Enforced by [`validate_provenance`]
/// at `act` dispatch (M3); the four variants mirror the domain write carriers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvenanceRequirement {
    /// `origins` — a non-empty array of note origins (notebook writes).
    Origins,
    /// `sourceEvidenceId` — the evidence a management claim is anchored to.
    SourceEvidence,
    /// `citationsJson` — a non-empty serialized citation array (qualitative verdicts).
    CitationsJson,
    /// `sourceDocumentRef` — a manual KPI fact's citation. `attribution` is
    /// deliberately NOT an alternate carrier (epic #285 T9): it is the fact's
    /// slot dimension (`total` | `owners_of_parent` | `nci`, hashed into
    /// uniqueness), not a citation — accepting prose there let an agent
    /// satisfy the gate while minting a phantom slot instead of citing a
    /// source.
    FactCitation,
    /// A non-blank `reportDocumentId` AND a non-blank `citation` on EVERY
    /// entry of `facts` (the `record_financial_facts` batch shape, ADR 0093
    /// dec. 6) — never the broken `FactCitation` shape (`attribution` is a
    /// slot dimension, not a citation carrier).
    DocumentAndPerFactCitations,
}

impl ProvenanceRequirement {
    /// The input field(s) a caller must populate to satisfy this requirement —
    /// used in the rejection message.
    fn carrier_label(self) -> &'static str {
        match self {
            ProvenanceRequirement::Origins => "a non-empty `origins` array",
            ProvenanceRequirement::SourceEvidence => "a non-empty `sourceEvidenceId`",
            ProvenanceRequirement::CitationsJson => "a non-empty `citationsJson` array",
            ProvenanceRequirement::FactCitation => "a non-blank `sourceDocumentRef` citation",
            ProvenanceRequirement::DocumentAndPerFactCitations => {
                "a non-blank `reportDocumentId` and a non-blank `citation` on every entry of `facts`"
            }
        }
    }
}

// ============================================================================
// Registry entry
// ============================================================================

/// Schema + wiring for an exposed MCP tool. `schema` is the schemars generator
/// (ADR 0088 dec. 1); `handler` deserializes strict input, runs the tool, and
/// serializes the payload.
#[derive(Clone, Copy)]
pub struct ToolSpec {
    pub description: &'static str,
    pub schema: fn() -> Value,
    pub handler: fn(&AppState, &Value) -> Result<ToolOutcome, ToolCallError>,
}

/// One row of the registry: the classification of a Tauri command plus, when
/// exposed, its tool metadata. `command_name` is the key the classification gate
/// matches against the `invoke_handler` inventory (unique per command).
pub struct RegistryEntry {
    /// The MCP tool name advertised over `tools/list`. For classification-only
    /// rows this mirrors `command_name`; for exposed tools it is the tool's
    /// public name (which may differ — e.g. `get_company_dossier` over
    /// `get_fundamentals_coverage`).
    pub tool_name: &'static str,
    /// The Tauri command this row classifies (the classification-gate key).
    pub command_name: &'static str,
    pub tier: CapabilityTier,
    /// The provenance carrier required for `act` writes; `None` for reads,
    /// excluded commands, and non-provenance-carrying `act` actions.
    pub provenance: Option<ProvenanceRequirement>,
    /// Dispatchable now? `true` ⇒ listed in `tools/list` and routable in
    /// `tools/call`. M2/M3 flip this per tool as coverage lands.
    pub exposed: bool,
    /// Tool schema + handler; present iff `exposed`.
    pub spec: Option<ToolSpec>,
}

// ---- Entry constructors ----------------------------------------------------

/// A non-exposed `read` classification row (a read command the broad M2 wave
/// will expose; not yet listed in `tools/list`).
fn read(command: &'static str) -> RegistryEntry {
    RegistryEntry {
        tool_name: command,
        command_name: command,
        tier: CapabilityTier::Read,
        provenance: None,
        exposed: false,
        spec: None,
    }
}

/// An `act` classification row, with the provenance carrier it will require at
/// M3 dispatch (`None` for non-provenance-carrying actions/triggers).
fn act(command: &'static str, provenance: Option<ProvenanceRequirement>) -> RegistryEntry {
    RegistryEntry {
        tool_name: command,
        command_name: command,
        tier: CapabilityTier::Act,
        provenance,
        exposed: false,
        spec: None,
    }
}

/// An `excluded` (permanent denylist) classification row.
fn excluded(command: &'static str) -> RegistryEntry {
    RegistryEntry {
        tool_name: command,
        command_name: command,
        tier: CapabilityTier::Excluded,
        provenance: None,
        exposed: false,
        spec: None,
    }
}

/// An exposed `read` tool over one backing command.
fn exposed_read(
    tool_name: &'static str,
    command_name: &'static str,
    description: &'static str,
    schema: fn() -> Value,
    handler: fn(&AppState, &Value) -> Result<ToolOutcome, ToolCallError>,
) -> RegistryEntry {
    RegistryEntry {
        tool_name,
        command_name,
        tier: CapabilityTier::Read,
        provenance: None,
        exposed: true,
        spec: Some(ToolSpec {
            description,
            schema,
            handler,
        }),
    }
}

/// An exposed `act` (write) tool over one backing command (ADR 0088 M3). Listed
/// in `tools/list` ALWAYS (discoverability); the write gate — `mcpWritesEnabled`
/// and, where present, the `provenance` carrier — is enforced at `tools/call`
/// dispatch ([`call`]), never by hiding the tool.
fn exposed_act(
    command_name: &'static str,
    provenance: Option<ProvenanceRequirement>,
    description: &'static str,
    schema: fn() -> Value,
    handler: fn(&AppState, &Value) -> Result<ToolOutcome, ToolCallError>,
) -> RegistryEntry {
    RegistryEntry {
        tool_name: command_name,
        command_name,
        tier: CapabilityTier::Act,
        provenance,
        exposed: true,
        spec: Some(ToolSpec {
            description,
            schema,
            handler,
        }),
    }
}

// ============================================================================
// The registry
// ============================================================================

/// The full registry: every command classified exactly once. The four exposed
/// MVP read tools come first (fixing `tools/list` order — the frozen contract),
/// then every remaining command's classification.
pub fn entries() -> Vec<RegistryEntry> {
    let mut entries = exposed_tools();
    entries.extend(read_wave_tools());
    entries.extend(act_wave_tools());
    entries.extend(classifications());
    entries
}

/// The four exposed MVP read tools (ADR 0078 dec. 5), each over its backing
/// command. `get_company_dossier`/`get_quality_assessment` are composite reads;
/// they are attached to their primary backing command (fundamentals coverage /
/// framework evaluations) — the other commands they read through carry their own
/// `read` classification rows below.
fn exposed_tools() -> Vec<RegistryEntry> {
    vec![
        exposed_read(
            "get_company_dossier",
            "get_fundamentals_coverage",
            "One company's research dossier: identity, fundamentals coverage per fiscal period, confirmed financial facts, and quality-scorecard summaries. Sourced from the user's own research; decision support only.",
            tools::tool_schema::<GetCompanyDossierInput>,
            tools::get_company_dossier_handler,
        ),
        exposed_read(
            "search_research",
            "search",
            "Full-text search across the user's research workspace (notes, report documents, transcripts, claims, facts). Returns ranked matches with snippets.",
            tools::tool_schema::<SearchResearchInput>,
            tools::search_research_handler,
        ),
        exposed_read(
            "list_claims_due",
            "list_claims_to_verify",
            "Management claims whose verification period has arrived (due), passed (overdue), or is approaching (upcoming), per company.",
            tools::tool_schema::<ListClaimsDueInput>,
            tools::list_claims_due_handler,
        ),
        exposed_read(
            "get_quality_assessment",
            "list_framework_evaluations",
            "Quality-framework state for one company: the latest stored scorecard evaluation per framework, plus stored qualitative verdicts. The in-app qualitative-assessment writer was retired (ADR 0084) — this tool reads only stored verdicts; agents record new verdicts with provenance via the `set_qualitative_verdicts` write-tool (until then a criterion reads as unassessed). Decision support only — never an investment recommendation.",
            tools::tool_schema::<GetQualityAssessmentInput>,
            tools::get_quality_assessment_handler,
        ),
    ]
}

/// The broad UI-parity **read wave** (ADR 0088 dec. 2 `read` tier): one exposed
/// tool per domain read so a connected agent can READ every domain. Handlers +
/// strict inputs live in [`super::reads`]; each backing command's classification
/// row is carried HERE (removed from [`classifications`]) so the gate still
/// classifies every command exactly once. Order fixes `tools/list` after the
/// four MVP tools — the frozen snapshot contract.
fn read_wave_tools() -> Vec<RegistryEntry> {
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
    ]
}

/// The exposed **act** (write) wave (ADR 0088 dec. 2/3, M3): the meaningful
/// agent write surface — every provenance-carrying research write, the broad
/// no-provenance research writes, workspace actions, and the light/fail-fast job
/// triggers. Handlers + input types live in [`super::acts`]; each backing
/// command's classification row is carried HERE (removed from [`classifications`])
/// so the gate still classifies every command exactly once. Every row is listed
/// in `tools/list` ALWAYS — the write gate (`mcpWritesEnabled` + provenance) is
/// enforced at call time ([`call`]), never by hiding the tool.
fn act_wave_tools() -> Vec<RegistryEntry> {
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
    ]
}

/// Classification of every command NOT already carried by an exposed tool row.
/// Seeded per ADR 0088 dec. 2 (`read` = domain reads; `act` = research writes /
/// workspace actions / job triggers, provenance named where a carrier exists;
/// `excluded` = deletes, undo, settings/credentials, MCP self-management,
/// dev/diagnostic mutations). Reviewed by the orchestrator; the classification
/// gate keeps it exhaustive.
fn classifications() -> Vec<RegistryEntry> {
    vec![
        // ---- Reads classified but deliberately NOT exposed -----------------
        // The broad read wave (M2) exposes the LIST + targeted GET per domain
        // via `read_wave_tools()`; the reads below stay `read`-tier but
        // unexposed. Every skip carries its one-line justification — a silent
        // skip is a defect (ADR 0088 dec. 2). Grouped by why.
        //
        // Infra / liveness / diagnostics — no investor-facing data:
        read("health"),                     // process liveness probe
        read("database_status"),            // DB engine/migration diagnostics
        read("backup_status"),              // local-backup infra status
        read("get_history_sweep_progress"), // quote-backfill job progress (UI)
        read("get_backfill_progress"),      // backfill job progress (UI)
        read("get_scheduler_status"),       // scheduler/ops status
        read("list_diagnostic_events"),     // diagnostics/ops event log
        read("get_diagnostic_summary"),     // diagnostics/ops rollup
        read("list_source_reconciliation"), // source-reconciliation diagnostics
        read("get_log_status"),             // log-file status (ops)
        read("list_log_entries"),           // app log lines (ops)
        read("get_local_metrics_snapshot"), // local perf metrics (ops)
        // Settings / config / credentials — sensitive or UI-only config:
        read("get_settings"),       // app settings surface (UI/config)
        read("get_license_status"), // licensing state (config)
        read("get_provider_credential_status"), // credential presence (sensitive)
        read("list_source_adapters"), // source-adapter enable/config catalog
        read("list_cockpit_layouts"), // UI dashboard layout persistence
        read("list_company_autopilot_modes"), // autopilot-mode picker presets (UI)
        read("get_company_autopilot"), // per-company autopilot mode (config; runs exposed)
        // Reference / lookup / autocomplete plumbing:
        read("lookup_company"), // registry autocomplete + directory bootstrap; tools resolve tickers internally
        read("list_company_registry_entries"), // GPW registry directory dump (reference)
        read("get_company_ir_reports_url"), // single config URL (subset of get_company_basic_info intent)
        read("get_company_sector"),         // subset of get_company_basic_info
        read("list_company_sectors"),       // sector-picker preset values (UI)
        read("list_available_metric_keys"), // criterion-editor autocomplete (UI)
        read("validate_criterion_expression"), // criterion-editor validator (authoring aid)
        read("list_kpi_relevance"),         // per-company KPI-relevance config (scorecard editor)
        // Superseded by a richer exposed tool:
        read("list_report_documents"), // superseded by get_report_documents_view (adds period + canonical flag)
        read("get_pre_report_card"), // pre-report UI card; data via list_report_season + list_report_expectations
        read("expectation_review"), // derived per-event review panel; underlying data via list_report_expectations
        read("get_framework_evaluation"), // single stored evaluation; latest-per-framework via get_quality_assessment
        read("get_quality_framework"), // one framework's detail; catalog via list_quality_frameworks
        read("list_fact_provenance"), // folded into list_financial_facts (provenance travels with each fact)
        // Data-management import/export plumbing (destructive counterparts excluded):
        read("export_research_data"), // bulk export dump; content via search + per-domain reads
        read("preview_research_import"), // import-preview plumbing
        read("export_settings_data"), // settings export (config)
        read("preview_settings_import"), // settings import-preview plumbing
        // Research-workspace internals / aggregates already covered elsewhere:
        read("list_research_evidence"), // research timeline aggregates already-exposed per-domain reads
        read("list_company_timeline"),  // company-scoped alias of list_research_evidence
        read("list_watchlist_timeline"), // watchlist-scoped alias of list_research_evidence
        read("list_research_review_state"), // per-scope "reviewed" checkpoints (UI markers)
        read("list_evidence_links"), // internal evidence-graph edges (UI); agent reads the items directly
        read("list_research_reminders"), // personal follow-up reminders (UI workflow)
        // list_alert_rules is now an exposed read (act_wave companions read their
        // own rules) — carried by read_wave_tools().
        // Triage surfaces: list_unmatched_source_items stays classified-only
        // (unmatched-source triage is a separate family, no tool this slice);
        // list_flagged_extraction_outcomes + list_unclassified_filings are now
        // EXPOSED reads — carried by read_wave_tools() (ADR 0088 dec. 4, M4).
        read("list_unmatched_source_items"), // unmatched-source triage (no MCP tool yet)
        // ---- Act: exposed writes -------------------------------------------
        // Every provenance-carrying write, the broad no-provenance research
        // writes, workspace actions, and the light/fail-fast job triggers are
        // now EXPOSED — carried by `act_wave_tools()` (removed from here so each
        // command is classified exactly once). Only the act commands NOT yet
        // exposed remain below, each with its one-line justification (a silent
        // skip is a defect, ADR 0088 dec. 2).
        //
        // ---- Act: writes classified but deliberately NOT exposed -----------
        // Niche period plumbing (facts reference an existing periodId; period
        // create/update is a rare authoring step, UI-driven):
        act("create_financial_period", None),
        act("update_financial_period", None),
        // Framework cloning (whole-framework duplication; niche authoring aid):
        act("clone_framework", None),
        // reset_framework_to_template destroys user customization (delete-class,
        // orchestrator ruling at M1 review) — permanently UI-only.
        excluded("reset_framework_to_template"),
        // Company config edits (single-field URL/sector setters; UI config):
        act("set_company_ir_reports_url", None),
        act("set_company_sector", None),
        act("rename_watchlist", None), // watchlist rename (UI config)
        act("rename_cockpit_layout", None), // saved-view rename (UI config, issue #89)
        act("resolve_transcript_job_company", None), // transcript-triage UI step
        // Report-pipeline job triggers (multi-stage document machinery; UI-driven
        // per-document, not a clean headless agent surface):
        act("extract_report_sections", None),
        act("fetch_report_document", None),
        act("reclassify_report_documents", None),
        act("resolve_ir_report", None),
        act("extract_report_document_data", None),
        // Admin / one-off job triggers — genuinely not part of the agent research
        // surface (whole-corpus rebuilds, quote-history sweeps, registry refresh,
        // and derived-fact backfills that the exposed extraction/refresh tools
        // already cover). (The networked source-refresh / extraction / company
        // backfill triggers ARE exposed — see `act_wave_tools`.)
        act("backfill_company_health_facts", None),
        act("backfill_ownership_extraction", None),
        act("run_history_sweep", None),
        act("rebuild_fundamentals", None),
        act("refresh_gpw_company_registry", None),
        act("refresh_gpw_company_registry_if_stale", None),
        // Video-transcript lifecycle (in-app AI transcript provider; UI-driven
        // enqueue/run, and the only in-app AI dependency — kept off the agent
        // surface):
        act("create_video_transcript_job", None),
        act("update_video_transcript_job", None),
        act("run_video_transcript_job", None),
        // Local-filesystem write (the export save path; issue #106) — an agent
        // must never write arbitrary files on the owner's machine; agents read
        // exports through export_research_data/export_settings_data instead.
        excluded("write_export_file"),
        // ---- Excluded: deletes ---------------------------------------------
        excluded("delete_company"),
        excluded("delete_watchlist"),
        excluded("delete_cockpit_layout"),
        excluded("delete_research_question"),
        excluded("delete_evidence_link"),
        excluded("delete_research_reminder"),
        excluded("delete_notebook_entry"),
        excluded("delete_management_claim"),
        excluded("delete_financial_period"),
        excluded("delete_kpi_relevance"),
        excluded("delete_financial_fact"),
        excluded("delete_quality_framework"),
        excluded("delete_framework_criterion"),
        excluded("delete_framework_evaluation"),
        excluded("delete_video_transcript_job"),
        excluded("delete_alert_rule"),
        // ---- Excluded: undo ------------------------------------------------
        excluded("undo_autopilot_run"),
        // ---- Excluded: bulk import / backups (destructive data management) --
        excluded("apply_research_import"),
        excluded("apply_settings_import"),
        excluded("create_backup"),
        excluded("restore_backup"),
        // ---- Excluded: settings / configuration ----------------------------
        excluded("update_settings"),
        // UI-session semantics: Today's bulk "was on screen" seen-marking for the
        // sidebar badge (ADR 0097 dec. 5). Agents mark individual events via the
        // exposed mark_attention_event_seen.
        excluded("mark_attention_events_seen"),
        excluded("save_cockpit_layout"),
        excluded("set_source_adapter_enabled"),
        excluded("set_company_autopilot"),
        excluded("set_companies_autopilot"),
        // ---- Excluded: credentials / licensing -----------------------------
        excluded("submit_license_key"),
        excluded("clear_license_key"),
        excluded("set_provider_api_key"),
        excluded("clear_provider_api_key"),
        // ---- Excluded: MCP self-management (reads included — sensitive) -----
        excluded("regenerate_mcp_token"),
        excluded("revoke_mcp_token"),
        excluded("mcp_token_status"),
        excluded("set_mcp_enabled"),
        excluded("mcp_status"),
        // ---- Excluded: dev / diagnostics mutations + OS side effects -------
        excluded("clear_diagnostic_events"),
        excluded("open_logs_directory"),
        excluded("disable_developer_mode"),
        excluded("unlock_developer_mode"),
    ]
}

// ============================================================================
// Provenance validation (scaffold for M3 `act` dispatch, ADR 0088 dec. 3)
// ============================================================================

/// Reject an `act` write whose provenance carrier is absent or empty with a
/// typed `invalid_input` error — never an "empty default" (ADR 0088 dec. 3).
/// Wired into `act` dispatch in M3; unit-tested here now.
pub fn validate_provenance(
    requirement: ProvenanceRequirement,
    input: &Value,
) -> Result<(), CommandError> {
    let satisfied = match requirement {
        // A non-empty `origins` array (notebook create) OR a non-empty
        // `transcriptSegmentIds` selection (the transcript-note origin).
        ProvenanceRequirement::Origins => {
            nonempty_array(input, "origins") || nonempty_array(input, "transcriptSegmentIds")
        }
        ProvenanceRequirement::SourceEvidence => nonempty_str(input, "sourceEvidenceId"),
        // A top-level `citationsJson` (single-verdict shape) OR a non-empty
        // `results` array where every entry carries its own non-empty
        // `citationsJson` (the `set_qualitative_verdicts` batch shape).
        ProvenanceRequirement::CitationsJson => {
            citations_nonempty(input.get("citationsJson")) || results_all_cited(input)
        }
        // #285 T9: `attribution` is the fact's slot dimension (`total` |
        // `owners_of_parent` | `nci`), never an alternate citation carrier —
        // accepting it here let an agent satisfy the gate with prose that
        // minted a phantom uniqueness slot instead of citing a source.
        ProvenanceRequirement::FactCitation => nonempty_str(input, "sourceDocumentRef"),
        ProvenanceRequirement::DocumentAndPerFactCitations => {
            nonempty_str(input, "reportDocumentId") && facts_all_cited(input)
        }
    };
    if satisfied {
        Ok(())
    } else {
        Err(CommandError::new(
            CommandErrorCode::ProvenanceRequired,
            format!(
                "this write must carry provenance: {}",
                requirement.carrier_label()
            ),
        ))
    }
}

/// True when `input.results` is a non-empty array and every entry carries a
/// non-empty serialized `citationsJson` array (the batch-verdict provenance
/// shape used by `set_qualitative_verdicts`).
fn results_all_cited(input: &Value) -> bool {
    input
        .get("results")
        .and_then(Value::as_array)
        .map(|results| {
            !results.is_empty()
                && results
                    .iter()
                    .all(|result| citations_nonempty(result.get("citationsJson")))
        })
        .unwrap_or(false)
}

/// True when `input.facts` is a non-empty array and every entry carries a
/// non-blank `citation` (the `record_financial_facts` per-fact provenance
/// shape, ADR 0093 dec. 6) — a blank citation on ANY entry fails the WHOLE
/// batch before the handler runs (atomic refusal, never a partial write).
fn facts_all_cited(input: &Value) -> bool {
    input
        .get("facts")
        .and_then(Value::as_array)
        .map(|facts| !facts.is_empty() && facts.iter().all(|fact| nonempty_str(fact, "citation")))
        .unwrap_or(false)
}

/// True when `input[key]` is a present, non-empty array.
fn nonempty_array(input: &Value, key: &str) -> bool {
    input
        .get(key)
        .and_then(Value::as_array)
        .map(|array| !array.is_empty())
        .unwrap_or(false)
}

/// True when `input[key]` is a present, non-blank string.
fn nonempty_str(input: &Value, key: &str) -> bool {
    input
        .get(key)
        .and_then(Value::as_str)
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

/// True when the value is a string holding a JSON array with at least one entry.
fn citations_nonempty(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
        .and_then(|parsed| parsed.as_array().map(|array| !array.is_empty()))
        .unwrap_or(false)
}

// ============================================================================
// Dispatch (the single routing surface for protocol.rs)
// ============================================================================

/// The `tools` array of the `tools/list` response, built from the exposed
/// registry entries (never a hand-rolled list). The insta snapshot of the full
/// response freezes this contract (ADR 0078 G-1).
pub fn descriptors() -> Value {
    Value::Array(
        entries()
            .into_iter()
            .filter(|entry| entry.exposed)
            .filter_map(|entry| {
                entry.spec.map(|spec| {
                    serde_json::json!({
                        "name": entry.tool_name,
                        "description": spec.description,
                        "inputSchema": (spec.schema)(),
                    })
                })
            })
            .collect(),
    )
}

/// Route a `tools/call` by name through the registry. `read` tools dispatch
/// directly; `act` (write) tools pass the write gate first (ADR 0088 M3): the
/// live `mcpWritesEnabled` setting, then — for a provenance-carrying write — the
/// [`validate_provenance`] check on the raw arguments BEFORE the handler runs, so
/// a rejected write never touches storage. Both gate rejections are typed
/// domain failures (`ToolOutcome::Failure`, `isError: true`) — never protocol
/// errors. An unexposed or unknown name is `UnknownTool`.
pub fn call(state: &AppState, name: &str, arguments: &Value) -> Result<ToolOutcome, ToolCallError> {
    for entry in entries() {
        if entry.exposed && entry.tool_name == name {
            if let Some(spec) = entry.spec {
                if entry.tier == CapabilityTier::Act {
                    if let Some(gate) = act_gate(state, entry.provenance, arguments) {
                        return Ok(ToolOutcome::Failure(gate));
                    }
                }
                return (spec.handler)(state, arguments);
            }
        }
    }
    Err(ToolCallError::UnknownTool(name.to_owned()))
}

/// The `act`-tier write gate (ADR 0088 M3). Returns `Some(error)` when the call
/// must be rejected before the handler runs — `writes_disabled` when the live
/// setting is off, `provenance_required` when a mandated carrier is empty — or
/// `None` when the write may proceed.
fn act_gate(
    state: &AppState,
    provenance: Option<ProvenanceRequirement>,
    arguments: &Value,
) -> Option<CommandError> {
    let writes_enabled = match state.get_settings() {
        Ok(settings) => settings.mcp.writes_enabled,
        Err(error) => return Some(CommandError::from(error)),
    };
    if !writes_enabled {
        return Some(CommandError::new(
            CommandErrorCode::WritesDisabled,
            "the MCP write tier is disabled — enable it in Settings → MCP server (write tools \
             require citations; deletes and settings stay UI-only)",
        ));
    }
    if let Some(requirement) = provenance {
        if let Err(error) = validate_provenance(requirement, arguments) {
            return Some(error);
        }
    }
    None
}

/// Frozen exposed-tool count — the single source of truth for BOTH assertion
/// sites (the registry descriptor test below and the server `tools/list`
/// round-trip in `mcp::server`). Adding a tool bumps exactly this constant.
/// Itemization: 44 read (4 MVP + 34 read wave + get_kpi_comparison +
/// get_sector_percentiles + list_valuation_runs (ADR 0089) + list_alert_rules +
/// list_flagged_extraction_outcomes + list_unclassified_filings) + 58 act incl.
/// compute_comparative_valuation (ADR 0089), classify_filing, ADR 0088 dec. 2/3/4,
/// record_financial_facts (ADR 0093 dec. 6, epic #285 T7 — MCP-only, no Tauri
/// command twin), capture_report_document (ADR 0093 dec. 5, epic #285 T8 —
/// gated fetch, promoted from classified-but-unexposed).
#[cfg(test)]
pub(crate) const FROZEN_EXPOSED_TOOL_COUNT: usize = 102;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeSet;

    /// ADR 0088 dec. 2 classification gate. The command inventory is derived
    /// from the REAL registration point — the `invoke_handler` block in
    /// `lib.rs` — never a hand-copied list, so a new command that is neither
    /// exposed nor denylisted fails this test until it is classified.
    #[test]
    fn every_registered_command_is_classified() {
        let source = include_str!("../lib.rs");
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

        let tools = descriptors();
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
        let listed: BTreeSet<String> = descriptors()
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
            let outcome = call(&state, name, arguments)
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
            "create_management_claim",
            &json!({ "companyId": company_id, "statement": "guidance" }),
        )
        .expect("domain outcome");
        assert_eq!(failure_code(outcome), CommandErrorCode::ProvenanceRequired);

        // FactCitation — create fact without a citation.
        let outcome = call(
            &state,
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

        let listed: BTreeSet<String> = descriptors()
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
            let off = call(&state, name, arguments).expect("domain outcome");
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
            let outcome = call(&state, name, arguments)
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
            validate_provenance(Origins, &json!({ "origins": [{ "sourceType": "report" }] }))
                .is_ok()
        );

        // SourceEvidence — a non-blank sourceEvidenceId.
        assert!(validate_provenance(SourceEvidence, &json!({})).is_err());
        assert!(validate_provenance(SourceEvidence, &json!({ "sourceEvidenceId": "  " })).is_err());
        assert!(
            validate_provenance(SourceEvidence, &json!({ "sourceEvidenceId": "ev_1" })).is_ok()
        );

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
        assert!(
            validate_provenance(FactCitation, &json!({ "sourceDocumentRef": "doc_1" })).is_ok()
        );
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
            "capture_report_document",
            &json!({ "companyId": company_id, "url": url }),
        )
        .expect("domain outcome");
        let second_id = match second {
            ToolOutcome::Success(value) => {
                value["documentId"].as_str().expect("documentId").to_owned()
            }
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

        let listed: BTreeSet<String> = descriptors()
            .as_array()
            .expect("tools array")
            .iter()
            .map(|tool| tool["name"].as_str().expect("name").to_owned())
            .collect();
        assert!(listed.contains("list_unclassified_filings"));
        assert!(listed.contains("classify_filing"));

        // The read tool returns the seeded filing.
        let read = call(&state, "list_unclassified_filings", &json!({})).expect("domain outcome");
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
}
