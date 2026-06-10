# Brawler Release

Use this workflow only from the Brawler repository root when the user explicitly asks to wrap up, close, or release a milestone/epic.

## Release Boundary

The normal workflow is:

1. The user commits feature work.
2. The user explicitly asks the agent to wrap up or close the milestone.
3. The agent performs release-only changes and may create exactly one release commit.
4. The agent creates the matching annotated release tag for that release commit.

Do not merge, push, publish, seed publicly, or rewrite history unless the user explicitly asks for that operation.

## Preconditions

Before making release changes:

- Confirm the user has explicitly signed off on milestone closure.
- Check `git status --short`.
- If unrelated dirty files exist, stop and ask which files belong in the release wrap-up.
- Confirm the target version from the milestone number unless the user specified another version.
  - Milestone 9 closes as `0.9.0`.
  - Milestone 10 closes as `0.10.0`.

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

- `docs/roadmap.md`: add or update `Status: completed in `X.Y.Z`.` under the milestone heading.
- `docs/kanban.md`: move completed milestone work to the correct completed/archive state and record the version bump where the project currently tracks delivered work.
- Radicle/Radboard issues: close or relabel the relevant milestone/epic/task issues, using existing project label conventions.

## Changelog Rule

Do not hand-edit `CHANGELOG.md` as the changelog generation step.

Use the Makefile target dedicated to changelog generation:

```bash
make changelog
```

After running the target, review the generated output for obviously broken formatting or incorrect release boundaries. If the Makefile-generated output is wrong, fix the release tooling or ask the user before manually curating the changelog.

## Validation

Run release validation with the repo Makefile first:

```bash
make release-check
```

Then run the relevant project checks with RTK-filtered commands unless the release is docs-only and `make release-check` is sufficient for the changed files:

- `rtk npm run typecheck`
- `rtk npm test -- --run`
- `rtk npm run build`
- `rtk cargo fmt --check` from `src-tauri`
- `rtk cargo clippy --all-targets -- -D warnings` from `src-tauri`
- `rtk cargo test` from `src-tauri`

If only docs changed after checks, rerun only checks affected by later edits.

## Release Commit And Tag

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

Do not push the commit or tag unless the user explicitly asks.

## Guardrails

- Do not use `npm version`; it can create git tags or extra metadata changes.
- Do not use `cargo set-version` unless the user explicitly approves adding that workflow.
- Keep `package-lock.json` changes limited to the root package version fields unless dependencies changed separately.
- If a version assertion test fails, update the expected Brawler version rather than weakening the test.
- Do not push, merge, publish, seed publicly, or rewrite history unless the user explicitly asks.
