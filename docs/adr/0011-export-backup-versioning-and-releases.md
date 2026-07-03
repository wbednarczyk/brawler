# ADR 0011: Export, Backup, Versioning, and Releases

Status: Accepted

## Context

Brawler will hold personal research data. The project also needs clear pre-v1 versions for builds, migrations, packaging, and debugging.

## Decision

Export is part of normal v1 implementation. Notes export as Markdown with metadata. Watchlists, companies, and settings export as structured JSON or YAML, with settings export excluding secrets.

Import/restore and full local backup are late-v1 roadmap items. Cloud backup/sync is a future feature and requires a separate design discussion and ADR.

Brawler uses SemVer-style `0.x.y` versioning from the first scaffold.

Initial version mapping:

- `0.1.0`: desktop shell, theme, health command
- `0.2.0`: SQLite/storage, companies, watchlists, sample feed
- `0.3.0`: inbox and company workspace
- `0.4.0`: notebooks and claims
- `0.5.0`: GPW adapter

## Consequences

- Export should be built alongside the related feature areas instead of deferred entirely.
- Full restore and backup should not block the early core product.
- Git tags mark meaningful build candidates.
- Public release automation waits until packaging is ready.
- A changelog starts once code exists.
