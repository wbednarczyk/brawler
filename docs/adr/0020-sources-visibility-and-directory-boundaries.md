# ADR 0020: Sources Visibility And Company Directory Boundaries

Status: Accepted

## Context

Milestone 22 changes Sources from an implementation inventory into a normal-user trust and control surface. The app also adds the NewConnect company directory source and needs to keep later company-directory sources decoupled from lookup, feed ingestion, and source-candidate planning.

## Decision

Normal Sources shows implemented sources only. It does not show disabled placeholders, unimplemented candidates, source IDs, fetch modes, rate-limit notes, policy notes, or unmatched diagnostics as normal-user content.

Source visibility has three tiers:

- `required`: required application support sources. These are visible and refreshable, but not user-disableable.
- `optional`: implemented runtime sources. These are visible, refreshable, and user-enableable/user-disableable.
- `developer`: candidates, placeholders, review-only sources, and implementation diagnostics. These are hidden from normal Sources and visible only in Developer mode and docs.

Source enablement is persisted through the source adapter state already owned by local storage. Batch refresh and scheduled refresh must respect optional source enablement. Required sources are protected from normal-user disabling. Developer-tier sources cannot be enabled from normal UI.

Company-directory sources are a separate source category from feed, event, media, and research sources. The GPW company list and NewConnect company list are presented as company-directory/lookup support. Each directory uses a separate source adapter behind the same cache and UI boundary, with stable source IDs, market-specific matching, source attribution, active/inactive listing state, and exact identifier matching where available.

Developer mode may expose source IDs, source type, fetch mode, policy notes, candidate status, and unmatched diagnostics because those are troubleshooting and owner/developer concerns.

## Consequences

- The normal Sources command returns required and optional sources by default.
- Developer tooling can request developer-tier sources explicitly.
- Optional source toggles affect real ingestion behavior, not only UI state.
- The app keeps source candidate planning visible in docs and Developer mode without making unimplemented features look available to normal users.
- NewConnect is implemented as `newconnect-company-directory` over the existing company-directory cache, using `NC:<ticker>` company identity and the official NewConnect company list.
- Later company directories should follow the same adapter boundary instead of being treated as generic feed sources.
