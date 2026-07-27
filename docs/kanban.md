# GitHub Issues & Board Tracking

Active project tracking lives in **GitHub Issues + the "Brawler board" Project** ([ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md)), not in this file. This file is only the tracking pointer and label/state conventions:

- **Milestone intent and the active/upcoming plan** → [Roadmap](roadmap.md).
- **Delivered/release history** → [CHANGELOG.md](../CHANGELOG.md) and [Kanban Archive](kanban-archive.md).
- **Live epic and task status, and their IDs** → GitHub Issues + the "Brawler board" Project.

## Pointer

- Repository: `github.com/wbednarczyk/brawler` (canonical for code, issues, board, CI, releases).
- Board: the **"Brawler board"** Project (Projects v2).
- Second path: Radicle (`rad:z3yTYrLFsFx5qcPtV3XiFYFBpQWuh`) is a code mirror only — `make sync-rad` pushes master + tags asynchronously; no process depends on it. Historical `rad:<hex7>` issue references resolve via `docs/archive/radicle-issue-map.json`.

## Conventions

| Concern | Home | Notes |
| --- | --- | --- |
| **State** | the board `Status` field | `Backlog / Ready / In progress / Review / Done`. **The only home of state — never a `state:*` label** (SSOT; state in two places guarantees drift). |
| Priority | label | `priority:critical\|high\|medium\|low`. |
| Area | label | e.g. `area:fundamentals`, `area:research-workspace`, `area:release-workflow`, `area:packaging` (repeated flags, never comma-joined). |
| Epic | `epic` label + native **sub-issues** | Major capability issues; tasks attach as GitHub sub-issues (replacing `parent:<hex7>`). |
| Release increment | `release:*` label on the PR | Exactly one of `release:major\|minor\|patch\|skip` per PR (required check); the merge ships the version ([ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md) § 5). Owner's decision — agents never set it to force a release. |
| Milestone | native GitHub milestone | Migrated `milestone:vX.Y.Z` labels are **historical grouping only** — new work groups by epics + sub-issues, not version milestones. |
| Type | `bug` label | Deferred bugs; state via the board, priority/area via labels; `blocked:<n>` as a body link. |

## Commands

- `gh issue list` / `gh issue list --label epic` — open issues / open epics.
- `gh issue view <n>` — a specific issue.
- `gh project item-list <project> --owner <owner>` — the board with `Status`.
- `gh project item-edit ...` — move a card's `Status` (agents may run reads unattended; a merge/settings mutation is owner-only).
- `gh issue close <n> --reason completed` — close a delivered task/epic (project automation moves it to Done).

Create a GitHub issue for every reported or discovered bug that will not be fixed immediately in the current work.

## Cut-over

Tracking moved from Radicle to GitHub on **2026-07** ([ADR 0090](adr/0090-github-canonical-forge-and-continuous-release.md)): the ~390 open issues were migrated (full ~550 exported to `docs/archive/radicle-issues-2026-07.jsonl`; `hex7 → #n` map in `docs/archive/radicle-issue-map.json`); Radicle issues are frozen as history.
