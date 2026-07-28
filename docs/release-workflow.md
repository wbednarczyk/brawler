# Release Workflow

Doc map: [CLAUDE.md](../CLAUDE.md) § Required Reading. Related: [Engineering Workflow](engineering-workflow.md), [Kanban](kanban.md), [Kanban Archive](kanban-archive.md), [ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md). The epic-closure runbook is the [brawler-release skill](../.claude/skills/brawler-release/SKILL.md); this doc is the policy the repo enforces.

## Model: continuous release driven by a PR label ([ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md))

**Every merged PR carrying a release label is a new version and a public release.** There is no "cut a release" step, no `make release`, no release PR, no hand-curated changelog. Since every merge passes the full CI gate + a real Windows boot-smoke, every merge is a working app — so it can ship.

- **Each PR carries exactly one `release:*` label** — `release:major` / `release:minor` / `release:patch` / `release:skip` (non-releasable: docs, CI, user-invisible refactor). The **owner decides** the increment; the `release-label` required check reddens on zero or more than one label. **Agents never set/change a `release:*` label to force a release.**
- **Tags drive the version; the repo always shows the released version** (ADR 0090 amendment, 2026-07-28). `release.yml` computes the next version = last tag + the merged PR's label increment, `make package-release-artifacts VERSION=x.y.z` injects it into the binary/UI/manifests at build time, and after tagging the bot commits the same stamp back to master so the repo manifests never trail a release for more than minutes. Guardrail: `check-version-sync.sh` also fails when the manifest version differs from the newest reachable `v*` tag.
- **`release.yml`** (trigger: `push` on master, + `workflow_dispatch` for re-runs; amended 2026-07-28 — no gate re-run, parallel cached builds):
  1. **detect** — find the PR by SHA (`gh api commits/:sha/pulls`), read the label. `release:skip` ends here (no test, no tag — the up-to-date ruleset already proved the tree). **The same argument retires the pre-release gate re-run for labeled merges**: required checks green + branch up-to-date make the pushed master tree bit-identical to the PR-tested tree, so the release starts building immediately (~8 min less latency; ADR 0090 Decision 2 amendment).
  2. **version** — last tag + label increment.
  3. **build-linux ∥ build-windows** — the two artifact builds run in parallel on two runners (`make package-release-linux` / `package-release-windows`; version stamped at build time). Per-target `rust-cache` is **saved on every master build**, so a release recompiles only what changed since the previous one (`make package-release-artifacts` stays the sequential local-parity path).
  4. **publish** — download both artifact sets, preflight the stamp-push path, and **only after both successful builds** tag → GitHub Release + artifact upload + auto-notes (a "tag exists → skip" guard makes re-runs idempotent, so ordering never orphans a tag). Local recovery parity: `make release-publish VERSION=… EXE=…`.
- **`CHANGELOG.md` stays the canon, written by the bot.** After a successful tag, the release job generates the `previous-tag..tag` entry with git-cliff and commits it **together with the version stamp** (`chore(release): vX.Y.Z [skip ci]`; `[skip ci]` prevents a loop; each push attempt regenerates the commit on a fresh `origin/master`, so merge races cannot conflict). The same text is the GitHub Release body. The stamp/changelog commit for X naturally lands one commit after tag X — cosmetic, by design. The stamp step runs **unconditionally** (also on republish recovery runs) — it is idempotent, and a recovery run must be able to heal a stamp its failed predecessor never landed (the v0.61.2 hole).
- **The stamp push rides the release-stamp deploy key** (v0.61.3 postmortem, 2026-07-28): `GITHUB_TOKEN` cannot bypass the master ruleset on a personal repo, so the stamp pushes over SSH with the key in the `RELEASE_STAMP_DEPLOY_KEY` secret, and the "master protection" ruleset grants the **DeployKey** bypass. Required one-time repo config (both halves): a write-enabled deploy key titled `release-stamp (release.yml bot push)` whose private half is that secret, and the ruleset bypass entry. A **preflight step fails the release before the tag exists** when the push path is broken — a shipped-but-unstamped release (tag/Release published, master never told) must be impossible. The preflight probes **capability, not configuration**: it SSH-authenticates the secret's key against github.com and requires the deploy-key banner to name this repo (the ruleset's `bypass_actors` is admin-only — unreadable to `GITHUB_TOKEN`, which false-negatived the v0.61.5 rerun; that API read stays only as a best-effort positive-failure check).
- **Radicle mirror is asynchronous**: `make sync-rad` (push master + tags to `rad`) whenever the owner finds it convenient — a code mirror, not a process step.

The human/agent role shrinks to: **the right `release:*` label on the PR** (owner) + **`make sync-rad`** when convenient. Everything else is automatic.

## Commit convention (commit messages ARE the release notes)

Commits use Conventional Commits `<type>(optional-scope): <subject>`. Allowed types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`. Single `[a-z0-9._-]+` scope (no commas). **No subject-length limit** (dropped 2026-07-27, ADR 0090) — the schema stays, enforced by `commit-lint` in CI (every commit in the PR) and the local `commit-msg` hook (`scripts/release/validate-commit-message.sh`). Breaking changes use `!`. git-cliff turns these into release notes (`feat`/`fix`/`perf` surface; `chore`/`refactor`/`test`/`docs` filtered), so a good commit subject IS the release note — there is no curation pass.

Examples:

```text
feat(research): add company evidence timeline
fix(sources): include NewConnect lookup results
feat(api)!: change research timeline result shape
```

## SemVer

Brawler uses SemVer-style `0.x.y` before `1.0.0`. The `release:*` label maps to the increment: `major` → `x`, `minor` → `y`, `patch` → `z`. `1.0.0` waits until the app is stable enough for external users. Prerelease/build metadata only for a concrete packaging/testing need.

## Epic closure (retro cadence)

Under continuous release, **versions are not "closed" — epics are.** Closing an epic runs the [brawler-release skill](../.claude/skills/brawler-release/SKILL.md): retrospective (both domains, still-open items honest) presented inline to the owner, the [Definition of Done §I](engineering-workflow.md#definition-of-done-the-handover-gate) audit, `wiki/` confirmed updated (it is updated **in the behavior-changing PR**, not at release), and `gh issue close <n> --reason completed` for the delivered task/epic issues (the board automation moves them to Done). The owner signs off before the tracking is closed.

## Release artifacts

`release.yml` builds Linux `amd64` `.deb`/`.rpm`/`.AppImage` and the Windows `x64` portable via the Linux `cargo-xwin` path, on standard `ubuntu-latest`, and uploads them to the matching GitHub Release. GitHub Releases are the durable public binary download surface; Actions artifacts are temporary. `git-cliff` is provided by the project `flake.nix`.

## Retroactive tags (historical)

Pre-continuous-release tags were annotated and pushed after auditing the exact release commit for each version. Old commits are not rewritten; historical `chore(release)` commits and their tags remain valid history.
