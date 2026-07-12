//! Read-only MCP server MVP ([ADR 0078](../../../docs/adr/0078-mcp-external-surface.md)).
//!
//! A second **driving adapter** (ADR 0039) over the existing `AppState` read
//! models: a hand-rolled MCP Streamable-HTTP subset served by `tiny_http` on
//! `127.0.0.1:<port>` only. The module is deliberately layered so each piece is
//! testable without the others:
//!
//! - [`protocol`] — pure JSON-RPC 2.0 dispatcher (no HTTP, no IO beyond the
//!   `AppState` reads the tools perform).
//! - [`tools`] — the four read-only tools, their strict input types, and the
//!   hand-written JSON Schemas returned by `tools/list` (the insta snapshot of
//!   that response is the **frozen tool contract**, ADR 0078 G-1).
//! - [`server`] — the loopback-only `tiny_http` transport: bearer-token auth
//!   (constant-time SHA-256 digest compare), Host-header validation, 1 MiB
//!   body cap, graceful stop.
//!
//! Lifecycle wiring (enable/disable from Settings, `mcp_status`) is owned by
//! the `lib.rs` setup closure and the M3 slice — this module only provides the
//! pieces. **Read-only guarantee:** tools call read paths only; adding any
//! mutating tool requires a new ADR (ADR 0078 G-2 tripwire).

pub mod lifecycle;
pub mod protocol;
pub mod server;
pub mod tools;
