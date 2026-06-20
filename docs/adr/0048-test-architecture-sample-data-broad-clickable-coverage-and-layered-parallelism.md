# ADR 0048: Test architecture — canonical sample-data factory, broad clickable coverage, and layered parallelism

Status: Accepted

## Context

The full-coverage testing policy (`AGENTS.md` Testing Expectations,
[testing.md](../testing.md)) requires every behavior — command/contract, read
model, migration, adapter, provider mapping, job, UI workflow, and fixed
regression — to have a test that fails when it breaks. An audit of the
pre-policy codebase found material debt: ~36 of 158 Tauri commands untested, no
dedicated migration safety/idempotency tests across 51 migration versions (the
class that caused the `v0.40.0` "no such table" production failure), the
fundamentals expression parser untested, the `Watchlists` screen with zero
Vitest coverage, and broad clickable Playwright coverage limited to ~3 journeys
over ~6 of 12 screens.

The owner's goal is a **regression loop**: a broad, fast test base that lets any
change — backend or UI — be verified end to end so regressions are caught before
manual testing. Two enablers surfaced during planning that the rest of the
backfill depends on, and they turn out to be the same piece of work:

1. **Broad clickable UI coverage needs realistic, mutable state.** The browser
   smoke layer mocks the Tauri `invoke` boundary
   (`src/test/browserSmokeRuntime.ts`); today it is only lightly stateful and
   uses global state, so a "create" click cannot be asserted (the write is a
   no-op that never reflects into a subsequent read), and most screens have no
   data to act on.
2. **Safe parallelism needs per-test isolation.** Playwright currently runs
   `fullyParallel: false` with one worker *precisely because* the mock state is
   global — parallel workers would stomp each other. The same global state
   blocks both goals.

The keystone is therefore a **canonical sample-data factory** that produces a
**fresh, isolated** dataset per test and is projected into both the Rust seed
builder and the browser mock runtime. This ADR records the test-architecture
decisions for the foundational test-architecture epic (extends issue `428c4c3`).

## Decisions

### 1. Canonical sample-data factory — seed builders, not a shared golden DB

The single source of realistic test data is a **deterministic seed/factory**, not
a checked-in mutable `.sqlite` snapshot. A pre-built binary DB is explicitly
rejected as the primary fixture: it is not diff-reviewable, it drifts against
migrations (a second source of truth for the schema), and sharing one mutable
instance re-introduces the global-state coupling this epic removes.

- The factory defines a small set of **named scenarios** — at least `empty`,
  `minimal`, and `rich` — plus per-area scenarios as needed. Each test picks the
  **smallest** scenario that proves its behavior (the lean-and-fast rule); a
  single mega-dataset that every test loads is rejected because it makes every
  test slower and assertions vaguer.
- Datasets are **materialized fresh per test** (a fresh in-memory DB for Rust via
  `open_in_memory_database`; a fresh runtime-state clone for the browser layer).
  Tests never share a mutable instance.
- Datasets are **deterministic**: fixed IDs, fixed timestamps, no random or
  wall-clock data, so results are stable across parallel workers.
- The factory builds on the **current migrated schema**, so it cannot go stale:
  the same canonical dataset shape is projected into (a) a **Rust seed builder**
  and (b) the **browser mock runtime's initial state**, giving both layers one
  source of truth for realistic data.

### 2. Stateful, per-test-isolated mock Tauri runtime (the keystone)

`browserSmokeRuntime.ts` becomes **stateful**: command handlers that mutate
(create/update/delete) write into per-test runtime state, and subsequent reads
reflect those writes, so a clickable journey (create watchlist → it appears;
delete note → it is gone) can be asserted end to end. State is **seeded from the
canonical factory** (Decision 1) and **reset/cloned per test** so workers and
tests do not interfere. Unhandled commands keep throwing from the `switch`
default (a clear "add a case" signal), per the existing design.

**Delivered (mock-layer unification, issue `749a5a8`).** The canonical factory
and stateful runtime live in `src/test/scenarios/`: `entities.ts` (deterministic
builders typed against the ts-rs-generated DTOs), `scenarios.ts`
(`buildScenario(empty|minimal|rich)`, a fresh `structuredClone` per call), and
`runtime.ts` (`createMockRuntime` — **one** stateful router over every IPC
command; an unhandled command rejects). Both TS mock layers became thin adapters
over this one runtime: the Vitest harness (`appWorkflowHarness.tsx`, which keeps
its legacy `appTestState`/`handleAppCommand` surface as a view over the runtime
store) and the Playwright browser runtime (`browserSmokeRuntime.ts`, which
injects its browser seed into the runtime store and exposes
`window.__brawlerMockReset`). The two previously-independent command routers and
seed sets are gone — there is no longer a second router to drift. `minimal`
carries one of every object (legacy screen-test data preserved via overrides);
`rich` is the dense 28-company browser dataset. A completeness guardrail
(`runtime.test.ts`) fails if any whole-collection read returns empty under
`minimal`/`rich`, so a new feature that forgets to seed its entity is caught.

### 3. Migration corpus — the one place a binary DB earns its keep

Migration tests use a small set of **historical-schema `.sqlite` snapshots** at
prior migration versions, used **only** to prove that migrations upgrade real
old data without loss (the `v0.40.0` failure class). This is a targeted artifact
for the migration-safety cluster, not the general fixture; snapshots are small,
versioned, and append-only alongside the append-only migrations they exercise.

### 4. Broad clickable coverage across all 12 primary screens

Real clickable journeys are extended to every primary screen (Companies,
Diagnostics, Events, Inbox, License, Notebooks, ReportSeason, Research, Settings,
Sources, Transcripts, Watchlists), riding the now-stateful runtime. Broad
clickable coverage **stays in the Playwright browser-smoke layer with mock
Tauri** — real-Tauri E2E is not introduced (consistent with
[ADR 0021](0021-browser-ui-regression-testing.md)). Behavior that genuinely needs
the real runtime (OS keychain, file dialogs, WebView2, packaged behavior) stays
in the live/packaging smoke and native-Windows paths, not here. The preference
order from ADR 0021 holds: make a bug structurally impossible > assert an
invariant > assert a journey > screenshot.

### 5. Layered parallelism — within and across frameworks

Test execution is parallelized to shorten the loop, **without lowering quality**.
Isolation (Decisions 1–2) is the precondition that makes this safe.

- **Within frameworks:** Rust adopts **`cargo nextest`** (parallel by default,
  better per-test isolation) in the check loop; Vitest remains parallel (default
  pool); Playwright enables **`fullyParallel`** with multiple workers, now safe
  because runtime state is per-test isolated.
- **Across frameworks:** `make check` becomes a **staged concurrent
  orchestrator**: a fast-fail stage (typecheck ‖ fmt ‖ lint ‖ stylelint) runs
  first, then the heavy independent suites (Rust clippy+nextest ‖ Vitest ‖
  frontend build) run concurrently. The primary win is overlapping the **Rust
  compile** (whose link/codegen phases leave cores idle) with the JS suites that
  need no Rust build; stacking two already-CPU-bound test phases is not expected
  to pay and is not the goal.

**Quality guardrails (non-negotiable):** per-framework worker counts are **capped
so the sum ≈ core count** to avoid oversubscription-induced flakiness (a
Playwright timeout caused by CPU starvation is a false failure); the orchestrator
**captures per-task output and prints grouped pass/fail** and preserves
**hard-stop on first failure**, with a serial mode available for clean logs; no
two parallel tasks write the same `target/` or build artifacts (host/Nix cargo
must not be mixed). Wall-clock is **measured before/after** and a parallelism
change is kept only if it actually wins on the WSL2 reference machine.

### 6. Gate promotion — clickable suite toward a default gate

Once the extended clickable suite is fast, stable, and parallel, it is **promoted
from opt-in/periodic toward a default/pre-merge gate** (the regression loop the
owner asked for). This amends the opt-in posture of
[ADR 0021](0021-browser-ui-regression-testing.md) (which already anticipated "a
later milestone may promote a small subset to default local checks or CI"). The
lean-and-fast constraint still binds: promotion happens **only** while the suite
keeps `make check` within the seconds-to-low-minutes range (parallelism is what
buys this headroom); if it would balloon the per-change gate, it stays in
`check-epic`. Live/credentialed/packaging smoke remain opt-in/periodic and never
enter the default gate.

### 7. Generated API types — Rust is the single source for the IPC contract

The TypeScript DTOs crossing the Tauri IPC boundary are **generated from the Rust
structs** with `ts-rs` (behind the off-by-default `ts-export` feature), so a Rust
DTO and its hand-written TS shape can no longer silently drift — a whole class of
contract bug that previously slipped through (the `v0.45.0` KPI scope mismatch was
of this family). `make types` emits `src/api/generated/`; the `src/api/*Types.ts`
barrels re-export those bindings (keeping stable import paths and any command-wrapper
functions); `make types-check` is the drift guard. The mechanics and per-field
conventions live in [testing.md → Generated API types](../testing.md#generated-api-types-rust--typescript); the durable decisions are:

- **`make types` runs with `TS_RS_LARGE_INT=number`** so `i64`/`u64` render as JS
  `number`, not ts-rs's default `bigint` — one global setting instead of a per-field
  override on every count/timestamp/id. This is sound because Brawler has no DTO
  field that exceeds `Number.MAX_SAFE_INTEGER` (row counts, millis, slug ids).
- **String-literal unions are single-sourced via marker enums** in
  [`src-tauri/src/api_ts_unions.rs`](../../src-tauri/src/api_ts_unions.rs) (gated,
  generation-only) and referenced from `String` fields with `#[ts(as = "...")]`.
  This preserves the **narrowed** union contract (not a widened `string`) without
  forcing the storage layer to adopt Rust enums, and keeps each union defined once.
  Genuinely frontend-only unions/inputs (no backing Rust DTO) stay hand-written.
- **Migration is incremental** (`6214fd5`), one module at a time behind the drift
  guard, so the contract is never half-generated in a broken state.

This extends the enforcement-as-guardrails posture ([ADR 0038](0038-enforcement-as-guardrails.md)):
the generator + `types-check` is the gate that keeps the cross-language contract
coherent without every agent holding both sides in context.

## Consequences

- The foundational test-architecture epic is sequenced **keystone-first**:
  Decisions 1–2 (sample-data factory + stateful isolated runtime) and Decision 5
  (parallelism) land before the broad backfill, because both the coverage and the
  parallelism goals depend on them.
- The backfill is split into keystone-first child tasks: (1) sample-data factory
  + stateful isolated runtime, (2) nextest + Playwright `fullyParallel` + staged
  concurrent `make check`, (3) high-risk backend backfill (migrations safety +
  corpus, `restore_backup`, destructive feed cleanup), (4) broad clickable
  journeys for the uncovered screens + Watchlists, (5) Vitest fill + remaining
  command contracts.
- Going forward, the per-area Definition of Done is extended: adding a feature
  also extends the canonical sample-data factory with that feature's seed
  entities, so the dataset stays current as the app grows rather than via a
  periodic catch-up. This follows the
  [ADR 0045](0045-guardrail-harvest-loop.md) harvest posture.
- Canonical docs updated in the same change: [testing.md](../testing.md) (factory,
  scenarios, parallelism, gate policy) and
  [engineering-workflow.md](../engineering-workflow.md) (the staged concurrent
  check loop and nextest). [ADR 0021](0021-browser-ui-regression-testing.md) is
  amended to record the gate promotion.
- Risk: oversubscription flakiness and false failures if worker caps are not
  honored — mitigated by Decision 5's guardrails and the measured before/after.
- **Successor:** [ADR 0049](0049-test-architecture-v2-data-transform-correctness.md)
  extends this foundation into the *data-transform* domain (property/invariant
  testing, golden snapshots, behavioral scale gates + periodic benches, parser
  fuzzing on stable, the dual-execution mock-fidelity contract, and e2e ingestion
  pipeline tests) ahead of the data-heavy v0.46+ epics. Its dual-execution
  contract is the behavioral complement to Decision 7's shape-level `ts-rs`
  contract.
</content>
