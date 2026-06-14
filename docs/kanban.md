# Radicle/Radboard Tracking

Active project tracking lives in **Radicle issues rendered by Radboard**, not in this file. This file is only the tracking pointer and label conventions:

- **Milestone intent and the active/upcoming plan** → [Roadmap](roadmap.md).
- **Delivered/release history** → [CHANGELOG.md](../CHANGELOG.md) and [Kanban Archive](kanban-archive.md).
- **Live epic and task status, and their IDs** → Radicle/Radboard (`rad issue list --all`).

## Radicle Pointer

- Radicle repository: `rad:z3yTYrLFsFx5qcPtV3XiFYFBpQWuh`
- Visibility: public
- Public seed: `seed.mikolajczyk.org:8776`
- Public seeding policy: owner node syncs releases to the public seed

## Radboard Conventions

- Milestones: app version labels such as `milestone:v0.38.0`.
- Epics: major capability issues labeled `epic`, one per milestone.
- Tasks: reviewable work-slice issues linked to their epic with `parent:<epic-hex7>`.
- Bugs: deferred bugs labeled `bug`, with `state:*`, `priority:*`, and an `area:*` label; link with `parent:<epic-hex7>` or `blocked:<bug-hex7>` when relevant.
- Use repeated label flags only (never comma-joined), e.g. `area:fundamentals`, `area:research-workspace`, `area:release-workflow`, `area:packaging`, `state:*`, `priority:critical|high|medium|low`.

Use `rad issue list --all` or Radboard for the live epic and task list and status. Create a Radicle issue for every reported or discovered bug that will not be fixed immediately in the current work.
