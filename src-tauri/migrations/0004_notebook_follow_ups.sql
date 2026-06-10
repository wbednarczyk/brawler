ALTER TABLE notebook_entries RENAME COLUMN review_after TO follow_up_after;
ALTER TABLE notebook_entries RENAME COLUMN review_date TO follow_up_date;
DROP INDEX IF EXISTS idx_notebook_entries_review_date;
CREATE INDEX idx_notebook_entries_follow_up_date ON notebook_entries(follow_up_date);
