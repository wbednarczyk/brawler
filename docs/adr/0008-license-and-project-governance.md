# ADR 0008: License and Project Governance

## Status

Accepted

## Context

Brawler is intended to keep future monetization options open, but the exact license and commercial boundary are not ready to decide. The repository is currently private on GitHub, so the lack of a license must be intentional and documented.

## Decision

Brawler is all rights reserved for now. No open-source license will be added until a future ADR resolves the license posture and commercial boundary.

The future monetization model is undecided. Open core plus paid convenience features is a possible path, but not a committed direction.

Docs, ADRs, and contracts are canonical for product and architecture decisions. Radicle issues rendered through Radboard are used for active project tracking, but no important decision should live only in issue or PR comments.

## Consequences

- External contribution is not expected while no license exists.
- License must be revisited before public releases or accepting outside contributions.
- License and monetization must be revisited before making the repository public.
- Radicle/Radboard replaces `docs/kanban.md` as the high-level active planning board.
- Radboard milestones track app version targets, epics track major capability slices, and tasks track reviewable implementation work.
- Deferred bugs are tracked as Radicle issues with the `bug` label so they are visible in Radboard.
- PRs should reference a Radicle issue when implementation begins.
