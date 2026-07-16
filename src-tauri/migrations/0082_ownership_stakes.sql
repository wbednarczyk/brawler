-- Ownership structure: shareholder stakes, history, and holder classification
-- (v0.56.0, ADR 0072). Records who owns each tracked company and how that
-- changes over time — founder/insider stakes, funds (TFI/OFE), the State
-- Treasury, treasury shares — as append-only snapshots per (source, as-of).
-- History is the product: prior snapshots are never rewritten. Capital % and
-- votes % are stored SEPARATELY (preferred-vote shares are common on the GPW and
-- the gap is itself investor signal). Free float is NOT stored — it is derived at
-- read time (100 − Σ disclosed stakes) with an uncertainty note.
--
-- Forward-only, idempotent and self-healing (data-model migration rules): tables
-- use IF NOT EXISTS, and the holder-dictionary seed is an idempotent upsert that
-- re-runs without duplicating or disturbing existing rows.

PRAGMA foreign_keys = ON;

-- Append-only stake snapshots. One row per (company, source, as_of, holder):
-- re-ingesting the same disclosure updates that row in place; a new as_of always
-- inserts a fresh row, preserving the timeline.
CREATE TABLE IF NOT EXISTS ownership_stakes (
    id TEXT PRIMARY KEY,
    company_id TEXT NOT NULL REFERENCES companies(id) ON DELETE CASCADE,
    -- Holder name exactly as printed in the disclosure, and a normalized form
    -- (trim + collapse whitespace + uppercase) used for grouping and dictionary
    -- lookup. Normalization deepens in T5 (legal-form suffix stripping).
    holder_name_raw TEXT NOT NULL,
    holder_name_normalized TEXT NOT NULL,
    -- Classification (ADR 0072); NULL = not yet classified (dictionary miss awaiting
    -- AI/manual re-type). NULLable rather than a sentinel so an unclassified stake
    -- is representable without a magic value.
    holder_type TEXT CHECK(holder_type IN (
        'founder_insider', 'family_foundation', 'tfi', 'ofe_pension',
        'state_treasury', 'parent_company', 'treasury_shares',
        'other_institutional', 'free_float_rest'
    )),
    -- Two SEPARATE percentages, decimal-exact TEXT (financial_facts.value_numeric
    -- convention; parsed with rust_decimal). Each nullable: reports sometimes
    -- disclose only capital or only votes.
    capital_pct TEXT,
    votes_pct TEXT,
    as_of TEXT NOT NULL,
    source TEXT NOT NULL CHECK(source IN (
        'report_document', 'espi_filing', 'aggregator', 'manual'
    )),
    -- Provenance to the exact document / filing that carried the stake (nullable;
    -- ON DELETE SET NULL keeps snapshots when their source row is pruned).
    report_document_id TEXT REFERENCES report_documents(id) ON DELETE SET NULL,
    feed_item_id TEXT REFERENCES feed_items(id) ON DELETE SET NULL,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(company_id, source, as_of, holder_name_normalized)
);

CREATE INDEX IF NOT EXISTS idx_ownership_stakes_company_asof
    ON ownership_stakes(company_id, as_of);

-- Holder-type dictionary, seeded AS DATA (not code): normalized alias → holder
-- type (+ display name). The deterministic classifier looks holders up here; the
-- residual goes to AI/manual. Extensible without a code change — add rows.
CREATE TABLE IF NOT EXISTS ownership_holder_dictionary (
    alias_normalized TEXT PRIMARY KEY,
    holder_type TEXT NOT NULL CHECK(holder_type IN (
        'founder_insider', 'family_foundation', 'tfi', 'ofe_pension',
        'state_treasury', 'parent_company', 'treasury_shares',
        'other_institutional', 'free_float_rest'
    )),
    display_name TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Starter set: major Polish TFI, OFE, State entities, and treasury-share patterns.
-- Aliases are stored already-normalized (uppercase) to match the Rust lookup key.
INSERT INTO ownership_holder_dictionary (alias_normalized, holder_type, display_name) VALUES
    -- TFI (mutual-fund managers)
    ('PKO TFI', 'tfi', 'PKO TFI'),
    ('PZU TFI', 'tfi', 'PZU TFI'),
    ('PEKAO TFI', 'tfi', 'Pekao TFI'),
    ('NN INVESTMENT PARTNERS TFI', 'tfi', 'NN Investment Partners TFI'),
    ('INVESTORS TFI', 'tfi', 'Investors TFI'),
    ('ESALIENS TFI', 'tfi', 'Esaliens TFI'),
    ('QUERCUS TFI', 'tfi', 'Quercus TFI'),
    ('AGIOFUNDS TFI', 'tfi', 'AgioFunds TFI'),
    ('ROCKBRIDGE TFI', 'tfi', 'Rockbridge TFI'),
    ('ALLIANZ TFI', 'tfi', 'Allianz TFI'),
    ('AVIVA INVESTORS POLAND TFI', 'tfi', 'Aviva Investors Poland TFI'),
    ('SANTANDER TFI', 'tfi', 'Santander TFI'),
    ('GENERALI INVESTMENTS TFI', 'tfi', 'Generali Investments TFI'),
    ('UNIQA TFI', 'tfi', 'Uniqa TFI'),
    ('GOLDMAN SACHS TFI', 'tfi', 'Goldman Sachs TFI'),
    ('SKARBIEC TFI', 'tfi', 'Skarbiec TFI'),
    ('IPOPEMA TFI', 'tfi', 'Ipopema TFI'),
    ('MILLENNIUM TFI', 'tfi', 'Millennium TFI'),
    ('BNP PARIBAS TFI', 'tfi', 'BNP Paribas TFI'),
    -- OFE (open pension funds)
    ('NATIONALE-NEDERLANDEN OFE', 'ofe_pension', 'Nationale-Nederlanden OFE'),
    ('ALLIANZ POLSKA OFE', 'ofe_pension', 'Allianz Polska OFE'),
    ('OFE PZU ZŁOTA JESIEŃ', 'ofe_pension', 'OFE PZU "Złota Jesień"'),
    ('AEGON OFE', 'ofe_pension', 'Aegon OFE'),
    ('GENERALI OFE', 'ofe_pension', 'Generali OFE'),
    ('UNIQA OFE', 'ofe_pension', 'Uniqa OFE'),
    ('VIENNA OFE', 'ofe_pension', 'Vienna OFE'),
    ('PKO BP BANKOWY OFE', 'ofe_pension', 'PKO BP Bankowy OFE'),
    -- State entities
    ('SKARB PAŃSTWA', 'state_treasury', 'Skarb Państwa'),
    ('POLSKI FUNDUSZ ROZWOJU', 'state_treasury', 'Polski Fundusz Rozwoju'),
    ('PFR', 'state_treasury', 'PFR'),
    ('AGENCJA ROZWOJU PRZEMYSŁU', 'state_treasury', 'Agencja Rozwoju Przemysłu'),
    ('ARP', 'state_treasury', 'ARP'),
    ('BANK GOSPODARSTWA KRAJOWEGO', 'state_treasury', 'Bank Gospodarstwa Krajowego'),
    ('BGK', 'state_treasury', 'BGK'),
    -- Treasury (own) shares
    ('AKCJE WŁASNE', 'treasury_shares', 'Akcje własne'),
    ('AKCJE WŁASNE SPÓŁKI', 'treasury_shares', 'Akcje własne spółki')
ON CONFLICT(alias_normalized) DO UPDATE SET
    holder_type = excluded.holder_type,
    display_name = excluded.display_name,
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now');
