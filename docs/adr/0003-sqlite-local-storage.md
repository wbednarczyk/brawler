# ADR 0003: SQLite Local Storage

Status: Accepted

## Context

V1 needs reliable local persistence for watchlists, companies, source state, feed items, jobs, settings, and AI outputs. The database should be portable, queryable, and simple to operate in a desktop application.

## Decision

Use SQLite as the v1 local datastore.

Schema changes must be migration-managed from the first code milestone.

## Consequences

- The app can run without external infrastructure.
- Backups and portability are straightforward.
- Querying feed history and watchlists is easier than a files-first approach.
- Future cloud sync must map from the local SQLite model to an explicit sync contract.
