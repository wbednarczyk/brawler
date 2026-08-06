# ADR 0062: Single Mandatory Test Gate (Pre-Commit) and Test-Driven Development Loop

Status: Accepted; gate placement superseded by [ADR 0096](0096-quality-gate-architecture-under-continuous-release.md) — the gate runs as the PR's required checks, never pre-commit (hooks deleted); the single-gate principle and TDD loop stand

This ADR makes **one gate the whole truth**: every deterministic/hermetic test suite is a hard-fail step of `make check`, that gate runs **before every commit**, and the project's development loop is **test-driven** (spec-driven for *intent*, test-driven for the *loop*). It executes the browser-suite promotion foreseen in [ADR 0048](0048-test-architecture-sample-data-broad-clickable-coverage-and-layered-parallelism.md) Decision 6 and closes an [ADR 0045](0045-guardrail-harvest-loop.md) guardrail class: *a suite that is not a hard-fail step of the one mandatory gate rots.*

## Context

Test suites that are not on a mandatory gate **rot**. Concretely, in this repo:

1. The full Playwright browser suite and `knip` lived only in `make check-epic` (epic-closure cadence), so per-change work never exercised them.
2. In `check-epic` those steps were **`-`-prefixed** in the Makefile, so make **ignored their exit code** — the target printed FAILURES and still exited `0` ("run-and-report"). The composable-shell→cockpit migration ([ADR 0057](0057-composable-views-and-curated-dashboard.md)) left the browser suite **28-red for two sessions**, masked exactly this way, and was caught only at a later wrap-up.
3. The pre-push hook ran only the `smoke-walk` spec; **no test-CI on push/PR exists** (only `release-artifacts.yml` on tags), by the conservative-CI posture.
4. The ts-rs generated-DTO drift guard (`make types-check`) was outside `make check` too.

Brawler deliberately encodes its architecture and posture as automated checks whose purpose is to **halt an agent about to do the wrong thing** ([ADR 0038](0038-enforcement-as-guardrails.md)). A gate that runs late, or ignores its own exit code, cannot halt anything. And because Brawler is a data-heavy app where **tests are the guardrails** — many exist and many more must be written continuously — the development loop itself must be organized around tests, not merely permit them.

## Decisions

### 1. One mandatory gate: `make check` = every deterministic suite, hard-fail

`make check` runs, as hard-fail steps (no `-` prefix), under Nix:

- `npm run check` — frontend (typecheck · ESLint · stylelint · Vitest · build) + Rust (fmt · clippy `-D warnings` · nextest · doc);
- `npm run knip` — dead-code audit;
- `make types-check` — regenerate ts-rs DTOs and fail on drift;
- `npm run test:browser:install` (idempotent) + `npm run test:browser` — the **full** Playwright browser suite across the viewport matrix;
- `node scripts/check/gate-integrity.mjs` — the meta-guard (Decision 2).

This is the promotion ADR 0048 Decision 6 foresaw ("promote the clickable suite toward a default/pre-merge gate **while it keeps `make check` in the seconds-to-low-minutes range**"); the precondition is met (the full browser suite runs in ~tens of seconds parallelized). `make check` is the **only** proof-of-done gate. `make check-fast` (parallel core, no browser/knip/types/gate-integrity) is for inner-loop iteration only and is **never** proof of done.

**Deliberately excluded** (stay periodic/manual, each disqualified from a per-commit hard gate for a stated reason): `coverage` (slow instrumented build), `mutants` (30 min–2 h), `bench` (machine-dependent wall-clock), the live Gemini / OS-keyring smokes (credentials · network · OS), and packaging (OS · toolchain). `make check-epic` = the full gate + the coverage ratchet (also hard-fail); `make mutants` and `make bench` remain separate closure-cadence targets (see [engineering-workflow.md](../engineering-workflow.md) §I).

### 2. Anti-rot meta-guard (`scripts/check/gate-integrity.mjs`)

A step of `make check` parses the `check` and `check-epic` recipes and **fails** when:

- any recipe line in either target is **`-`-prefixed** (exit-ignored) — no silent red is permitted in a gate target;
- any **mandatory-suite marker** (`npm run check`, `npm run knip`, `types-check`, `npm run test:browser`, `gate-integrity`) is **absent** from `check` — no suite can be quietly dropped from the gate.

This converts the defect *class* ("a suite fell off the gate / a step's exit code is ignored") into a self-enforcing, self-referential check (it guards the target that runs it). Pure Makefile read + string checks; no new dependency.

### 3. Enforcement is split by git phase: fast core at commit, full gate at push-to-master

> **Superseded in part (2026-07-15, owner decision).** The original decision below ran the WHOLE `make check` on every commit. In practice the ~15-min Playwright viewport matrix on every checkpoint was too costly (owner: "za dużo tych testów… pre-commit gate powinien obowiązywać tylko przy mergu do mastera"). The gate is now **split by git phase** — see the 2026-07-15 amendment. The original text is kept for the rationale it still carries (a commit must be a real checkpoint; a green master is non-negotiable).

`.githooks/pre-commit` (installed by `make install-git-hooks`, which sets `core.hooksPath`) runs the whole `make check` before a commit is created; a red gate blocks the commit. Rationale (owner decision): **at push it is too late — at commit we must already be sure.** A commit is a checkpoint ([AGENTS.md](../../AGENTS.md): "commit at meaningful checkpoints"), so a full-gate-per-commit is acceptable and infrequent.

**Amendment (2026-07-13, owner decision) — scope the gate to what changed.** A **docs-only** commit runs `make check-docs` (the doc meta-guards only: mandatory-read byte budgets + parity via `gate-integrity`, and cross-doc `docs-drift`) instead of the full `make check`; **any** code/config change still runs the whole gate. This is sound because a docs-only changeset **cannot** alter code behavior — types, lint, tests, build, browser, knip, and ts-rs drift have nothing to re-verify — while the checks a doc *can* break (a mandatory-read doc growing past its ADR 0063 budget; docs drifting out of sync) still run. Precise rule (`.githooks/pre-commit`): the changeset is docs-only iff **every** staged path is `docs/**`, `wiki/**`, or `*.md`; anything else (src, tests, scripts, config, `Makefile`, `.githooks/**`, Cargo/package manifests) is code and gets the full gate. `check-docs` is a `GUARDED_TARGET` (Decision 2), so it too can never carry an exit-ignored step. The guarantee is unchanged in substance: **a green commit still proves every check that its changes could affect has passed** — it only stops re-running the code suites against changes that cannot touch code. Motivation: eliminate ~5-minute full-gate runs on pure documentation commits (owner: "tests run too often").

**Fail-not-skip:** unlike the pre-push smoke hook (which *skips* if Playwright is absent), a missing tool in pre-commit is a **failure with a fix instruction** — a silent skip is the very hole that lets a suite rot. `make check` installs Chromium itself, so the usual missing tool self-heals. `git commit --no-verify` remains for genuine WIP/emergency commits, but a hand-back or "done" claim is **never** valid on a `--no-verify` commit. The pre-push hook stays as a cheap redundant re-check. No new CI is added (conservative-CI posture; local pre-commit suffices for a solo/local-first project) — a push/PR test-CI is a possible future belt-and-suspenders, out of scope here.

**Amendment (2026-07-15, owner decision) — split the gate by git phase.** Running the full `make check` (dominated by the ~15-min Playwright viewport matrix) on *every* commit was too costly across a working session and a release (owner: "za dużo tych testów; pre-commit gate powinien obowiązywać tylko przy mergu do mastera"). The gate is now split at the two boundaries that matter:

- **pre-commit → `make check-fast`** (the parallel core: typecheck, lint, stylelint, Rust `nextest`, frontend `vitest`, build — **no** browser matrix / knip / types-check). ~2–4 min. It catches the overwhelming majority of breaks at commit time. A **docs-only** changeset still runs `make check-docs` (the 2026-07-13 rule above is unchanged).
- **pre-push → the full `make check`, only when the push targets `master`** (`.githooks/pre-push` parses stdin for `refs/heads/master`). This is the boundary where code becomes shared. A non-master push keeps the cheap `smoke-walk` only.

This holds the **substantive guarantee — master is never advanced past a red full gate** — because there is still **no CI mirror of `make check`** (only `mutants.yml` on dispatch and `release-artifacts.yml` on tags), so the local pre-push hook is that guarantee. What changed is *where* the heavy matrix is paid: once per push-to-master instead of once per commit. The browser matrix can now lag by a few fast-gated commits, surfacing at push rather than at each checkpoint — an accepted trade for a solo/local-first loop where the pusher is the same person.

**Escape hatch `BRAWLER_GATE_ALREADY_GREEN=1`** skips the pre-push full check on a master push. It existed solely for `make release`, which ran `make check` itself immediately before pushing; without the hatch the identical tree would be gated twice back-to-back. `make release` is retired under continuous release (ADR 0090, 2026-07-28 cleanup) — the hatch stays as a manual vouch for mirror/recovery pushes of a tree the caller just gated. It is a caller *vouching it already ran the gate*, not a way to weaken it — a plain `git push` (no env var) always runs the full gate.

The earlier "at push it is too late" rationale is narrowed, not discarded: the *fast* core still runs at every commit (so a commit is never trivially broken), and the *full* gate still runs locally before master advances (so "done"/hand-back on master is still gate-proven). `check-fast`-gated commits are the inner-loop checkpoints; the push-to-master is the proof-of-done boundary.

### 4. The development loop is test-driven

Brawler stays **spec-driven for intent** (the docs/ADRs define behavior before code) and is now explicitly **test-driven for the loop**:

- **Every behavior change is born with a test** (alongside or before the code); the test is a guardrail, not an afterthought. A feature is not "done" until a test **reddens when it breaks**.
- **"Which test where"** is documented as a single scannable map (in [engineering-workflow.md](../engineering-workflow.md) and [testing.md](../testing.md)): *type of change → test layer/suite + how to run the targeted subset*. It consolidates rules previously scattered across testing.md/AGENTS.md — including the recurring gaps (a new IPC command needs its mock-runtime handler + fidelity-corpus step; a new transform needs proptest + golden + scale gate).
- **The loop:** (1) write/extend the test for the behavior; (2) iterate against a **targeted, fast** subset (seconds); (3) before commit, the **full `make check`** (pre-commit enforces it) — the floor, not the ceiling.
- **Anti-rot rule:** every deterministic/hermetic suite MUST be a hard-fail step of `make check`; a suite may sit outside it **only** when non-deterministic (wall-clock/machine), credentialed, network, OS-specific, or heavy-periodic (mutants/coverage) — each exclusion written down with its reason. **No gate step may be `-`-prefixed.** Enforced by Decision 2.

### 5. Amendments to prior ADRs

- **ADR 0048 Decision 6** — the browser-suite promotion "toward a default/pre-merge gate" is now **done**: the full suite is a hard-fail step of the single per-commit gate.
- **ADR 0038 / 0045** — extends the enforcement-as-guardrails / guardrail-harvest line: the "run-and-report" (`-`-prefixed) posture of the old `check-epic` is **removed**; a gate that ignores exit codes cannot halt a wrong move, so all gate steps hard-fail and the meta-guard keeps them that way.

## Consequences

- `make check` is heavier (adds knip, a ts-export rebuild for the drift guard, and the full browser suite) and runs on **every commit** — including the pending fundamentals (S5c) commit. This is the intended cost of "sure at commit time." Inner-loop iteration uses targeted subsets and `make check-fast`, so the heavy gate is hit at checkpoints, not per edit.
- The ts-rs drift guard builds with the `ts-export` feature; interleaving it with the non-feature Rust gate can recompile Rust crates. Accepted for drift safety at checkpoint cadence.
- A new deterministic suite must be added to `make check` (the meta-guard will not force a *specific* new suite in, but the anti-rot rule and DoD require it, and any exclusion must be justified in the docs). Adding a suite outside the gate without a stated reason is the anti-pattern this ADR forbids.

## Alternatives considered

- **Keep the promotion at `check-epic` (closure cadence).** Rejected: that is precisely the cadence that let the browser suite rot 28-red across two sessions.
- **Pre-push instead of pre-commit.** Rejected by the owner: at push it is too late; certainty is required at commit.
- **A push/PR test-CI.** Deferred (conservative-CI posture); local pre-commit is sufficient for a solo/local-first project. A future belt-and-suspenders is a separate decision.
- **Skip missing tools in pre-commit** (mirror pre-push). Rejected: a silent skip is the rot hole; fail with a fix instruction instead.

## Amendment (2026-07-27, ADR 0090) — the gate now runs in CI

The "**there is still no CI mirror of `make check`**" premise (Decision 3, 2026-07-15 amendment) is superseded by [ADR 0090](0090-github-canonical-forge-and-continuous-release.md): the repo is public with free Actions minutes, and `full-check.yml` runs the full gate on every PR (required checks) and on every release-labeled master push via `workflow_call`. The **master-never-past-a-red-gate** guarantee is unchanged in substance but **moves server-side** — a master ruleset requires the full-check jobs green **and** the branch up-to-date before merge, so the merge commit's tree is bit-for-bit the tested one. The local pre-push full-check hook stays as a dormant break-glass net, no longer the sole guarantee. The single-gate anti-rot contract (Decision 1/2) is unchanged; the CI jobs invoke the same decomposed `make` targets that compose `make check`, and gate-integrity asserts CI uses only `make <target>` steps.
