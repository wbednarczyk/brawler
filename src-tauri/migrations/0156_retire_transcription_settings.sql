-- ADR 0111 (#463): video transcription is retired. The transcript tables
-- stay (migrations are append-only) with no reader; the provider settings
-- and any search-index rows of the retired type go. Forward, idempotent.

DELETE FROM settings
WHERE key IN (
  'youtube_transcription_provider',
  'youtube_transcription_model',
  'youtube_transcription_timeout_seconds'
);

DELETE FROM search_index WHERE content_type = 'transcript_segment';
