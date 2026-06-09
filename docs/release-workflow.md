# Release Workflow

Use [Project Brief](project-brief.md) for the full documentation map. Related references: [Project Practices](project-practices.md), [Engineering Workflow](engineering-workflow.md), [Kanban](kanban.md), and [Kanban Archive](kanban-archive.md).

## Intent

Release workflow should make Brawler's version history, changelog, and commit discipline predictable without turning normal development into ceremony.

## SemVer Policy

Brawler uses SemVer-style `0.x.y` versions before `1.0.0`.

Rules:

- Milestone releases bump the minor version, for example `0.24.0` to `0.25.0`.
- Patch releases bump the patch version, for example `0.24.0` to `0.24.1`.
- Prerelease and build metadata are allowed only when there is a concrete packaging or testing need, for example `0.25.0-rc.1`.
- `1.0.0` waits until the app is stable enough for external users.
- Version files must stay synchronized:
  - `package.json`
  - `package-lock.json`
  - `src-tauri/Cargo.toml`
  - `src-tauri/Cargo.lock`
  - `src-tauri/tauri.conf.json`
- Milestone version bump still requires manual user signoff before closure.

## Commit Convention

New commits use Conventional Commits:

```text
<type>(optional-scope): <subject>
```

Allowed types:

- `feat`
- `fix`
- `docs`
- `test`
- `refactor`
- `perf`
- `build`
- `ci`
- `chore`
- `style`

Examples:

```text
feat(research): add company evidence timeline
fix(sources): include NewConnect lookup results
docs(release): document changelog workflow
chore(release): bump version to 0.25.0
```

Breaking changes use `!`:

```text
feat(api)!: change research timeline result shape
```

## Local Commit Enforcement

The repository uses native Git hooks, not the external `pre-commit` framework.

Reason:

- commit message validation belongs in `commit-msg`, because `pre-commit` runs before the message exists
- native hooks keep the dependency footprint small
- the hook is inspectable and works in WSL without hosted services

Install hooks in a checkout:

```bash
make install-git-hooks
```

This runs:

```bash
git config core.hooksPath .githooks
```

The hook delegates to:

```text
scripts/release/validate-commit-message.sh
```

## Changelog

`CHANGELOG.md` is the user/reviewer-facing release history.

Historical entries through `0.24.1` are curated from `docs/kanban-archive.md` because early commits predate the commit convention. Future entries are generated with `git-cliff` from Conventional Commits and may be edited before release for clarity.

Commands:

```bash
make changelog
make changelog-check
```

`git-cliff` is provided by:

- project `flake.nix`, for reproducible repo-local development
- the owner's separate `../setup-workstation/` workstation setup, outside this repo

Agents must not edit `../setup-workstation/` unless the user explicitly asks for that repository to be changed.

## Release Checks

Run:

```bash
make release-check
```

This validates:

- commit message convention samples
- synchronized version files
- `git-cliff` changelog generation

Run `make check` separately for product code validation.

## Retroactive Tags

Old commits are not rewritten. Historical commit messages remain as-is.

Retroactive version tags are allowed after auditing the exact release commit for each version. Tags should be annotated and local first. Push tags only after manual user approval.

Recommended tag shape:

```bash
git tag -a v0.24.1 <commit> -m "Brawler 0.24.1"
```

Historical release commits can usually be identified from merge commits and version-bump commits, but each tag must be checked before creation.
