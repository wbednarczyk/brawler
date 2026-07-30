---
name: brawler-release
description: Complete a Brawler epic under the continuous-release model — a PRE-MERGE gate (retro, ADR audit, human docs, live-drive §G, CI epic-gate ratchets) executed as commits on the epic branch, because merging the epic PR IS the release. Heavy suites run ONLY in CI. Use when wrapping up or closing an epic; after the merge only issue/board mechanics remain.
---

# Brawler Epic Completion (pre-merge gate)

Use this workflow from the repository root when the user asks to wrap up / close an **epic**. Premise ([ADR 0090](../../../docs/adr/0090-github-canonical-forge-and-continuous-release.md)): **merging the epic PR IS the release** — the `release:*` label ships a version from the merge commit automatically. Therefore every completion activity happens **BEFORE the merge, as commits on the epic branch**: a doc updated after merge is a doc the release does not contain, and a test run after merge validates a binary that already shipped. After the merge only tracking mechanics remain.

## What the agent does NOT do

- **Never bump versions, edit `CHANGELOG.md`, or create tags** — `release.yml` owns them (version = last tag + label increment, stamped into the manifests by the post-release `chore(release): vX.Y.Z [skip ci]` commit).
- **Never apply a `release:*` label** — the increment is the owner's decision. The agent may report which label it believes fits, then wait.
- **Never merge** — owner-only (server ruleset). Agents create branches and PRs unattended.

## Completion sequence (ALL on the epic branch, before asking for merge)

1. **Verify the continuous obligations already landed** — wiki entries and canonical-doc updates belong to the behavior-changing commits, not to closure. If something was deferred, add it to the branch NOW.
2. **Write the retrospective, presented INLINE in chat** before sign-off (a committed file the owner never saw does not count — owner 2026-07-12): both domains (app + dev loop) × went-well / went-wrong (incl. unexpected gaps) / stop / improve, closed vs still-open marked honestly. Template: [docs/retros/TEMPLATE.md](../../../docs/retros/TEMPLATE.md). **Guardrail harvest is part of this step**: every flagged defect's class closed same-branch or carded.
3. **Spec-conformance audit** of the epic's ADR(s), decision by decision: verify a **live-path invocation** exists (`repoctx callers` from the real job/command/UI entry, not only unit tests); verdict per decision (conforms / partial / deviates / not built), presented in chat.
4. **Human docs on the branch**: update `docs/for-the-author.md` (state line, layer summaries) and republish the "Brawler — mapa systemu" Artifact if the system map changed; give the owner a plain-language "co nowego" summary in chat.
5. **Real-behavior verification (DoD §G) pre-merge**: drive the epic's user-visible behavior on the REAL Windows app using the PR's cross-built exe — `make pr-binary PR=<n>` downloads it, `scripts/windows/dev-live.ps1` launches it with CDP, `tests/live/` drives it. A WSL build is never desktop evidence. If the behavior needs states unreachable on a healthy system, say so honestly and record what WAS verified.
6. **The epic gate runs in CI, never locally** (owner rule 2026-07-30: no heavy suites on the local machine; running `make check-epic` locally is retired). Evidence = the epic PR's green required checks (the full gate on the exact tree) **plus** `gh workflow run epic-gate.yml --ref <epic-branch>` (coverage ratchet; bench when a hot kernel changed). The real-DB honesty class is enforced in default CI by the synthetic shape corpus; the local real-DB ratchet (`make realdata-honesty-check`) runs ONLY when the owner explicitly asks. Triage every failure: fix on the branch or file a tracked issue.
7. **Mutants** (standing rule 2026-07-29): the agent runs `gh workflow run mutants.yml` when worthwhile — at least once per epic — and triages findings into cards, never blockers. NEVER `make mutants` locally (OOM-freezes WSL). Shards are 0-indexed (`0/8`…`7/8`).
8. **Owner sign-off → label → merge.** Present the findings of 2-7; the owner signs off, sets the `release:*` label, merges. The merge ships the release; `release.yml` tags, publishes artifacts + notes, and pushes the stamp commit. Failed-release recovery: `gh workflow run release.yml -f tag=vX.Y.Z` (republish path) — tags go over SSH via the deploy key.
9. **Post-merge mechanics only**: task issues auto-close via the PR's `Closes` tags; `gh issue close <epic> --reason completed` with a short closure comment; the project automation moves the board (state lives ONLY in the board `Status` field, never `state:*` labels). `make sync-rad` mirrors asynchronously — the owner's convenience, not a closure step.

## Standing permissions (epic completion)

When part of this workflow, the agent may run unattended: `gh issue close` / `gh issue edit` for the epic's issues (never `state:*` labels), read-only git inspection, `gh pr create` for the epic/completion branch. Never: merges, version bumps, tags, `release:*` labels, repo-setting mutations, publication/seeding changes, new remotes.

## Guardrails

- A "gate green" claim needs the gate's own exit evidence (grep `EXIT=`), under Nix — host green is not a verdict.
- If a version assertion flags a placeholder/version mismatch, the fix is the build-time stamping path, never weakening the test.
- Do not publish, seed publicly, change visibility, or rewrite history unless the user explicitly asks.
