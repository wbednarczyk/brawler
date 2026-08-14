//! The local MCP server ([ADR 0078](../../../docs/adr/0078-mcp-external-surface.md),
//! writes per [ADR 0088](../../../docs/adr/0088-mcp-write-tier.md), acquisition
//! scope per [ADR 0099](../../../docs/adr/0099-acquisition-mcp-surface-mechanics.md)).
//!
//! A second **driving adapter** (ADR 0039) over the existing `AppState` domain
//! stores: a hand-rolled MCP Streamable-HTTP subset served by `tiny_http` on
//! `127.0.0.1:<port>` only. Two bearer credentials resolve to two scopes
//! (`Full`, `KpiAcquisition`); the module is layered so each piece is testable
//! without the others:
//!
//! - [`protocol`] — pure JSON-RPC 2.0 dispatcher (no HTTP).
//! - [`registry`] — every command classified exactly once; tool wiring, the
//!   scope allowlist, the act-tier write gate; the frozen `tools/list`
//!   snapshot is the tool contract (ADR 0078 G-1).
//! - [`tools`]/[`reads`]/[`acts`] — the read/act tool handlers and inputs.
//! - [`kpi_ingest`] — the acquisition-workflow tools (ADR 0099, #384+): run
//!   lifecycle over MCP with credential-owned leases and content-addressed
//!   source blobs.
//! - [`server`] — the loopback-only `tiny_http` transport: dual bearer auth
//!   (constant-time digest compare), Host validation, 1 MiB cap, graceful stop.
//! - [`lifecycle`] — enable/disable/status wiring for both credentials.

pub mod acts;
pub mod kpi_ingest;
pub mod lifecycle;
pub mod protocol;
pub mod reads;
pub mod registry;
pub mod server;
pub mod tools;
