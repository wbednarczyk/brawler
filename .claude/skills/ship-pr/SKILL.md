---
name: ship-pr
description: Use when a change is implemented and needs to land as a PR — branching, evidence, commits, PR creation, CI watching, or when CI jobs fail and you must decide rerun-vs-fix. Also use when a "green" claim is about to be made.
---

# Ship a PR

Operational sequence for landing work under continuous release ([ADR 0090](../../../docs/adr/0090-github-canonical-forge-and-continuous-release.md)/[0096](../../../docs/adr/0096-quality-gate-architecture-under-continuous-release.md): merge = release; the PR's required checks are the only gate). Rules live in [engineering-workflow.md](../../../docs/engineering-workflow.md) (Definition of Done, §K evidence) — this skill is the command-level ritual.

## Sequence

1. **Branch from a fresh master**: `git checkout master && git pull --ff-only && git checkout -b <type>/<slug>`.
2. **Implement**; iterate with targeted runs (the DoD's which-test-where map), one heavy suite at a time.
3. **Evidence before handover** (§K — the recurring failure this skill exists for):
   - `make check-local` under Nix; green = the recipe's own final `CHECK_LOCAL_EXIT=0` line. A wrapper/pipe exit code is NEVER evidence (it reflects `tail`, not `make`).
   - `make check-docs-gates` separately — **check-local does not cover docs gates** (caught live: MCP catalog drift reached CI, #346).
   - Re-run after the last edit; never hand over a stale run.
4. **Simplification pass before committing**: run the over-engineering review (`/ponytail-review` when the plugin is available) on the diff — delete speculative abstractions, reinvented helpers, dead flexibility. The repo's own contract (tests-that-redden, proptest/insta for transforms, doc updates) is *requested work*, never a simplification target.
5. **Commits**: Conventional, single `[a-z0-9._-]+` scope, English. **Zero AI attribution anywhere on the forge** — commits, trailers, PR bodies (a footer slipped into PR #349 and had to be scrubbed; check before and after `gh pr create`).
6. **PR**: `gh pr create --label "release:<major|minor|patch|skip>"` — the agent sets the *proposed* label matching scope (CLAUDE.md § continuous release); merging and the final label call stay owner-only. Body: what/why, guardrails added, honest §K evidence block (what ran, what did NOT).
7. **Watch CI to the exact head SHA** (`gh pr checks <n>`; confirm `headRefOid` matches your push). Poll loop:
   ```bash
   for i in $(seq 1 60); do out=$(gh pr checks <n>); \
     echo "$out" | grep -q "pending" || break; sleep 30; done
   ```
   Wait only on `pending` — never on `skipping`: skipped jobs are **terminal** (a docs-only PR keeps them forever and still satisfies required checks), so a loop that waits for `skipping` to clear hangs to timeout.

## Failure triage (decide class BEFORE touching anything)

| Signal | Class | Action |
| --- | --- | --- |
| Fails in **Set up job** ("Failed to resolve action download info", "Service Unavailable"), or second-long jobs dying after minutes | GitHub infra | Check `https://www.githubstatus.com/api/v2/status.json`; wait out `indicator: major`, then `gh run rerun <id> --failed` |
| Job runs your step and asserts | Real | Fix on the branch; fix the class (guardrail-harvest), never weaken the gate |
| Known flake signature, local green | Flake | One rerun + file/annotate the tracked flake issue — never a blind timeout bump |

## Common mistakes

- Claiming green off a local run or a partial log — a failed stage **aborts later stages**; only the marker/`gh pr checks` count.
- Merging, editing `release:*` post-creation, version bumps, changelog edits — all owner/automation-only.
- Fixing "infra" failures by changing code, or rerunning real failures hoping they pass.
