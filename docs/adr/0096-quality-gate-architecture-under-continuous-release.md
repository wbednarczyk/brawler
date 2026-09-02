# ADR 0096: Quality Gate Architecture Under Continuous Release

Status: Accepted (2026-08-05, owner decision after sol consultation + adversarial review)

Deciders: maintainer. Area: CI, testing, release workflow, agent process.

## Context

Continuous release ([ADR 0090](0090-github-canonical-forge-and-continuous-release.md)) made the
PR the only pre-ship gate — every merge is a release, so nothing post-merge can block delivery,
only audit it. Against that reality, three pieces of the gate stack were legacy: the coverage
ratchet ran post-hoc (`make coverage`, periodic/closure-cadence) instead of gating the PR that
moved it; `make check-epic` (full gate + ratchets + `realdata-gt-check`) was dead in practice —
epics ship via normal PRs now, and the target had no caller left that mattered; and the local git
hooks (`pre-commit` → `check-fast`, `pre-push` on master → full `check`) gated commits that, under
continuous release, ship nothing by themselves — the PR gate is what ships. Epic #335 (owner
decision 2026-08-05, sol consultation + adversarial review) reworks the gate architecture to match.

## Decisions

1. **Only the PR blocks; everything post-merge is audit.** A required check on the PR is the sole
   pre-ship gate. Nothing that runs after merge — mutation sweep, bench, epic closure — may block
   or revert a shipped change; findings become tracked cards.
2. **Job count is irrelevant; every job is individually visible.** No aggregate/rollup contexts
   hiding which specific check failed. Optimize **wall-clock** (parallelism, caching) — never
   runner-minutes, which are free on this public repo.
3. **The change classifier stays binary docs-vs-code, fail-closed** (every changed path must
   match the docs allowlist for the docs-only path to apply — one code path routes to the full
   gate). No finer scope buckets (no per-area skip lists). Classification may decide what is
   **SAID** (advisory hints, PR comments) — never what is **CHECKED**: a wrong classification must
   never silently skip a real check.
4. **Coverage ratchet becomes two scoped PR required checks** — `Frontend coverage ratchet`
   (`make coverage-frontend`, floor 80.0%) and `Rust coverage ratchet` (`make coverage-rust`,
   floor 86.5%), both against `coverage-baseline.json`. **Amends ADR 0090's single-cargo-cache-writer
   rule** to **one writer per key family**: the instrumented coverage build needs its own
   `cargo-llvm-cov` cache key, distinct from the plain test-build cache. Both families share the
   repo's 10 GB cache quota and may evict each other under pressure — accepted and monitored, not
   solved here.
5. **Audits are risk-triggered and advisory, never blocking.** Mutation audit auto-runs on
   `master` pushes touching the five monitored risk paths (`fundamentals/expr/**`,
   `storage/migrations.rs`, `storage/feed_matching.rs`, `source_adapters/parsing.rs`,
   `entity_resolution.rs`), plus manual dispatch; performance audit is a manual base-vs-head run on
   the same runner (separate card, #336); epic closure is a pure **post-delivery** audit —
   retrospective, ADR spec-conformance, guardrail harvest, card triage — never a gate a merge waits
   on ([kanban.md](../kanban.md) § Epic closure).
6. **Local layer shrinks to the commit-msg hook.** `pre-commit` and `pre-push` are deleted;
   `commit-msg` (Conventional Commits validation) is the only surviving git hook. `make
   check-local` (renamed from `check-fast`) is the developer inner loop and the pre-handover DoD
   step — invoked deliberately, never hook-triggered. Heavy suites (full gate, coverage, mutants,
   the bench audit) never run locally, unchanged from prior practice; `make audit-bench` (a quick
   local criterion run self-comparing against the previous local run) stays allowed as advisory.

**Amendment (2026-08-05, #336/#337).** The performance audit is `bench-audit.yml`: one manually
dispatched runner benches the merge-base (in a detached worktree, head's orchestration
authoritative), then head (`--baseline-lenient`), and `bench-compare.mjs` flags a kernel only when
criterion's median-change confidence interval sits entirely above +30%. This supersedes
[ADR 0049](0049-test-architecture-v2-data-transform-correctness.md)'s committed-floor ratchet
(`bench-baseline.json` + `bench-ratchet.mjs`, deleted — cross-machine floors compare apples to
oranges on hosted runners). Principle 5's advisory owner evidence gains its tooling: the
`Live-drive hint (advisory)` PR job (dumb path→hint list; says, never checks) and
`make pr-live-cycle PR=n` (drives the PR's cross-built exe over the real data with no WSL rebuild).

**Supersedes** the closure-cadence portions of [ADR 0048](0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md)
(coverage as a periodic/closure-cadence run), [ADR 0062](0062-mandatory-test-gate-and-test-driven-loop.md)
(ratchet placement, hook composition), and [ADR 0090](0090-github-canonical-forge-and-continuous-release.md)
(the `epic-gate.yml` mention; the single-cargo-cache-writer rule, amended per decision 4).
[ADR 0084](0084-retire-in-app-ai-layer.md) and [ADR 0091](0091-failure-path-and-real-state-testing.md)
are untouched.

## Consequences

- **Deleted:** `make check-epic`, `realdata-gt-check`, `realdata-extraction-metrics` (target +
  the `extraction_metrics` module it ran), `epic-gate.yml`, `.githooks/pre-commit`,
  `.githooks/pre-push`.
- **Renamed:** `check-fast` → `check-local`; `make mutants` → `make audit-mutants`; `mutants.yml`
  → `mutation-audit.yml` (gains the risk-path master-push auto-trigger; manual dispatch kept).
- **New required checks:** `Frontend coverage ratchet`, `Rust coverage ratchet` (decision 4).
- **Amendment (2026-09-02, #448):** new required context `Visual baselines (pinned renderer)`
  (`full-check.yml` job `visual`, `make check-visual`) — pixel comparison moves from
  maintainer-WSL-only to CI, run in the official Playwright image at the locked
  `@playwright/test` version, zero tolerance. Added to the branch ruleset by the owner after the
  first green run, before merge.
- Owner-machine realdata rituals (`realdata-gt-score`, `realdata-extraction-check`,
  `realdata-honesty-check`, `make live-cycle`) stay advisory, on-demand, never required checks.
- The #182 ESEF/positional ground-truth scorer stays a diagnostic (no precision/recall floor)
  until measurement v2 (#331) narrows it to ESEF-only per [ADR 0095](0095-retire-html-positional-tier.md).
- Follow-on cards tracked separately: bench rework to a base-vs-head audit (#336), live-drive
  hints (#337), anti-archaeology sweep (#338).

## Alternatives considered

- **Keep `check-epic` as a manual closure ritual.** Rejected: nothing calls it under continuous
  release, and a target nobody runs is exactly the rot this ADR removes.
- **Finer path-based scope buckets in the classifier.** Rejected: more buckets is more surface for
  a real code change to slip through a narrow allowlist; binary docs-vs-code with fail-closed
  quantification is the simplest rule that cannot silently under-check.
- **Keep the coverage ratchet periodic.** Rejected: a floor nobody enforces per-PR drifts
  unnoticed between periodic runs; a scoped required check catches the regression at the PR that
  caused it.
