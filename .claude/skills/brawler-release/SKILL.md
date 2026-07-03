---
name: brawler-release
description: Run the Brawler milestone/patch release workflow — synchronized version bump, changelog, release-check, single chore(release) commit, tag, push to both remotes. Use when closing a milestone, cutting a release, or preparing curated release notes.
---

# Brawler Release

Use this workflow only from the Brawler repository root when the user explicitly asks to wrap up, close, or release a milestone/epic.

> **This workflow is the driver, not a reference.** When closure is signed off, do the scope-specific closure it cannot infer (roadmap/kanban text, Radicle/Radboard issue state), then run **`make release VERSION=x.y.z`** for the mechanical bump/changelog/check/commit/tag/push. **Do not** hand-edit version files or run `scripts/release/bump-version.mjs` yourself — the target performs the bump and **aborts if the version is already bumped**, so a manual bump actively fights it. Reach for the target; do not re-assemble its steps by hand.

## Closure Sequence (milestone/epic)

Full closure order per [DoD §I](../../../docs/engineering-workflow.md#definition-of-done-the-handover-gate) and CLAUDE.md Working Rules:

1. **Write the retrospective** for the user, before closure sign-off: both domains (app + development loop) × went-well / went-wrong (incl. unexpected gaps) / stop / improve — closed vs still-open marked honestly.
2. **Spec-conformance audit** of the epic's ADR(s), decision by decision: for each, verify a **live-path invocation** exists (`repoctx callers` from the real job/command/UI entry, not only unit tests) and record a verdict (conforms / partial / deviates / not built).
3. **`make check-epic`** (full gate + coverage ratchet) + **`make mutants`** (+ **`make bench`** if a hot kernel changed) — triage every failure: fix it or file a tracked Radicle issue.
4. **`wiki/`** entries for every user-facing change delivered by the epic.
5. **`docs/roadmap.md`** status line + **`docs/kanban-archive.md`** completed-card entry + `rad issue state --solved` for delivered tasks/epic (never `--closed` for delivered work).
6. **Squash-merge to master** (default integration — see Release Boundary).
7. **`make release-prepare`** → curate → **`make release`**.

**The user must sign off between the (1)–(2) findings and executing (5)–(7)** — do not move to roadmap/archive/merge/release until that sign-off is given.

## Release Boundary

The normal workflow is:

1. The user commits feature work.
2. The user explicitly asks the agent to wrap up or close the milestone.
3. The agent performs release-only documentation and tracking closure, then may create exactly one release commit.
4. The agent creates the matching annotated release tag for that release commit.
5. After the release commit and tag are verified, the agent syncs both canonical/mirror remotes when the user has asked to complete the release.

Do not merge, publish, seed publicly, or rewrite history unless the user explicitly asks for that operation.

**Integrating a feature branch to master for release uses squash-merge by default.** When the work for a milestone/epic is on a feature branch and the user asks to wrap/release, the default integration is a squash-merge into master (`git checkout master && git merge --squash <branch> && git commit`), producing **one** feature commit for the milestone — then the separate single `chore(release)` commit on top. Do not ask which merge strategy to use; squash is the default. (A non-squash merge or a different strategy is used only if the user asks for it.)

## Standing Release Permissions

The project owner grants standing permission for agents to run these commands unattended when they are part of this Brawler release workflow:

- any `gh release ...` command
- read-only Git inspection needed to perform the release, including `git status`, `git diff`, `git log`, `git show`, `git rev-parse`, `git describe`, and `git tag --list`
- `git add ...` for release wrap-up files
- `git commit ...` for the single release commit
- `git tag ...` for the matching release tag
- `git push ...` for syncing the release commit and tag to the existing `origin` and `rad` remotes
- any `rad issue ...` command for release-scoped Radicle/Radboard task and epic state updates
- tag replacement commands, such as deleting/recreating a local tag or pushing an updated tag, only when the user explicitly asks to overwrite or repair an existing release tag

This permission is narrow. It applies only after the user has asked to close, wrap up, or release a milestone/epic/patch and only for release-scoped work. It does not authorize feature commits, unrelated file staging, branch manipulation, merges, rebases, history rewrites, force pushes unrelated to an explicitly requested release tag repair, repository setting changes, publication/seeding policy changes, or new remotes.

## Preconditions

Before making release changes:

- Confirm the user has explicitly signed off on milestone closure.
- Check `git status --short`.
- If unrelated dirty files exist, stop and ask which files belong in the release wrap-up.
- Confirm the target version from the milestone label (`milestone:vX.Y.0`) or the user's explicit choice; confirm it with the user when ambiguous.

## Required Version Files

When bumping the app version, update the same version string in all of these places:

- `package.json`: root `"version"`
- `package-lock.json`: root `"version"` and `packages[""].version`
- `src-tauri/Cargo.toml`: `[package].version`
- `src-tauri/Cargo.lock`: `[[package]] name = "brawler"` `version`
- `src-tauri/tauri.conf.json`: root `"version"`
- `src-tauri/src/lib.rs`: `health_reports_ok` expected version assertion

Keep only the app/package version changed; do not touch dependency versions.

## Required Release Docs And Tracking

For milestone closure, update:

- **`wiki/` — required.** Create or update the user-facing `wiki/` entries (how-to guides, references, instructions) for every new or changed user-facing capability in the release. The wiki is the end-user documentation, distinct from the canonical `docs/` specs; a feature is not release-ready until its user-facing guide exists or is updated. Do this as part of `release-prepare`, before the release commit, so the docs ship with the release.
- `docs/roadmap.md`: add or update `Status: completed in `X.Y.Z`.` under the milestone heading.
- `docs/kanban-archive.md`: add the completed-card detail entry. Live state closes via `rad issue state --solved` on the relevant milestone/epic/task issues — `docs/kanban.md` is only the pointer + label conventions, not a place to hand-edit status.
- If closure added or changed any ADR, regenerate `docs/adr/INDEX.md`: `node scripts/check/docs-drift.mjs --write-adr-index` (the docs-drift gate fails on a stale index).

## Changelog Rule

Do not hand-edit `CHANGELOG.md` as the changelog generation step.

Use the Makefile target dedicated to changelog generation:

```bash
make changelog
```

`make changelog` produces a **scaffold**: one terse line per commit (the commit subject, with scope). It is not the finished release notes.

After running the target:

1. **Verify the heading is versioned**, not `## v -`. An empty version means `APP_VERSION` did not resolve (it is read from `package.json` by the Makefile). A `## v -` heading silently breaks the GitHub release: the release workflow runs `extract-changelog-entry.sh "vX.Y.Z"` against the **tagged** `CHANGELOG.md`, finds no `## vX.Y.Z` section, exits non-zero, and **no GitHub release or binaries are published**. This happened to `v0.41.2` and `v0.42.0` after the Node 18 removal left `node` off the bare PATH.
2. **Curate the new section into detailed, user-facing release notes** before the release commit — this is the published GitHub release body. Expand the scaffold's one-liners into what changed and why it matters for a user (grouped under `### Added` / `### Changed` / `### Fixed`), drawing on the commit bodies. This curation is expected every release; the scaffold is the starting point, not the deliverable.

Because the GitHub release notes are sliced from the `CHANGELOG.md` **at the release tag**, the curated section must be in the release commit (i.e. curate before `make release`, or the curation must land in the tagged commit). Curating on `master` after the tag does not change an already-published release — update it with `gh release edit vX.Y.Z --notes-file <file>` (or move the tag, only with explicit approval).

### Preferred flow: prepare → curate → finalize

`make release` in one shot generates a **terse** scaffold and immediately commits it, so the tagged notes are terse — forcing a post-hoc `gh release edit` and a second `CHANGELOG` commit (the second commit is a common source of unasked-follow-up-commit mistakes). To get **curated notes into the tagged commit with a single release commit**, use the two-step flow:

```bash
make release-prepare VERSION=X.Y.Z   # bumps version + generates the changelog scaffold, then stops
# → curate the new "## vX.Y.Z" section in CHANGELOG.md into real release notes
# → also ensure wiki/ entries for new/changed user-facing behavior exist (see above)
make release VERSION=X.Y.Z           # validates, makes the single chore(release) commit, tags, pushes
```

`release` is skip-if-already-done: it sees the version already bumped and the `## vX.Y.Z` section present, skips regenerating, and commits the **curated** changelog. No `gh release edit`, no second commit.

A one-shot `make release VERSION=X.Y.Z` (without `release-prepare`) still works for a trivial release — it bumps, generates the terse scaffold, and commits it inline, exactly as before. That target also runs `make release-check`, `make check`, creates the annotated tag `vX.Y.Z`, and pushes `master` plus the tag to both `origin` and `rad`. Run `make check` validation under Nix counts — the host toolchain can be split (see [engineering-workflow.md](../../../docs/engineering-workflow.md) Agent Day-To-Day Check Loop).

Do not use the target before completing scope-specific closure work that it cannot infer, such as roadmap text, kanban archive entries, and Radicle/Radboard issue state.

## Validation

`make release` itself runs `make release-check` **and** the full `make check` — the single mandatory gate ([ADR 0062](../../../docs/adr/0062-mandatory-test-gate-and-test-driven-loop.md)). Never substitute piecemeal subset checks (a bare `typecheck`/`test`/`build`/`clippy` run) as release validation — that posture predates ADR 0062 and is not equivalent to the gate.

For **milestone/epic closure**, run `make check-epic` (full gate + coverage ratchet) before sign-off, plus the closure-cadence suites per [DoD §I](../../../docs/engineering-workflow.md#definition-of-done-the-handover-gate): `make mutants`, and `make bench` if a hot kernel changed.

Validation counts only under Nix — the host toolchain can be silently split. A "gate green" claim needs the gate's own exit code as evidence: for a backgrounded run, grep the echoed `EXIT=` line rather than trusting a wrapper/task-notification exit code.

## Release Commit, Tag, And Remote Sync

During explicit milestone closure, the agent may create exactly one release commit limited to release wrap-up files: version files, `CHANGELOG.md`, roadmap/kanban docs, and release metadata.

Use this commit message format:

```text
chore(release): bump version to X.Y.Z
```

Do not include unrelated feature work in the release commit. If unrelated changes are present, stop and ask.

After the release commit is created, create the matching annotated release tag on that commit:

```bash
git tag -a vX.Y.Z -m "vX.Y.Z"
```

After the release commit and tag are created and validated, sync both remotes:

```bash
git push origin master
git push origin vX.Y.Z
git push rad master
git push rad vX.Y.Z
```

`origin` is the GitHub source mirror/backup and public binary mirror. `rad` is the Radicle forge remote. Pushing the `vX.Y.Z` tag to `origin` triggers the GitHub Release artifact workflow, which builds and uploads release binaries through Makefile packaging targets.

These pushes update existing remotes only; they are not permission to publish, seed publicly, change Radicle visibility, or change GitHub repository settings.

## Guardrails

- Do not use `npm version`; it can create git tags or extra metadata changes.
- Do not use `cargo set-version` unless the user explicitly approves adding that workflow.
- Keep `package-lock.json` changes limited to the root package version fields unless dependencies changed separately.
- If a version assertion test fails, update the expected Brawler version rather than weakening the test.
- Do not merge, publish, seed publicly, change repository visibility, or rewrite history unless the user explicitly asks.
