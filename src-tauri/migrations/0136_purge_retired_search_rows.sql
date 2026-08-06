-- ADR 0084 clean-cut completion: migration 0102 dropped the AI research-brief
-- and digest source tables, but `DROP TABLE` fires no delete triggers, so
-- their rows in the `search_index` FTS table survived and stayed searchable.
-- Forward, idempotent purge of the orphaned derived rows; the live content
-- types are untouched.
DELETE FROM search_index WHERE content_type IN ('research_brief', 'digest');
