INSERT OR IGNORE INTO settings (key, value, value_type)
VALUES
    ('general_analysis_model', 'gemini-2.5-flash', 'string'),
    ('general_analysis_timeout_seconds', '90', 'integer');

CREATE TABLE ai_analysis_jobs (
    id TEXT PRIMARY KEY,
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    prompt_preset_id TEXT NOT NULL DEFAULT 'default_summary',
    custom_question TEXT,
    provider_id TEXT NOT NULL,
    model TEXT NOT NULL,
    prompt_version TEXT NOT NULL,
    status TEXT NOT NULL,
    error_code TEXT,
    error TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    started_at TEXT,
    finished_at TEXT
);

CREATE INDEX idx_ai_analysis_jobs_feed_item_id ON ai_analysis_jobs(feed_item_id);
CREATE INDEX idx_ai_analysis_jobs_status ON ai_analysis_jobs(status);
CREATE INDEX idx_ai_analysis_jobs_prompt ON ai_analysis_jobs(feed_item_id, prompt_preset_id, custom_question);

ALTER TABLE ai_analysis_results
ADD COLUMN ai_analysis_job_id TEXT REFERENCES ai_analysis_jobs(id) ON DELETE SET NULL;

ALTER TABLE ai_analysis_results
ADD COLUMN prompt_version TEXT NOT NULL DEFAULT 'm13.source_grounded.v1';

CREATE INDEX idx_ai_analysis_results_feed_item_id ON ai_analysis_results(feed_item_id);
CREATE INDEX idx_ai_analysis_results_job_id ON ai_analysis_results(ai_analysis_job_id);
