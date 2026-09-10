-- ADR 0025 amendment (#465): event/signal review reminders are no longer
-- auto-generated; the automatic rows (deterministic signatures) are closed
-- as dismissed, dated. Deliberately created reminders of these kinds keep
-- their state. Forward and idempotent: a re-run matches zero rows.

UPDATE research_reminders
SET status = 'dismissed',
    dismissed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE status = 'open'
  AND (
    (reminder_kind = 'event_review' AND id GLOB 'reminder_event_*')
    OR (reminder_kind = 'signal_review'
        AND source_type = 'company_signal'
        AND body LIKE 'High-signal disclosure classified as %')
  );
