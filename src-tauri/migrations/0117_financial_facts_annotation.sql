-- One-off annotation on a financial fact (#156, owner request 2026-07-08).
--
-- A reported value can contain a one-off event (e.g. CBF Q3 2023 net_profit
-- includes 628 tys. of discontinued operations). The value stays exactly as
-- reported; the annotation carries the user's comment and renders as a visible
-- '*' marker next to the figure. User-authored only: no extraction path writes
-- it. Nullable, no backfill.
ALTER TABLE financial_facts ADD COLUMN annotation TEXT;
