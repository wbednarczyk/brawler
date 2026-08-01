-- ADR 0093 decision 4 (epic #285 T9): a durable origin marker on
-- `kpi_definitions` (`seed | user | agent`) so agent-minted definitions
-- (issuer-characteristic metrics the catalog lacks) are honestly
-- distinguishable from the app-seeded catalog and the owner's own custom
-- metrics — reviewable, and the #272 characteristic-KPI UI can surface them
-- honestly.
--
-- Append-only, idempotent, self-healing: ADD COLUMN with a DEFAULT so it
-- converges on every database regardless of prior state.
--
-- Backfill predicate (`id NOT GLOB '*__*'`): every migration-seeded row keeps
-- a BARE id — `kpidef_<metric_key>` for the canonical packs (`0034`'s runtime
-- `create_kpi_definition`/`kpi_definition_id` builder mints this exact same
-- shape for `scope = 'canonical'`, but no live writer has ever used that
-- scope — the UI's `CustomKpiManager` only ever creates `scope = 'company'`,
-- and every `non-canonical` scope the id builder handles gets a
-- `__<marker>_<discriminator>` suffix (`__c_<company>` / `__s_<sector>` /
-- `__user_`), so a bare id is reliably migration-only) or a hand-written
-- prefixed id (`kpidef_bank_nim`, migrations `0034`/`0048`/`0089`/…, same "no
-- suffix" shape). Every runtime-created row (UI `company`-scope custom KPIs,
-- `user`-scope quality-framework metrics) carries the suffix and is left at
-- the column DEFAULT (`user`) — correct, since none of those are seeded.
-- Honest limitation: this cannot distinguish an MCP-agent-created definition
-- from a UI-created one for any row written BEFORE this migration ships
-- (both left DEFAULT `user`) — there is no marker to reconstruct that
-- retroactively; going forward, the MCP `create_kpi_definition` handler
-- stamps `origin = 'agent'` explicitly.

ALTER TABLE kpi_definitions ADD COLUMN origin TEXT NOT NULL DEFAULT 'user';

UPDATE kpi_definitions SET origin = 'seed' WHERE id NOT GLOB '*__*';
