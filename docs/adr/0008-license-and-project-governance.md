# ADR 0008: License and Project Governance

## Status

Accepted

## Context

Brawler is intended to keep future monetization options open, but the exact license and commercial boundary are not ready to decide. The repository is currently private on GitHub, so the lack of a license must be intentional and documented.

## Decision

Brawler is all rights reserved for now. No open-source license will be added until a future ADR resolves the license posture and commercial boundary.

The future monetization model is undecided. Open core plus paid convenience features is a possible path, but not a committed direction.

Docs, ADRs, and contracts are canonical for product and architecture decisions. GitHub Issues may be used for implementation tracking, but no important decision should live only in issue or PR comments.

## Consequences

- External contribution is not expected while no license exists.
- License must be revisited before public releases or accepting outside contributions.
- License and monetization must be revisited before making the repository public.
- `docs/kanban.md` remains the high-level planning board.
- PRs should reference an issue or Kanban card when implementation begins.
