-- Repair the EPS facts stored with `currency = 'shares'` (issue #93, epic #229
-- T4). The trust audit over the maintainer's real database counted exactly 76
-- such rows: 38 `eps_basic` + 38 `eps_diluted`, and nothing else.
--
-- Root cause (fixed in the same change, `fundamentals/extraction/esef.rs`): an
-- EPS unit is a RATIO declared as an `xbrli:divide` — numerator
-- `iso4217:PLN`, denominator `xbrli:shares`. The unit map inserted every
-- `<measure>` it saw, so the LAST measure (the denominator) won and the fact's
-- currency became "shares". The parser now takes the numerator; the write-side
-- guard in `storage/financials.rs` makes the shape unstorable from any writer.
--
-- Why PLN is the correct value: the affected set is entirely GPW ESEF filings,
-- whose divide-unit numerator is `iso4217:PLN` in the source documents. No
-- non-PLN reporting company appears in the affected rows.
--
-- Forward, idempotent by construction: after the update no row matches
-- `currency = 'shares'` any more, so a re-run is a no-op. Only the two EPS
-- metric keys are touched — any other metric carrying a non-ISO currency is
-- deliberately left for its own audited repair (never a blanket rewrite).

UPDATE financial_facts
   SET currency = 'PLN',
       updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
 WHERE currency = 'shares'
   AND definition_id IN (
        SELECT id FROM kpi_definitions
         WHERE metric_key IN ('eps_basic', 'eps_diluted'));
