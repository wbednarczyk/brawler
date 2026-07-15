# ADR 0062: Single Mandatory Test Gate (Pre-Commit) and Test-Driven Development Loop

Status: Accepted

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

### 3. Enforcement is **pre-commit**, not pre-push

`.githooks/pre-commit` (installed by `make install-git-hooks`, which sets `core.hooksPath`) runs the whole `make check` before a commit is created; a red gate blocks the commit. Rationale (owner decision): **at push it is too late — at commit we must already be sure.** A commit is a checkpoint ([AGENTS.md](../../AGENTS.md): "commit at meaningful checkpoints"), so a full-gate-per-commit is acceptable and infrequent.

**Amendment (2026-07-13, owner decision) — scope the gate to what changed.** A **docs-only** commit runs `make check-docs` (the doc meta-guards only: mandatory-read byte budgets + parity via `gate-integrity`, and cross-doc `docs-drift`) instead of the full `make check`; **any** code/config change still runs the whole gate. This is sound because a docs-only changeset **cannot** alter code behavior — types, lint, tests, build, browser, knip, and ts-rs drift have nothing to re-verify — while the checks a doc *can* break (a mandatory-read doc growing past its ADR 0063 budget; docs drifting out of sync) still run. Precise rule (`.githooks/pre-commit`): the changeset is docs-only iff **every** staged path is `docs/**`, `wiki/**`, or `*.md`; anything else (src, tests, scripts, config, `Makefile`, `.githooks/**`, Cargo/package manifests) is code and gets the full gate. `check-docs` is a `GUARDED_TARGET` (Decision 2), so it too can never carry an exit-ignored step. The guarantee is unchanged in substance: **a green commit still proves every check that its changes could affect has passed** — it only stops re-running the code suites against changes that cannot touch code. Motivation: eliminate ~5-minute full-gate runs on pure documentation commits (owner: "tests run too often").

**Fail-not-skip:** unlike the pre-push smoke hook (which *skips* if Playwright is absent), a missing tool in pre-commit is a **failure with a fix instruction** — a silent skip is the very hole that lets a suite rot. `make check` installs Chromium itself, so the usual missing tool self-heals. `git commit --no-verify` remains for genuine WIP/emergency commits, but a hand-back or "done" claim is **never** valid on a `--no-verify` commit. The pre-push hook stays as a cheap redundant re-check. No new CI is added (conservative-CI posture; local pre-commit suffices for a solo/local-first project) — a push/PR test-CI is a possible future belt-and-suspenders, out of scope here.

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
