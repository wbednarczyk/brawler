import { callCommand } from "./tauri";
import type { CredentialStatus } from "./types";
import type { McpStatus } from "./generated/McpStatus";
import type { McpTokenGenerated } from "./generated/McpTokenGenerated";

// Read-only MCP server management surface (ADR 0078 decision 4). Two concerns:
// the bearer token lifecycle (keychain-held, shown once) and the live server
// lifecycle (enable → start/stop, status). Types are GENERATED from the Rust
// command layer via ts-rs (ADR 0048): `McpTokenGenerated` from M1's
// `commands/mcp.rs`, `McpStatus` from M3's lifecycle commands.
export type { McpStatus } from "./generated/McpStatus";
export type { McpTokenGenerated } from "./generated/McpTokenGenerated";

/** Whether an MCP token is configured, where it lives, dev-fallback availability. Never carries the token. */
export function mcpTokenStatus() {
  return callCommand<CredentialStatus>("mcp_token_status");
}

/**
 * Generate + store a fresh bearer token, returning the plaintext **exactly
 * once** (ADR 0078 G-3). The caller must never persist `token` beyond the
 * component's transient reveal state.
 */
export function regenerateMcpToken() {
  return callCommand<McpTokenGenerated>("regenerate_mcp_token");
}

/** Remove the bearer token from the keychain; returns the post-clear status. */
export function revokeMcpToken() {
  return callCommand<CredentialStatus>("revoke_mcp_token");
}

/** Start/stop the listener live; returns the resulting server status. */
export function setMcpEnabled(enabled: boolean) {
  return callCommand<McpStatus>("set_mcp_enabled", { enabled });
}

/** Current server status: `{ running, port, error }`. Bind/token errors surface here. */
export function mcpStatus() {
  return callCommand<McpStatus>("mcp_status");
}
