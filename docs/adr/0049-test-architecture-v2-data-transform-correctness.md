# ADR 0049: Test architecture v2 — data-transform correctness at volume

Status: Accepted

## Context

[ADR 0048](0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)
built the **foundation**: a canonical sample-data factory, a stateful per-test
mock runtime, broad clickable coverage, layered parallelism, and a generated
IPC contract. That epic (`5eabef7`) makes Brawler's *current, CRUD-shaped*
behavior verifiable end to end.

Brawler's roadmap, however, is squarely about **munching a lot of structured
data from many heterogeneous sources into one unified set**: story clustering
across sources (v0.46), the autonomous report pipeline (v0.49), report-over-report
diff (v0.47), and cross-company KPI comparison (v0.53). The correctness risk of
that work is a *different class* than CRUD: it lives in **data transforms** —
dedup, normalization, entity matching/reconciliation, merge — which are
invariant-rich and fail on the long tail of real-world input and at volume, not
on the happy path. A QA review of the current harness found the matching gaps:

- **Property-based testing is essentially absent** — exactly one `proptest!`
  (the fundamentals expression parser). The transforms above are precisely where
  example-based tests miss edge cases.
- **No golden/snapshot testing** (`insta` absent) — parser and normalization
  outputs are hand-asserted field-by-field, which under-covers shape and is
  costly to extend.
- **No performance/scale testing** — no benchmarks, no volume tests. The
  `v0.45.0` `find_similar` UI freeze (re-embedding every feed item synchronously)
  is exactly the regression class this leaves uncaught.
- **No parser fuzzing** — real RSS/HTML/XML from many sources is hostile;
  sample-based tests cover only a few hand-written error paths.
- **Mock-runtime fidelity is unverified** — the unified TS mock runtime
  (`src/test/scenarios/runtime.ts`) re-implements backend semantics (dedup, slug
  derivation, settings nesting). `ts-rs` guarantees the DTO *shapes* match Rust;
  nothing guarantees the *behavior* does, so the whole frontend suite can be
  green against a mock that lies.
- **No end-to-end ingestion pipeline test** — storage and jobs are covered in
  isolation, but nothing crosses adapter → storage → read-model in one go, which
  is exactly the seam the unification logic will occupy.
- **Mutation testing is narrow** (`expr/**` + `migrations.rs`) and will not
  follow the new transform surface unless its scope is deliberately extended.

This ADR records the test-architecture decisions for the **v2 epic** —
*data-transform correctness at volume* — the successor to `5eabef7`. Like its
predecessor it is a non-product, cross-cutting test-infra epic (no
`milestone:vX.Y.0` label) that lands ahead of v0.46 so the harness exists
*before* the data features it must guard.

## Decisions

### 1. Property-based & invariant testing is a first-class layer

Data transforms are tested by the **invariants** they must satisfy, not only by
examples. `proptest` (already a dependency) generates inputs; a small set of
**reusable invariant helpers** assert the algebraic properties every transform
in this domain shares, so future data epics plug into the same harness rather
than re-deriving it. The invariants this codebase commits to:

- **Idempotence** — re-running a dedup/normalization/merge over its own output is
  a no-op (`f(f(x)) == f(x)`).
- **Order-independence (commutativity)** — dedup/reconciliation of the same items
  in any arrival order yields the same canonical set. This is the core property
  for "the same real-world entity arrives from multiple sources."
- **Round-trip** — `normalize ∘ parse` (and where applicable `render ∘ parse`)
  preserves meaning; a normalized value re-normalizes to itself.
- **Determinism & stable identity** — the same input always produces the same
  output and the same canonical id (no wall-clock/random leakage).
- **Associativity of merge** — merging source A then B then C equals any
  re-association; multi-source unification cannot depend on grouping.
- **Totality / no-panic** — the transform returns a result (or a typed error) for
  every input in its domain, never panics.

Property tests run in the **normal stable test binary** and are part of
`make check` (bounded case counts keep them fast); heavier case counts run under
`make check-epic`.

### 2. Golden snapshots (`insta`) for parser and normalization output

Complex structured outputs — source-adapter parse results and KPI/financial
normalization results — are locked with **`insta` snapshots** rather than
hand-asserted field-by-field. Snapshots are committed, **diff-reviewable**, and
**regenerated, not hand-edited** (`cargo insta review`/`accept`). This is the
cheapest way to lock a complex shape and to make a future change to that shape a
reviewable diff instead of a silent drift. Snapshot tests are deterministic and
part of `make check`. Golden snapshots are used for *output shape*, never as a
substitute for the behavioral assertions and invariants of Decisions 1 and 4.

### 3. Parser fuzzing via `proptest` structured generators on the stable toolchain

Source parsers are hardened against malformed real-world input with **`proptest`
structured generators** that synthesize adversarial HTML/RSS/XML and assert the
parser **never panics and never amplifies** (output item count bounded by input
length; no unbounded loop/allocation). Every Brawler parser consumes `&str`, so a
proptest string/recursive strategy generates adversarial *text* directly — this
is the right tool. **`arbitrary` is *not* added**: its raw-bytes → structured
input deriving earns its keep on byte-oriented parsers, which this codebase does
not have; adding it (or `cargo-fuzz`) for string parsers would be an unused
dependency at best and a nightly-toolchain split at worst. It is reserved for a
future byte-oriented parser, should one appear.

**`cargo-fuzz` is explicitly rejected.** It requires a **nightly** toolchain
(libFuzzer + SanitizerCoverage are passed via `-Z` unstable flags), which would
force a *second, pinned-nightly* toolchain into [`flake.nix`](../../flake.nix)
alongside the single `rust-bin.stable.latest.default` it pins today. That is the
exact **split-toolchain** posture `AGENTS.md` and
[engineering-workflow.md](../engineering-workflow.md) warn against (a toolchain
split silently produces false `cargo test --doc` failures and hides clippy
lints), plus a standing maintenance/closure cost (pinning + bumping nightly,
aligning sanitizer/LLVM components) for a job that is periodic either way.
`proptest`/`arbitrary` on stable buys the bulk of the value — the
malformed-input long tail, deterministic, seeded, shrinking, in-suite — at zero
toolchain cost. What it forgoes is *coverage-guided* input evolution, which earns
its keep on deep binary formats and state machines far more than on the shallow,
structured grammars (RSS/HTML/XML) Brawler parses. If a specific parser ever
proves genuinely adversarial, `cargo-fuzz` is revisited as a one-off,
out-of-tree investigation — never a permanent flake fixture.

### 4. Scale is tested behaviorally and deterministically; perf is benched periodically

Performance/scale correctness is split into two mechanisms so that the **hard
gate stays deterministic** and machine-dependent timing never flakes CI:

- **Behavioral scale gates (in `make check`).** Deterministic assertions that a
  hot path is **offloaded** (a `#[tauri::command]` doing meaningful CPU/IO work is
  `async` + `spawn_blocking`, per the AGENTS.md UI-thread rule) and
  **algorithmically bounded** — it scans the *persisted derived index*, not the
  whole corpus, and is `O(rows)` not `O(rows²)`. These are asserted via structure
  (the function is `async`/offloads) and via instrumented counters or row-count
  invariants over a volume dataset, **not** wall-clock. This is the mechanism that
  catches the `v0.45.0` `find_similar` regression class.
- **Periodic `criterion` benchmarks with a relative ratchet (never a hard
  gate).** `criterion` micro-benchmarks cover the genuinely hot kernels (vector
  similarity, expression eval, dedup). `make bench` runs them; a **relative
  ratchet** — mirroring the existing `coverage-ratchet.mjs` — flags a regression
  beyond a tolerance against a committed baseline. Because wall-clock is
  machine-dependent, this is **periodic and informational**; it never fails
  `make check`. Absolute ms budgets as a hard gate are explicitly rejected.
  *Superseded ([ADR 0096](0096-quality-gate-architecture-under-continuous-release.md) amendment, #336):
  the committed baseline is retired; the audit is base-vs-head on one runner (`bench-audit.yml`).*

### 5. Mock-runtime fidelity via a dual-execution contract

The unified TS mock runtime is made a **verified** proxy of the backend, not an
unverified second implementation. A **shared journey corpus** — a
language-neutral list of `(command, input)` steps with the expected observable
result — is replayed against **both**:

1. the TS mock runtime (`createMockRuntime`), and
2. a **headless real-Rust harness** that drives the same commands through the
   `AppState`/storage layer against a fresh `open_in_memory_database` (the layer
   the thin `#[tauri::command]` wrappers delegate to — per
   [testing.md](../testing.md) the `tauri::State` wrapper itself is not
   unit-constructible, so the contract targets the layer beneath it),

and asserts the two produce **equivalent observable output** for each step. Where
they disagree, the mock is wrong (or the corpus encodes an expectation the
backend does not meet) — either way a real defect surfaces instead of hiding.
The corpus starts with the **core CRUD/read commands** and grows with the
factory. This closes the mock-drift gap that the entire frontend suite rests on,
and complements `ts-rs` (which already guarantees shapes) by guaranteeing
*behavior*.

### 6. End-to-end ingestion pipeline tests (adapter → storage → read-model)

A small set of tests exercises the **full ingestion seam in one go**: feed sample
source payloads through a real adapter, into real storage, and assert the
**unified, deduped** read model (feed/events/registry) that results. This is
distinct from the isolated adapter-parse and storage tests; it is the only layer
that proves the *unification* — dedup across sources, company matching, event
derivation — works as a pipeline, which is the heart of the roadmap. It uses the
canonical sample data and in-memory DB, stays deterministic, and is part of
`make check`.

### 7. Mutation scope follows the highest-risk transform logic

`cargo-mutants` scope is **extended to the transform modules** introduced/covered
by Decision 1 (dedup, matching, normalization), not left at `expr/**` +
`migrations.rs`. Mutation testing is the strong signal that the property and
golden tests actually *kill* defects (line coverage does not prove this). It
stays **periodic** (`make mutants`, part of the closure-cadence `check-epic`
neighborhood, not the per-change gate) with the cadence documented in
[engineering-workflow.md](../engineering-workflow.md), so the scope grows with
the transform surface rather than rotting at its original two globs.

## Consequences

- **New dev-dependencies, none shipped in the binary:** `insta` (snapshot
  review) and `criterion` (benchmarks). Both are `[dev-dependencies]`; `proptest`
  is already present and covers both the invariant layer (Decision 1) and the
  parser fuzzing (Decision 3), so no `arbitrary`/`cargo-fuzz` is added. This stays
  within the conservative-dependency posture (`AGENTS.md`): dev-only, widely used,
  each mapping to a decision above.
- **Gate placement:** `make check` (fast/hard) gains the property tests (Decision
  1), golden snapshots (2), bounded-iteration parser fuzz (3), behavioral scale
  gates (4), the dual-execution contract (5), and the e2e pipeline tests (6) —
  all deterministic and fast. `make check-epic` gains heavier fuzz iterations and
  the expanded mutation scope (7). The `criterion` benches + ratchet (4) are
  **periodic** and never hard-gate.
- **The epic is implemented big-bang** (all seven, ahead of v0.46), building the
  harness against *today's* transforms (dedup keys, company matching,
  KPI/financial normalization, vector similarity, expr eval, the source-refresh
  pipeline) so the framework exists before the data epics; v0.46+ features then
  extend the same invariant/golden/fidelity harness rather than inventing their
  own.
- **Definition of Done is extended** (the [ADR 0045](0045-guardrail-harvest-loop.md)
  harvest posture, propagated into `AGENTS.md` Testing Expectations): a new data
  **transform** ships with its invariants (Decision 1) and a golden snapshot of
  its output (2); a new **command** adds a step to the dual-execution corpus (5);
  a new **hot path** adds a behavioral scale gate (4). The harness stays current
  as the app grows rather than via periodic catch-up.
- **Canonical docs updated in the same change:** [testing.md](../testing.md)
  (property/invariant, golden, scale & perf, fuzzing, mock fidelity, e2e pipeline
  sections; mutation section updated) and
  [engineering-workflow.md](../engineering-workflow.md) (gate placement, bench
  cadence, DoD additions). This ADR is the durable record; the docs carry the
  mechanics.
- **Relationship to ADR 0048:** this ADR *extends* it (same factory, same runtime,
  same parallelism and gate philosophy) into the data-transform domain; 0048 is
  not superseded. The dual-execution contract (Decision 5) is the behavioral
  complement to 0048 Decision 7's shape-level `ts-rs` contract.
- **Risk:** the dual-execution corpus (Decision 5) is the highest-effort item and
  could drift toward asserting the mock against itself if the Rust side is stubbed
  rather than real — mitigated by requiring the Rust side to run against a real
  `open_in_memory_database` through the actual `AppState`/storage layer.
</content>
</invoke>
