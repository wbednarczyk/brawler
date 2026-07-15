-- Seed the deterministic `auditor_opinion` signal category (v0.55.0, ADR 0069
-- decision 4; taxonomy base ADR 0034). Flags auditor red flags in official
-- filings — qualified opinions, disclaimers of opinion, negative opinions and
-- going-concern emphasis — via the rule classifier over the filing title.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): the
-- upsert re-seeds the category on re-run without touching existing signals, and
-- tolerates a database whose earlier migration recorded but did not persist the
-- row. Not a forward-looking dated category, so it derives no calendar event
-- (derives_event = 0), consistent with the other high-signal categories
-- (insider_transaction, profit_warning).

INSERT INTO signal_categories (id, key, display_name, derives_event, rule_definition_json) VALUES
    ('sigcat_auditor_opinion', 'auditor_opinion', 'Auditor opinion', 0,
     '{"patterns":["opinia z zastrzeżeniem","raport z zastrzeżeniem","z zastrzeżeniem","zastrzeżenie","odmowa wyrażenia opinii","odmowa opinii","opinia negatywna","kontynuacja działalności","kontynuacji działalności"],"confidence":0.93}')
ON CONFLICT(key) DO UPDATE SET
    display_name = excluded.display_name,
    derives_event = excluded.derives_event,
    rule_definition_json = excluded.rule_definition_json,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
