---
name: brawler-release
description: Close a Brawler epic under the continuous-release model — retrospective, Definition of Done §I audit, wiki + docs closure, and GitHub issue/board close. Use when wrapping up or closing an epic; releases themselves are automatic (a PR's release:* label ships the version on merge).
---

# Brawler Epic Closure

Use this workflow from the Brawler repository root when the user explicitly asks to wrap up or close an **epic**. Under continuous release ([ADR 0090](../../../docs/adr/0090-github-canonical-forge-and-continuous-release.md)) **there is no "cut a release" step**: every merged PR carrying a `release:major|minor|patch` label auto-ships a version through `release.yml` (tags are truth, manifests are stamped at build time, `CHANGELOG.md` is written by the release bot). Versions are not "closed"; **epics are.** This skill is the epic-closure runbook, not a release driver.

## What the agent does NOT do

- **Never bump version files, run a version script, hand-edit `CHANGELOG.md`, or create tags.** All of that is `release.yml`'s job, driven by the PR's `release:*` label. The manifests hold a `0.0.0-dev` placeholder on purpose.
- **Never apply a `release:*` label to move the version** — the increment (`major`/`minor`/`patch`/`skip`) is an owner decision on the PR. An agent may *report* which label it believes fits and why, then wait.
- **Never merge** — only the owner merges PRs (ADR 0090 § ruleset). Agents create branches and PRs (`gh pr create`) without asking; mutating repo settings needs the owner.

## Closure Sequence (per epic)

Full order per [DoD §I](../../../docs/engineering-workflow.md#definition-of-done-the-handover-gate) and CLAUDE.md Working Rules:

1. **Write the retrospective** for the user, before closure sign-off: both domains (app + development loop) × went-well / went-wrong (incl. unexpected gaps) / stop / improve — closed vs still-open marked honestly. **Present its content (and the harvested-guardrails list) to the owner INLINE in chat — a committed file the owner never saw does not count** (owner feedback, 2026-07-12). Template: [docs/retros/TEMPLATE.md](../../../docs/retros/TEMPLATE.md).
2. **Human-only docs refresh**: update `docs/for-the-author.md` (state line, layer/domain summaries, "gdzie co znaleźć") and republish the "Brawler — mapa systemu" Artifact if systems changed; give the owner a short **"co nowego po ludzku"** summary in chat.
3. **Spec-conformance audit** of the epic's ADR(s), decision by decision: for each, verify a **live-path invocation** exists (`repoctx callers` from the real job/command/UI entry, not only unit tests) and record a verdict (conforms / partial / deviates / not built).
4. **`make check-epic`** (full gate + coverage ratchet) green under Nix. **Mutants are NOT a closure step** (owner 2026-07-16): run `gh workflow run mutants.yml` ONLY on an explicit owner request, then triage; NEVER `make mutants` locally (OOM-freezes WSL). (+ **`make bench`** if a hot kernel changed.) Triage every failure: fix it or file a tracked GitHub issue.
5. **`wiki/`** entries for every user-facing change delivered by the epic. Under continuous release the wiki is updated **in the PR that changes user-facing behavior**, so by closure this should already be done — confirm it, don't defer it.
6. **Close the tracking**: `gh issue close <n> --reason completed` for the delivered task issues and the epic; the project automation moves them to **Done** on the board. Do not touch `state:*` labels — state lives only in the board's `Status` field (ADR 0090 § board). `make sync-rad` mirrors master + tags to Radicle asynchronously (the owner runs it when convenient; it is not part of closure).

**The user must sign off between the (1)–(3) findings and executing (5)–(6)** — do not close issues or move the board until that sign-off is given.

## Standing permissions (epic closure)

The owner grants standing permission for agents to run, unattended, when part of this closure workflow:

- `gh issue close ...` / `gh issue edit ...` for the epic's delivered task/epic issues (never `state:*` labels)
- read-only Git inspection (`git status`, `git diff`, `git log`, `git show`, `git rev-parse`, `git describe`, `git tag --list`)
- `gh pr create ...` for the closure PR (docs/wiki/tracking changes)

This is narrow: it applies only after the owner asks to close an epic, and never authorizes merges, version bumps, tag creation, `release:*` labelling, repository setting changes, publication/seeding policy changes, or new remotes.

## How a version actually ships (reference)

The owner (or the agent, on the owner's instruction) sets exactly one `release:*` label on the PR. On merge, `release.yml`:

1. **detect** — finds the PR by SHA, reads the label. `release:skip` ends here (no test, no tag).
2. **check** — calls `full-check.yml` (`workflow_call`); green on the merge-commit SHA.
3. **release** — computes `last-tag + label increment`, `make package-release-artifacts VERSION=x.y.z` (version stamped into the binary/UI/manifests at build), then tags → GitHub Release + artifacts + auto-notes, then commits the `docs(changelog): vX.Y.Z [skip ci]` entry.

For a failed release, recovery is local via make targets (`make release-publish VERSION=… EXE=…`) — parity with the workflow. Details: [ADR 0090](../../../docs/adr/0090-github-canonical-forge-and-continuous-release.md) § 5, [release-workflow.md](../../../docs/release-workflow.md).

## Validation

`make check-epic` (full mandatory gate + coverage ratchet) is the closure gate — run it green under Nix before sign-off, plus `make bench` if a hot kernel changed ([DoD §I](../../../docs/engineering-workflow.md#definition-of-done-the-handover-gate)). Never substitute piecemeal subset checks. A "gate green" claim needs the gate's own exit code as evidence: for a backgrounded run, grep the echoed `EXIT=` line rather than trusting a wrapper/task-notification exit code. Validation counts only under Nix — the host toolchain can be silently split.

## Guardrails

- Do not hand-edit version files, `CHANGELOG.md`, or create tags — `release.yml` owns them (build-time stamping; ADR 0090 § 5).
- Do not apply/change a `release:*` label to force a version — that increment is the owner's decision.
- Do not merge, publish, seed publicly, change repository visibility, or rewrite history unless the user explicitly asks.
- If a version assertion test flags a placeholder mismatch, the fix is the build-time stamping path, never weakening the test.
