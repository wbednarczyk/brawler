# ADR 0102: Full-capture staging contract — excluded observations and chunked drafts

Status: Accepted (2026-08-19, epic #399 delivered — PR #405 merged, §G live-verified; proposed 2026-08-18 at planning, sol r1 folded). Amends [ADR 0098](0098-mcp-native-kpi-acquisition-lifecycle.md) decisions 4 and 6, and [ADR 0099](0099-acquisition-mcp-surface-mechanics.md) decisions 1 and 7. Implementation: epic #399.

Deciders: maintainer. Area: fundamentals, MCP port, data trust.

## Context

Epic #399 (agent-path full capture for untagged documents) proved two structural blockers against the tree (master 0011143), on top of the tenth-tool resolution in [ADR 0101](0101-agent-proposed-kpi-definitions-and-full-catalog-visibility.md):

1. **Transport.** `stage_kpi_observations` is defined as one call = one complete revision: `OBSERVATIONS_MAX=100` (`kpi_ingest_submit.rs:36`), proven against the 1 MiB per-call body cap. A real preliminary/interim report routinely discloses well over 100 numbers once "capture everything" is the doctrine, and `STAGEABLE_STATUSES={Extracting, ValidationFailed}` (`kpi_ingest_staging.rs:47-50`) makes a second stage call on the same run impossible — the first call flips the run to `staged`, and there is no `Staged→Staged` transition. A >100-observation report physically does not fit today's contract.
2. **Doctrine vs. validation.** Full-capture rules require staging every unmappable number as `mappingStatus="unmapped"` rather than inventing a key. But `mapping_unresolved = status!="mapped" || definition.is_none()` (`kpi_ingest_context.rs:431-434`) makes every `unmapped` row a `MappingUnresolved` diagnostic at `Flagged` severity, and any flagged diagnostic drives the run to `Failed`. Obeying the doctrine as literally written is indistinguishable from failing the run.

Both blockers are closed by decisions below: transport by an optional chunked-draft extension to the existing single-call contract (D1), doctrine by giving "deliberately excluded, with a reason" its own sealed disposition distinct from "unmapped" (D3-adjacent; the actual exclusion mechanics are decisions 1–5 below).

Facts bounding the design:

- `finalize_committing` derives the terminal run status from `receipt.terminal_status`; `partial` today comes **exclusively** from incompleteness of the `expected` set (`:504-513`) — a run with no expected snapshot is never partial, and a `divergent` outcome does not make a run partial either.
- The ingest workflow is headless-only end to end (ADR 0099 Consequences: "the UI surface is credential management... driving runs is an agent activity by design") — there is no run-detail UI to extend today.
- `validate_kpi_ingest` returns the **entire manifest**, unpaged (`:585-602`); a paged read already exists in parallel (`get_kpi_ingest_context section:"manifest"`, page 50, 256 KiB budget) but the tool response does not use it.
- Chunking necessarily breaks the documented invariant "complete snapshot per revision" (`contracts.md:49`, `data-model.md:1636`) as literally written; the per-call transport proof (100 obs / 1 MiB) survives as a per-**chunk** bound, and a new aggregate bound is required.

## Decisions

### 1. `excluded` is a sealed disposition in the manifest (amends ADR 0098 dec. 4)

A `ManifestObservation` seals its disposition (`excluded`), its reason, and the **raw label** — without the label the promised receipt ledger `{label, reason}` is impossible to produce, since commit only ever sees the sealed manifest. Content-projection (`staging.rs:282`, `kpi_manifest.rs:1039`) is extended to compare disposition, reason, and label too, with the invariant stated explicitly in `seal`. The existing missing-reasons ledger (ADR 0098 dec. 4) is untouched — this is a new, independent disposition, not a repurposing of it.

### 2. `excluded` is Info severity, evaluated before definition-dependent validation; accidental `unmapped` stays Flagged

The evaluation path branches to `excluded` **before** any check that dereferences a resolved definition (today's code would panic at a definition `.expect(...)` otherwise), and the new diagnostic code carries **Info** severity, not Flagged — `DiagnosticDetail::Reason` legality is widened to admit it. An observation that is merely `unmapped` (no exclusion recorded) stays exactly as flagged as it is today. This is the doctrine fix: deliberate, reasoned exclusion is not the same event as an accidental mapping failure, and only the deliberate case gets a path around `Failed`.

### 3. Commit skips only validated-excluded observations; tamper after validation is refused

Commit writes no fact for an observation sealed `excluded` in the validated manifest — the existing guardrails (`:239-249`, `:426-429`) are extended to release only for a sealed-excluded disposition; a row whose status was flipped to `excluded` in storage *after* validation (or whose reason/label changed) still fails the content-projection compare from decision 1, or trips `CorruptStoredManifest`. The outcome vocabulary grows from four to five (`created | reobserved | upgraded | divergent | excluded`). **Receipt schema becomes v2** (`outcomes_schema_version: 2`); the reader accepts both v1 and v2 — v1's existing shape invariant (every non-divergent outcome carries a `factId`, divergent never does, `:632-645`) gains a third legal case under v2 only: `excluded` with no `factId`, carrying its reason. v1 readers and the v1 invariant are not loosened.

### 4. Terminal `partial` stays denominator-only; exclusions are always ledgered (amends ADR 0098 dec. 4/5 — deliberate deviation from issue #399)

**This is a deliberate deviation from issue #399's own wording** ("terminal partiality that counts exclusions"), surfaced by adversarial review and accepted at epic planning: making an excluded observation count toward `partial` would contradict ADR 0098 decisions 4/5, where `partial` is defined purely by incompleteness of the `expected` denominator. This ADR keeps that definition intact: `partial` triggers **only** when an `expected` member is missing from the committed set. Exclusions are never silent regardless — every excluded observation is **always** carried in the receipt's `excludedCount` and `{label, reason}` ledger, committed or not, partial or not. The one case that does interact with `partial`: if the excluded observation *was* an expected key, that key is still missing from the denominator, and the run's `missingReasons` must carry an entry for it — exclusion of a non-expected number never touches completeness; exclusion of an expected one still leaves it counted as missing.

### 5. Visibility stays headless-only; the upgrade path is a Coverage-panel line

Exclusions are readable through `get_kpi_ingest_status` and the commit receipt — consistent with the acquisition workflow's existing headless-only posture (ADR 0099 Consequences). No run-detail UI ships with this ADR. When any UI for runs exists, the documented upgrade path is a line in the Coverage panel (the same precedent #398 set for tagged-fact coverage) — recorded here so the deferral is a decision, not an oversight.

### 6. Chunked drafts: server-issued draft id, one active draft per run, no new run states (amends ADR 0098 dec. 6)

`stage_kpi_observations` gains an optional `draft` object; its absence keeps the existing single-call path byte-for-byte identical to today (decision 14). Opening a draft (`draft:{open:true}`) is issued by the **server**, not supplied by the client — a `draftId` minted server-side, bound to the current lease epoch (a takeover invalidates it), with exactly one active draft permitted per run. The run itself **stays** in `extracting`/`validation_failed` — `STAGEABLE_STATUSES` and the ADR 0098 decision 6 state machine are unchanged; a draft is a sub-resource of the run, never a new state.

### 7. Append re-checks the live lease per call; never bumps revision; invisible to validation

Each chunk append re-verifies the caller holds the live lease — not only at open and finalize, closing the gap a long multi-call capture session would otherwise leave open. An append never bumps the run's manifest revision and is structurally invisible to validation: chunks live in their own table, never merged into the staged-observations rows validation reads, until finalize installs them.

### 8. Chunk identity is (draftId, chunkIndex); the hash is server-computed; replay is idempotent, conflict is typed

A chunk is identified by its draft id and an index. Its content hash is computed **by the server**, never trusted from the client, so identity is canonical. Replaying the same `(draftId, chunkIndex)` with matching server-computed content is an idempotent acknowledgment (safe retry over an unreliable transport); the same index with **different** content is a typed `draft_chunk_conflict`, never a silent overwrite. The declared expected-observation count on the draft is cross-checked against what finalize actually assembles.

### 9. Finalize is one Immediate transaction through a shared connection-level install-helper

Finalize (`draft:{draftId, final:true}` plus `missingReasons`) checks chunk contiguity, enforces the aggregate caps (decision 10), re-checks the live lease, then performs bump-revision + install-observations + flip-state + delete-drafts **in one transaction** — via a new connection-level install helper extracted from today's single-call `stage_observations` and shared by **both** routes. Finalize does not call the existing single-call path as a black box (that path opens its own transaction); it shares the helper on `&Connection` instead. Global observation ordinals — contiguous, store-assigned, matching today's installed behavior — are assigned by the server at finalize, ordered by `(chunkIndex, position-within-chunk)`.

### 10. Per-call cap arithmetic is unchanged and now bounds a chunk; aggregate caps are added at finalize (amends ADR 0099 dec. 7)

The proven per-call transport bound (100 observations / 1 MiB) is not relaxed — it now bounds a single **chunk**, not necessarily the whole revision. A new aggregate ceiling applies at finalize: `AGGREGATE_OBSERVATIONS_MAX=1000` (headroom over #398's measured ~426 tagged facts per package) plus a **frozen aggregate byte cap** (computed from the per-chunk arithmetic and frozen in `contracts.md`, an issue requirement) and an aggregate `missingReasons` cap of 128, matching the existing per-call `MISSING_REASONS_MAX`.

### 11. Draft lifecycle: single-call with an open draft is a typed refusal; cancel/failure/reclaim clear drafts; drafts never validate or commit

Calling the single-call form while a draft is open on the run is a typed refusal — an explicit abort is required, never a silent orphan. Run cancellation, failure, and startup lease-reclaim all clear any open draft and its chunks (a drafts table with `ON DELETE CASCADE` to the run makes this structural, not a bookkeeping step to remember in three places). `get_kpi_ingest_status` reports an open draft's id, chunks received, and expected observation count, so a restarted agent session can resume instead of guessing. A draft — open or complete-but-unfinalized — is never itself validated or committed; only an installed revision is.

### 12. Both `validate_kpi_ingest` and `commit_kpi_ingest` return bounded summaries; full data moves to paged context reads (amends ADR 0099 dec. 1)

`validate_kpi_ingest` stops returning the entire manifest inline; it returns a bounded summary (`manifestHash`, `revision`, `outcome`, `severityCounts`) — the full manifest is read via the existing paged `get_kpi_ingest_context section:"manifest"`. `commit_kpi_ingest` likewise returns a bounded summary (`terminalStatus`, `counts`, `manifestHash`, `revision`) instead of the full receipt with all outcomes — a 1000-row ledger (decision 10's aggregate ceiling) cannot ride an unpaged response. The full receipt is read through a paged context section. Live tests that consumed the full inline manifest/receipt move to context paging in the same slice.

### 13. Doctrine repair is versioned: `@v1` stays frozen, `@v2` carries excluded/propose, `@v2` packs stay minimal

The annual@v1 doctrine defect this ADR's context describes (blocker 2) is not patched in place — `gpw_preliminary@v1`/`gpw_ifrs_annual@v1` are frozen exactly as they resolve today, and new `@v2` profile versions carry the "map, propose, or exclude with a reason" rule this ADR and ADR 0101 make possible. `@v2` expected-KPI packs stay minimal by design: the denominator is not the same axis as capture doctrine, and widening it would inflate `partial` and plausibility cost for reasons unrelated to what got captured.

### 14. The single-call path is byte-for-byte unchanged when `draft` is absent

The wire contract is an explicit union: today's request shape (`observations` + `missingReasons`, both required, `deny_unknown_fields`) is preserved exactly when `draft` is absent. `missingReasons` becomes optional only when `draft` is present and non-final — a non-final draft append with reasons attached is a typed refusal, since reasons belong to a complete revision. Backward compatibility is a wire-shape guarantee, not an aspiration: the existing single-call snapshot test is the enforcement.

## Consequences

- `contracts.md`'s "complete snapshot per revision" invariant (`:49`, and the related note at `:21`) is rewritten as: complete snapshot proven per **chunk**, plus the new aggregate bound at finalize; `data-model.md:1636` gets the equivalent correction.
- The scoped and Full `tools/list` snapshots move again in the slice that ships chunking (`stage_kpi_observations`'s input schema changes; `validate_kpi_ingest`/`commit_kpi_ingest`'s output schemas change) — on top of the tenth-tool move ADR 0101 already causes.
- Two new tables land under migrations reserved for this epic (0150 for the excluded-disposition rebuild of the existing `mapping_status` CHECK, 0151 for the two draft tables) — schema detail belongs to `data-model.md`, not this ADR.
- The outcome vocabulary, receipt schema version, and aggregate caps are additive; no existing committed run's stored data changes shape.
- The epic's DoD — every disclosed number mapped, proposed, or excluded with a reason, never silently absent — gets its "excluded with a reason" and "the report physically fits" legs from this ADR; "proposed" is [ADR 0101](0101-agent-proposed-kpi-definitions-and-full-catalog-visibility.md).
