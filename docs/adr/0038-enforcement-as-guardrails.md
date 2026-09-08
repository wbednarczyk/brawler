# ADR 0038: Enforcement as guardrails — gates are a hard stop

Status: Accepted.

## Context

Brawler is a spec-driven, largely agent-built project. Agents act with partial context: an agent editing one area often cannot see every architectural decision, convention, or constraint that applies, and may not realize a change is wrong or that it should have consulted the user first. Relying on each agent (or reviewer) to remember every rule does not scale and has already let drift through — grey buttons from a CSS-less variant, mixed-language UI from an English-fallback i18n bug, primitive bypasses, swallowed errors, a stale-migration production failure.

We already have many checks (type checks, ESLint/stylelint, guard and contract tests, the translation/pluralization/a11y guards, layout/viewport tests, release `release-check`, `engine-strict`). The decision here is about their **purpose and how they must be treated**, not about adding a specific one.

## Decision

**Automated checks encode the project's good practices, architecture, and posture, and exist to produce a hard stop when an agent is about to do the wrong thing** — especially when acting without consulting the user, or without enough context to know the change is wrong.

1. **A failing gate is a stop-and-reconsider signal, not an obstacle to clear.** Agents must not weaken, delete, skip, `--no-verify`, baseline-away, loosen, or work around a check, rule, or assertion to make it pass. If a gate looks wrong, raise it with the user and change the rule deliberately, with the matching doc/ADR update.

2. **New capability ships with the gate that protects it.** When a change introduces a decision, contract, convention, or invariant, it should also add (or extend) the check that keeps future changes — by any agent — from silently violating it. Coherence is maintained by mechanical enforcement, not by every agent having full context.

3. **Gates must be non-restrictive but firm.** A gate should fail on genuine violations and not on legitimate, intended cases. Where a legitimate exception exists, it is made explicit and documented at the site (e.g. an inline `eslint-disable … -- <reason>`, a baseline entry with rationale), never by disabling the gate globally. "It was faster" or "it was in my way" is never a reason to bypass.

4. **Prefer a hard stop over a silent pass.** When in doubt, a check should block and prompt consultation rather than allow a questionable change through. `engine-strict` (refuse installs on an unsupported Node), `--max-warnings 0` (no silent lint backlog), and the primitive-first/error-line bans are examples of this posture.

The single toolchain (one version locally and in nix; see [engineering-workflow.md](../engineering-workflow.md)) is part of this: divergent or pinned-old tooling produces gates that pass in one place and fail in another, which defeats the hard-stop guarantee.

## Consequences

- Agents get caught by a gate instead of shipping silent drift; the correct response is to stop, reconsult the spec/user, and fix the cause — not the check.
- Reviewers and the human maintainer can trust that a green `check` means the encoded practices held, without re-verifying each by hand.
- The check surface grows with the codebase: every durable decision should leave behind an enforcing gate. This is intentional, not bureaucracy.
- This complements rule 2 (doc-first): docs record intent; gates make the intent mechanically binding. It is referenced as the third of the "Three Always-On Rules" in [AGENTS.md](../../AGENTS.md).

**Amendment 2026-09-07 (owner, tests audit):** rules that bound *agent behaviour* are enforced by tooling, never by reading — the hook layer joins the gate layer. Wave 1 hard gates: `one-heavy-build` PreToolUse hook (testing.md § Resource discipline), advisory workflows must card their findings (`gh issue create` step asserted by gate-integrity; mutation audit baseline guarded against runtime cross-tree reads), escaped-defects report validated in `check-docs-gates`, fidelity-corpus membership + Rust-side command reachability + transform-manifest + assertion-free-test source scans (ratchets: baselines only shrink), test-hygiene lints (`.only`/reasonless `.skip`), paint-not-attribute browser assertions, English-in-PL locale detector. Pending owner answers: git-boundary hook (commit/push grant), mutation gating in PR, `retries: 0`. Catalogue and rationale: the tests audit report (owner-private artifact, 2026-09-07).

**Amendment 2026-09-08 (owner, tests audit wave 2):** the pending owner answers above are resolved. **T1** — commit + push to a feature branch need no explicit consent; still forbidden: anything that lands on `master` (push/merge), force push, `--no-verify`, and `checkout/restore/stash/reset --hard/clean` on tracked files. **T2** — the mutation audit is never a PR check (too slow); it triggers post-merge on `master` (risk paths) or on a cron, and findings auto-create issues (G3). **T3** — Playwright CI `retries: 1 → 0`. Wave 2 hard gates: **G1** `git-boundaries` PreToolUse hook (mechanizes T1 for Claude's Bash tool); **G8** own-connection multi-write pin (a `src/storage/**` fn that checks out its own connection and issues ≥ 2 writes must wrap them in a transaction; today's offenders are a frozen, shrink-only pin); **G9** command-shape pin (an `async` Tauri `State` command must offload via `spawn_blocking`; today's 160 sync `State` commands are a frozen identity list — a new one must be async + offloaded); **G14** tests-touched check (a PR's code diff needs test evidence or the `tests:not-needed` label); **G15** per-directory coverage floors (a directory absent from the baseline is admitted only at ≥ 70% lines).
