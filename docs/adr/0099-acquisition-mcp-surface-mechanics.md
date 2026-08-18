# ADR 0099: Acquisition MCP surface mechanics

Status: Accepted (2026-08-13, owner decision at epic #353 planning; 4 adversarial review rounds). Amends ADR 0098 (decision 8); implements ADR 0098 decision 9. Implementation: epic #353 (#381–#389).

Deciders: maintainer. Area: MCP port, fundamentals, security.

## Context

Epic #352 delivered the transactional ingestion core (durable runs, staging, deterministic manifests, atomic commit, queue integration) — but headless: `create_run_if_absent` has no production caller and the only agent path is the demoted `record_financial_facts`. Epic #353 builds the real surface: a compact run-based MCP workflow behind an acquisition-scoped credential (ADR 0098 dec. 9). This ADR freezes the mechanics the wire contract in [contracts.md § KPI acquisition workflow tools](../contracts.md) is written against. Wire shapes themselves live in contracts.md; this ADR records the decisions and their rejected alternatives.

## Decisions

### 1. Agent-path validation and commit execute synchronously in the tool call (amends ADR 0098 dec. 8)

`validate_kpi_ingest` runs `validate_kpi_ingest_run` synchronously and returns the full manifest in the response; `commit_kpi_ingest` runs `commit_manifest` synchronously and returns the receipt (idempotent replay per #363; `CommitContention` is typed and retryable). `stage_kpi_observations` does NOT enqueue a validate job. The repair loop closes in one round — validation is deterministic and millisecond-scale; polling `get_kpi_ingest_status` for a result the agent needs in full is pure overhead. The MCP server's single worker thread blocking for milliseconds on a loopback transport with one client is acceptable.

The job queue (`jobs/kpi_ingest_queue`) remains the executor for #354 automation and crash recovery. Convergence of the two paths is required from day one (startup reconciliation arms jobs for `staged`/`ready_to_commit` runs): revision guards in `apply_validation_outcome` already exclude generation corruption; the synchronous loser returns a typed `superseded` result carrying the current `(status, revision, manifestHash)` tuple instead of a bare conflict. The inherited E1 obligations — the single-armer revisit and a generation-guarded `mark_failed` on both terminalization callers — land with the stage/validate/commit tools (#386).

Rejected: stage→enqueue + status polling as the agent path (bloats the repair loop; the whole validation result must return to the agent anyway).

**Amended (2026-08-18, [ADR 0102](0102-full-capture-staging-contract-excluded-observations-and-chunked-drafts.md) dec. 12):** `validate_kpi_ingest` and `commit_kpi_ingest` responses become bounded summaries, not the full manifest/receipt; full data moves to paged `get_kpi_ingest_context` reads.

### 2. A second bearer credential with its own enablement gate

The `kpi_acquisition` credential is a second bearer token in the OS keychain (`brawler/mcp/kpi_acquisition_token`, descriptor beside the primary), with its own generate/rotate/revoke/status commands and a Settings → MCP entry point. The server holds two digests; a digest match resolves to `McpScope::Full | McpScope::KpiAcquisition`, threaded through the whole server → protocol → registry boundary (today auth collapses to a boolean before dispatch and `tools/list` is unconditional — the signatures gain an identity parameter).

`kpiAcquisitionEnabled` (setting, default false, mutable only via the excluded `update_settings` — self-enable stays impossible) gates the ENTIRE scope at authentication: disabled ⇒ the acquisition token is rejected like an unknown token (401), covering reads including document bytes — token revocation is not the only kill switch. Without a separate gate the unattended credential is either dead (`mcpWritesEnabled` off) or forces every write family open on the primary token (on). Full scope is unchanged (global `mcpWritesEnabled` gates its act tools).

Dual-token lifecycle truth table: the primary token is required to start (as today); the acquisition token is optional — absent/unreadable means the scope is simply unavailable (status reports it; server start does not fail); rotating or revoking either token restarts the listener (existing disable→enable pattern; digests are read at start).

Rejected: the global bearer as the acquisition credential (ADR 0098 dec. 9); token-embedded signed claims (loopback transport, overkill).

### 3. The acquisition allowlist is exactly the nine workflow tools

`start_kpi_ingest`, `list_pending_kpi_ingests`, `get_kpi_ingest_context`, `get_kpi_ingest_document`, `stage_kpi_observations`, `validate_kpi_ingest`, `commit_kpi_ingest`, `get_kpi_ingest_status`, `cancel_kpi_ingest` — no additional read tools: the context read model covers catalog, comparison facts and document bytes. Widening the allowlist is a deliberate ADR change. Calling any tool outside the allowlist returns `-32602` unknown-tool — the surface does not exist for that identity. Full scope is a superset (it sees the nine new tools; the #365 capability-conditional demotion of `record_financial_facts` activates automatically). `record_financial_facts`, `create_financial_fact`, `update_financial_fact`, `capture_report_document` stay outside the acquisition scope (repair and capture belong to Full).

Producer/handoff contract: the acquisition scope discovers and captures nothing. Capture and URL→`documentId` resolution belong to the UI, the Full scope, or the #354 planner; "process all pending KPI ingests" presumes pending runs exist (or a `documentId` handed to the agent). Before #354, the #366 fixture is prepared by the owner/Full scope and handed off.

**Amended (2026-08-18, [ADR 0101](0101-agent-proposed-kpi-definitions-and-full-catalog-visibility.md) dec. 1):** the allowlist is deliberately widened 9→10 with `propose_kpi_definition` — the freeze rationale above covered read tools only; it never reasoned about a write gap on this scope.

Additive `CommandError` codes (the enum is additive-only and the envelope has no structured detail field, so subtypes must be codes): `run_lease_expired`, `run_taken_over`, `response_budget_exceeded` — retry semantics in the contracts.md code table.

### 4. The credential owns the lease; claims are targeted and idempotent; reads are pure

The lease holder derives from the credential (`"mcp:kpi_acquisition"` / `"mcp:full"`) — the agent never echoes a holder (the run id is the only handle, ADR 0098 dec. 2). Limitation recorded deliberately: the CREDENTIAL, not the agent session, owns the lease — two agents sharing one token are indistinguishable (accepted: single-instance app, one owner).

A new atomic `claim_run(run_id, holder, lease_seconds)` primitive (targeted — `claim_next` takes the globally oldest claimable run and cannot target) claims when `status ∈ claimable AND (lease NULL | expired | same holder)`; the same-holder-live branch renews WITHOUT incrementing `attempt_count`, making `start_kpi_ingest(runId)` an idempotent retry and the explicit keepalive. Reads (`get_kpi_ingest_context`/`status`/`document`) are side-effect-free — no implicit heartbeats (a heartbeat after `ready_to_commit` would be illegal anyway: the transition clears the lease). A lapsed lease surfaces as typed `run_lease_expired`; recovery is `start(runId)`. If another holder took the run after expiry, the original agent gets typed `run_taken_over` and abandons it — convergence, not a lease fight. The ~30-minute TTL is a constant, not a correctness mechanism; the typed refusals are.

### 5. Content-addressed source blobs pin the bytes a run reads

`start_kpi_ingest` pins the source: copy the stored local file to a blob, hash the COPY, atomically rename to `report_snapshots/{sha256}`, then `mark_source_captured(hash)`. Document recapture overwrites `local_path` in place, so a run never reads `local_path` — `get_kpi_ingest_document` resolves bytes by the run's frozen `source_content_hash`; recapture updates the document's "latest" pointer and never touches a run-referenced blob. Blobs are content-shared (natural dedup). Retention: a blob lives while ANY durable run in ANY state references it — terminal runs keep citations and the source hash auditable; GC may remove only unreferenced blobs.

### 6. A versioned in-code extraction-profile registry

A const registry: `profile_id ∈ {gpw_ifrs_annual, gpw_interim, gpw_preliminary, nc_uor, company_characteristic}` with per-`statement_type` expected-KPI packs (the existing `industrial|banking|insurance|specialty_finance|brokerage|reit` vocabulary). `profile_version` = `{profile_id}@v{N}`, validated at run creation. The expected-KPI stamp moves to `create_run_if_absent`: the union of the company's `expected_primary_metric_keys` and the profile pack, with a non-null `packVersion`; validation consumes the creation-time stamp (validation-time stamping remains only as the legacy-null fallback). Agent-minted definitions never enter the denominator (ADR 0093 dec. 4). A run whose `profile_version` is outside the registry gets a typed refusal at `start(runId)` (remedy: cancel and start a fresh run; no production legacy rows exist).

### 7. Response budgets are runtime mechanisms, not test fixtures

Numeric caps with defined overflow behavior — never silent truncation. String limits are UTF-8 BYTE limits; control characters (U+0000–U+001F) are rejected in all text fields, which bounds serde_json escaping expansion at ×2 and makes the transport arithmetic provable (a schema-valid stage request always reaches tool validation below the 1 MiB body cap). Context overflow returns a truncated section plus a cursor (`section`-scoped follow-up calls), never a dead end. The scoped `tools/list` gets its own frozen snapshot and count (frozen in #386 when all nine tools exist); byte gates are regression coverage, not the enforcement. Full-scope frozen tool count after the epic: 111. Numbers are frozen in contracts.md; raising a cap is an additive change.

**Amended (2026-08-18, [ADR 0102](0102-full-capture-staging-contract-excluded-observations-and-chunked-drafts.md) dec. 10):** the per-call cap arithmetic here is unchanged and now bounds a chunk, not necessarily the whole revision; a new aggregate cap applies at finalize.

### 8. Execution metadata is diagnostic and atomic

`cost_json` = `{"schemaVersion": 1}` + the `ExecutionMeta` object (`client` required; `model`, `skillVersion`, `repairRounds`, `tokensIn`, `tokensOut`, `costUsd` optional, numerics non-negative) — one schema, identical in the tools' optional `execution` field and in storage. Written only by atomic merge inside the stage and commit transactions (no loose last-writer-wins update); readable back in `RunStatus.execution`; never part of the trust verdict (ADR 0098 dec. 2).

## Consequences

- The ingest workflow is headless-only: the UI surface is credential management in Settings → MCP (#382); driving runs is an agent activity by design.
- The acquisition skill is rewritten around the run workflow with a ban on direct canonical-fact writes (#387); document-reading doctrine survives.
- `docs/data-model.md` § KPI Ingest Queue Integration reflects the amended execution ownership; the queue module doc's "production trigger is #353's stage tool" is corrected to #354 when #386 lands.
- New typed refusals (`run_lease_expired`, `run_taken_over`, `response_budget_exceeded`) are additive `CommandErrorCode`s; the lease pair maps from storage through the wildcard-free `code_for`, while `response_budget_exceeded` is minted at the MCP layer (as-built correction, 2026-08-14 — it is not a storage refusal). The mock-fidelity-corpus clause applies to tools with a Tauri command twin; the nine MCP-only workflow tools have no mock-runtime half — their coverage is registry/storage integration tests (`registry::call` against real storage) plus the shared `CommandErrorCode` vocabulary (as-built correction, 2026-08-14; testing.md § dual-execution scope exemption).
