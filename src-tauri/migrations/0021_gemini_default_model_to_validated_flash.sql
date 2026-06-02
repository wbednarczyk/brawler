UPDATE settings
SET value = 'gemini-2.5-flash'
WHERE key = 'youtube_transcription_model'
  AND value = 'gemini-2.5-flash-lite';
