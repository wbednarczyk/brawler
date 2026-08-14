# ADR 0098: MCP-native KPI acquisition lifecycle

Status: Accepted (2026-08-10, owner decision after the 2026-08 vendor study). Supersedes (on completion) ADR 0086 decisions 2 and 7; amends ADR 0093. Implementation: epics #352–#356.

Deciders: maintainer. Area: fundamentals, MCP port, data trust, sources.

## Context

The owner's verdict on the app's core weakness (2026-08-09): the data is not trusted enough or complete enough to make Brawler a daily tool, and the multi-tier deterministic ladder keeps producing repair work ("plastry na dużą ranę"). A vendor study (2026-08) found no paid API covering GPW + NewConnect quarterlies with per-filing provenance at an individual price — EODHD is the only global candidate and leaves PL depth/latency unknowns; StockWatch's feed is B2B; Notoria's UMCS description is a user-facing database claim, not a machine-delivery contract. The conclusion is to stop searching for a "good-enough" external feed and build acquisition in-house.

The building blocks exist but are not yet a production ingestion protocol, verified against the tree:

- `record_financial_facts` (ADR 0093 dec. 6) validates the set first but then commits fact-by-fact, each write in its own `IMMEDIATE` transaction (`storage/kpi_extraction.rs:839-848`); a storage error at fact 10 leaves facts 1–9 committed. The financial period is deliberately created before validation resolves (`jobs/record_financial_facts.rs:171-183`), so an all-rejected batch leaves an empty `finper_` row.
- There is no durable ingest run: no manifest, no checkpoints, no instruction/profile version, no attempt id, no whole-document completion evidence. Idempotency covers a single fact slot, not "process this report".
- The durable job queue (`jobs/queue.rs`) has retry, backoff, crash reclaim and isolated lanes — but rows are claimed by the in-process worker and a handler return immediately settles them; there is no lease/heartbeat surface an external agent could hold.
- `report_documents` records a SHA-256 (`content_hash`) but no publication date; identity is `(company_id, url)` (migration 0035). Repo policy forbids `created_at` as a recency proxy (data-model.md § conventions).
- The MCP surface is 102 tools behind one shared bearer token and a single global `mcpWritesEnabled` switch (`mcp/server.rs`, `mcp/registry.rs:1324`); `tools/list` is unconditional.

Before this ADR, ADR 0093 was still marked Proposed despite being implemented — `SourceTier::Agent`, `record_financial_facts`, `capture_report_document` are live — and its ladder places `agent` above `html_aggregator`, which contradicts ADR 0086's "BiznesRadar primary / agent additive" framing. This ADR resolves that drift and sets the program's architecture. Epic map: #352 (transactional core), #353 (workflow MCP surface), #354 (GPW/NC automation), #355 (conformance suite), #356 (protocol modernization).

## Decisions

### 1. Source of truth and role split — agent-first acquisition

The **issuer's report is the source of truth**. A rich LLM (BYOA, over MCP — ADR 0084 posture unchanged) is the **reader and operator**, never a data source: it resolves the heterogeneity of real filings, and everything it produces is a *proposal* until Brawler's deterministic machinery accepts it. Brawler owns process state, provenance, validation, idempotency and the atomic commit.

This supersedes, **on completion of the run workflow**, ADR 0086 decision 2 (BiznesRadar as PRIMARY for core KPIs) and decision 7 (agent as additive option): BiznesRadar stops being the completeness foundation and becomes a **witness/complement** — the daily pull keeps running, keeps filling slots the agent path has not covered, and keeps corroborating; the takeover is **per-slot via tier precedence** (decision 7 below), never a flag-day cutoff. The BR pull also keeps its second job — refreshing the `kpi_relevance` layers (`jobs/aggregator_fundamentals_pull.rs:250`) — until epic #354 delivers a replacement cadence; disabling BR outright would silently kill relevance convergence.

### 2. Durable `KpiIngestRun`

One run represents exactly one (report document, company, extraction profile). The run row carries: the document reference (URL + SHA-256), company, period, scope (standalone/consolidated), `data_quality`, extraction-profile and instruction versions (as-built, 2026-08-14/#384: `instruction_version` records the server-stamped acquisition-orchestration-contract version — `acquisition-mcp@v1` — not an agent skill version; the skill's own version lands in execution metadata, #388), manifest hash + revision, attempt count, lease (holder + expiry + heartbeat), progress, optional cost/tokens (diagnostic metadata, never part of the trust verdict), and the versioned expected-KPI snapshot with explicit missing reasons. The run id is an **opaque handle** the agent passes back on every call (the MCP stateful-tools pattern).

**Publication date.** `report_documents` gains a canonical `published_at` column (forward migration, epic #354 — corrected 2026-08-13: migration 0137/#352 deliberately did not add it; data-model assigns it to #354), filled by a **typed origin-date resolver**: the linked feed item via `origin_ref` → a date carried by the document itself → explicit absence (a typed state, never a guess). Using `created_at`/`fetched_at` as publication proxies is **rejected** — backfills invert ingestion and publication order (data-model.md § conventions).

### 3. Staging before canonical

LLM output lands in new **run-owned** records, not in `financial_facts`:

- **Staged observations**: raw label + raw value, normalized number + currency + scale, period dimensions (measure window, attribution, scope), candidate `metric_key`, citation locator (page/table/row), mapping status, per-observation validation state and error codes.
- **Commit receipt**: an immutable record of a commit's outcome (accepted facts, outcomes, manifest hash/revision), written inside the commit transaction.

Relation to the existing machinery (the drift risk this section exists to kill): staging is **pre-canonical workflow state** — no fact reader ever sees it. The canonical `data_quality` + `supersedes_id` axis (ADR 0093 dec. 2 — preliminary/final coexistence in the slot, supersession stamped at final-fact creation) is **unchanged**: a run ingesting wstępne wyniki commits canonical facts with `data_quality='preliminary'`, and the audited report later supersedes them exactly as today. The existing `fundamentals_extraction_outcomes` table (a current-verdict upsert with `attempt_count`) is **untouched** — run audit is new immutable records, not a reuse of a table with overwrite semantics. Observations and receipts are retained after commit as the run's audit trail.

**Retention**: a document referenced by **any durable `KpiIngestRun`** (any state, including failed/cancelled runs from early phases) joins the report-bytes protection contract (data-model.md § report document retention), so page/table citations stay verifiable.

### 4. Deterministic validation → versioned manifest

Validation is Brawler's **internal contract**, not a provider comparison: units/currency/scale; period grammar (Q1/H1/9M/FY, flow vs instant, **cumulative-only for interim publications** — ADR 0093 dec. 3 upheld); standalone vs consolidated; parent/NCI/total attribution; balance identities; plausibility vs the company's history (with explicit thin-history abstention); duplicates and conflicts; a **mandatory citation per fact**; completeness or an explicit missing reason. An LLM cannot be promised never to err; the guarantee is that an error is **auditable, repairable, and cannot corrupt the canonical store silently**.

The validator's output is a **versioned manifest**: content hash, revision, validator version, and **stable diagnostic codes** per observation (the Arelle conformance pattern — codes survive refactors, so repair loops and the #355 conformance suite can pin behavior). Rejected observations return **typed repair results** grouped per observation; the agent re-stages only what failed.

**Completeness denominator.** A run's expected-KPI snapshot is a **versioned union** of the curated primary `kpi_relevance` set and the extraction profile's pack for the issuer type, stamped on the run at start. Agent-minted definitions remain extras, never denominator entries (ADR 0093 dec. 4). Coverage reporting splits into **deterministic coverage** (ESEF+WDF+BR, today's recall harness) and **acquisition coverage** (runs) — two metrics, never one blended number.

### 5. Atomic, idempotent commit

`commit(runId, manifestHash, revision)`: re-verify manifest freshness → open **one transaction on one connection handle** → create the period, write all accepted facts + provenance + supersession + the commit receipt + the terminal run state → commit only on full success. Any failure rolls back everything, including the period row (closing the empty-period hole).

Mechanics: a new **transaction-owning storage method** operating on `&Connection` — the shape already exists (the connection-level fact primitive `storage/kpi_extraction.rs:360`, the BR batch write in one transaction `:931`). Composing existing public store methods (which check out their own connections and open their own transactions) inside the outer transaction is **prohibited**; rusqlite 0.32 nests only via savepoints. This is a contained storage refactor, not a rewrite.

Idempotency: retrying a committed manifest **returns the stored commit receipt** — it never re-executes the write primitives (today's primitives would answer `Reobserved` on a replay, a different verdict than the original `Created`; the receipt is the only stable answer). A stale `manifestHash`/`revision` gets a **typed conflict**, never an overwrite. A per-(company, period) lock prevents concurrent commits. **No default partial commits**: a knowingly-partial report is an explicit manifest policy with a complete missing-reason ledger.

### 6. A closed run state machine

Lifecycle: `discovered → source_captured → extracting → staged → validation_failed | ready_to_commit → committing → complete | partial | failed | cancelled`.

The implementing epic ships an **exhaustive transition table** (state × allowed caller × lease requirement × revision effect × terminality) covering at minimum: repair `validation_failed → staged` (re-staging bumps the revision), manifest invalidation `ready_to_commit → staged`, source-failure retry `source_captured → extracting`, cancellation from every pre-commit state, and crash recovery. **Cancellation during `committing` does not exist**: the transaction either rolls back (run returns to a retryable state) or completes (`complete`/`partial`). A crash in `committing` is resolved at startup reclaim by receipt presence: receipt exists → the commit landed, finalize the run; no receipt → the transaction rolled back, the run is retryable. Illegal transitions are typed refusals.

### 7. The trust ladder, resolved

**Live write precedence** (highest first): `manual` > `esef` / `espi_cover_note` > `agent` > `html_aggregator`. The ADR 0093 ordering is **confirmed**: the agent reads the issuer's own document, so it stands above the third-party aggregator and below the deterministic issuer tiers; witnessing semantics stay "does the stored tier outrank `html_aggregator`?" (the BR pull corroborates or records disagreement against agent-held slots and never overwrites them — enforced today by `SourceTier` ordering and `aggregator_owns_slot`).

`structured_xhtml` and `pdf` are **legacy read tokens**: orderable for old snapshots, no new writes. Precisely: `pdf` already has an enforced write refusal (`storage/kpi_extraction.rs:737`, ADR 0095); `structured_xhtml` has no live producer but the provenance compatibility map still accepts it (`fundamentals/extraction/mod.rs:170`) — **this ADR closes that gap**: new `structured_xhtml` provenance writes are refused by the same mechanism as `pdf` (a small change in epic #352).

Status mechanics: ADR 0093 moves to **"Accepted; amended by ADR 0095 and ADR 0098"** (the path is implemented; the batch write stops being the normal path — decision 8 here). ADR 0086 gains **"decisions 2/7 superseded on completion by ADR 0098"**. `rebuild_fundamentals` is re-scoped as **deterministic refill**: it reconstructs only BR/ESEF/WDF facts (`jobs/rebuild_fundamentals.rs:131`) and neither recreates agent runs nor pretends to — agent-acquired data is rebuilt by re-running ingestion, not by the refill command.

### 8. Execution ownership — the run is the agent's worklist

**Model A.** The `KpiIngestRun` table is the **external agent's worklist and lease**: lease expiry + heartbeat live on the run row; the agent claims a run over MCP, works, and reports through the workflow tools. The in-process job queue executes only **short deterministic steps** — discovery/planning of pending runs, validation, commit — and **never holds a `running` job row while waiting for an LLM**. *(Amended 2026-08-13, [ADR 0099](0099-acquisition-mcp-surface-mechanics.md) dec. 1: on the agent path Brawler executes validation and commit synchronously inside the MCP tool call; the job queue remains the executor for #354 automation and crash recovery.)* The queue's four states (`pending/running/succeeded/failed`) are job bookkeeping; the run's lifecycle is the domain state machine.

Rejected (model B): making the external agent a claimant of `job_queue` rows over MCP — it would require durable lease/heartbeat/cancellation semantics and a row↔run mapping the queue does not have, and would conflate transport-level job retry with domain-level run repair.

Automation shape (BYOA): Brawler discovers publications and creates pending runs (epic #354); the owner points Claude/Codex at "process all pending KPI ingests"; MCP never wakes an external LLM by itself.

### 9. Acquisition-scoped authorization (constraint for E2)

Unattended ingestion gets a **separate `kpi_acquisition` scope/credential** that sees only the ingest workflow tools and the read models they need — no notes, settings, deletes, watchlists, or other write families. Architectural consequence, recorded now so #352 does not wall it off: authorization context must flow through the transport → `tools/list` → `tools/call` (today: one shared bearer digest `mcp/server.rs:37`, unconditional `tools/list` `mcp/protocol.rs:79`, one global `mcpWritesEnabled` gate `mcp/registry.rs:1324`). Reusing the unrestricted global token as the acquisition credential is **rejected**. Implementation lands with epic #353.

## Consequences

- The fact store gains a protocol, not just an API: every canonical fact committed through the normal run-based report-ingestion path traces to a run, a manifest revision, a document hash and a citation (the low-level repair writes are the documented exception — corrected 2026-08-13); a half-written report becomes structurally impossible on that path.
- BiznesRadar's role inverts gradually and honestly: witness/complement per slot, with its relevance-cadence side job preserved until #354 replaces it. The recall harness splits deterministic vs acquisition coverage rather than blending them.
- `record_financial_facts` (ADR 0093 dec. 6) stops being the normal path: it remains a standalone, explicitly low-level repair tool (as built — corrected 2026-08-13: the commit step writes through its own `record_pinned_fact` primitive, #362, so RFF is not the engine under commit); the agent skill (epic #353) routes normal ingestion exclusively through the run workflow. `capture_report_document` and its security gates (0093 dec. 5) are unchanged and feed the run's `source_captured` state.
- The program's epics implement this ADR: #352 (runs, staging, manifest, atomic commit, state machine, the `structured_xhtml` write refusal), #353 (workflow tools, scoped credential, skill), #354 (discovery, universe, watermarks, backfill, `published_at` — corrected 2026-08-13), #355 (conformance corpus, chaos, replay fixtures), #356 (protocol modernization — independent track).
- Closure evidence for #352 is a real re-ingest of XTB RB 18/2026 by a live agent against the real Windows app (the ADR 0088 dogfooding ritual).

## Rejected options

- **Continuing the search for a global paid fundamentals API** — the 2026-08 study found no vendor with GPW+NC quarterly coverage, per-filing provenance and individual pricing (EODHD closest, PL depth/latency unproven; StockWatch B2B; Bankier is Notoria-fed downstream). The single remaining question is the binary Notoria RFI (#367); if Notoria qualifies, its data still enters through the same staging/validation pipeline — no side door without provenance.
- **Multi-tier completeness (ESEF/WDF/BR) as the foundation** — rejected by the owner: annual-only or partial per tier; the goal is one complete, trusted path per report.
- **Extending `record_financial_facts` without a durable run/staging ledger** — repeats the current failure class (partial writes, no resume, no audit).
- **Using canonical `financial_facts.data_quality` as a staging status** — it is a slot dimension of canonical facts (migration 0034 uniqueness); staging state does not belong in fact identity.
- **Resurrecting the `kpi_extraction_jobs`/`kpi_extraction_proposals` review ledger** (dropped by migration 0102) — that was an in-app AI review queue; this design has no human ratification step (ADR 0086 dec. 5 stands).
- **Treating a four-state `job_queue` row as the ingest run** — job bookkeeping cannot represent lease, revision, repair or partiality.
- **`created_at`/`fetched_at` as publication time** — backfill inverts ingestion order (data-model.md § conventions).
- **Flag-day BR shutdown** — would also silently kill the `kpi_relevance` refresh cadence.
- **The global MCP bearer/write toggle as the unattended-acquisition credential** — grants every write family to an automated process that needs one.
- **Recomputing outcomes on idempotent replay** — the primitives answer differently on replay (`Created` → `Reobserved`); only the stored receipt is a stable answer.
- **Partial commit as the default** — silent loss; partiality must be an explicit, ledgered manifest policy.
- **A second workflow engine beside the job queue** — the queue plus the run table cover the need; two engines drift.
- **Embedding Arelle/Python in the app** — at most an external oracle in CI and a golden-fixture generator for #355 (decision deferred to that epic).
