INSERT INTO settings (key, value, value_type)
VALUES ('youtube_transcription_model', 'gemini-2.5-flash', 'string')
ON CONFLICT(key) DO NOTHING;
