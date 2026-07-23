# ADR 0088: MCP surface v2 — UI-parity tool registry, capability tiers, provenance-mandatory writes

Status: Accepted (2026-07-22, owner sign-off at v0.60 planning)

Deciders: maintainer. Area: MCP, architecture, security.

Extends [ADR 0078](0078-mcp-external-surface.md) (read-only MVP) and clears its G-2 tripwire
(any mutating tool requires a new ADR — this is that ADR). Realizes the NS1 slice pulled
forward by [ADR 0084](0084-retire-in-app-ai-layer.md): after the in-app AI retirement the MCP
port is the app's sole intelligence channel (BYOA), so its surface is a first-class product
surface, not an integration afterthought.

## Context

The MVP ships four hand-written read tools. The v2 goal set at v0.60 planning (owner,
2026-07-22) is explicitly broader: **an agent connected over MCP can do everything the user can
do in the UI, except destructive and configuration operations.** Hand-writing a wrapper, JSON
schema, and snapshot per command does not scale to the ~UI-parity surface and invites drift
between the command layer and the MCP layer. The codebase already contains the precedent for a
name-keyed command dispatch table (the mock-fidelity corpus) and every command already has typed
serde inputs/outputs (ts-rs-generated on the frontend side).

Write-paths and their provenance carriers already exist in the domain types
(`NewNotebookEntry.origins`, `NewManagementClaim.source_evidence_*`,
`QualitativeCriterionResult.citations_json`, fact citations) — what is missing is the exposure,
the enforcement, and the safety posture.

## Decisions

1. **A tool registry over the typed command layer — no hand-written wrappers.** Every MCP
   exposure is a registry entry `{command, tier, provenance requirement}` in `mcp/registry.rs`;
   input/output JSON schemas are **generated from the existing serde types** (`schemars` derive —
   a justified runtime dependency addition), never written by hand. The tools/list contract
   stays frozen by the insta snapshot (ADR 0078 G-1): regenerating it is a reviewed spec change.
   The MVP's four tools are re-expressed through the registry (names and shapes preserved).
2. **Three capability tiers.**
   - `read` — every domain read command (companies/watchlists, feed, signals, facts **with
     provenance tier + citation**, coverage/flagged periods, quotes + history, ownership + free
     float, insiders, analyst recommendations, health/red flags, report documents + text +
     diffs, transcripts, notes, claims, expectations, journal, research questions, quality
     frameworks, calendar/report season, autopilot runs, attention events, briefing). Active
     whenever the MCP server is enabled.
   - `act` — gated by a new `mcpWritesEnabled` setting (Settings → MCP, **default OFF**):
     research writes (notes create/update, claims create/update + verdicts, qualitative
     criterion verdicts, expectations, journal entries, research questions, **manual KPI facts**
     — the top of the trust ladder, citation mandatory), workspace actions (watchlist add/pin,
     mark read/saved, dismiss attention, claim status), and job triggers (autopilot run, source
     refresh, aggregator pull, briefing generation, flagged-period re-run) — triggers are
     idempotent and reversible through existing app mechanisms (e.g. run Undo stays UI-side).
   - `excluded` — a permanent explicit denylist: deletes, undo, settings/credentials mutations,
     MCP self-management, dev/diagnostic mutating commands. These remain UI-only.
   **Every command must be classified**: a command absent from the registry (neither exposed nor
   denylisted) fails a gate — new commands cannot silently leak into or stay out of the MCP
   surface undecided.
3. **Provenance is mandatory on every write, enforced at the boundary.** A shared validation
   layer rejects any `act` write whose provenance carrier is empty (`origins` /
   `source_evidence_*` / `citations_json` / manual-fact citation) with a typed error — never an
   "empty default". Calls to `act` tools while `mcpWritesEnabled` is off return a typed error.
   Advisory gating is unchanged: no tool phrases buy/sell/hold output (ADR 0042).
4. **Triage tool for unclassified filings.** A new storage read materializes the orphaned
   `UnclassifiedFiling` DTO (official-report feed items with no `company_signals` row);
   `list_unclassified_filings` (read) + `classify_filing` (act; creates a signal with
   provenance) fulfil the contracts.md promise of an explicit unclassified bucket that is never
   guessed at.
5. **Documentation ships with the surface.** User-facing wiki how-to (connect an agent step by
   step, tiers and the writes switch, citation rules, per-domain tool catalog, example
   workflows) and a repo skill **`brawler-mcp`** instructing a client agent (discovery, write
   provenance requirements, typical read→analyze→write-with-citation sequences, what the
   denylist excludes). A drift gate keeps the documented tool catalog consistent with the
   tools/list snapshot. The closure dogfooding ritual (a real talk-to-your-research session
   against the live app, with the skill loaded) is a mandatory closure step recorded in
   testing.md.
6. **Transport, auth, and protocol posture unchanged.** Loopback-only tiny_http + bearer token,
   app-open-only lifetime, hand-rolled protocol module; the rmcp SDK re-evaluation stays
   deferred (ADR 0078 decision 8) — the registry changes what is exposed, not how.

## Consequences

- `schemars` derives land on exposed command input/output types; schema generation is tested by
  the frozen tools/list snapshot plus per-domain golden outputs on seeded data and the existing
  "dispatcher never panics" proptest extended over the registry.
- Negative-path tests are part of the contract: write without provenance → typed error; writes
  toggle off → typed error; denylisted command unreachable over MCP.
- contracts.md's MCP section is rewritten around tiers + the registry rule (documenting the
  classification of every command) instead of per-tool prose; data-model.md gains
  `mcpWritesEnabled`.
- The v0.61 references sprinkled through contracts.md ("provenance return via MCP write-tools
  (v0.61.0)", the unclassified-bucket note, qualitative verdicts) are updated to v0.60.0 at
  roadmap-reshuffle time.
- Risk accepted: UI parity multiplies the frozen contract's size, so snapshot churn becomes the
  cost of command-layer changes — mitigated by the registry being the single place a change is
  made and reviewed.
- Agent-authored signals stay distinguishable in provenance: `classify_filing` (dec. 4) writes
  `company_signals.classified_by = 'agent'` — never `'rule'` (the deterministic classifier) — with
  the CHECK relaxed to `rule | ai | agent` by migration `0113`. Honest origin labels are core to
  the app's posture (ADR 0084/0086), so a triage classification must not masquerade as deterministic.
