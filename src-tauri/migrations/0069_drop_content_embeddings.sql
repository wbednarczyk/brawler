-- Retire the embedding model's disposable vector index (ADR 0080 decision 4).
-- `content_embeddings` was a derived cache (ADR 0035: "the vector index is
-- disposable"), so dropping it loses zero canonical data. Queued (or terminal)
-- `content_embedding` jobs are purged with it: the job kind no longer has a
-- registered handler, and a leftover pending row would sit unclaimed forever.
--
-- Forward, idempotent, self-healing: DROP TABLE IF EXISTS + a DELETE that is a
-- no-op once clean, so it converges on every database regardless of prior state.
-- The legacy `similarity_strategy='embedding'` settings row is left in place;
-- reads map it to 'static' (storage::settings::get_similarity_strategy).

DROP TABLE IF EXISTS content_embeddings;

DELETE FROM job_queue WHERE kind = 'content_embedding';
