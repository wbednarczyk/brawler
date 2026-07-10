-- Data repair (card f64cea2): fold legacy out-of-spec `financial_periods`
-- rows with period_type='annual' into the canonical FY label.
--
-- period_type is a fiscal label (data-model.md: FY, H1, H2, Q1..Q4, 9M,
-- M01..M12). A pre-spec ingest wrote one 'annual' row; it splits the
-- Fundamentals matrix and any period_id-keyed view even though the coverage
-- read model normalizes labels for grouping. This forward, idempotent,
-- self-healing migration merges each 'annual' row into its
-- (company_id, fiscal_year, FY) sibling — repointing every table that
-- references financial_periods before deleting — honoring
-- UNIQUE(company_id, fiscal_year, period_type). A lone 'annual' with no FY
-- sibling is relabeled to FY in place.
--
-- Merge rule when both periods hold a fact in the same
-- idx_financial_facts_slot: the canonical FY row's fact wins and the annual
-- duplicate is dropped (the card merges the out-of-spec row INTO its canonical
-- sibling, so FY is authoritative). All other annual-side facts are repointed.
--
-- Case-insensitive on the label so 'Annual'/'ANNUAL' are covered too. Idempotent:
-- after one run no 'annual'-labelled rows remain, so every statement below is a
-- no-op on re-run and on an already-clean database.

-- 1) Drop annual-side facts that would collide with an existing FY-side fact in
--    the same fully-qualified slot (keep the canonical FY row's value).
DELETE FROM financial_facts
WHERE id IN (
    SELECT af.id
    FROM financial_facts af
    JOIN financial_periods ap
        ON ap.id = af.period_id AND UPPER(ap.period_type) = 'ANNUAL'
    JOIN financial_periods fp
        ON fp.company_id = ap.company_id
        AND fp.fiscal_year = ap.fiscal_year
        AND fp.period_type = 'FY'
    JOIN financial_facts ff
        ON ff.period_id = fp.id
        AND ff.definition_id = af.definition_id
        AND ff.statement_basis = af.statement_basis
        AND ff.attribution = af.attribution
        AND ff.variant = af.variant
        AND ff.measure_window = af.measure_window
        AND ff.data_quality = af.data_quality
);

-- 2) Repoint remaining annual-side facts onto the FY sibling.
UPDATE financial_facts
SET period_id = (
        SELECT fp.id
        FROM financial_periods ap
        JOIN financial_periods fp
            ON fp.company_id = ap.company_id
            AND fp.fiscal_year = ap.fiscal_year
            AND fp.period_type = 'FY'
        WHERE ap.id = financial_facts.period_id
    ),
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE period_id IN (
    SELECT ap.id
    FROM financial_periods ap
    WHERE UPPER(ap.period_type) = 'ANNUAL'
        AND EXISTS (
            SELECT 1 FROM financial_periods fp
            WHERE fp.company_id = ap.company_id
                AND fp.fiscal_year = ap.fiscal_year
                AND fp.period_type = 'FY'
        )
);

-- 3) Repoint every other table that references financial_periods (all ON DELETE
--    SET NULL, so without this the link would be lost on the delete below).
UPDATE report_documents
SET period_id = (
        SELECT fp.id FROM financial_periods ap
        JOIN financial_periods fp
            ON fp.company_id = ap.company_id
            AND fp.fiscal_year = ap.fiscal_year
            AND fp.period_type = 'FY'
        WHERE ap.id = report_documents.period_id
    )
WHERE period_id IN (
    SELECT ap.id FROM financial_periods ap
    WHERE UPPER(ap.period_type) = 'ANNUAL'
        AND EXISTS (
            SELECT 1 FROM financial_periods fp
            WHERE fp.company_id = ap.company_id
                AND fp.fiscal_year = ap.fiscal_year
                AND fp.period_type = 'FY'
        )
);

UPDATE framework_evaluations
SET period_id = (
        SELECT fp.id FROM financial_periods ap
        JOIN financial_periods fp
            ON fp.company_id = ap.company_id
            AND fp.fiscal_year = ap.fiscal_year
            AND fp.period_type = 'FY'
        WHERE ap.id = framework_evaluations.period_id
    )
WHERE period_id IN (
    SELECT ap.id FROM financial_periods ap
    WHERE UPPER(ap.period_type) = 'ANNUAL'
        AND EXISTS (
            SELECT 1 FROM financial_periods fp
            WHERE fp.company_id = ap.company_id
                AND fp.fiscal_year = ap.fiscal_year
                AND fp.period_type = 'FY'
        )
);

UPDATE management_claims
SET source_period_id = (
        SELECT fp.id FROM financial_periods ap
        JOIN financial_periods fp
            ON fp.company_id = ap.company_id
            AND fp.fiscal_year = ap.fiscal_year
            AND fp.period_type = 'FY'
        WHERE ap.id = management_claims.source_period_id
    )
WHERE source_period_id IN (
    SELECT ap.id FROM financial_periods ap
    WHERE UPPER(ap.period_type) = 'ANNUAL'
        AND EXISTS (
            SELECT 1 FROM financial_periods fp
            WHERE fp.company_id = ap.company_id
                AND fp.fiscal_year = ap.fiscal_year
                AND fp.period_type = 'FY'
        )
);

-- 4) Delete the now-emptied annual rows that had an FY sibling.
DELETE FROM financial_periods
WHERE UPPER(period_type) = 'ANNUAL'
    AND EXISTS (
        SELECT 1 FROM financial_periods fp
        WHERE fp.company_id = financial_periods.company_id
            AND fp.fiscal_year = financial_periods.fiscal_year
            AND fp.period_type = 'FY'
    );

-- 5) Any remaining annual rows had no FY sibling: relabel them to FY in place.
UPDATE financial_periods
SET period_type = 'FY',
    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
WHERE UPPER(period_type) = 'ANNUAL';
