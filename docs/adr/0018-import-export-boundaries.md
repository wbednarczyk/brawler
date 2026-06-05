# ADR 0018: Import And Export Boundaries

## Status

Accepted for M20 implementation.

## Context

M20 adds import/export for companies, watchlists, notebook entries, and non-secret settings. The feature must stay local-first, avoid exposing secrets, and leave a clean path for future full backup, restore, cloud sync, and alternate file formats.

The project-wide architecture principle is that modules should be extensible behind clear logical boundaries. Import/export is likely to grow beyond the first sections, so the first implementation should not hard-code one file parser directly into UI commands.

## Decision

- Import/export is implemented as a dedicated domain with format, validation, preview/planning, apply, storage-operation, command, and UI workflow boundaries.
- M20 exposes separate user-facing flows for research data and settings, backed by shared section-adapter internals.
- Research data exports use JSON in M20 and include companies, watchlists, memberships, and notebook entries.
- Settings exports use YAML in M20 and include only allowlisted non-secret settings.
- Import always previews before apply. Apply is transactional for each import operation.
- Company import matches existing records by exchange-qualified ticker. Existing local fields win; missing optional fields may be filled from import data.
- Watchlist import preserves imported IDs when they are absent locally. Existing IDs merge memberships while keeping local name and description.
- Watchlist memberships must resolve to an existing company, a company included in the same import, or an explicit future repair result. M20 does not create placeholder companies.
- Notebook entries are imported for existing companies or companies included in the same import. Duplicate note IDs are skipped with a preview warning.
- Imported notebook origins preserve source URL and label metadata even when referenced feed items or transcript segments are not part of the import.
- Provider secrets, license tokens, private signing material, logs, diagnostics, metrics, feed items, transcripts, and full backup data remain out of M20 scope.
- React handles file upload/download content through typed commands. The backend does not receive broad filesystem access in M20.

## Consequences

- The first implementation is more structured than a direct JSON dump, but it avoids a later rewrite when more sections, bundles, or sync adapters are added.
- M20 is not a full backup/restore feature. Feed items, transcripts, logs, diagnostics, metrics, credentials, and license secrets remain excluded.
- Ticker-change recovery is limited. If an exported company record is present, import can restore it. If only a membership reference remains and lookup cannot resolve it, the membership is blocked or skipped with a warning.
- A later bundle adapter can combine existing section adapters without changing the core validation/apply model.
