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

use super::tools::{ToolCallError, ToolOutcome};
use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};

mod provenance;
#[cfg(test)]
mod tests;
mod tools_acquisition;
mod tools_act;
mod tools_classification;
mod tools_read;

use provenance::validate_provenance;
use tools_acquisition::{exposed_tools, kpi_acquisition_tools};
use tools_act::act_wave_tools;
use tools_classification::classifications;
use tools_read::read_wave_tools;

// ============================================================================
// Capability tiers + provenance
// ============================================================================

/// The authenticated identity of an MCP request (ADR 0099 dec. 2). Resolved
/// by the server from the matched bearer digest and threaded through
/// `dispatch` → `descriptors`/`call`: `Full` sees the whole exposed surface;
/// `KpiAcquisition` sees only [`KPI_ACQUISITION_TOOLS`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpScope {
    Full,
    KpiAcquisition,
}

/// The acquisition scope's tool allowlist — exactly the ten workflow tools,
/// nothing else (ADR 0099 dec. 3; widening it is a deliberate ADR change).
/// Complete at nine since #386; `propose_kpi_definition` joined as the tenth
/// (ADR 0101, epic #399 S4). Declared in `entries()` order (the ordered wire
/// contract, contract positions 1-10).
pub const KPI_ACQUISITION_TOOLS: &[&str] = &[
    "start_kpi_ingest",
    "list_pending_kpi_ingests",
    "get_kpi_ingest_context",
    "get_kpi_ingest_document",
    "stage_kpi_observations",
    "propose_kpi_definition",
    "validate_kpi_ingest",
    "commit_kpi_ingest",
    "get_kpi_ingest_status",
    "cancel_kpi_ingest",
];

impl McpScope {
    /// Whether a tool name exists on this scope's surface. Out-of-scope names
    /// are indistinguishable from unknown tools (`-32602`).
    fn allows(self, tool_name: &str) -> bool {
        match self {
            McpScope::Full => true,
            McpScope::KpiAcquisition => KPI_ACQUISITION_TOOLS.contains(&tool_name),
        }
    }
}

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
    pub handler: ToolHandler,
}

/// A tool's dispatch fn. Almost every handler is scope-blind (`Plain`);
/// `Scoped` passes the authenticated [`McpScope`] through for the one consumer
/// that derives its lease holder from the credential (`start_kpi_ingest`,
/// ADR 0099 dec. 4).
#[derive(Clone, Copy)]
pub enum ToolHandler {
    Plain(fn(&AppState, &Value) -> Result<ToolOutcome, ToolCallError>),
    Scoped(fn(&AppState, McpScope, &Value) -> Result<ToolOutcome, ToolCallError>),
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
            handler: ToolHandler::Plain(handler),
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
            handler: ToolHandler::Plain(handler),
        }),
    }
}

/// [`exposed_act`], but the handler receives the authenticated scope
/// (`ToolHandler::Scoped`) — consumers: `start_kpi_ingest` (#384),
/// `stage_kpi_observations` (#386) and `propose_kpi_definition` (ADR 0101,
/// epic #399 S4), the three lease-holding tools.
fn exposed_act_scoped(
    command_name: &'static str,
    provenance: Option<ProvenanceRequirement>,
    description: &'static str,
    schema: fn() -> Value,
    handler: fn(&AppState, McpScope, &Value) -> Result<ToolOutcome, ToolCallError>,
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
            handler: ToolHandler::Scoped(handler),
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
    entries.extend(kpi_acquisition_tools());
    entries.extend(classifications());
    entries
}

// ============================================================================
// Dispatch (the single routing surface for protocol.rs)
// ============================================================================

/// The `tools` array of the `tools/list` response, built from the exposed
/// registry entries (never a hand-rolled list), filtered to the caller's
/// scope (ADR 0099 dec. 3). The insta snapshot of the Full-scope response
/// freezes that contract (ADR 0078 G-1).
pub fn descriptors(scope: McpScope) -> Value {
    Value::Array(
        entries()
            .into_iter()
            .filter(|entry| entry.exposed && scope.allows(entry.tool_name))
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
/// errors. An unexposed or unknown name is `UnknownTool` — and so is any name
/// outside the caller's scope allowlist (ADR 0099 dec. 3: the surface does
/// not exist for that identity).
pub fn call(
    state: &AppState,
    scope: McpScope,
    name: &str,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    if !scope.allows(name) {
        return Err(ToolCallError::UnknownTool(name.to_owned()));
    }
    for entry in entries() {
        if entry.exposed && entry.tool_name == name {
            if let Some(spec) = entry.spec {
                // The acquisition scope bypasses the Full-scope writes toggle
                // by design (ADR 0099 dec. 2): its gate already ran at
                // authentication (`kpiAcquisitionEnabled` — disabled means the
                // token never authenticates), and without the bypass the
                // unattended credential is dead whenever `mcpWritesEnabled` is
                // off. Full-scope calls stay gated like every act tool.
                if entry.tier == CapabilityTier::Act && scope == McpScope::Full {
                    if let Some(gate) = act_gate(state, entry.provenance, arguments) {
                        return Ok(ToolOutcome::Failure(gate));
                    }
                }
                return match spec.handler {
                    ToolHandler::Plain(handler) => handler(state, arguments),
                    ToolHandler::Scoped(handler) => handler(state, scope, arguments),
                };
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
/// #384 (+4, all MCP-only): 46 read (+ list_pending_kpi_ingests +
/// get_kpi_ingest_status) + 60 act (+ start_kpi_ingest + cancel_kpi_ingest).
/// #385 (+2, MCP-only): 48 read (+ get_kpi_ingest_context +
/// get_kpi_ingest_document).
/// #386 (+3, MCP-only): 63 act (+ stage_kpi_observations + validate_kpi_ingest
/// + commit_kpi_ingest) — the nine-tool acquisition surface is complete.
///
/// #398 (ADR 0100 decision 11, three more): 50 read, adding
/// `get_report_tagged_fact_coverage` and `get_pipeline_reextraction_progress`;
/// 64 act, adding `run_pipeline_reextraction`.
/// `promote_uncrosswalked_concept` stays permanently `Excluded` — an agent may
/// re-read and measure, only the owner may name (decision 10).
///
/// #399 S4 (ADR 0101, +1, MCP-only): 65 act, adding `propose_kpi_definition`
/// — the ten-tool acquisition surface.
#[cfg(test)]
pub(crate) const FROZEN_EXPOSED_TOOL_COUNT: usize = 115;
