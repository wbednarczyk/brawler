-- Ownership dictionary: abbreviation aliases sharing one identity (owner
-- dogfooding 2026-07-16, v0.56). Disclosure tables print the same pension-fund
-- manager as "NN PTE" in one attachment and "Nationale-Nederlanden PTE S.A."
-- in another; rows resolving to a dictionary entry with the SAME display_name
-- merge in the current-state read model (fundamentals::ownership::classify::
-- HolderIdentityMap). Also classifies PTE managers as ofe_pension.
--
-- Forward-only, idempotent, self-healing (data-model migration rules).

INSERT INTO ownership_holder_dictionary (alias_normalized, holder_type, display_name) VALUES
    ('NN PTE', 'ofe_pension', 'Nationale-Nederlanden PTE'),
    ('NATIONALE-NEDERLANDEN PTE', 'ofe_pension', 'Nationale-Nederlanden PTE'),
    ('PTE ALLIANZ POLSKA', 'ofe_pension', 'PTE Allianz Polska'),
    ('GENERALI PTE', 'ofe_pension', 'Generali PTE'),
    ('PZU PTE', 'ofe_pension', 'PZU PTE')
ON CONFLICT(alias_normalized) DO UPDATE SET
    holder_type = excluded.holder_type,
    display_name = excluded.display_name,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
