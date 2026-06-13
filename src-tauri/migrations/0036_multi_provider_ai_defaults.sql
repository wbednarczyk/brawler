-- Multi-provider AI (ADR 0028): bump the default analysis/transcription model to
-- gemini-3.5-flash and pin a concrete default analysis provider. Only rows still
-- holding the superseded default are updated so explicit user choices are kept.

UPDATE settings
SET value = 'gemini-3.5-flash'
WHERE key = 'general_analysis_model'
  AND value = 'gemini-2.5-flash';

UPDATE settings
SET value = 'gemini-3.5-flash'
WHERE key = 'youtube_transcription_model'
  AND value = 'gemini-2.5-flash';

UPDATE settings
SET value = 'provider_gemini'
WHERE key = 'general_analysis_provider'
  AND value = '';
