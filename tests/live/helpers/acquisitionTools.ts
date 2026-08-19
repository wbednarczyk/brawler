// The acquisition credential's frozen scope: exactly the ten KPI-ingest
// workflow tools, in contract order (ADR 0099 dec. 3, widened to ten by ADR
// 0101 dec. 1 / epic #399 S4; the Rust source of truth is
// `KPI_ACQUISITION_TOOLS` in `src-tauri/src/mcp/registry.rs`, frozen by the
// `tools_list_schema_acquisition` snapshot + the ≤16 KiB gate). Every live spec
// asserts `tools/list` against THIS list, so a scope change reddens one place —
// not each spec, which is how the lifecycle (4) and context (6) specs silently
// rotted while the surface grew to nine (#389).
export const ACQUISITION_TOOLS = [
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
] as const;
