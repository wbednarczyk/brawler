# Testing

The single home for Brawler's testing strategy, layers, and the manual/live/packaging smoke procedures. The day-to-day build/validation discipline and the Definition of Done live in [Engineering Workflow](engineering-workflow.md); this doc is the detailed testing reference it points at.

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related: [Engineering Workflow](engineering-workflow.md), [ADR 0007: GitHub Build and Lean Testing](adr/0007-github-build-and-lean-testing.md), [ADR 0021: Browser UI Regression Testing](adr/0021-browser-ui-regression-testing.md), [ADR 0048: Test architecture foundation](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md), [ADR 0049: Test architecture v2 — data-transform correctness](adr/0049-test-architecture-v2-data-transform-correctness.md).

## Strategy

**Aim for automated coverage of every behavior.** Every command/contract, read model, UI workflow, migration, source adapter, provider mapping, job, and fixed regression should have a test that fails when that behavior breaks. "It's hard to test" or "it's only a small thing" is not a reason to skip coverage — find the cheapest layer that exercises it. The goal is *full behavior coverage*, not partial.

The constraint is that the suite stays **lean and fast** — coverage of everything must never mean a bloated suite or a gate that takes hours. Two rules hold both at once:

1. **Test behavior and contracts, not implementation details.** One clear test per behavior; assert the observable result (the command's output, the rendered state, the stored row), not internal mechanics. **Delete tests that no longer protect behavior, and never add a redundant or brittle test "to be safe"** (especially screenshot-diff tests) — that bloat is exactly what makes a suite slow, flaky, and ignored. More tests is not the goal; covering every behavior *once, well* is.
2. **Keep the bulk in the fast layers; every *deterministic* suite is on the one mandatory gate, and only the genuinely-slow/flaky/credentialed ones stay out.** `make check` is the single hard-fail gate ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)) — frontend + Rust + `knip` + the ts-rs drift guard + the **full Playwright browser suite** + `gate-integrity` + `docs-drift` (spec↔code enforcement — contracts/IA/data-model vs. the real commands/screens/settings, [ADR 0065](adr/0065-spec-code-drift-gates.md)) — and `.githooks/pre-commit` runs it before every commit. It must stay in the seconds-to-low-minutes range (the browser suite parallelizes to ~tens of seconds). Only suites disqualified from a per-commit hard gate stay periodic/manual, each for a stated reason: `coverage` (slow instrumented build), `mutants` (30 min–2 h), `bench` (machine-dependent), live provider / OS-keyring smokes (credentials/network/OS), packaging (OS/toolchain). This is the anti-rot contract: a deterministic suite that is *not* on the gate rots (the browser suite went 28-red for two sessions when it lived only in `check-epic` behind `-`-prefixed steps). Default CI/local checks stay deterministic and secret-free.

**Hermetic tests must not read the developer's shell.** A test asserting missing-credential behavior (provider skipped, "configure Settings" error) MUST call `providers::credentials::scrub_provider_env_fallbacks()` first — an exported dev-fallback var (e.g. `GEMINI_API_KEY` from direnv) otherwise gives the provider a credential and the test passes on one machine and fails on another (guardrail 2026-07-11: three `jobs::` tests). Same rule for any future env-fallback: hermetic tests scrub it at the top; scrubbing after the action under test is cleanup, not hermeticity. Safe under nextest's process-per-test model. **Sibling class — shared fixed network resources:** a test that binds a socket must probe a free port (`TcpListener::bind("127.0.0.1:0")` → read → drop) — never a fixed port and never settings-clamped `0` (the MCP clamp maps `0` to the 1024 floor, so every "ephemeral" test raced for the same port under parallel nextest; reddened a commit gate intermittently, 2026-07-12). The probe→bind window itself still races between socket tests, so every socket-binding test also joins the serialized `loopback-sockets` nextest group (`.config/nextest.toml`).

**Workflow tests near the timeout budget flake under gate load.** A Vitest test that walks multiple screens/sections through the full app render must carry an explicit `it(..., timeout)` budget with a rationale comment — under the gate's parallel transform load such tests exceed the 5s default while passing in isolation (three distinct tests flaked this way in one v0.50 closure day; card `b6b866f`). Give the *specific* test a deliberate budget; never absorb the class with global retries or a global timeout bump — a silently retried suite stops reporting real races.

**Await the *specific* thing you assert, not a proxy.** A screen fed by several independent async effects (e.g. Today/Pulse: autopilot runs, claims-to-verify, report season each load on their own effect) must, before asserting a category/row is present, `await` a marker of *that* data — not just the first row that happens to render. Awaiting one effect's marker (`findByRole("Details")` = autopilot) and then asserting a *sibling* effect's rows (verify) races the slower load: green in isolation, intermittently drops the un-awaited category under full-suite scheduling pressure (card `a91d260`: to-verify dropped; reproduced deterministically by delaying only `list_claims_to_verify` by a macrotask). Fix = await the asserted category (`await findCategoryRow(container, "verify")`), never a retry or timeout bump. Sibling class: a relative-time assertion that reads wall-clock `new Date()` on both the input *and* inside the formatter (`companyEventDueLabel`/`Class`) can straddle midnight — pin the clock with `vi.setSystemTime(NOW)` (the harness idiom) rather than trusting the offset to hold.

### Expensive-gate economics

Harvested at the v0.50.0 closure (ADR 0045) — each rule saves a burned ~15-minute gate run:

- **Pre-validate the commit subject.** The commit-msg hook runs *after* the pre-commit gate, so a bad subject wastes a green run: `scripts/release/validate-commit-message.sh --message "<subject>"` (Conventional Commits, **72-byte** subject cap — multibyte characters count as bytes).
- **Regenerate generated files, never hand-edit.** `docs/adr/INDEX.md` → `node scripts/check/docs-drift.mjs --write-adr-index`; docs-drift is the gate's *last* step, so a stale index fails a full run at the finish line. Always-loaded docs (`CLAUDE.md`, `engineering-workflow.md`, the session hook) carry ADR 0063 byte budgets checked by gate-integrity — trim or move content rather than grow them.
- **A runaway mutant must not abort the whole `mutants` run.** Some mutants loop and eat memory — that is exactly why the jail exists — but a systemd scope's default `OOMPolicy=stop` turns the jail's first OOM-kill into termination of the *entire* run (four consecutive runs died this way at the v0.50 closure; `journalctl --user` showed `Failed with result 'oom-kill'` on each, initially misread as an external process reaper). The jail therefore sets `-p OOMPolicy=continue`: the kernel reaps the runaway test process, cargo-mutants records that mutant as failed/timeout, and the run continues. Corollaries: diagnose a dead long run from `journalctl --user`/`dmesg` before blaming the environment, and when driving `make mutants` from an agent harness, launch it as an independent unit (`systemd-run --user --collect --unit=<name> -p WorkingDirectory=<repo> bash -lc 'make mutants > <log> 2>&1'`, login shell for the Nix PATH) so tool timeouts can't SIGTERM it mid-run, and avoid running it concurrently with a full gate on a small-RAM box.

**The layers (push coverage down to the cheapest layer that proves the behavior):**

- many **Rust unit/contract tests** — domain logic, command contracts, parsing, dedupe, migrations, provider mapping, jobs; the bulk of coverage, milliseconds each;
- **frontend component/workflow tests** (Vitest) for every UI state and workflow, not just critical ones;
- **test-sample-backed integration tests** for source adapters and migrations (parsing, dedupe keys, company matching, error handling, migration safety, data persistence);
- **browser UI smoke** (Playwright) for layout/scroll/overflow regressions Vitest/jsdom cannot catch — a hard-fail step of `make check`, not periodic;
- **accessibility guards (two layers):** `src/app/screens.a11y.test.tsx` runs jest-axe over **every** primary screen (Today, Inbox, Cockpit, Companies, Watchlists, Research, Notebooks, Sources, Events, Settings — zero exclusions) in jsdom, catching ARIA/role/structure regressions; the real-browser layer (`expectNoA11yViolations` in `tests/browser/helpers/harness.ts`, `@axe-core/playwright`, WCAG 2.0/2.1 A/AA tags) runs in the smoke-walk per destination and in journeys at key screens, across the full viewport matrix incl. the light-theme project — the only layer that catches real **color-contrast** in both palettes × modes. Best-practice rules (region / heading-order / landmark-*) stay out of scope (structural desktop-workspace choices, not conformance failures);
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

**Resource discipline (WSL OOM guardrail, 2026-07-10).** The WSL VM has ~15 GB RAM vs 24 cores; parallel `rustc` test builds saturate it and can kill the whole VM. Rules for every agent and script running tests OUTSIDE `make check` (whose orchestrator already stages suites): run **one heavy build/test invocation at a time** — never two cargo/nextest/vitest processes concurrently; bound compile parallelism with `CARGO_BUILD_JOBS=8` (or `-j 8`); scope `cargo nextest` to touched modules, never the whole suite ad hoc. A crashed VM can leave a **corrupted test binary** behind — a SIGSEGV in `nextest --list` right after a crash means delete the stale `target/debug/deps/<crate>-*` binary and relink, not a code bug.

**Delegation contracts name the consumers of a changed boundary (harvested 2026-07-10, ADR 0045).** When a delegated slice changes what a creation/normalization boundary produces (e.g. a create call starts folding a legacy label), scoped module tests miss the OTHER modules whose seeds or reads assumed the old shape — the collision surfaces only at the full gate. The slice contract must enumerate the boundary's consumers (`repoctx callers <fn>` / `rdeps`) as modules the agent runs tests for, and any test that needs the legacy shape seeds it via raw SQL like migration tests do, never through the now-normalizing public surface.

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

**Corollary — content before metadata verdicts (harvested 2026-07-10).** A diagnosis
that real data is mis-associated/contaminated must inspect the CONTENT of the accused
artifacts before any repair is designed. A verdict built from metadata alone (URLs,
slugs, filenames) once stood for two days accusing documents of belonging to other
issuers — opening the files disproved it in minutes (misleading CDN slugs; the
literal metadata-based repair would have deleted 36 legitimate rows).
The cost of the up-front measurement is an afternoon; the cost of skipping it is a
fully-built feature that has to be reverted.

### Fundamentals structured-extraction recall/precision harness

The structured-first extraction pipeline (`fundamentals::extraction::pipeline::run_pipeline`,
[ADR 0061](adr/0061-deterministic-fundamentals-data-gathering.md)) has an `#[ignore]` real-data
harness — `storage::tests::real_data_extraction::real_data_extraction_recall_precision` — that
measures recall/precision against a hand-labeled ground-truth set, per ADR 0061's guardrail
("a `#[ignore]` real-data harness measures recall/precision on the owner's filings before any
default flip"). It also gates the [ADR 0060](adr/0060-ai-capability-routing-and-openai-compatible-provider.md)
decision-3 document-tier default-model change. It complements
`storage::tests::autopilot::autopilot_real_data_validation`, which smoke-tests the pipeline
end-to-end (queue drain, terminal state) but does not measure accuracy.

**Setup:** refresh the DB copy per `private/realdata/README.md`, then work on a throwaway copy
so tests never mutate the master snapshot. **Never measure against the live DB file in place**
(guardrail, 2026-07-19): a `?immutable=1` read ignores the WAL, so with the app running you see
a stale pre-checkpoint state — two agents read wildly different row counts minutes apart this
way. Copy + `wal_checkpoint(TRUNCATE)` first; query only the copy.

**Env:**

- `BRAWLER_REAL_DB` — path to the (throwaway) real DB copy. Required; absent → the test skips
  with an `eprintln` (never fails CI).
- `BRAWLER_REAL_DATA_DIR` — the Tauri data dir holding the actual fetched report files (same
  convention as `autopilot_real_data_validation`). Without it, report bytes cannot be read and
  every document fails to resolve.
- `BRAWLER_GROUND_TRUTH` — path to the ground-truth JSON. Defaults to
  `private/realdata/ground_truth.json` (gitignored, hand-authored, never committed).

**Ground-truth format** — a JSON file with one entry per labeled report document:

```json
{
  "documents": [
    {
      "ticker": "CBF",
      "match": { "urlContains": "raport-q1-2026" },
      "periodEnd": "2026-03-31",
      "periodType": "Q1",
      "fiscalYear": 2026,
      "facts": { "revenue": "245253000", "net_profit": "-1200000" }
    }
  ]
}
```

`match` takes `urlContains` and/or `contentHash` (at least one, matched against the resolved
company's `report_documents` rows). `facts` maps `metric_key` → expected value as a decimal
string in signed base units. A match uses the pipeline's own `Tolerance` (0.5% relative / 1
base-unit absolute).

**Run:**

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml real_data_extraction -- --ignored --nocapture
```

Prints a per-document line (tier, acceptance, recall, precision, unlabeled-emitted count), a
per-tier (esef/pdf/html_aggregator) rollup, and an overall summary.

**Policy:** the harness currently asserts sanity only (ground truth resolves; overall recall >
0) — no precision/recall floor yet. A quality threshold is a deliberate follow-up gate before
relying on this pipeline by default or before the ADR 0060 decision-3 default-model flip, per
the real-data-validation-precedes-implementation guardrail above.

### Corpus-wide deterministic sweep — the official coverage number (v0.59 A4)

The labeled harness above measures precision where hand labels exist; it cannot say **how much
of the owner's real filing corpus the deterministic-only pipeline actually reads**. That is what
`storage::tests::real_data_extraction::deterministic_pipeline_real_data_sweep` measures. Since
[ADR 0084](adr/0084-retire-in-app-ai-layer.md) retired the AI tier, this is the *official*
fundamentals coverage number. The ratification question it once gated — whether cover-note facts
graduate from `auto_unreviewed` — is now closed: facts are review-free
([ADR 0086](adr/0086-aggregator-primary-fundamentals.md) dec. 5), landing `confirmed` regardless
of tier.

It drives the **real job-layer path** (`jobs::structured_extraction::run_structured_extraction`),
not just the pure pipeline, because the typed `reason_code` vocabulary and the aggregator
fallback only exist there. It therefore **writes** (facts, provenance, outcome rows) and refuses
to start against `brawler.sqlite3` or any path under `/mnt/d/` — point it at a throwaway copy.

| Reports | Meaning |
| --- | --- |
| Eligible by `doc_kind`, with the no-period-derivable share | Documents stored but not routable to a reporting period. Split by kind because a `governance` document legitimately has none and a `periodic_ssf` that has none is a real gap. |
| Attempts by route, `read_rate` | ESEF vs non-ESEF route (structured xHTML / positional — the PDF fact arm is retired, ADR 0086; PDF documents spawn no attempt), and the share that yielded ≥1 tracked fact. Counts produced **and** re-observed facts — on the owner's DB most slots are already filled, so "newly created" would read as 0. |
| Document-level recall | Attempted documents yielding ≥1 tracked fact, plus facts per reading document. The recall denominator that *is* available (see below). |
| Resolved tier / validation-gate verdict | Which tier read the document; `accepted` / `accepted_unreviewed` / `flagged` / `empty`. |
| Flagged-gap breakdown by `reason_code` | `emitted`, `validation_failed`, `structure_drift`, `witness_disagreement`, `no_deterministic_tier`, `document_unreadable` (plus legacy-only `witness_fallback` rows, retained readable — [ADR 0086](adr/0086-aggregator-primary-fundamentals.md)). |
| Facts by provenance tier, **filing- vs aggregator-sourced** | Required by the [ADR 0085](adr/0085-biznesradar-fundamentals-witness.md) amendment: a coverage number that silently counts third-party fallback values overstates how well filings are read. |
| Ground-truth-backed precision | Only the hand-labeled espi-wdf cover-note corpus (347 facts). Everything else is printed as a **production count with no precision claim**. |

**Recall caveat, by design:** recall "against the company's expected primary-KPI profile" is
**not measurable** on the owner's database — `expected_primary_metric_keys` derives that profile
from `kpi_relevance` rows ranked `primary`, and the table is empty. The harness prints
`NOT MEASURABLE` with the reason rather than substituting an invented denominator, and falls back
to document-level recall. The same emptiness means the ADR 0061 dec. 4d completeness downgrade is
inert in production — a finding, not a harness defect.

**Env:** `BRAWLER_REAL_DB` + `BRAWLER_REAL_DATA_DIR` as above (absent → skips with a printed
message, never fails CI), plus:

- `BRAWLER_A4_LIMIT` — documents to attempt (default `250`; `0` = uncapped). Runtime is
  ~1.7 s/document, dominated by PDF text extraction.
- `BRAWLER_A4_LIVE_WITNESS=1` — install the real BiznesRadar fetcher. **Off by default**, so a
  default run is offline and repeatable and the aggregator-sourced count is 0 *by construction*
  (the harness says so explicitly rather than letting 0 read as a measurement).

```bash
cp private/realdata/brawler.sqlite3 private/realdata/a4_worktest.sqlite3
BRAWLER_REAL_DB=../private/realdata/a4_worktest.sqlite3 \
  BRAWLER_REAL_DATA_DIR=/mnt/d/Brawler/Builds/latest/data \
  BRAWLER_A4_LIMIT=400 \
  rtk cargo nextest run -p brawler deterministic_pipeline_real_data_sweep \
    --run-ignored all --no-capture
```

**Policy:** asserts harness sanity only (something was attempted) plus one contract — every
`reason_code` reaching the outcome row is in the typed vocabulary ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)
§6), since an untyped reason makes every flagged-gap number above uninterpretable. It pins **no
coverage floor on purpose**: the deliverable is the real number, and a harness asserting the
number it hopes for is worse than no harness.

### Ownership shareholders-table harness (v0.56 T1)

The ownership parser (`fundamentals::ownership::parse_shareholders`, ADR 0072) has its own
real-data recall/precision net, `storage::tests::real_data_ownership` — same **inert-in-CI**
gating as above (`BRAWLER_REAL_DB` + `BRAWLER_REAL_DATA_DIR`). It runs `extract_report` →
`parse_shareholders` per labeled document and prints **row recall** (matched labeled rows /
labeled rows) and **row precision** (matched / emitted holder rows), keyed by `SourceFormat`
(`xhtml` vs `pdf`), plus per-document missed/spurious holders. Sanity assertion only (some
document resolved) — the metrics decide the deterministic-vs-AI tier split before T2/T3.

`BRAWLER_OWNERSHIP_GROUND_TRUTH` overrides the path (default
`private/realdata/ownership_ground_truth.json`, gitignored, hand-authored). Shape — one entry
per labeled document, holder rows with **capital % and votes % separate** (either may be null
for a one-sided disclosure):

```json
{
  "documents": [
    {
      "ticker": "CBF",
      "match": { "urlContains": "raport_roczny", "contentHash": null },
      "asOf": "2025-12-31",
      "rows": [ { "holder": "Jan Kowalski", "capitalPct": "12.34", "votesPct": "15.00" } ]
    }
  ]
}
```

`match` takes `urlContains` and/or `contentHash` (at least one). A labeled row matches an emitted
row on normalized holder name (case-fold, whitespace-collapse, minimal legal-suffix strip) with
0.01 absolute tolerance on each non-null percentage side.

**Labeling rules (harvested from two real T1 labeling errors, 2026-07-16):** label from the FULL
section dump of the source document, never a truncated grep window (a cut window dropped a real
KRU holder row); documents can carry the section twice — an infographic/image first and a real
table later — so scan the whole document before declaring a negative case (MDV). A
parser-vs-label disagreement is investigated on the source document before either side changes;
the ground truth stays orchestrator/owner-owned — extraction agents never edit it.

```bash
cargo test -p brawler --lib real_data_ownership_recall_precision -- --ignored --nocapture
```

### Real-data extraction corpus (structural regression)

A second, coarser real-data net pins the **extraction outcome** of every document in a real
company's filing set, so a format-handling regression (e.g. ESEF `.xbri` packages silently
misclassified) is caught without hand-labeling fact values. Corpus layout: `private/realdata/
t7-cbf/` (gitignored) holds `brawler.sqlite3` (a full DB copy) plus `report_documents/**` (the
fetched files at their DB-recorded `local_path`). The pinned baseline
`src-tauri/src/storage/tests/t7_cbf_corpus_expectations.json` is **committed** — it carries only
document ids/titles + outcome class + fact count + period, never report content.

```bash
make realdata-extraction-check                              # compare vs baseline
BRAWLER_UPDATE_EXPECTATIONS=1 make realdata-extraction-check  # deliberately re-pin
```

A **regression fails** (fewer facts, an accepted doc now flagged/empty, a period drift, or a
pinned doc vanished); an **improvement is a soft report** (new/more facts) that prints the
refreshed table and passes — re-pin deliberately after review, same philosophy as the Playwright
visual baseline. Same `BRAWLER_REAL_DB` + `BRAWLER_REAL_DATA_DIR` gating (skips cleanly when absent).

The same target also runs the T7-F **double-extraction idempotency anchor**
(`t7_cbf_double_extraction_is_idempotent_on_the_real_corpus`): it drives the full write path twice
over the real `.xbri` filing and asserts the second run creates nothing, reports its re-observed
slots as skipped, and leaves `financial_facts` unchanged. It **writes to the corpus DB** — the
corpus copy is throwaway by contract (refresh it from the live DB when in doubt).

### Ground-truth metrics ratchet (recall/precision, G-3)

The value-level companion to the structural corpus net ([ADR 0077](adr/0077-trusted-extraction-foundations.md)
T0.1): `storage::tests::extraction_metrics::extraction_metrics_recall_precision_ratchet` grades
the deterministic pipeline's emitted fact **values** against hand-labeled ground truth.

- **Ground truth** — one JSON per (document, period) in `private/realdata/t7-cbf/ground_truth/`
  (gitignored, never committed): `{ document_file, company, fiscal_year, period_type,
  facts: [{ metric_key, value, unit, statement, page }] }`. `metric_key`s come from the seeded
  KPI catalog only; `value` is a decimal string in signed base units (tys.-PLN statements are
  normalized ×1000). Labeling is **double-pass**: the agent proposes values read off the
  statement pages, the owner verifies; anything not certain carries `"uncertain": true` (+
  `why_uncertain`) instead of a silent guess.
- **Metrics** — per document and overall: *recall* = labeled facts emitted with a matching value;
  *precision* = emitted facts for labeled metrics that match the label (emitted facts nobody
  labeled are excluded and reported as `unlabeled_emitted`). Values compare as numbers under the
  pipeline's own `Tolerance` (0.5% relative / 1 base-unit absolute), never as strings. A
  per-document `metric_key | labeled | extracted | match` table prints for eyeballing.
- **Ratchet rule** — the floors (`RECALL_FLOOR`/`PRECISION_FLOOR` in `extraction_metrics.rs`) are
  pinned at the measured baseline minus 0.02 slack (2026-07-08, after the owner's pass-2 label
  decisions: recall 12/37 = 0.32, precision 12/12 = 1.00 → floors 0.30 / 0.98). They move only
  **deliberately** — raise after a
  verified improvement lands; never lower to absorb a regression.

```bash
make realdata-extraction-metrics   # same REALDATA_DB/REALDATA_DIR guards; skips without corpus/ground_truth
```

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

The document taxonomy (ADR 0077 §1) uses a stronger form of the same idea: a
**committed labeled corpus** `src-tauri/testdata/doc_titles_labeled.json` of
real GPW title shapes with expected `doc_kind` labels (guardrail **G-2**,
`fundamentals::extraction::classify::tests::contract_corpus_holds`). Unlike a
regenerable snapshot, labels are hand-assigned — a failing row is resolved by
a conscious relabel, never `insta accept`. Extend it with every newly observed
misclassified real title.

The **EspiCoverNote parser** (ADR 0061 tier 2a) follows the same property + golden
pattern: proptest invariants (no panic, determinism, unique metric/basis identity)
plus a golden snapshot over a synthetic body authored in the test itself, never
copied from `private/` real data. Its acceptance bar is a separate `#[ignore]`
real-data test against the hand-labeled spike corpus (347 facts; recall and
precision both pinned at 347/347, zero false values), skipping cleanly when
`private/` is absent. The retired tier-4 OCR parser's tests were removed with the
layer ([ADR 0084](adr/0084-retire-in-app-ai-layer.md)).

### Scale & performance gates

Scale correctness is split so the **hard gate stays deterministic** and timing
never flakes CI:

- **Behavioral scale gates (in `make check`).** Deterministic assertions that a
  hot path is **offloaded** (a meaningful-work `#[tauri::command]` is `async` +
  `spawn_blocking`, per the CLAUDE.md UI-thread rule) and **algorithmically
  bounded** — it scans the persisted derived index, not the whole corpus, and is
  `O(rows)` not `O(rows²)`. Asserted via structure and via instrumented
  counters / row-count invariants over a volume dataset, **not** wall-clock. The
  rule was harvested from the `v0.45.0` `find_similar` UI-freeze regression
  (that feature is retired, [ADR 0080](adr/0080-retire-embedding-model.md); the
  rule stands for every hot path).
- **Periodic `criterion` benches with a relative ratchet (never a hard gate).**
  `make bench` runs micro-benchmarks on the hot kernels (feed parse,
  expression eval); a relative ratchet (mirroring `coverage-ratchet.mjs`)
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

### Mapping guardrails — metric semantics under enforcement (2026-07-22)

Finding-1 class (a source row filed under the wrong metric / silently dropped) is enforced by
three gates:

- **G1 — emittable-keys catalog contract** (`every_emittable_metric_key_has_a_canonical_definition`,
  `storage/tests/migration_safety.rs`): scans the mapper SOURCES (shared dictionary, WDF row
  mapper, ESEF concept map) at test time; every emitted `metric_key` must resolve to a seeded
  canonical `kpi_definitions` row, else facts silently drop at `NoDefinition`. A new mapper key
  reddens the gate until its seed migration lands (first catch: 16 unseeded WDF keys → 0112).
- **G2 — source-vocabulary contract** (`source_vocabulary_contract_golden`, `html/tests.rs`): a
  golden snapshot pinning every statement row of every checked-in real BiznesRadar page as
  `(data-field, label, mapping outcome)`. A BR vocabulary change OR a dictionary prefix silently
  capturing another row flips a line in the diff and forces a human decision — an unmapped row is
  a REVIEWED skip, never an accident (first catch: the discontinued-operations row shadowing group
  `net_profit`).
- **G3 — mapping-suspect threshold** (`aggregator_fundamentals_pull`): the same metric disagreeing
  with issuer/manual-held slots at ≥5 distinct companies in one pull run surfaces as
  `mappingSuspects` on the pull summary + a `mapping_suspect` diagnostic + a warn log — the
  systematic signature of a mismapped row, distinct from scattered per-company noise.

### Mock-runtime fidelity — the dual-execution contract

**Scope exemption (owner decision, 2026-07-22):** commands with **no frontend
caller and no mock-runtime handler** — the documented headless-only network/
acquisition drivers `run_aggregator_fundamentals_pull` and `rebuild_fundamentals`
(contracts.md) — are exempt from corpus membership: the corpus verifies the TS
mock against the backend, and these commands have no mock half to keep faithful
(same reason `run_structured_extraction` is absent). The exemption expires for a
command the moment it gains a frontend/mock caller — add it to the corpus in the
same change.

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
fresh `open_in_memory_database`); it loads the corpus via `BRAWLER_FIDELITY_CORPUS`
(set by `src-tauri/build.rs` from `BRAWLER_SCENARIOS_DIR` — `make mutants`
exports that dir as an absolute path since `cargo-mutants`' scratch copy
excludes anything above the workspace). The same rule covers every shared
fixture under `src/test/scenarios/`: Rust tests embed one via
`include_str!(concat!(env!("BRAWLER_SCENARIOS_DIR"), "/<file>"))`, never a
literal `../../../` path — a cross-tree literal compiles everywhere except the
mutants sandbox (#110); the `source_tree_guards` unit test reddens on any such
literal.
Journeys use seed-free root entities and
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
is covered by the unified `upsert_feed_item`, exercised by every feed-item
adapter's ingest test. Adding an ingest path keeps it on the spine
(`upsert_feed_item` + `record_source_outcome`); a new identity/matching
transform in `storage::feed_matching` (the normalization SSOT) ships with the
ADR 0049 invariant treatment. (The story-key stage and its golden/proptest
suites were removed with the write-only story-key path, [ADR 0080](adr/0080-retire-embedding-model.md).)

### Mutation testing scope

`make mutants` (`cargo-mutants`) is the strong signal that the property and
golden tests actually *kill* defects — line coverage does not prove this. Its
`-f` scope **follows the highest-risk transform logic**: it starts at
`src/fundamentals/expr/**` + `src/storage/migrations.rs` and is **extended to the
dedup/matching/normalization modules** as they land, rather than rotting at the
original globs. It stays **periodic** (closure-cadence, in the `make check-epic`
neighborhood — not the per-change gate). Policy: [ADR 0049](adr/0049-test-architecture-v2-data-transform-correctness.md).

The target is **resource-capped by default** (`nice -19`, `CARGO_BUILD_JOBS=2`,
`test-threads=2` via the nextest `mutants` profile, one mutant at a time,
and a hard `systemd-run` memory jail `MUTANTS_MEMORY_MAX=11G`) — uncapped sweeps
OOM-froze the whole WSL VM twice (2026-07-03; memory, not CPU, is the killer —
the jail OOMs only the sweep scope). Raise via `MUTANTS_BUILD_JOBS`/`MUTANTS_MEMORY_MAX`
on a dedicated box, or split a sweep across quiet moments with
`make mutants MUTANTS_SHARD=k/n`.

Sweeps also run off-box via the manual `mutants.yml` GitHub Actions workflow
(`gh workflow run mutants.yml [-f shard=1/8]`) on standard `ubuntu-latest`,
`MUTANTS_JAIL=off` since a hosted runner has no user systemd manager to jail —
`make mutants` (jailed by default) stays the documented local equivalent for a
dedicated box.

## Generated API types (Rust → TypeScript)

The TypeScript DTOs that cross the Tauri IPC boundary are **generated from the Rust source** with `ts-rs`, so a Rust struct and its TS shape cannot silently drift (ADR 0048). The Rust DTO carries `#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]` + `#[ts(export, export_to = "../../src/api/generated/")]` (ts-rs honors the existing `#[serde(rename_all = "camelCase")]` via serde-compat); `make types` emits `src/api/generated/`, and the hand-written `src/api/*Types.ts` module re-exports the generated types so consumers keep a stable import path. `make types-check` regenerates and fails if regeneration changes the working-tree bindings (a before/after hash self-consistency check — independent of git staging state, so it works both mid-work and in the pre-commit gate). `ts-rs` is behind the off-by-default `ts-export` feature, so it never ships in the binary; `src/api/generated/` is lint-excluded and **knip-ignored** (`ignore` in `knip.json`), because it is a generated contract mirror governed by `types-check`, not authored source — knip's unused-file check would flag generated leaf DTOs that nothing imports, and deleting them is invalid (regenerate, don't hand-edit). Authored dead-code detection stays strict everywhere else.

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

A small Playwright browser-smoke layer catches UI/layout regressions Vitest/jsdom cannot (overflow, scroll-ownership, clipping, fixed chrome). It targets the Vite preview app in Chromium with deterministic mock data — it does **not** read live sources or the user's local database. Policy: [ADR 0021](adr/0021-browser-ui-regression-testing.md). Under [ADR 0048](adr/0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md) it was extended to **broad clickable coverage of all primary screens** on a stateful, per-test-isolated mock runtime; the ADR 0048 Decision 6 promotion is now **complete** ([ADR 0062](adr/0062-mandatory-test-gate-and-test-driven-loop.md)): the **full suite is a hard-fail step of `make check`** and runs before every commit (it parallelizes to ~tens of seconds, keeping the gate in the seconds-to-low-minutes range). It is **no longer opt-in** — a browser-suite failure blocks the commit.

Setup and run:

- `make ui-smoke-install` — first-time Chromium download into the local Playwright cache.
- `make ui-smoke` — run the suite. (`npm run test:browser:install` / `npm run test:browser` are the direct equivalents.)
- Run a subset: `npx playwright test journeys smoke-walk`.
- **Build-freshness guard** (`tests/browser/global-setup.ts`, bug 2059fd8): the run aborts up front if a **reused** dev server (`reuseExistingServer` is on locally) is serving a stale build — one from another worktree/branch or from before your latest edit — which would false-green visual/density baselines. The config stamps the server with the newest `src/` mtime and `global-setup` refuses to proceed on a mismatch. Fix: stop the stale dev server on port 4321 so a fresh one starts (a normal `make ui-smoke` with no pre-existing server is unaffected).

Use it for repeated layout risks: fixed app chrome + no global scrollbar; independently scrollable panels (Companies, Watchlists, Notebooks, Inbox, Events, Sources); dense row/category sizing; and **viewport regressions across the matrix in `playwright.config.ts`** — compact (1366×768), wide (1920×1080), and the tall/narrow quarter-ultrawide at 100% (1280×1440) and 125% (1024×1152) scaling, per the UI scaling requirement in [CLAUDE.md](../CLAUDE.md).

How it's wired (assertion-driven, not screenshot-only):

- a shared harness (`tests/browser/helpers/harness.ts`) gives an auto console-error gate (any `console.error`/uncaught error fails the test) and reusable invariants (`expectNoPageOverflow`, a deep `expectNoHorizontalOverflow` scan, `expectInternalScroll`);
- `tests/browser/journeys.spec.ts` asserts full flows end to end; `tests/browser/smoke-walk.spec.ts` walks every primary screen asserting no page-level horizontal overflow and deep-scans the detail rail where `overflow:hidden` hides the symptom;
- mock data comes from the canonical factory (`src/test/scenarios/`): `browserSmokeRuntime.ts` builds `createMockRuntime("rich")`, injects the browser-specific seed into its store, and routes Tauri `invoke` through the one runtime (only Tauri plugin commands are handled locally). The runtime **rejects** an unknown command (the "add a case to `runtime.ts`" signal); a new screen command is added once, in the shared router, not per layer. `?locale=pl` previews the app in Polish for screenshot specs; the typed `window.__brawlerMock` bridge (ADR 0081 Q2, below) re-seeds — optionally with overlays — and drives controlled async between interactions.

Two harvested rules (ADR 0045), both from the `v0.47.0` report-diff panel:

- **A no-horizontal-scroll check must assert the inner scroll containers, not just the document.** `document.documentElement.scrollWidth` is **0** when the overflow lives in an inner `overflow: auto` element (which shows *its own* scrollbar) — so a document-level check passes while the user sees a scrollbar. Assert `scrollWidth <= clientWidth + 1` on the actual scroll container(s) in the subtree (e.g. `.company-list`) and on the offending panel, across the narrow viewports. The report-diff overflow was invisible to the document check for several rounds because the scroll lived on `.company-list`.
- **A new IPC command that drives a primary-screen panel ships with (a) its case in the shared runtime router and (b) a narrow-window layout assertion for that panel.** Vitest mocks the api module, so a panel can be green in Vitest yet untested for layout (the Playwright runtime didn't know the command, so the panel never rendered with data under `make check`/`check-epic`). Add the router case + a `tests/browser/*-layout.spec.ts` overflow assertion in the same change as the command. Example: `report-diff-layout.spec.ts`.

**The harness also renders any screen for visual review** — drive a throwaway spec to the screen and `await page.screenshot(...)`. "No GUI in WSL" is not a reason to skip looking at a UI change (see [Engineering Workflow → Definition of Done](engineering-workflow.md#definition-of-done-the-handover-gate)).

Evidence policy: DOM/layout assertions are the pass/fail signal; screenshots and traces are retained only on failure; visual snapshot comparison is deferred until a stable use case justifies it.

Do **not** use this layer for: live external source/API testing; real Tauri file dialogs/keychain/taskbar/packaging/WebView2; broad end-to-end coverage of every workflow; or screenshot comparison as pass/fail evidence — those are the live/packaging smoke and native Windows paths.

## Frontend test responsibilities

Every behavior lives at the **cheapest authoritative layer** — the layer that can actually observe it, run fastest, and fail loudly ([ADR 0081](adr/0081-ux-quality-loop-v2.md) Q8). The current audit ledger is [ux-quality-loop-v2-test-inventory.md](plans/ux-quality-loop-v2-test-inventory.md).

| Responsibility | Layer | Why here |
| --- | --- | --- |
| Controlled component state, forms, validation, error branches, async transitions within one screen | **Vitest** (`src/**/*.test.tsx`) | jsdom is fastest; the assertion is local to the component |
| Pure logic (parsers, descriptor round-trips, preference resolvers) | **Vitest** unit | no render needed |
| Integration seams — last-intent-wins, response ordering, stale-response suppression | **Vitest `renderHook`** on the controller | the seam has no cheaper surface than the hook; the full app cannot hold two async responses cleanly |
| Structural a11y (axe) per screen | **Vitest** (`screens.a11y.test.tsx`) | unique fast structural layer; real-browser contrast is a deferred `axe-playwright` follow-up |
| Cross-screen journeys — act in screen A, land in screen B | **Playwright** (`tests/browser/journeys/`, `smoke-walk.spec.ts`) | only a real browser exercises navigation + layout faithfully; jsdom cannot mount Dockview panel bodies after a rebuild |
| Real layout / overflow / density / Dockview panel-body render | **Playwright** viewport matrix | jsdom has no layout engine |
| Real Tauri backend / real DB / WebView2 / native desktop | **live drive** (`tests/live/`) + native Windows | only the real runtime is authoritative |

**Rules.** (1) A cross-screen case is **moved** to Playwright only when the browser is genuinely the cheaper authoritative layer *and* churn is justified by **measured flake**, never speculatively (Q8 STOP-AND-ASK). (2) Playwright coverage does **not** count toward Vitest V8 line coverage — retain/extract equivalent component coverage before deleting any Vitest assertion, and **never lower `coverage-baseline.json`**. (3) A multi-slice task names its layer split in planning via the [experience-contract template](plans/EXPERIENCE-CONTRACT-TEMPLATE.md) § 12, not at the gate.

## User-journey E2E and step budgets (ADR 0074)

Journeys — the cross-screen tasks a user actually comes to do — are specced in [ux-journeys.md](ux-journeys.md) and enforced by dedicated Playwright specs (one per journey, `tests/browser/journeys/j1…j7-*.spec.ts`, tagged `@journey`, on the same mock runtime):

- **One spec per journey**, asserting the full cross-screen path (trigger → steps → done-well criteria), not per-screen features, keeping `expectNoPageOverflow` + `expectNoA11yViolations` at key screens. The v0.44–0.49 E2E backfill (autopilot trust ladder, report season, quality frameworks, claims, transcripts/research) lands in this form — the coverage gap and the journey net are the same work.
- **Interaction step budgets as assertions**: each spec drives its path through the explicit `journey(page, id)` wrapper (`helpers/harness.ts`, implemented in `helpers/journey.ts`) — one wrapper call (`click`/`fill`/`press`/`selectOption`) = one counted user interaction, navigation included. `assertBudget()` reads `tests/browser/journeys/budgets.json` and reddens when a metric exceeds its floor; each floor is the first measured count +1 (interactions never above the `ux-journeys.md` ceiling), ratcheted **down** — like coverage — when a journey gets measurably shorter, so a UX regression reddens the gate. The budget is now **schema v2** with four friction metrics per journey — see the journey-metrics subsection below.
- Closure hook: [Definition of Done §I](engineering-workflow.md#definition-of-done-the-handover-gate) requires every user-facing capability to name its journey (or be declared a utility) and `budgets.json` to be green.

## UX quality loop v2 — overlays, journey metrics, contact sheet (ADR 0081, pilot)

Canonical home for the loop's test mechanics ([ADR 0081](adr/0081-ux-quality-loop-v2.md)): composable hostile/dense/partial/stale/conflicting/mixed-locale **scenario overlays** and **controlled-async** controls on the one mock runtime (plan Q2); **journey metrics** beyond click counts — interactions, screen transitions, modal opens, context loss (plan Q3); the frontend **test-layer ownership** audit (plan Q8); and a local **contact-sheet** review over the existing visual scenarios (plan Q5). Everything stays off `make check` except the deterministic behavior/layout/affordance contracts; contact sheets and timing reports are review evidence, and clarity/usefulness/trust remain human verdicts. Mechanics land with each pilot task; this section is the home they attach to.

### Minimal failure-injection seam (epic `0db7a7a`, Radicle `5be14c9`)

`src/test/scenarios/runtime.ts`'s `MockRuntime.failNext(command, error)` queues a **one-shot** rejection for the NEXT invocation of `command`: instead of running its handler, `invoke` settles with `error` (a plain `{code, message}` object, the ADR 0070 envelope) UNCHANGED — the same shape a real typed backend rejection uses, so `isCommandError`/`CommandInvocationError` on the frontend fire identically to a real failure. `reset()` clears every queued failure. Persistent failures are the chaos seam below (epic #40 S1); poor-state seeds and the real-DB evaluator remain separate slices. Q2's `controls.reject(id, error)` (below) delegates to this seam for a `before-handler` hold rather than reproducing the envelope mapping.

### Chaos seam — persistent failures (epic #40 S1, ADR 0091)

`failNext` above is one-shot; `MockRuntime.chaos(command, error)` is its persistent twin: while the rule is installed, EVERY invocation of `command` settles with the same untouched ADR 0070 envelope — the "this read is broken for the whole session" state a one-shot queue cannot express. A queued `failNext` for that command still wins first and is consumed; `clearChaos()` drops every rule, and `reset()` clears both seams.

In the browser the rule must exist **before** the app's once-at-bootstrap reads. Two equivalent entry points, both installed pre-boot: the URL param `?chaos=<command>[:<code>]` (code defaults to `internal`, message names the command; an unknown code falls back to `internal`), and `primeChaos(page, rules)` from `tests/browser/helpers/mockRuntime.ts` when the spec wants its own message or several rules. `failNext`/`chaos`/`clearChaos` are also on the `window.__brawlerMock` bridge for mid-test use.

```ts
await openApp(page, "/?chaos=list_companies"); // or: await primeChaos(page, [{ command: "list_companies", error: { code: "network", message: "…" } }])
await expect(page.getByText(/Companies command failed/)).toContainText("list_companies");
```

The assertion that matters is that the failure is **NAMED** on screen — a broken read must never degrade into an indistinguishable empty state (`tests/browser/chaos-seam.spec.ts`). `primeMockScenario(page, spec)` lives beside `primeChaos` in the same helper (scenario seeding is applied before chaos rules regardless of call order, since `reset()` clears chaos).

### Scenario overlays and controlled async (plan Q2, Radicle `a9992e2`)

Composable, deterministic adversarial data and async-ordering controls on the ONE mock runtime — no second router, no per-combination mega-scenario.

**Overlays** (`src/test/scenarios/overlays.ts`) layer onto a base scenario (`empty | minimal | rich`) via `buildScenario({ base, overlays })`, still accepting the bare `ScenarioName` for pre-Q2 callers:

| Overlay | Adds |
| --- | --- |
| `hostile-content` | unbreakable long URL/filename, long issuer/title/body, Polish diacritics |
| `dense-history` | ~250 feed rows — ONLY when explicitly selected |
| `partial-data` | a financial period with NO matching financial fact (the missing-relevant-read state) |
| `stale-processing` | an old visible research-evidence result plus a `running` brief job for the same scope |
| `conflicting-statuses` | a source adapter reporting `attention` while its latest ingestion result shows a clean run — two independent reads deliberately disagree |
| `mixed-locale` | realistic Polish + English feed items — real source content, never a planted UI-translation literal |
| `failed-autopilot-run` | a coherent FAILED overnight run — `status: "failed"` + a concrete `lastError` + the `notable` severity the backend derives, plus its `autopilot_run_completed` event and the rule it fired from |
| `degraded-sources` | source adapters whose last fetch failed — `lastError`/`lastErrorAt` + `healthStatus: "attention"` + the failed-detail counters and warning text |

The last two are the **poor-state seeds** (epic #40 S2, [ADR 0091](adr/0091-failure-path-and-real-state-testing.md)): they apply LAST in the fixed order (the attention-set-owning overlays would otherwise drop the seeded event) and back the flow walks in `tests/browser/poor-state-*.spec.ts`, which walk Today / Sources / the company cockpit on poor state and assert the failure is **named** on screen. `SCENARIO_OVERLAY_NAMES` (exported from `overlays.ts`) is the one enumeration of the overlay set — an unknown name throws instead of being silently skipped.

Each overlay reassigns the collections it touches (never mutates an entity in place — the same store-mutation contract handlers follow) and uses a fixed, overlay-dedicated `CompanySpec` so simultaneous overlays never collide. Application order is fixed internally, independent of the order the caller lists overlay names, and a repeated name is idempotent. `applyScenarioOverlays` is pure.

**Controlled async** (`src/test/scenarios/controlledAsync.ts`) wraps `MockRuntime.invoke` exactly ONCE (never inside individual handlers) with `hold`/`pending`/`release`/`reject`/`releaseAll`:

- `hold({command, args?, phase?})` registers interest in the NEXT matching invocation and returns the id it will be held under.
- `before-handler` (the default) holds an invocation BEFORE its handler runs — legal for reads AND mutations, since nothing has happened yet.
- `after-handler` holds delivery of an ALREADY-computed response — legal ONLY for read handlers (a mutating handler has already changed the store by then).
- `reject(id, error)`: a `CommandError` on a `before-handler` hold delegates through `failNext` (above); a bare `Error`, or any `after-handler` reject, settles directly — nothing to delegate to since the handler already ran or the caller opted out of the typed envelope.
- `reset()`/`releaseAll()` clear every held invocation so no promise leaks across tests.

The typed test-only `window.__brawlerMock` bridge (`src/test/browserSmokeRuntime.ts`) exposes `reset(spec)`, `hold`, `pending`, `release`, `reject`, `releaseAll` (plus the chaos-seam trio `failNext`/`chaos`/`clearChaos` above) to Playwright via `tests/browser/helpers/mockRuntime.ts`. **Setup order is always base scenario → `seedBrowserStore` (this file's browser-specific projection) → overlays**, on both initial install and every `reset()` — overlays run LAST so the projection can never silently clobber hostile/dense/partial/stale/conflicting/mixed-locale data.

`tests/browser/research-controlled-async.spec.ts` is the canonical controlled-async proof: it holds two `list_research_evidence` responses for different company intents, releases newest-then-oldest, and asserts the OLDER response cannot replace the newer state — `useResearchController`'s `requestVersionRef` "last-intent-wins" seam.

**Which UI changes need an adversarial pass:** any screen/panel reading a collection that can legitimately be long, foreign-scripted, partially available, or asynchronously superseded (feed/evidence lists, KPI/claim labels, source-health cards, cross-panel status) should be exercised with the relevant overlay(s) and/or a controlled-async race in its browser spec — not just the happy-path `rich` scenario. A purely mechanical/internal change (styling, refactor with no new read/async path) does not.

### Journey metrics beyond clicks (plan Q3)

The flat click counter is upgraded to deterministic friction metrics. The pure accounting core is `src/test/journeyMetrics.ts` (Playwright-free, unit-tested by `journeyMetrics.test.ts`); the Playwright adapter is `tests/browser/helpers/journey.ts` (re-exported from `helpers/harness.ts`). One accounting implementation — the browser wrapper delegates all counting/evaluation to the core, so there is nothing to drift.

- **Four hard/ratcheted metrics** per journey in `budgets.json` **schema v2** (`schemaVersion: 2`, `journeys.{J1…J7}.{interactions, screenTransitions, modalOpens, contextLosses, byProject}`): `interactions` (one per wrapper call), `screenTransitions` (a `markScreen(name)` whose name differs from the last — the first mark and repeats are free), `modalOpens` (a `markModal(name)`, ignoring a dialog already open at the prior marker; a screen transition closes the modal so the same name can legitimately reopen), and `contextLosses` (a `preserveContext(key)` whose non-null key changes unexpectedly; `preserveContext(null)` is a deliberate reset that never counts). Every floor is first-measured +1 (interactions also bounded by the `ux-journeys.md` ceiling); a breach names journey, metric, actual, limit, project, viewport, and the event trace.
- **`byProject`** overrides a metric floor for a named Playwright project **only where a real narrow-pane flow genuinely needs MORE actions** on that project (with a recorded reason); it must not hide a common regression. A project that needs FEWER actions (e.g. a taller pane skipping a density-tier disclosure) is simply absorbed by the shared floor — no entry needed.
- **Advisory, never a hard gate** (ADR 0081 "no wall-clock UX hard gate"): `clickPrimary(surface, action)` records whether the primary action was inside its surface's scrollport before Playwright auto-scroll, and `expectFeedback(locator)` records feedback-visible elapsed time — both attached to the Playwright result as annotations, not asserted on milliseconds.
- **Semantic observation points** (not test-only business state): `data-app-section` on the main workspace (`AppShell`), `data-company-id` on the cockpit root (`CockpitScreen`), and `aria-current="page"` on the active nav button. A user journey takes the **real disclosure path** at the current project viewport — density tests may force a pane, journeys may not.

### UX contact sheet (plan Q5, Radicle `81313f0`)

A local, self-contained HTML grid assembled from the **existing** visual scenarios
(`tests/browser/visual/*.spec.ts`, `chromium-visual`/`chromium-visual-light` projects) so a human
can review a batch of screens cheaply. It adds no second screenshot framework and no
Sharp/ImageMagick/native image dependency — PNGs are inlined as base64. **Committed Playwright
baselines remain the regression mechanism**; the contact sheet is review evidence only.

- **Catalog** (`tests/browser/visual/catalog.ts`): the single source of truth mapping every
  stable screen id to its owning spec file, supported states, and dark-project tiers (S/M/L, or
  M-only for a bare `shootScreen` with no forced tiers). `shootPanel`/`shootScreen`
  (`tests/browser/visual/helpers.ts`) validate every call against it — an uncataloged screen/state
  throws immediately instead of shooting unreviewed evidence.
- **Evidence capture**: with `BRAWLER_CONTACT_SHEET_DIR` set, the shoot helpers write a PNG +
  JSON metadata sidecar (screen, state, tier, theme, project, build stamp) for the settled locator
  **before** the existing `toHaveScreenshot` assertion — the assertion still runs and still gates.
  Sidecar filenames are unique per worker (`workerIndex` + a per-worker counter), so
  `fullyParallel` workers never race on the same path.
- **CLI** (`rtk node scripts/ux/contact-sheet.mjs`, or `make ux-contact-sheet`): accepts
  `--screens=<a,b,c>` or `--changed` (maps a **read-only** `git diff` through the catalog — an
  unmapped/unknown changed file is an error, never a silent empty selection, unless `--screens` is
  also given), plus optional `--state` and `--theme=dark|light`. Runs only the two visual
  projects for the owning spec files, merges the sidecars, and emits
  `.artifacts/ux-contact-sheets/<build>/index.html` (gitignored). A missing expected cell is
  reported as a failure. If the underlying Playwright run itself fails, the sheet is still built
  from whatever evidence was written, then the script exits with Playwright's **original**
  non-zero exit code.

```bash
rtk make ux-contact-sheet SCREENS="today,fundamentals"
rtk make ux-contact-sheet CHANGED=1
```

- **Baseline updates** (`make visual-update SCREEN=<name> REASON="why"`, wrapping
  `npm run visual-update` → `scripts/ux/visual-update-guard.mjs`): refuses to run unless both
  `SCREEN` and a non-empty `REASON` are given, and prints both into the run log. Never run this
  during ordinary implementation — only for a deliberate, reviewed visual change.

### Escaped-defect interpretation (plan Q7)

`docs/retros/TEMPLATE.md` carries a marked escaped-defect table (`Ref | Origin class | Detected at | Earliest prevention point | Disposition | Status`) against the canonical origin-class/detection-stage/disposition enums it documents inline. `rtk npm run report:escaped-defects` (`scripts/ux/escaped-defects-report.mjs`, also `make report-escaped-defects`) parses only explicitly marked tables, validates the enums/required cells/unique refs, and prints counts by origin/stage plus repeated classes (count ≥ 2) — it is advisory, stays off `make check`, and never fails because counts increased (only a malformed opted-in row does). **Not every escape needs a new automated test:** a repeated class becomes a precise automated guardrail when cleanly detectable, otherwise a documented human-checklist line (`disposition: human-checklist`) — see the guardrail-harvest loop ([ADR 0045](adr/0045-guardrail-harvest-loop.md)). A single occurrence with a known fix stays `fixed-instance-only`; a tracked-but-unfixed gap uses `tracked:<hex7>`; a deliberate non-fix is `accepted-limitation`. Historical retros without a marked table remain valid and are silently skipped, never backfilled without owner direction.

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

## Live drive (real app via CDP)

Drives the **real running Windows app** (real Tauri backend, real local SQLite DB) via WebView2's Chrome DevTools Protocol, so an agent on WSL (no GUI) can verify a change against the real app directly instead of relying on the manual desktop smoke checklist above. Policy: [ADR 0066](adr/0066-live-drive-remote-debugging.md).

**Standing owner authorization (2026-07-09): an agent may ALWAYS drive the owner's real Windows app live** — `make live-cycle` (or `live-up` + `live-drive`) needs no per-session permission. Prefer it whenever real-runtime/real-DB evidence would strengthen a verification or handover; do not report "not verified on the real Windows app" without having tried this path first.

**One command from WSL** (rebuild → launch on Windows → wait for CDP → run the live suite):

```bash
make live-cycle
```

Or the pieces separately:

- `make live-up` — `package-windows-from-linux` (stops any running brawler, rebuilds and copies the portable exe), launches it on Windows via `powershell.exe` + `scripts/windows/dev-live.ps1` with the CDP port open (`LIVE_CDP_PORT`, default 9222), then polls `…/json/version` every 2s (90s timeout, firewall hint on failure) — localhost first, then the `/etc/resolv.conf` nameserver IP (the WSL2-NAT route to the Windows host). The reachable URL is written to `/tmp/brawler-live-cdp-url`.
- `make live-drive` — runs `tests/live/` via `playwright.live.config.ts`, exporting `BRAWLER_CDP_URL` from `/tmp/brawler-live-cdp-url` when the env var isn't already set.
- Manual launch (a human at the Windows machine, e.g. against an already-built exe): `powershell -ExecutionPolicy Bypass -File scripts/windows/dev-live.ps1`, then `make live-drive` from WSL.

`BRAWLER_CDP_URL` overrides the connection target (default `http://localhost:9222`); the helper (`tests/live/helpers/liveConnect.ts`) applies the same localhost → nameserver-IP fallback order when it is unset. Dev-only; **never** part of `make check`/`make check-epic` — no default/CI environment has a live GUI app with an open debug port.

**Scoped runs + UX checkpoints (Q6, [ADR 0081](adr/0081-ux-quality-loop-v2.md)).** `make live-drive`/`live-cycle` default to the **full** historical live suite; set `LIVE_SPEC=<path>` to drive **one** spec (empty preserves the full-suite default), e.g. `make live-cycle LIVE_SPEC=tests/live/ux-checkpoint.live.spec.ts`. The generic UX checkpoint (`tests/live/ux-checkpoint.live.spec.ts`) drives the **mechanical** J1 path and records evidence for a human charter — it never judges clarity/usefulness ("UX good" is never emitted). It reads `BRAWLER_UX_JOURNEY`, `BRAWLER_UX_CARD`, `BRAWLER_UX_STAGE=vertical|mid|release` (`requireCheckpointMeta` **refuses** to run without them) and `BRAWLER_UX_DATASET` (optional non-sensitive label). Evidence (`manifest.json` + screenshots) is written under gitignored `test-results/live/checkpoints/<stage>-<card>/`; the manifest records build/version, Windows-native confirmation, viewport/DPR, locale/theme, and a **dataset LABEL only** — **never** the database path or contents. Outside a checkpoint run (no metadata) the spec skips, so an ordinary full live sweep is unaffected. Cadences + human charter: [dogfooding.md](dogfooding.md). The env-gated `tests/live/rebuild-fundamentals.live.spec.ts` driver (`BRAWLER_REBUILD=1`) runs the [ADR 0086](adr/0086-aggregator-primary-fundamentals.md) wipe+rebuild path (BR-primary pull + ESEF/WDF re-scan) against a live app; off by default so an ordinary sweep never triggers it.

**Live-probe honesty rules (harvested 2026-07-10, ADR 0045).** Two classes that green-washed real defects on the owner's app:
- **A punchline probe asserts the OUTCOME, not just settlement.** A probe whose purpose is "the flow produced X" must require a nonzero/expected result (`Wydobyto [1-9]`, a row count delta, a DB assertion) — "it settled" with a zero result is a probe FAILURE. A settle-only assert once passed while the feature under test silently did nothing.
- **Panes are company-scoped.** A live cockpit layout can hold panels pinned to OTHER companies (the owner may be using the app concurrently); an unscoped `.first()` locator can silently read a neighbour's pane. Always scope by `data-company-id` (present on coverage + review panels for exactly this reason).
- **Locators are region-scoped (harvested 2026-07-28, issue #129).** Real-app toasts float at page level and inject arbitrary matching text (a per-company skip toast can name "GPW ESPI/EBI"), so a bare page-level `getByText` in a live spec strict-mode-collides intermittently. Scope every content locator to the panel/list container (`getByLabel` on the region), never the page.

## UI dogfooding finding ⇒ overlay (standing rule, ADR 0045 harvest)

A UI defect found by dogfooding/live checkpoint on real data is fixed **twice** in the same
change: the fix itself, **and an overlay in the scenario runtime reproducing the data state
that exposed it** (`src/test/scenarios/runtime.ts` overlays — the ADR 0081 mechanism), so the
state renders in CI forever. Precedents (v0.60): stale-urgent wall, orphaned evidence
(cascade-pruned signals), pruned-feed titles. What a gate CANNOT judge — whether copy is
*understandable* — stays a human verdict (ADR 0081 rule 4); everything mechanical about the
state must redden without a human.

## MCP dogfooding ritual (closure step, ADR 0088 dec. 5)

A **real talk-to-your-research session** over the live MCP port is a mandatory closure step for
any milestone that changes the MCP surface (standing since `v0.60.0`). Mock-green tool tests are
never completion evidence for the port — the ritual is the MCP equivalent of the desktop
dogfooding pass.

Setup: the real Windows app running (live drive above), MCP enabled with a real token, the
client agent with the **`brawler-mcp` skill loaded** (`.claude/skills/brawler-mcp/SKILL.md` —
the ritual also tests the skill itself). Checklist (evidence: transcript summary + tool-call
results in the closure chat, never raw DB contents):

1. **Read every changed domain** — at least one successful call per read tool the milestone
   added/changed, on the owner's real data.
2. **Write with provenance** — one real research write (note/claim/verdict) with citations;
   verify the entry appears in the UI with its origin label.
3. **Refusal honesty** — a write with empty provenance → typed `provenance_required`; any act
   call with the writes toggle OFF → typed `writes_disabled`; a denylisted command is absent
   from `tools/list`.
4. **Trigger + observe** — fire one job trigger (the hermetic-test-exempt networked triggers
   are exercised HERE) and observe its result through a read tool.
5. **Skill fidelity** — note anywhere the skill's guidance mismatched reality; a mismatch is a
   docs defect to fix in the same closure.

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
