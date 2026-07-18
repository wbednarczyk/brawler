-- v0.57 Company Health — insider substrate (ADR 0083 Decision 6 + the 2026-07-17
-- ground-truth amendment; data-model § Company Health). Three concerns, all
-- forward-only / idempotent / self-healing (data-model migration rules):
--
--   1. Re-seed the `insider_transaction` rule patterns. The original seed
--      (0041/0042/0043) matched 0/22 real MAR art. 19 filings — its phrase forms
--      ("art. 19 ust. 1 mar", "osób pełniących obowiązki zarządcze") never appear
--      verbatim in the real Bankier titles. The corrected substrings are
--      validated against the 22-filing hand-labeled corpus (case-insensitive
--      substring semantics of the RuleClassifier), and DO NOT match the other
--      categories' titles (dividend / general meeting / significant contract /
--      major-holdings art. 69). Confidence stays 0.95. The reclassification
--      backfill is the existing `classify_pending_feed_items` sweep, which
--      re-reads these patterns and (idempotently) tags the previously-unmatched
--      filings on the next refresh / startup catch-up.
--
--   2. `insider_transactions` — the parsed MAR art. 19 substrate. Deterministic
--      id from (feed_item_id, unit_index): a filing commonly carries several
--      enumerated notifications, so the unit index disambiguates. Nullable
--      role / direction / instrument / figures — the Bankier body is only the
--      ESPI cover note; volume/price/tx-date live in the attachment PDF for ~90%
--      of transactions (filled later by T4b) and are NEVER guessed here.
--
--   3. `insider_espi_unparsed` — the once-per-filing parking marker (mirrors
--      `ownership_espi_unparsed`) for a classified insider filing whose cover
--      note yields no writable unit (all Ambiguous / NotFound), so the parse
--      sweep attempts each filing exactly once and re-runs create zero rows.

PRAGMA foreign_keys = ON;

-- 1. Corrected `insider_transaction` classification patterns (validated corpus).
INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_insider_transaction', 'insider_transaction', 'Insider transaction', 0,
     '{"patterns":["art. 19 mar","art. 19 ust. 1 mar","powiadomienie o transakcji","powiadomienie o transakcjach","powiadomienia o transakcji","powiadomienia o transakcjach","obowiązki zarządcze","blisko związan","transakcji na akcjach","transakcjach na akcjach","transakcje na akcjach","nabyciu akcji emitenta"],"confidence":0.95}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');

-- 2. Parsed MAR art. 19 transactions (data-model § Company Health).
CREATE TABLE IF NOT EXISTS insider_transactions (
    -- Deterministic: insidertx_<slug(feed_item_id)>_<unit_index>. A re-parse of
    -- the same filing upserts each unit in place — re-ingest never duplicates.
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    feed_item_id TEXT NOT NULL REFERENCES feed_items(id) ON DELETE CASCADE,
    -- Position of the notification unit within the filing (0-based), part of the id.
    unit_index INTEGER NOT NULL,
    person_name_raw TEXT NOT NULL,
    person_normalized TEXT NOT NULL,
    -- Nullable: a bare "osoba pełniąca obowiązki zarządcze" with no board
    -- qualifier stays NULL, never guessed.
    role TEXT CHECK(role IN ('management', 'supervisory', 'closely_associated')),
    -- For closely_associated rows: the anchoring PDMR (the skin-in-the-game join
    -- needs the natural person behind the vehicle, not the vehicle entity).
    related_pdmr_raw TEXT,
    related_pdmr_normalized TEXT,
    related_pdmr_role TEXT CHECK(related_pdmr_role IN ('management', 'supervisory', 'closely_associated')),
    -- Nullable: ~1/4 of cover notes state only "powiadomienie o transakcjach";
    -- the buy/sell nature then lives in the attachment PDF.
    direction TEXT CHECK(direction IN ('buy', 'sell', 'other')),
    instrument TEXT CHECK(instrument IN ('shares', 'subscription_warrants', 'other')),
    -- Decimal-exact TEXT (financial_facts.value_numeric convention), all nullable
    -- (PDF-only for most filings; never fabricated).
    volume TEXT,
    price TEXT,
    currency TEXT,
    tx_date TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(feed_item_id, unit_index)
);

CREATE INDEX IF NOT EXISTS idx_insider_transactions_company
    ON insider_transactions(company_id);
CREATE INDEX IF NOT EXISTS idx_insider_transactions_person
    ON insider_transactions(company_id, person_normalized);

-- 3. Once-per-filing parking marker for classified insider filings whose cover
-- note yields no writable unit. NO transaction row is written — never guess.
CREATE TABLE IF NOT EXISTS insider_espi_unparsed (
    feed_item_id TEXT PRIMARY KEY REFERENCES feed_items(id) ON DELETE CASCADE,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- Machine-stable reason: not_found | person_unresolved | <parser reason>.
    reason TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);
