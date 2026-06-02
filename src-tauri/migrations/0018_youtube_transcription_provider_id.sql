UPDATE settings
SET value = 'provider_gemini'
WHERE key = 'youtube_transcription_provider'
  AND value = 'gemini';
