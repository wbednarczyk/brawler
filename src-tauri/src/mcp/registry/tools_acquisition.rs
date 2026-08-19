//! The acquisition-workflow tools plus the four exposed MVP read tools —
//! wired into [`super::entries`].

use super::{exposed_act, exposed_act_scoped, exposed_read, RegistryEntry};
use crate::mcp::tools::{
    self, GetCompanyDossierInput, GetQualityAssessmentInput, ListClaimsDueInput,
    SearchResearchInput,
};

/// The acquisition-workflow tools (ADR 0099, #384) — MCP-only
/// (`tool_name == command_name`, no Tauri command twin), visible to BOTH
/// scopes (Full is a superset; the acquisition allowlist is
/// [`KPI_ACQUISITION_TOOLS`]). Order here == the allowlist == the wire.
pub(super) fn kpi_acquisition_tools() -> Vec<RegistryEntry> {
    use crate::mcp::{kpi_ingest, kpi_ingest_context, kpi_ingest_submit};
    vec![
        exposed_act_scoped(
            "start_kpi_ingest",
            None,
            "Start or resume a KPI ingest run (ADR 0099). Fresh: documentId + profileId \
             (+ optional scope/dataQuality/period) creates the run, claims the lease, pins \
             the source bytes, and enters extraction once context is complete. Resume: runId \
             re-claims idempotently (the explicit keepalive) and attaches missing context \
             set-once. Provenance is the run pipeline itself; no citation carrier here.",
            tools::tool_schema::<kpi_ingest::StartKpiIngestInput>,
            kpi_ingest::start_kpi_ingest_handler,
        ),
        exposed_read(
            "list_pending_kpi_ingests",
            "list_pending_kpi_ingests",
            "List claimable KPI ingest runs (discovered/source_captured/extracting/\
             validation_failed), newest first, keyset-paginated (limit ≤ 50, default 20).",
            tools::tool_schema::<kpi_ingest::ListPendingKpiIngestsInput>,
            kpi_ingest::list_pending_kpi_ingests_handler,
        ),
        exposed_read(
            "get_kpi_ingest_context",
            "get_kpi_ingest_context",
            "Everything one report's extraction needs, within hard budgets (≤256 KiB): run \
             status, document metadata, the derived-period hint, the expected+minted KPI \
             catalog, validator-equivalent plausibility evidence per slot, profile doctrine \
             and repair-manifest access. Sections (catalog/plausibility/manifest) paginate \
             via cursors; the manifest is served only via section calls. Pure read.",
            tools::tool_schema::<kpi_ingest_context::GetKpiIngestContextInput>,
            kpi_ingest_context::get_kpi_ingest_context_handler,
        ),
        exposed_read(
            "get_kpi_ingest_document",
            "get_kpi_ingest_document",
            "Chunked bytes (offset/length ≤ 256 KiB, base64) from the run's content-addressed \
             source blob, verified against the frozen sourceContentHash — the portable \
             document delivery channel. Available once the source is captured. Pure read.",
            tools::tool_schema::<kpi_ingest_context::GetKpiIngestDocumentInput>,
            kpi_ingest_context::get_kpi_ingest_document_handler,
        ),
        exposed_act_scoped(
            "stage_kpi_observations",
            None,
            "Stage the COMPLETE revision snapshot of extracted observations (1..100, with \
             citations) plus the REQUIRED missingReasons declaration ({} = explicitly none), \
             written in the same transaction. A repair resends every retained observation. \
             Requires the caller's live lease. Provenance is the run pipeline itself.",
            tools::tool_schema::<kpi_ingest_submit::StageKpiObservationsInput>,
            kpi_ingest_submit::stage_kpi_observations_handler,
        ),
        exposed_act_scoped(
            "propose_kpi_definition",
            None,
            "Mint (or reuse) a company-scoped, origin=agent KPI catalog entry for a \
             disclosed number the canon has no key for (ADR 0101). Guard order: this \
             company's own minted entry is returned as-is (created: false); a curated \
             kpi_aliases synonym refuses with a typed synonym_redirect naming the \
             canonical key and definitionId; an exact shared-canon key returns the \
             canonical definition (created: false, never a company shadow); only a \
             genuinely new key mints — never fuzzy matching. Page the full catalog \
             (get_kpi_ingest_context) before proposing. Requires the caller's live \
             lease.",
            tools::tool_schema::<kpi_ingest_submit::ProposeKpiDefinitionInput>,
            kpi_ingest_submit::propose_kpi_definition_handler,
        ),
        exposed_act(
            "validate_kpi_ingest",
            None,
            "Validate one staged revision synchronously (generation-pinned). Returns the \
             FULL manifest — a failed manifest is the typed repair report; a raced loser \
             gets outcome=superseded with the current run tuple.",
            tools::tool_schema::<kpi_ingest_submit::ValidateKpiIngestInput>,
            kpi_ingest_submit::validate_kpi_ingest_handler,
        ),
        exposed_act(
            "commit_kpi_ingest",
            None,
            "Atomically commit a ready manifest (runId + manifestHash + revision) and return \
             the immutable receipt. Idempotent: replaying a committed tuple returns the \
             stored receipt verbatim; a stale tuple is a typed conflict.",
            tools::tool_schema::<kpi_ingest_submit::CommitKpiIngestInput>,
            kpi_ingest_submit::commit_kpi_ingest_handler,
        ),
        exposed_read(
            "get_kpi_ingest_status",
            "get_kpi_ingest_status",
            "Full status of one KPI ingest run (state, context, lease, expected KPIs, \
             progress). Pure read — never touches the lease.",
            tools::tool_schema::<kpi_ingest::RunIdInput>,
            kpi_ingest::get_kpi_ingest_status_handler,
        ),
        exposed_act(
            "cancel_kpi_ingest",
            None,
            "Cancel a KPI ingest run in any pre-commit state (releases its lease). \
             Refuses `committing` and terminal states.",
            tools::tool_schema::<kpi_ingest::RunIdInput>,
            kpi_ingest::cancel_kpi_ingest_handler,
        ),
    ]
}

/// The four exposed MVP read tools (ADR 0078 dec. 5), each over its backing
/// command. `get_company_dossier`/`get_quality_assessment` are composite reads;
/// they are attached to their primary backing command (fundamentals coverage /
/// framework evaluations) — the other commands they read through carry their own
/// `read` classification rows below.
pub(super) fn exposed_tools() -> Vec<RegistryEntry> {
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
