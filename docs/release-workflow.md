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

Agents own changelog updates during milestone and patch closure. The project owner should not need to remember to run changelog commands manually during normal signoff. During wrap-up, the agent should generate the changelog entry, review/edit it for clarity, and include the updated `CHANGELOG.md` in the closure changes.

`make changelog` reads Git commit history. It does not see uncommitted working-tree changes. The normal closure workflow is commit-first:

1. The project owner commits all feature, fix, refactor, test, and documentation work for the epic or milestone.
2. The project owner asks the agent to close or wrap up the epic or milestone.
3. The agent confirms the feature work is committed before generating the release changelog.
4. The agent performs the closure-only work:
   - bump synchronized version files
   - run `make changelog`
   - review/edit the generated `CHANGELOG.md` entry for clarity
   - run release and relevant validation checks
   - update Radicle/Radboard issue state
   - create the final release commit
   - create the matching annotated release tag
   - push the release commit and tag to both `origin` and `rad`
5. The release commit message is:

   ```text
   chore(release): bump version to x.y.z
   ```

Do not use `make changelog` as proof of uncommitted feature work. If the feature work is still uncommitted, the agent should stop closure and ask the project owner to commit it first.

Release remote sync updates both project remotes:

```bash
git push origin master
git push origin vX.Y.Z
git push rad master
git push rad vX.Y.Z
```

`origin` is the GitHub read-only mirror/backup. `rad` is the Radicle forge remote. Release sync does not authorize `rad publish`, `rad seed`, public seeding policy changes, repository visibility changes, or other publication operations; those still require separate explicit owner approval.

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

## Wrap-Up Commit Message

When an epic or milestone is wrapped up and the app version is bumped, the agent must propose a Conventional Commit message for the project owner to use.

Rules:

- Use `chore(release): bump version to x.y.z` for pure closure/version-bump commits.
- Use `feat(<scope>): ...` when the commit primarily delivers user-visible capability.
- Use `fix(<scope>): ...` for patch releases.
- Use `build(release): ...` for release workflow, packaging, or changelog infrastructure.
- Mention the proposed commit message in the final wrap-up response after validation results.

## Retroactive Tags

Old commits are not rewritten. Historical commit messages remain as-is.

Retroactive version tags are allowed after auditing the exact release commit for each version. Tags should be annotated and local first. Push tags only after manual user approval.

Recommended tag shape:

```bash
git tag -a v0.24.1 <commit> -m "Brawler 0.24.1"
```

Historical release commits can usually be identified from merge commits and version-bump commits, but each tag must be checked before creation.
