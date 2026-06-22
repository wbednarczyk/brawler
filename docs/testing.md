# Testing

The single home for Brawler's testing strategy, layers, and the manual/live/packaging smoke procedures. The day-to-day build/validation discipline and the Definition of Done live in [Engineering Workflow](engineering-workflow.md); this doc is the detailed testing reference it points at.

Use [Project Brief](project-brief.md) for the full documentation map. Related: [Engineering Workflow](engineering-workflow.md), [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md), [ADR 0021: Browser UI Regression Testing](adr/0021-browser-ui-regression-testing.md), [ADR 0048: Test architecture foundation](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md), [ADR 0049: Test architecture v2 — data-transform correctness](adr/0049-test-architecture-v2-data-transform-correctness.md).

## Strategy

**Aim for automated coverage of every behavior.** Every command/contract, read model, UI workflow, migration, source adapter, provider mapping, job, and fixed regression should have a test that fails when that behavior breaks. "It's hard to test" or "it's only a small thing" is not a reason to skip coverage — find the cheapest layer that exercises it. The goal is *full behavior coverage*, not partial.

The constraint is that the suite stays **lean and fast** — coverage of everything must never mean a bloated suite or a gate that takes hours. Two rules hold both at once:

1. **Test behavior and contracts, not implementation details.** One clear test per behavior; assert the observable result (the command's output, the rendered state, the stored row), not internal mechanics. **Delete tests that no longer protect behavior, and never add a redundant or brittle test "to be safe"** (especially screenshot-diff tests) — that bloat is exactly what makes a suite slow, flaky, and ignored. More tests is not the goal; covering every behavior *once, well* is.
2. **Keep the bulk in the fast layers, and keep the slow ones out of the per-change gate.** `make check` must stay in the seconds-to-low-minutes range. The slow, flaky, or credentialed layers (Playwright browser smoke, live provider smoke, packaging smoke) are **opt-in/periodic** (`make check-epic`), never in `make check` — so "test everything" never means "every push takes hours." Default CI/local checks stay deterministic and secret-free; anything needing credentials/network/external services is manual or opt-in.

**The layers (push coverage down to the cheapest layer that proves the behavior):**

- many **Rust unit/contract tests** — domain logic, command contracts, parsing, dedupe, migrations, provider mapping, jobs; the bulk of coverage, milliseconds each;
- **frontend component/workflow tests** (Vitest) for every UI state and workflow, not just critical ones;
- **test-sample-backed integration tests** for source adapters and migrations (parsing, dedupe keys, company matching, error handling, migration safety, data persistence);
- **browser UI smoke** (Playwright) for layout/scroll/overflow regressions Vitest/jsdom cannot catch — periodic;
- a few **desktop / packaging / live-provider smoke** checks for what only the real runtime, package, or provider can prove — periodic/manual.

## Sample-data factory and per-test isolation

Realistic test data comes from a **canonical sample-data factory**, not a checked-in mutable `.sqlite` snapshot (a binary golden DB is rejected — it is not diff-reviewable, drifts against migrations, and sharing one mutable instance breaks isolation and parallelism). Policy: [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md).

- The factory defines a few **named scenarios** (`empty`, `minimal`, `rich`, plus per-area as needed); each test picks the **smallest** scenario that proves its behavior — never one mega-dataset that every test loads.
- Datasets are **materialized fresh per test** (a fresh in-memory DB for Rust via `open_in_memory_database`; a fresh runtime-state clone for the browser layer) and are **deterministic** (fixed IDs/timestamps, no random/wall-clock data) so they are stable across parallel workers.
- One canonical TypeScript factory (`src/test/scenarios/`: `entities.ts` builders typed against the ts-rs-generated DTOs → `scenarios.ts` `buildScenario(empty|minimal|rich)` → `runtime.ts` `createMockRuntime`, a single stateful router over **all** IPC commands) feeds **both** TS mock layers. The Vitest harness (`src/test/appWorkflowHarness.tsx`) and the Playwright browser runtime (`src/test/browserSmokeRuntime.ts`) are thin adapters over that one runtime — neither carries its own command router, so they cannot drift. The runtime is **stateful and per-test isolated** (writes reflect into subsequent reads; fresh deep-cloned store per test) so create/edit/delete clickable journeys can be asserted end to end. (The Rust seed builder remains a separate projection of the same scenario shapes.)
- **Migration corpus:** a small set of historical-schema `.sqlite` snapshots is the one place a binary DB earns its keep — used **only** by migration tests to prove migrations upgrade real old data without loss (the `v0.40.0` failure class), not as the general fixture.
- **Going forward:** adding a feature also extends the factory with that feature's seed entities, so the dataset stays current as the app grows (a per-area Definition-of-Done step, not a periodic catch-up).

**Mock runtime conventions** (`src/test/scenarios/runtime.ts`) — when adding or editing a command handler:

- **A mutating handler must REASSIGN its store collection (`d.x = [...]` / `.map` / the `mapReplace` helper), never mutate an entity in place** (`d.x.find(...).field = ...`, `d.x.push(...)`). An in-place mutation keeps the same array reference, so a UI read returns a reference-equal value and **React bails on re-render** — the clickable journey then passes the IPC-call assertion but the change never shows. This was the single biggest source of failures during the mock unification. Enforced by the `runtime.test.ts` **"re-render safety"** gate (every listed mutation must hand back a new collection reference).
- **Return the contract shape, not the entity.** A handler's return must match the command's generated return type — e.g. `capture_report_document` returns `DocumentCaptureResult` (`{ documentId, … }`), not the `ReportDocument`. Extract the authoritative shape from the api layer: `grep 'callCommand<…>("<command>"' src/api/*.ts`.
- **Reads project the store; an unknown command rejects** (the "add a case to `runtime.ts`" signal — add it once, in the shared router, never per layer). Every whole-collection read must be non-empty under `minimal`/`rich`, enforced by the `runtime.test.ts` **completeness guardrail**.

## Parallel execution

Suites run in parallel within and across frameworks to keep the loop fast, with isolation (above) as the precondition that keeps quality intact. Within frameworks: Rust uses `cargo nextest`, Vitest uses its default pool, Playwright runs `fullyParallel`. Across frameworks: `make check` is a staged concurrent orchestrator (fast-fail typecheck/fmt/lint stage, then heavy suites concurrently), the main win being the Rust compile overlapping the JS suites. Worker counts are capped so the sum ≈ core count (oversubscription causes false-timeout flakiness), output is grouped with hard-stop on first failure, and changes are kept only on a measured win. See [Engineering Workflow](engineering-workflow.md) and [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md).

## Coverage ratchet

`make coverage` measures line coverage — frontend via Vitest's v8 provider, Rust via `cargo-llvm-cov` — then runs a **ratchet** (`scripts/check/coverage-ratchet.mjs`) that fails if either layer drops below the committed floor in `coverage-baseline.json`. This enforces the full-coverage policy as a *trend* (never regress) without a brittle absolute target; when coverage rises it prints the new floors to commit. It is periodic (the instrumented Rust build is slow), not part of `make check`. Policy: [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md).

## Real-data validation precedes implementation for matching/ranking features

Any feature whose value rests on a **similarity / dedup / clustering / matching /
ranking** decision must be validated for precision/recall against a **real,
representative dataset before committing to an implementation approach** — not
after. Build a small hand-labeled ground-truth set from real data and measure the
candidate signals; a feature that only "works" on synthetic samples or the happy
path can be confidently semi-working and shipped before anyone notices. This is a
harvested guardrail ([ADR 0045](adr/0045-guardrail-harvest-loop.md)): cross-source
story clustering (`v0.46.0`) passed every synthetic test and a green `make check`,
yet real-data validation showed no local method reached trustworthy precision at
useful recall, and the feature was dropped ([ADR 0051](adr/0051-story-clustering-across-sources.md)).
The cost of the up-front measurement is an afternoon; the cost of skipping it is a
fully-built feature that has to be reverted.

The **report-over-report diff** (`v0.47.0`, [ADR 0052](adr/0052-report-over-report-diff.md))
followed this rule and is the worked example of it paying off: a pure-Rust
extraction + section-alignment spike measured against real watchlist report PDFs
(11 PDFs, 6 issuers) showed extraction is reliable everywhere, structured
financial statements align 85–92% by heading, but the narrative management report
(MD&A) aligns only 4% — so the milestone was narrowed to statements and the
MD&A/AI-summary work deferred *before* any production code was written. Its
shipped gates:

- **Golden extraction snapshot per issuer format** — an `insta` snapshot of the
  extracted section structure for a sample financial statement per issuer layout,
  so an extractor/heuristic change is visible in review.
- **Deterministic self-diff = empty (hard gate).** A financial statement diffed
  against itself must produce an all-`unchanged`, zero-delta result. This is the
  invariant that caught the naive exact-heading matcher cross-matching duplicate
  headings; alignment keys on heading + ordinal with positional consumption.
- **Alignment eval with a precision floor** over the real consecutive same-type
  pairs (kept under `private/`, out of CI), measured before relying on the
  heuristic — the same precision/recall discipline as above.
- **Panic-safety + no-text-layer gates.** Extraction was validated against the
  **whole GPW + NewConnect market** (613 real reports across 770 companies, via the
  shipped resolver): 89.4% extract clean, 10.4% are scanned/no-text-layer (correctly
  flagged), 0.16% panic `pdf-extract`, 0% silent garbage. That run surfaced three
  requirements now under test: `pdf-extract` runs in `catch_unwind` (a known-panicking
  PDF must yield `extraction_failed`, not a crash); a scanned report must classify
  `no_text_layer` by text density; and **ESEF/iXBRL `.xhtml`** is a first-class
  second format (some large issuers file xhtml-only). The 613-report corpus is the
  extraction-robustness reference set under `private/`.
- The diff transform also carries the property/invariant gates below
  (idempotence, order-stability).

## Data-transform correctness (property, golden, scale, fuzz, fidelity, pipeline)

Brawler's roadmap is about munching a lot of structured data from many sources
into one unified set (the autonomous report pipeline, report-over-report diff,
cross-company KPI comparison). That correctness risk
lives in **data transforms** — dedup, normalization, entity matching/reconciliation,
classification (denylist/keyword routing of inputs into typed buckets), merge —
which fail on the long tail and at volume, not on the happy path. The
following layers test that class. Policy: [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md)
(extends [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)).

### Property-based & invariant testing

Data transforms are tested by the **invariants** they satisfy, not only by
examples. `proptest` generates inputs; reusable invariant helpers assert the
algebraic properties this domain shares, so a new transform (and the data-heavy
roadmap epics) plug into the same harness instead of re-deriving it. The committed
invariants: **idempotence** (`f(f(x)) == f(x)`), **order-independence /
commutativity** (same canonical set regardless of arrival order — the core
property for "the same entity arrives from multiple sources"), **round-trip**
(`normalize ∘ parse` preserves meaning), **determinism & stable identity** (same
input → same output and same canonical id; no wall-clock/random leakage),
**associativity of merge** (multi-source unification cannot depend on grouping),
and **totality / no-panic** (a result or typed error for every input, never a
panic). Property tests run in the normal stable test binary and are part of
`make check` (bounded case counts); heavier counts run under `make check-epic`.

### Golden snapshots (`insta`)

Complex structured outputs — source-adapter parse results, KPI/financial
normalization results, and **classification/routing decisions over a representative
input corpus** (e.g. the report-over-report diff's statement classifier, which routes
real GPW/NewConnect filing-component titles into SSF/JSF or rejects them — its golden
is `report_diff::classify::tests::classification_corpus_golden`) — are locked with
**`insta` snapshots**, not hand-asserted
field-by-field. Snapshots are committed, **diff-reviewable, and regenerated, not
hand-edited** (`cargo insta review` / `cargo insta accept`). They lock *output
shape* and make a deliberate shape change a reviewable diff instead of silent
drift; they never replace the behavioral assertions/invariants above. Snapshot
tests are deterministic and part of `make check`.

### Scale & performance gates

Scale correctness is split so the **hard gate stays deterministic** and timing
never flakes CI:

- **Behavioral scale gates (in `make check`).** Deterministic assertions that a
  hot path is **offloaded** (a meaningful-work `#[tauri::command]` is `async` +
  `spawn_blocking`, per the AGENTS.md UI-thread rule) and **algorithmically
  bounded** — it scans the persisted derived index, not the whole corpus, and is
  `O(rows)` not `O(rows²)`. Asserted via structure and via instrumented
  counters / row-count invariants over a volume dataset, **not** wall-clock. This
  is the mechanism that catches the `v0.45.0` `find_similar` regression class.
- **Periodic `criterion` benches with a relative ratchet (never a hard gate).**
  `make bench` runs micro-benchmarks on the hot kernels (vector similarity,
  expression eval, dedup); a relative ratchet (mirroring `coverage-ratchet.mjs`)
  flags a regression beyond tolerance against a committed baseline. Wall-clock is
  machine-dependent, so this is **periodic and informational** — it never fails
  `make check`, and absolute ms budgets as a hard gate are rejected.

### Parser fuzzing (`proptest` / `arbitrary`, stable toolchain)

Source parsers are hardened against malformed real-world input with **`proptest`
structured generators** (`tests/parser_fuzz.rs`) that synthesize adversarial
HTML/RSS/XML and assert the parser **never panics and never amplifies** (output
item count bounded by input length; no unbounded loop/allocation). This runs in
the normal stable test binary (deterministic, seeded, shrinking) — bounded
iterations in `make check`, heavier via `PROPTEST_CASES` under `make check-epic`.
Every Brawler parser consumes `&str`, so a proptest text strategy is the right
generator; **`arbitrary`/`cargo-fuzz` are deliberately not used** (raw-bytes
deriving + coverage-guidance earn their keep on byte-oriented parsers Brawler
does not have, and `cargo-fuzz` needs a nightly toolchain that would split the
single Nix-pinned toolchain — full rationale in [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md)).

**A third-party parser we don't control runs inside `catch_unwind`, and a test
asserts a known-bad input is *flagged*, not a panic.** Our own parsers are total
(above), but `pdf-extract` panics on a small fraction of real PDFs (~0.16% across
the 613-report market corpus, ADR 0052). A panic on the offloaded extraction
thread would crash the job, so extraction wraps the call and turns a panic into an
`extraction_failed` state — covered by a panic-safety test
(`report_diff::extraction::tests::unreadable_pdf_is_flagged_not_panicked`). Any
new dependency that parses real-world files gets the same `catch_unwind` + a
flagged-not-crashed test.

### Mock-runtime fidelity — the dual-execution contract

The unified TS mock runtime (`src/test/scenarios/runtime.ts`) re-implements
backend semantics; `ts-rs` guarantees the DTO *shapes* match Rust but not the
*behavior*, so the frontend suite could be green against a mock that lies. A
**shared journey corpus** — a language-neutral list of `(command, input)` steps
with the expected observable result — is replayed against **both** the TS mock
runtime (`createMockRuntime`) and a **headless real-Rust harness** driving the
same commands through the `AppState`/storage layer against a fresh
`open_in_memory_database` (the layer the thin `#[tauri::command]` wrappers
delegate to — the `tauri::State` wrapper itself is not unit-constructible). The
two must produce **equivalent observable output** per step; a disagreement means
the mock is wrong (or the corpus encodes an expectation the backend does not
meet) — either way a real defect surfaces. The corpus starts with core CRUD/read
commands and grows with the factory. This makes the mock a *verified* proxy and
is the behavioral complement to the shape-level `ts-rs` contract.

Files: the corpus is `src/test/scenarios/fidelity-corpus.json` (language-neutral —
`command` + `input`, with `$name` capture/substitution and `expectField` /
`expectContains` / `expectAbsent` assertions); the TS replayer is
`src/test/scenarios/fidelity.test.ts` (over `createMockRuntime`); the Rust
replayer is `src-tauri/src/storage/tests/mock_fidelity.rs` (over `AppState` on a
fresh `open_in_memory_database`). Journeys use seed-free root entities and
seed-independent assertions, and reuse each created entity's returned id, so they
hold regardless of either side's id-derivation scheme. **Add a journey to the
corpus when you add or change a command's observable behavior.**

### End-to-end ingestion pipeline tests

A small set of tests feeds sample source payloads through a real adapter, into
real storage, and asserts the **unified, deduped** read model (feed/events/
registry) that results — the only layer that proves *unification* (cross-source
dedup, company matching, event derivation) works as a pipeline, distinct from the
isolated adapter-parse and storage tests. Deterministic, in-memory DB, part of
`make check`.

The shared ingestion spine (Architecture v2 / [ADR 0050](adr/0050-architecture-v2-domain-stores-source-pipeline-durable-jobs.md))
is covered at two levels: the **stages** in `storage::ingestion` (the `story_key`
derivation has its own unit tests + the golden snapshot in `entity_resolution`;
the unified `upsert_feed_item` is exercised by every feed-item adapter's
ingest test), and an integration test that ingests across two sources and asserts
the persisted **cross-source `story_key`** clusters matched items while unmatched
items get none. Adding an ingest path keeps it on the spine (`upsert_feed_item` +
`record_source_outcome`); a new identity/clustering transform in
`entity_resolution` ships with the ADR 0049 invariant + golden treatment.

### Mutation testing scope

`make mutants` (`cargo-mutants`) is the strong signal that the property and
golden tests actually *kill* defects — line coverage does not prove this. Its
`-f` scope **follows the highest-risk transform logic**: it starts at
`src/fundamentals/expr/**` + `src/storage/migrations.rs` and is **extended to the
dedup/matching/normalization modules** as they land, rather than rotting at the
original globs. It stays **periodic** (closure-cadence, in the `make check-epic`
neighborhood — not the per-change gate). Policy: [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md).

## Generated API types (Rust → TypeScript)

The TypeScript DTOs that cross the Tauri IPC boundary are **generated from the Rust source** with `ts-rs`, so a Rust struct and its TS shape cannot silently drift (ADR 0048). The Rust DTO carries `#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]` + `#[ts(export, export_to = "../../src/api/generated/")]` (ts-rs honors the existing `#[serde(rename_all = "camelCase")]` via serde-compat); `make types` emits `src/api/generated/`, and the hand-written `src/api/*Types.ts` module re-exports the generated types so consumers keep a stable import path. `make types-check` regenerates and fails if the committed bindings drift from the Rust source. `ts-rs` is behind the off-by-default `ts-export` feature, so it never ships in the binary; `src/api/generated/` is lint-excluded (regenerate, don't hand-edit).

**Generation conventions** (so the generated shape matches the hand-written contract exactly):

- **Integers.** `make types` runs with `TS_RS_LARGE_INT=number`, so `i64`/`u64` render as `number` (not ts-rs's default `bigint`) — matching our JS-safe row counts, millis, and ids. `usize`/`isize`/`f64` are already `number`. **No per-field numeric override is needed**; do not add `#[ts(type = "number")]`.
- **`Decimal` / monetary.** ts-rs cannot derive `rust_decimal::Decimal`; we serialize those as strings, so annotate the field `#[ts(type = "string")]`. (Most "numeric-looking" string fields, e.g. `valueNumeric`, are already `String` in Rust — no override.)
- **String-literal unions.** Many DTO fields are `String` in Rust but a closed string-literal union in TS. Define the union **once** as a marker enum in [`src-tauri/src/api_ts_unions.rs`](../src-tauri/src/api_ts_unions.rs) (gated behind `ts-export`, `#[serde(rename_all = ...)]` reproduces the wire values) and reference it from the field with `#[ts(as = "crate::api_ts_unions::Foo")]` (or `#[ts(as = "Option<crate::api_ts_unions::Foo>")]` for an optional field). ts-rs emits a named union + import — never a widened `string`. A genuinely one-off inline union may instead use `#[ts(type = "\"a\" | \"b\"")]`.
- **Optional vs nullable inputs.** Output DTOs render `Option<T>` as `T | null`. Input DTOs vary by module convention: `field?: T` → struct-level `#[ts(... optional_fields)]`; `field?: T | null` → `#[ts(... optional_fields = nullable)]`. Match the module's existing convention.
- **Name mismatch.** When the Rust struct name differs from the TS contract name (e.g. `NewManagementClaim` vs `NewManagementClaimInput`), add `rename = "TsName"` to the `ts(...)` attr so the generated type/file uses the contract name.
- **Frontend-only types** (string-literal unions and input shapes with **no** backing Rust DTO, e.g. `Theme`, `CompanyForm`) stay hand-written in the `*Types.ts` barrel alongside the re-exports.

**Migration is incremental** (tracked under `6214fd5`). Migrated: report-documents, financials, `types.ts`, research, quality-frameworks, management-claims, report-season. To migrate a module: add the gated `TS` derive to its Rust struct(s) applying the conventions above, run `make types`, confirm each generated shape matches the hand-written one, then replace the bodies in the `*Types.ts` barrel with `export type { X } from "./generated/X"` (keep any command-wrapper functions and frontend-only types).

## Per-area minimum gates

- **Ordinary change:** docs/contracts updated when behavior changes; relevant Rust + frontend tests pass; formatting/lint pass; dependency additions justified and license-reviewed when they affect runtime.
- **Source adapter:** test-sample parse/dedupe/matching/error-handling tests pass; source policy documented. Normal CI must not depend on GPW/Gemini/SEC/Nasdaq/media reachability; sample refresh is deliberate and reviewable. **Adding/changing a registered adapter** keeps the source-adapter registry (`source_adapters::registry`) the single source of truth — the `registry_matches_seeded_catalog` drift-guard test binds the Rust descriptors to the seed migrations field-for-field and must stay green (ADR 0050).
- **Storage domain store (Architecture v2):** new storage behavior goes on the relevant `*Store` and is tested at the store/`AppState` layer with `open_in_memory_database` (behavior-preserving — the existing storage suites cover the delegations). Do not add fresh methods to the `AppState` monolith.
- **Durable job queue (Architecture v2):** the `JobQueueStore` (enqueue idempotency, atomic claim, retry-with-backoff, terminal failure, crash-reclaim, counts) and the `JobWorker` dispatch are tested against a real in-memory DB (`storage::tests::job_queue`, `jobs::queue::tests`, `jobs::handlers::tests`). A **new job kind** adds a `JobHandler` in `jobs/handlers.rs` registered in `build_worker`, covered by the "every registered kind dispatches" test; the job's own logic keeps its direct `run_*_job` tests. Migrating a fire-and-forget job preserves its per-job status table + UI polling (the queue is execution, the table is status).
- **Interpretation vector index (Architecture v2):** a `VectorIndex` implementation is tested for **top-k parity with `BruteForceVectorIndex`** on separable vectors (plus empty/zero-k), so an ANN swap cannot regress ranking; the T4 behavioral scale gate still guards the persisted linear-scan contract (ADR 0050 / ADR 0049).
- **Frontend per-screen view-model context (Architecture v2):** screens read their view-model from a context (`screenViewModels`/`SettingsContext`/`SourcesContext`), not a prop bundle. They are covered by the full-app workflow tests (which render through `AppStateRoot`'s providers) and the Playwright smoke-walk; a direct component test must wrap the screen in its `Provider`. New cross-cutting settings flags get a `SettingsContext` selector rather than re-drilling.
- **AI provider:** provider contract mapping + prompt/result shape tested with samples, no live calls; normal CI requires no API keys; live checks manual/local-only.
- **Packaging:** app starts; Rust command boundary works; local SQLite opens; primary screen renders; packaged builds keep open-core navigation and preserve optional entitlement workflows.
- **Command / IPC layer (`#[tauri::command]`):** a `#[tauri::command]` fn takes `tauri::State<'_, AppState>`, which Tauri's DI constructs at runtime and which is **not** meaningfully constructible in a unit test — so do **not** write tests that build a `State` to call a command wrapper. Most wrappers are thin pass-throughs (`state.x().map_err(to_string)`) whose behavior is already covered at the **storage/jobs layer** (the `AppState` method and the `jobs::*` function it delegates to, tested with `open_in_memory_database`). When a wrapper carries **non-trivial logic** (input defaulting/parsing, branching, mapping), extract that logic into a pure helper or a function taking `&AppState` and test **that** — the established pattern (e.g. `commands::settings::developer_unlock_code_matches_value`). Adding `State`-construction tests for thin pass-throughs is redundant coverage and is rejected (see Strategy). Policy: [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md).

## Browser UI regression smoke (Playwright)

A small Playwright browser-smoke layer catches UI/layout regressions Vitest/jsdom cannot (overflow, scroll-ownership, clipping, fixed chrome). It targets the Vite preview app in Chromium with deterministic mock data — it does **not** read live sources or the user's local database. Opt-in/periodic (not in `make check`). Policy: [ADR 0021](adr/0021-browser-ui-regression-testing.md). Under [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md) this layer is being extended to **broad clickable coverage of all 12 primary screens** on a stateful, per-test-isolated mock runtime, and — once fast, stable, and parallel — **promoted toward a default/pre-merge gate** (only while it keeps `make check` within the seconds-to-low-minutes range).

Setup and run:

- `make ui-smoke-install` — first-time Chromium download into the local Playwright cache.
- `make ui-smoke` — run the suite. (`npm run test:browser:install` / `npm run test:browser` are the direct equivalents.)
- Run a subset: `npx playwright test journeys smoke-walk`.

Use it for repeated layout risks: fixed app chrome + no global scrollbar; independently scrollable panels (Companies, Watchlists, Notebooks, Inbox, Events, Sources); dense row/category sizing; and **viewport regressions across the matrix in `playwright.config.ts`** — compact (1366×768), wide (1920×1080), and the tall/narrow quarter-ultrawide at 100% (1280×1440) and 125% (1024×1152) scaling, per the UI scaling requirement in [AGENTS.md](../AGENTS.md).

How it's wired (assertion-driven, not screenshot-only):

- a shared harness (`tests/browser/helpers/harness.ts`) gives an auto console-error gate (any `console.error`/uncaught error fails the test) and reusable invariants (`expectNoPageOverflow`, a deep `expectNoHorizontalOverflow` scan, `expectInternalScroll`);
- `tests/browser/journeys.spec.ts` asserts full flows end to end; `tests/browser/smoke-walk.spec.ts` walks every primary screen asserting no page-level horizontal overflow and deep-scans the detail rail where `overflow:hidden` hides the symptom;
- mock data comes from the canonical factory (`src/test/scenarios/`): `browserSmokeRuntime.ts` builds `createMockRuntime("rich")`, injects the browser-specific seed into its store, and routes Tauri `invoke` through the one runtime (only Tauri plugin commands are handled locally). The runtime **rejects** an unknown command (the "add a case to `runtime.ts`" signal); a new screen command is added once, in the shared router, not per layer. `?locale=pl` previews the app in Polish for screenshot specs; `window.__brawlerMockReset(scenario)` re-seeds between interactions.

Two harvested rules (ADR 0045), both from the `v0.47.0` report-diff panel:

- **A no-horizontal-scroll check must assert the inner scroll containers, not just the document.** `document.documentElement.scrollWidth` is **0** when the overflow lives in an inner `overflow: auto` element (which shows *its own* scrollbar) — so a document-level check passes while the user sees a scrollbar. Assert `scrollWidth <= clientWidth + 1` on the actual scroll container(s) in the subtree (e.g. `.company-list`) and on the offending panel, across the narrow viewports. The report-diff overflow was invisible to the document check for several rounds because the scroll lived on `.company-list`.
- **A new IPC command that drives a primary-screen panel ships with (a) its case in the shared runtime router and (b) a narrow-window layout assertion for that panel.** Vitest mocks the api module, so a panel can be green in Vitest yet untested for layout (the Playwright runtime didn't know the command, so the panel never rendered with data under `make check`/`check-epic`). Add the router case + a `tests/browser/*-layout.spec.ts` overflow assertion in the same change as the command. Example: `report-diff-layout.spec.ts`.

**The harness also renders any screen for visual review** — drive a throwaway spec to the screen and `await page.screenshot(...)`. "No GUI in WSL" is not a reason to skip looking at a UI change (see [Engineering Workflow → Definition of Done](engineering-workflow.md#definition-of-done-the-handover-gate)).

Evidence policy: DOM/layout assertions are the pass/fail signal; screenshots and traces are retained only on failure; visual snapshot comparison is deferred until a stable use case justifies it.

Do **not** use this layer for: live external source/API testing; real Tauri file dialogs/keychain/taskbar/packaging/WebView2; broad end-to-end coverage of every workflow; or screenshot comparison as pass/fail evidence — those are the live/packaging smoke and native Windows paths.

## Manual desktop smoke

When automated coverage can't realistically catch it, run the app in the normal desktop path and record pass/fail notes in the milestone review before closure. `make frontend-preview` is acceptable for browser-only layout review but does not validate Tauri commands, keychain, or native window behavior.

Representative manual sweep:

- **Settings:** open every section; subnavigation stays stable; controls readable; the active panel scrolls independently. **Database:** adjust pool values, confirm out-of-range clamps + reset-to-defaults + the "applied on next launch" note.
- **Appearance:** switch dark/light/system, then `night-neon` / `midnight-horizon`; palette changes tokens without unexpectedly changing brightness mode.
- **Notebooks:** select company/note, create + edit a long note, tag-filter and clear; company list, note list, and editor scroll independently.
- **Inbox:** scan rows, change/clear filters and search, open details; the destructive cleanup action is separated from routine controls.
- **Sources:** adapters grouped by purpose, disabled/review candidates distinct, expanded rows readable, registry search + clear work.
- **Companies:** create a watchlist, toggle membership on/off, verify feedback/selected states, clear form fields.
- **Global search:** top-toolbar search → ranked results grouped by content type with snippets → select navigates → field clear returns focus.
- **Backups/restore:** in Developer Diagnostics, verify status + list, create a backup, exercise restore (warns + applies on relaunch).
- **Polish locale:** switch to Polish and check labels in Settings, Sources, Notebooks, Companies, licensing.

## Live smoke tests

Live smoke tests validate real external providers/sources. They require credentials, network, and external availability, so they are **never** part of default local checks or CI. The API key must not be stored in the repo, Nix files, `.envrc`, logs, exported settings, or SQLite settings.

### OS keyring persistence — `make smoke-keyring`

Proves the runtime OS credential backend can persist the Gemini transcription key target. The ignored Rust test `live_keyring_persists_gemini_transcription_secret` writes a temp secret to the real OS store, reads it back, clears it, and restores any pre-existing key. **Run it on the OS whose persistence you're validating** — a WSL run validates the WSL/Linux keyring, not the packaged Windows app's Windows Credential Manager. If the packaged Windows app can't persist credentials while WSL passes, add a Windows-runtime keyring validation path before closure.

### Gemini YouTube transcription — `make smoke-gemini-transcript`

Proves the configured Gemini model transcribes a real public YouTube URL into segments (ignored test `live_gemini_transcribes_youtube_url`). Default model `gemini-2.5-flash`.

Env: `GEMINI_API_KEY`, `BRAWLER_GEMINI_SMOKE_YOUTUBE_URL`, optional `BRAWLER_GEMINI_SMOKE_MODEL`, optional `BRAWLER_GEMINI_REQUEST_TIMEOUT_SECONDS` (use `45` for fail-fast; keep the `300` app default for real conference videos). First-validation URL: `https://www.youtube.com/watch?v=9hE5-98ZeCg` (Google's own Gemini video-understanding example) — validate wiring with this short input before longer videos.

```bash
GEMINI_API_KEY=... \
BRAWLER_GEMINI_SMOKE_YOUTUBE_URL='https://www.youtube.com/watch?v=9hE5-98ZeCg' \
make smoke-gemini-transcript
```

Failure interpretation: missing credentials → set `GEMINI_API_KEY`; provider limit/unavailable → retry or a smaller model; network timeout → use the short URL first, then raise the app timeout for long videos rather than treating Gemini as broken; provider rejection → try another public URL, and if the default model rejects supported URLs, change the default to the cheapest model that passes; parse error → fix provider prompting/parsing. **M10 cannot close until this passes at least once on the milestone branch.**

### Gemini feed-item analysis — `make smoke-gemini-analysis`

Proves the configured general-analysis model returns the provider-neutral AI analysis shape for one real feed-item-sized sample (ignored test `live_gemini_analyzes_feed_item`). Default model `gemini-2.5-flash`.

Required env: `GEMINI_API_KEY`, `BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL`, `BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE`, `BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY`. Optional: `BRAWLER_GEMINI_ANALYSIS_SMOKE_MODEL`, `BRAWLER_GEMINI_ANALYSIS_TIMEOUT_SECONDS` (`45` fail-fast; `90` app default), `BRAWLER_GEMINI_ANALYSIS_SMOKE_QUESTION` (custom-question path), and `..._SMOKE_{COMPANY,TYPE,SOURCE,LANGUAGE,ATTRIBUTION,SUMMARY,PROMPT_PRESET}`.

```bash
GEMINI_API_KEY=... \
BRAWLER_GEMINI_ANALYSIS_SMOKE_SOURCE_URL='https://example.com/report' \
BRAWLER_GEMINI_ANALYSIS_SMOKE_TITLE='Example company report' \
BRAWLER_GEMINI_ANALYSIS_SMOKE_BODY='Paste a short source excerpt here.' \
make smoke-gemini-analysis
```

Expect provider ID, model, `job_status=succeeded`, a non-empty summary, and ≥1 source reference. Failure interpretation mirrors the transcription smoke (set sample input explicitly — it does not read SQLite). **M13 cannot close until this passes once on the branch, or a documented provider outage/cost decision explicitly defers it.**

## Packaging smoke tests

Checklist for public release artifact candidates. Native Windows owns hands-on desktop/packaged-executable validation; a WSL Linux build does not validate Windows desktop behavior.

Build all artifacts from WSL/Linux with `make package-release-artifacts` (`.deb`/`.rpm` via Nix; AppImage via the host Ubuntu toolchain with `APPIMAGE_EXTRACT_AND_RUN=1` because the AppImage bundler must discover WebKitGTK; the GitHub workflow installs `libfuse2t64`, `librsvg2-dev`, `squashfs-tools`, `desktop-file-utils`, `appstream` and caches npm/Cargo/`src-tauri/target`/`.xwin-cache`). Subsets: `make package-linux-amd64`, `make package-windows-portable-zip`; the copy-and-run path is `make package-windows-from-linux` + `make package-windows-smoke-run`. Packaged builds enable shipped cargo features via `RELEASE_FEATURES` (see [Engineering Workflow](engineering-workflow.md)).

Expected files under `release-artifacts`: `brawler-<version>-linux-amd64.{deb,rpm,AppImage}` and `brawler-<version>-windows-x64-portable.zip`. Confirm each artifact's metadata version matches its filename.

Per-artifact smoke (each: start → window opens → open-core nav available → data dir + `brawler.sqlite3` created → create/import a small sample → close + reopen → data persists):

- **AppImage:** `chmod +x` if needed; data under `~/.brawler`.
- **.deb / .rpm:** inspect metadata (`dpkg-deb --info/--contents`, `rpm -qip/-qlp`); install on a disposable env; data under `~/.brawler`, not the install path. On WSL/WSLg, command-line startup must not abort with `Could not create default EGL display: EGL_BAD_PARAMETER` (Brawler applies a WSL-only WebKitGTK fallback; if it still fails, capture output and check whether `WEBKIT_DISABLE_DMABUF_RENDERER=1 brawler` changes behavior).
- **Windows portable zip:** extract; contains only `brawler.exe` + `README-portable-windows.txt`; a `data/` folder + `data/brawler.sqlite3` are created next to the exe.

Primary workflow check: open Inbox/Companies/Watchlists/Notebooks/Events/Sources/Research/Settings; add a company; create a watchlist + add the company; create a notebook entry; export research + settings; import into a fresh data folder; confirm source refresh returns a visible status or recoverable error.

Known candidate limitations: artifacts are unsigned (OS trust warnings possible); Windows portable relies on the system WebView2 runtime; Linux packages depend on the system WebKitGTK stack; the portable folder / `~/.brawler` must be writable; secrets stay in the OS keychain and may need re-entry after moving a portable folder to another profile/machine.
