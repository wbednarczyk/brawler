# ADR 0008: License and Project Governance

## Status

Superseded in part by [ADR 0023](0023-public-private-documentation-split.md) and the public-opening governance work.

The historical private/proprietary posture in this ADR is no longer the target public posture. Use this ADR as background context only.

## Context

Brawler keeps future monetization options open through an open-core posture. The historical lack of a license was intentional while the repository was private.

## Decision

Brawler used a private proprietary posture at this point in the project. Public-opening work later chose MPL-2.0 for the open desktop core.

Open core plus paid convenience features is the public posture. Detailed business experiments remain owner-only context.

Docs, ADRs, and contracts are canonical for product and architecture decisions. Radicle issues rendered through Radboard are used for active project tracking, but no important decision should live only in issue or PR comments.

## Consequences

- External contribution was not expected while no license existed.
- The public source license is MPL-2.0 after the public-opening decision.
- Detailed monetization strategy remains outside public docs.
- Radicle/Radboard replaces `docs/kanban.md` as the high-level active planning board.
- Radboard milestones track app version targets, epics track major capability slices, and tasks track reviewable implementation work.
- Deferred bugs are tracked as Radicle issues with the `bug` label so they are visible in Radboard.
- PRs should reference a Radicle issue when implementation begins.
