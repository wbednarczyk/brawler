# ADR 0032: Global Search, Local Backup, And Connection-Pool Boundaries

Status: Accepted

> **Update (2026-07-03):** the backup boundary gains an **offsite destination**: rotating
> backups can additionally target a user-chosen folder (e.g. a user-synced drive), same
> rotation and no-secrets rules, scheduled with Import/export v2 (`v0.52.0`). Local-first
> and "no cloud services" stand — the app only writes to a local path the user picks.

## Context

Milestone `v0.38.0` ("Search and data safety hardening", epic `2cc8bd6`) closes three gaps in the local data layer:

- **No content search.** The growing research corpus — feed items, notebook entries, transcript segments, AI research briefs, and digests — is only reachable by navigating to the workspace that owns it. The only text matching in storage today is a narrow `LIKE` company-name lookup (`companies.rs`) and a couple of id-prefix matches. [docs/data-model.md](../data-model.md) → "Search Inputs" deliberately left room for full-text search across company identity, feed item title/body, notebook title/body, and transcript text, and stated "SQLite FTS can be added after the base schema is stable." [docs/ui-information-architecture.md](../ui-information-architecture.md) → "Search" keeps search workspace-scoped and states the top toolbar "must not show a search box until a true cross-workspace result model exists."
- **No data safety net.** Migrations run as a single all-or-nothing rusqlite transaction with no snapshot taken first, and there is no automatic backup of `brawler.sqlite3`. A bad migration or corruption against real local data is unrecoverable.
- **No real concurrency.** The app holds a single `Arc<Mutex<Connection>>`; every read and write is serialized, so a long background job (AI analysis, transcription, source refresh) blocks the UI's reads. This is the mild concurrency debt that WAL alone does not fix — WAL needs multiple connections to deliver concurrent readers.

The database is otherwise healthy (normalized, FK-enforced, versioned embedded migrations). The owner is the only user and has accepted breaking changes in favor of forward-looking design, so this milestone modernizes the data layer (WAL, backups, a connection pool) rather than only bolting features onto the single-connection model. Semantic/embedding search and cloud backup are explicitly out of scope.

## Decision

### Search

1. **One unified FTS5 index.** A single standalone FTS5 virtual table holds the searchable text for every content type, with unindexed discriminator columns:

   ```sql
   CREATE VIRTUAL TABLE search_index USING fts5(
     title,
     body,
     content_type UNINDEXED,   -- 'company' | 'watchlist' | 'feed_item' | 'notebook_entry'
                               -- | 'transcript_segment' | 'event' | 'research_brief' | 'digest'
     source_id    UNINDEXED,   -- primary key of the owning source row
     company_id   UNINDEXED,   -- canonical company for scoping/grouping (nullable)
     tokenize = 'unicode61 remove_diacritics 2'
   );
   ```

   A single `MATCH` query ranks across all content with `bm25()`; the UI groups results by `content_type`. The index is **derived state**: it is populated by per-source `AFTER INSERT/UPDATE/DELETE` triggers and is fully rebuildable from the source tables, so schema evolution rebuilds rather than migrates the index. Rejected: per-source external-content FTS tables — they avoid text duplication but force a `UNION ALL` across N tables with no single bm25 ordering, a poor fit for one ranked cross-workspace result list.

2. **Tokenizer `unicode61 remove_diacritics 2`.** Language-neutral word tokenization with diacritic and case folding (so `spolka` matches `Spółka`), giving correct bm25 ranking for the Polish-primary, English-mixed corpus. Rejected: `porter` (English-only stemming, mangles Polish); `trigram` (substring matching but weak word ranking and a larger index) — revisit only if substring/infix matching becomes a requirement.

3. **Coverage = companies, watchlists, feed items, notebook entries, transcript segments, company events, research briefs, and digests.** Every stored entity with meaningful user-facing text and a navigation target is indexed (the guiding rule: searchable wherever it makes sense). Companies are indexed into the same unified table (ticker + display name into `title`) so they appear in global results alongside content; the existing dedicated company lookup stays for autocomplete. Watchlists (added in v0.38.0) and company events (added in v0.38.0) joined after the initial six types; transcript segments navigate to their owning transcript job via `parent_id`.

4. **One typed search boundary.** A single search command queries the index. Query text is sanitized before it reaches `MATCH` (user input is never interpolated as FTS5 syntax); results are ranked, carry a `snippet()`, `content_type`, and `company_id`, and support type/company scoping filters. The DTOs live in a focused `src/api/search.ts`; command modules contain no SQL. See [contracts.md](../contracts.md) → "Global Search".

5. **Global search affordance is unblocked.** The [ui-information-architecture.md](../ui-information-architecture.md) constraint forbidding a top-toolbar search box is lifted: v0.38 builds the cross-workspace result model the constraint waited for. A global, keyboard-reachable search box is added in the top toolbar; the existing per-workspace search/filter inputs (Inbox, Companies, Notebooks) remain.

### Backup and data safety

6. **`VACUUM INTO` is the single backup primitive.** Both automatic backups and pre-migration snapshots produce a consistent, compacted copy with one statement (`VACUUM INTO '<path>'`), safe to run on the live connection. Rejected: raw file copy (unsafe), and the online-backup API with hot restore (more moving parts, risky while jobs run).

7. **WAL mode.** The database runs in `PRAGMA journal_mode = WAL`, which enables concurrent readers alongside a single writer and is what makes the connection pool (below) worthwhile; `VACUUM INTO` remains correct under WAL. This is a one-time, owner-accepted change to on-disk journaling (a `-wal`/`-shm` sidecar appears).

8. **Pre-migration snapshot is mandatory.** Before the migration runner applies any pending migration, it writes a snapshot via `VACUUM INTO` named with the schema version and timestamp. If the snapshot cannot be written, migration is aborted with a clear error and no schema change is attempted. A failed migration leaves the snapshot intact for manual restore.

9. **Rotating automatic backups.** Periodic and on-close backups are written to `<app_data_dir>/backups/`, keeping the last N (rotation prunes the oldest). Backup status (last backup time, count) is visible. Pre-migration snapshots are retained alongside rotating backups.

10. **Restore is a restart operation, surfaced in Diagnostics.** Restore is offered from the Diagnostics screen with explicit confirmation. Because live connections hold the database open, restore stages the chosen backup and requires an app relaunch to swap it in; it does not attempt a hot in-place swap. This matches the roadmap's "restore safety while the app is running" concern.

11. **Backups are local-only and distinct from import/export.** A backup is a byte-faithful copy of `brawler.sqlite3` only; it is separate from the M20 import/export documents (which are portable, redacted, and exclude secrets). Secrets remain in the OS keychain and are never in the database, so they are absent from backups by construction. No cloud backup.

### Connection model and concurrency

12. **A connection pool replaces the single mutex'd connection.** `AppState` holds an `r2d2` pool (`r2d2` + `r2d2_sqlite`, small mature deps justified by this need) instead of `Arc<Mutex<Connection>>`. Under WAL this gives concurrent readers and keeps background jobs from blocking UI reads. The pool is **uniform** — any connection may read or write — and write contention is absorbed by `busy_timeout` (SQLite still permits only one writer at a time; a second writer waits rather than erroring). Rejected: a dedicated-writer + reader-pool split (more structure than the single-user workload needs now).

13. **Per-connection pragmas and async execution.** Each pooled connection sets `journal_mode = WAL`, `foreign_keys = ON`, and `busy_timeout` on creation. rusqlite is synchronous, so command handlers run DB work on blocking tasks (`spawn_blocking`), each checking out its own connection — consistent with the "AI/IO async by default" direction in [ADR 0028](0028-multi-provider-ai-boundary.md).

14. **Bootstrap ordering.** Settings live in the database but the pool needs configuration from settings, so startup uses a single bootstrap connection that (a) runs pending migrations, (b) writes the pre-migration snapshot, and (c) reads pool configuration — then the pool is built from that configuration and handed to the app. Migrations, snapshots, and restore staging run outside the pool (at startup/shutdown), where they have the database to themselves.

15. **Pool settings are user-configurable, restart-applied, and fail-safe.** `maxConnections`, `busyTimeoutMs`, and `acquireTimeoutMs` are exposed in a new **Settings → Database** section through the existing typed settings model. Because the pool is constructed at startup, changes persist immediately but take effect on the next launch (the UI states this). Stored values are validated and clamped to safe ranges, falling back to defaults if missing or invalid, so a bad value can never prevent the database from opening. A reset-to-defaults action is provided. See [contracts.md](../contracts.md) → "User Settings" and [data-model.md](../data-model.md).

## Consequences

- Any stored text becomes findable from one global affordance with relevance ranking and snippets; the workspace-scoped searches still serve their local lists.
- The search index is additive and rebuildable: schema changes drop and rebuild `search_index` rather than requiring index-shape migrations.
- Every migration is now preceded by a restorable snapshot; a destructive migration is recoverable.
- WAL plus a pool removes the serialized-access debt: background jobs and the UI read concurrently. Writes still serialize (SQLite invariant), absorbed by `busy_timeout`.
- WAL changes the on-disk format (a `-wal`/`-shm` sidecar appears); acceptable for the single local user.
- Restore requiring a restart is a deliberate safety tradeoff over a hot swap.
- The `Arc<Mutex<Connection>>` → pool change is a cross-cutting refactor of every storage call site; it is taken now while the data layer is being hardened rather than later.
- New runtime dependencies (`r2d2`, `r2d2_sqlite`) are added, justified by the concurrency requirement and kept conservative.
- New canonical-doc homes: search command + DTOs and the new pool settings keys in [contracts.md](../contracts.md); `search_index`, snapshot, backup, WAL, and the pool/bootstrap model in [data-model.md](../data-model.md); search-index, backup/snapshot, and connection-pool boundaries in [architecture.md](../architecture.md); the rewritten "Search" section, the Settings → Database section, and the Diagnostics restore flow in [ui-information-architecture.md](../ui-information-architecture.md) and [ui-flows.md](../ui-flows.md); user-facing search, backup/restore, and database settings in [product-spec.md](../product-spec.md); test, restore-restart, and pool steps in [engineering-workflow.md](../engineering-workflow.md).
- Out of scope and deferred: semantic/embedding search, cloud backup/sync, and feed retention (designed under `c45fb1e`, not implemented this milestone).
