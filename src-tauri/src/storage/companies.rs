use super::database::Database;
use super::sources::set_source_adapter_state;
use super::*;
use crate::source_adapters::gpw_company_registry::{
    GpwCompanyRegistryEntry, ADAPTER_ID as GPW_REGISTRY_ADAPTER_ID,
};
use crate::source_adapters::newconnect_company_directory::ADAPTER_ID as NEWCONNECT_DIRECTORY_ADAPTER_ID;

/// Company domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only company/registry operations. Reach it via `AppState::companies()`.
#[derive(Clone)]
pub struct CompanyStore {
    db: Database,
}

impl CompanyStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_companies(&self) -> StorageResult<Vec<Company>> {
        let connection = self.db.checkout()?;
        list_companies(&connection)
    }

    pub fn create_company(&self, input: NewCompany) -> StorageResult<Company> {
        let connection = self.db.checkout()?;
        create_company(&connection, input)
    }

    pub fn get_company_ir_reports_url(&self, company_id: &str) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;
        get_company_ir_reports_url(&connection, company_id)
    }

    pub fn set_company_ir_reports_url(
        &self,
        company_id: &str,
        url: Option<&str>,
    ) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;
        set_company_ir_reports_url(&connection, company_id, url)
    }

    pub fn get_company_sector(
        &self,
        company_id: &str,
    ) -> StorageResult<(Option<String>, Option<String>)> {
        let connection = self.db.checkout()?;
        get_company_sector(&connection, company_id)
    }

    /// The company's `statement_type` (ADR 0083 Decision 4 health-score gate).
    pub fn get_statement_type(&self, company_id: &str) -> StorageResult<String> {
        let connection = self.db.checkout()?;
        get_statement_type(&connection, company_id)
    }

    pub fn set_company_sector(
        &self,
        company_id: &str,
        sector: Option<&str>,
    ) -> StorageResult<Option<String>> {
        let connection = self.db.checkout()?;
        set_company_sector(&connection, company_id, sector)
    }

    /// The distinct directory-sourced sector taxonomy (active entries only) —
    /// the preset list a manual override picks from (ADR 0067 Decision 3).
    pub fn list_company_sectors(&self) -> StorageResult<Vec<String>> {
        let connection = self.db.checkout()?;
        list_company_sectors(&connection)
    }

    /// The latest recorded non-superseded `shares_outstanding` fact and the
    /// period it was reported for (e.g. `("41636000", "2025 FY")`) — the Basic
    /// info panel's read model input.
    pub fn latest_shares_outstanding(
        &self,
        company_id: &str,
    ) -> StorageResult<Option<(String, String)>> {
        let connection = self.db.checkout()?;
        latest_shares_outstanding(&connection, company_id)
    }

    pub fn lookup_company(
        &self,
        input: CompanyLookupInput,
    ) -> StorageResult<Option<CompanyLookupResult>> {
        let connection = self.db.checkout()?;
        lookup_company(&connection, input)
    }

    pub fn company_directories_need_bootstrap_refresh(&self) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        company_directories_need_bootstrap_refresh(&connection)
    }

    pub fn company_directories_are_stale(&self, stale_after_seconds: i64) -> StorageResult<bool> {
        let connection = self.db.checkout()?;
        company_directories_are_stale(&connection, stale_after_seconds)
    }

    pub fn refresh_gpw_company_registry(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        let mut connection = self.db.checkout()?;
        refresh_company_directory(
            &mut connection,
            GPW_REGISTRY_ADAPTER_ID,
            entries,
            fetched_at,
        )
    }

    pub fn refresh_newconnect_company_directory(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        let mut connection = self.db.checkout()?;
        refresh_company_directory(
            &mut connection,
            NEWCONNECT_DIRECTORY_ADAPTER_ID,
            entries,
            fetched_at,
        )
    }

    pub fn delete_company(&self, company_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_company(&connection, company_id)
    }
}

pub(super) fn list_companies(connection: &Connection) -> StorageResult<Vec<Company>> {
    let mut statement = connection.prepare(
        "
        SELECT id, exchange, ticker, qualified_ticker, display_name, isin, cik, lei
        FROM companies
        ORDER BY exchange, ticker
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(Company {
            id: row.get(0)?,
            exchange: row.get(1)?,
            ticker: row.get(2)?,
            qualified_ticker: row.get(3)?,
            display_name: row.get(4)?,
            isin: row.get(5)?,
            cik: row.get(6)?,
            lei: row.get(7)?,
        })
    })?;

    let companies = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(companies)
}

pub(super) fn create_company(connection: &Connection, input: NewCompany) -> StorageResult<Company> {
    let exchange = input.exchange.trim().to_uppercase();
    let ticker = input.ticker.trim().to_uppercase();
    let display_name = input.display_name.trim().to_owned();
    let qualified_ticker = format!("{exchange}:{ticker}");
    let id = company_id(&exchange, &ticker);

    connection.execute(
        "
        INSERT INTO companies (
            id,
            exchange,
            ticker,
            qualified_ticker,
            display_name,
            isin,
            cik,
            lei
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ",
        params![
            id,
            exchange,
            ticker,
            qualified_ticker,
            display_name,
            empty_string_to_none(input.isin),
            empty_string_to_none(input.cik),
            empty_string_to_none(input.lei),
        ],
    )?;

    // Seed the sector from the directory cache if the registry already knows it
    // (ADR 0067 Decision 3); `sector_source='registry'`, overridable later.
    connection.execute(
        "
        UPDATE companies
        SET sector = (
                SELECT cre.sector FROM company_registry_entries cre
                WHERE cre.qualified_ticker = ?2 AND cre.active = 1 AND cre.sector IS NOT NULL
                LIMIT 1
            ),
            sector_source = 'registry'
        WHERE id = ?1
          AND EXISTS (
                SELECT 1 FROM company_registry_entries cre
                WHERE cre.qualified_ticker = ?2 AND cre.active = 1 AND cre.sector IS NOT NULL
          )
        ",
        params![id, qualified_ticker],
    )?;

    connection
        .query_row(
            "
        SELECT id, exchange, ticker, qualified_ticker, display_name, isin, cik, lei
        FROM companies
        WHERE id = ?1
        ",
            [id],
            |row| {
                Ok(Company {
                    id: row.get(0)?,
                    exchange: row.get(1)?,
                    ticker: row.get(2)?,
                    qualified_ticker: row.get(3)?,
                    display_name: row.get(4)?,
                    isin: row.get(5)?,
                    cik: row.get(6)?,
                    lei: row.get(7)?,
                })
            },
        )
        .map_err(StorageError::from)
}

pub(super) fn refresh_company_directory(
    connection: &mut Connection,
    source_adapter_id: &str,
    entries: &[GpwCompanyRegistryEntry],
    fetched_at: &str,
) -> StorageResult<CompanyRegistryRefreshResult> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut entries_upserted = 0usize;

    transaction.execute(
        "
        UPDATE company_registry_entries
        SET active = 0,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE source_adapter_id = ?1
        ",
        [source_adapter_id],
    )?;

    for entry in entries {
        transaction.execute(
            "
            INSERT INTO company_registry_entries (
                id,
                exchange,
                ticker,
                qualified_ticker,
                display_name,
                isin,
                source_adapter_id,
                source_url,
                fetched_at,
                sector,
                active
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 1)
            ON CONFLICT(exchange, ticker) DO UPDATE SET
                qualified_ticker = excluded.qualified_ticker,
                display_name = excluded.display_name,
                isin = excluded.isin,
                source_adapter_id = excluded.source_adapter_id,
                source_url = excluded.source_url,
                fetched_at = excluded.fetched_at,
                sector = excluded.sector,
                active = 1,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                company_registry_entry_id(&entry.exchange, &entry.ticker),
                entry.exchange,
                entry.ticker,
                entry.qualified_ticker,
                entry.display_name,
                empty_string_to_none(Some(entry.isin.clone())),
                source_adapter_id,
                entry.source_url,
                fetched_at,
                empty_string_to_none(entry.sector.clone()),
            ],
        )?;
        entries_upserted += 1;
    }

    // Propagate the directory-sourced sector onto tracked companies (ADR 0067
    // Decision 3): a registry value fills `companies.sector` unless a manual
    // override is set — `sector_source='manual'` always wins and is never clobbered.
    transaction.execute(
        "
        UPDATE companies
        SET sector = (
                SELECT cre.sector FROM company_registry_entries cre
                WHERE cre.qualified_ticker = companies.qualified_ticker
                  AND cre.active = 1 AND cre.sector IS NOT NULL
                LIMIT 1
            ),
            sector_source = 'registry',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE (sector_source IS NULL OR sector_source <> 'manual')
          AND EXISTS (
                SELECT 1 FROM company_registry_entries cre
                WHERE cre.qualified_ticker = companies.qualified_ticker
                  AND cre.active = 1 AND cre.sector IS NOT NULL
            )
          -- Touch a row only when the propagation actually changes it, so a
          -- routine refresh does not churn every company's updated_at.
          AND (
                companies.sector IS NOT (
                    SELECT cre.sector FROM company_registry_entries cre
                    WHERE cre.qualified_ticker = companies.qualified_ticker
                      AND cre.active = 1 AND cre.sector IS NOT NULL
                    LIMIT 1
                )
                OR IFNULL(companies.sector_source, '') <> 'registry'
            )
        ",
        [],
    )?;

    let entries_deactivated = transaction.execute(
        "
        UPDATE company_registry_entries
        SET active = 0,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE source_adapter_id = ?1
            AND fetched_at <> ?2
            AND active = 1
        ",
        params![source_adapter_id, fetched_at],
    )?;

    transaction.execute(
        "
        UPDATE source_adapters
        SET last_success_at = ?1,
            last_error_at = NULL,
            last_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![fetched_at, source_adapter_id],
    )?;
    set_source_adapter_state(
        &transaction,
        source_adapter_id,
        "last_items_fetched",
        &entries.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        source_adapter_id,
        "last_items_created",
        &entries_upserted.to_string(),
    )?;

    transaction.commit()?;

    Ok(CompanyRegistryRefreshResult {
        adapter_id: source_adapter_id.to_owned(),
        entries_fetched: entries.len(),
        entries_upserted,
        entries_deactivated,
        fetched_at: fetched_at.to_owned(),
    })
}

pub(super) fn lookup_company(
    connection: &Connection,
    input: CompanyLookupInput,
) -> StorageResult<Option<CompanyLookupResult>> {
    let exchange = input.exchange.trim().to_uppercase();
    let ticker = input.ticker.as_deref().map(normalize_lookup_value);
    let isin = input.isin.as_deref().map(normalize_lookup_value);
    let display_name = input.display_name.as_deref().map(normalize_name_lookup);

    if let Some(result) =
        lookup_company_registry(connection, &exchange, &ticker, &isin, &display_name)?
    {
        return Ok(Some(result));
    }

    Ok(None)
}

pub(super) fn lookup_company_registry(
    connection: &Connection,
    exchange: &str,
    ticker: &Option<String>,
    isin: &Option<String>,
    display_name: &Option<String>,
) -> StorageResult<Option<CompanyLookupResult>> {
    if let Some(ticker) = ticker.as_deref().filter(|value| !value.is_empty()) {
        return connection
            .query_row(
                "
                SELECT exchange, ticker, qualified_ticker, display_name, COALESCE(isin, '')
                FROM company_registry_entries
                WHERE ticker = ?2
                    AND active = 1
                ORDER BY
                    CASE WHEN exchange = ?1 THEN 0 ELSE 1 END,
                    qualified_ticker
                LIMIT 1
                ",
                params![exchange, ticker],
                |row| registry_lookup_result(row, "company_directory"),
            )
            .optional()
            .map_err(StorageError::from);
    }

    if let Some(isin) = isin.as_deref().filter(|value| !value.is_empty()) {
        return connection
            .query_row(
                "
                SELECT exchange, ticker, qualified_ticker, display_name, COALESCE(isin, '')
                FROM company_registry_entries
                WHERE isin = ?2
                    AND active = 1
                ORDER BY
                    CASE WHEN exchange = ?1 THEN 0 ELSE 1 END,
                    qualified_ticker
                LIMIT 1
                ",
                params![exchange, isin],
                |row| registry_lookup_result(row, "company_directory"),
            )
            .optional()
            .map_err(StorageError::from);
    }

    if let Some(display_name) = display_name
        .as_deref()
        .filter(|value| value.chars().count() >= 3)
    {
        return connection
            .query_row(
                "
                SELECT exchange, ticker, qualified_ticker, display_name, COALESCE(isin, '')
                FROM company_registry_entries
                WHERE UPPER(display_name) LIKE '%' || ?2 || '%'
                    AND active = 1
                ORDER BY
                    CASE WHEN exchange = ?1 THEN 0 ELSE 1 END,
                    qualified_ticker
                LIMIT 1
                ",
                params![exchange, display_name],
                |row| registry_lookup_result(row, "company_directory"),
            )
            .optional()
            .map_err(StorageError::from);
    }

    Ok(None)
}

pub(super) fn company_directories_need_bootstrap_refresh(
    connection: &Connection,
) -> StorageResult<bool> {
    let stale_required_directories: i64 = connection.query_row(
        "
        SELECT COUNT(*)
        FROM source_adapters
        WHERE source_type = 'company_registry'
            AND enabled = 1
            AND NOT EXISTS (
                SELECT 1
                FROM company_registry_entries
                WHERE company_registry_entries.source_adapter_id = source_adapters.id
                    AND company_registry_entries.active = 1
            )
        ",
        [],
        |row| row.get(0),
    )?;

    Ok(stale_required_directories > 0)
}

pub(super) fn company_directories_are_stale(
    connection: &Connection,
    stale_after_seconds: i64,
) -> StorageResult<bool> {
    let stale_after_seconds = stale_after_seconds.max(60);
    let is_stale: bool = connection.query_row(
        "
        SELECT EXISTS(
            SELECT 1
            FROM source_adapters
            WHERE source_type = 'company_registry'
                AND enabled = 1
                AND (
                    last_success_at IS NULL
                    OR COALESCE(
                        ((julianday('now') - julianday(last_success_at)) * 86400.0) >= ?1,
                        1
                    )
                )
        )
        ",
        params![stale_after_seconds],
        |row| row.get(0),
    )?;

    Ok(is_stale)
}

pub(super) fn registry_lookup_result(
    row: &rusqlite::Row<'_>,
    source: &str,
) -> rusqlite::Result<CompanyLookupResult> {
    Ok(CompanyLookupResult {
        exchange: row.get(0)?,
        ticker: row.get(1)?,
        qualified_ticker: row.get(2)?,
        display_name: row.get(3)?,
        isin: row.get(4)?,
        source: source.to_owned(),
    })
}

pub(super) fn delete_company(connection: &Connection, company_id: &str) -> StorageResult<()> {
    connection.execute("DELETE FROM companies WHERE id = ?1", [company_id])?;

    Ok(())
}

/// The durable per-company investor-relations reports page URL (ADR 0029), or None.
pub(super) fn get_company_ir_reports_url(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Option<String>> {
    let url: Option<String> = connection.query_row(
        "SELECT ir_reports_url FROM companies WHERE id = ?1",
        [company_id],
        |row| row.get(0),
    )?;
    Ok(url.filter(|value| !value.trim().is_empty()))
}

/// Set (or clear, with None/empty) the per-company IR reports page URL.
pub(super) fn set_company_ir_reports_url(
    connection: &Connection,
    company_id: &str,
    url: Option<&str>,
) -> StorageResult<Option<String>> {
    let url = url.map(str::trim).filter(|value| !value.is_empty());
    let updated = connection.execute(
        "UPDATE companies SET ir_reports_url = ?2 WHERE id = ?1",
        rusqlite::params![company_id, url],
    )?;
    if updated == 0 {
        return Err(StorageError::MissingFinancialsReference {
            table: "companies".to_owned(),
            id: company_id.to_owned(),
        });
    }
    Ok(url.map(str::to_owned))
}

/// The company's current sector and its source (`registry` | `manual`), or `(None, None)`.
/// The company's `statement_type` discriminator (`industrial` / `bank` /
/// `insurance` / `broker` …), which selects canonical packs and gates the
/// health scores (ADR 0083 Decision 4 — financials are `NotApplicable`).
/// Defaults to `industrial` for an unknown company, matching the column
/// default and tolerating a missing row.
pub(super) fn get_statement_type(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<String> {
    let value = connection
        .query_row(
            "SELECT statement_type FROM companies WHERE id = ?1",
            [company_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    Ok(value.unwrap_or_else(|| "industrial".to_owned()))
}

pub(super) fn get_company_sector(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<(Option<String>, Option<String>)> {
    let row = connection.query_row(
        "SELECT sector, sector_source FROM companies WHERE id = ?1",
        [company_id],
        |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
                row.get::<_, Option<String>>(1)?,
            ))
        },
    )?;
    Ok(row)
}

/// Set a manual sector override (`sector_source='manual'`), which a later registry
/// refresh never clobbers (ADR 0067 Decision 3). An empty value clears back to no
/// manual override (leaving the field for the next registry refresh to fill).
pub(super) fn set_company_sector(
    connection: &Connection,
    company_id: &str,
    sector: Option<&str>,
) -> StorageResult<Option<String>> {
    let sector = sector.map(str::trim).filter(|value| !value.is_empty());
    let source = sector.map(|_| "manual");
    let updated = connection.execute(
        "UPDATE companies SET sector = ?2, sector_source = ?3,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ?1",
        rusqlite::params![company_id, sector, source],
    )?;
    if updated == 0 {
        return Err(StorageError::MissingFinancialsReference {
            table: "companies".to_owned(),
            id: company_id.to_owned(),
        });
    }
    Ok(sector.map(str::to_owned))
}

/// Distinct non-empty sectors across active directory entries, case-insensitive
/// sort — the registry-sourced taxonomy for manual-override suggestions.
/// The GPW and NewConnect taxonomies spell shared sectors with different
/// casing, so case variants fold into one entry (most frequent spelling wins;
/// SQLite bare-column-with-MAX picks that row deterministically).
pub(super) fn list_company_sectors(connection: &Connection) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "SELECT sector, MAX(n) FROM (
             SELECT sector, COUNT(*) AS n FROM company_registry_entries
             WHERE active = 1 AND sector IS NOT NULL AND TRIM(sector) <> ''
             GROUP BY sector
         )
         GROUP BY sector COLLATE NOCASE
         ORDER BY sector COLLATE NOCASE",
    )?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Latest non-superseded `shares_outstanding` fact for a company: the value
/// string plus a human period label, most recent period first (fiscal year,
/// then period end date, then recency of the row itself).
pub(super) fn latest_shares_outstanding(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Option<(String, String)>> {
    let mut statement = connection.prepare(
        "SELECT f.value_numeric, p.fiscal_year, p.period_type
         FROM financial_facts f
         JOIN financial_periods p ON p.id = f.period_id
         WHERE f.company_id = ?1
           AND f.definition_id = 'kpidef_shares_outstanding'
           AND NOT EXISTS (SELECT 1 FROM financial_facts s WHERE s.supersedes_id = f.id)
         ORDER BY p.fiscal_year DESC, IFNULL(p.period_end_date, '') DESC, f.created_at DESC
         LIMIT 1",
    )?;
    let row = statement
        .query_row([company_id], |row| {
            let value: String = row.get(0)?;
            let fiscal_year: i64 = row.get(1)?;
            let period_type: String = row.get(2)?;
            Ok((
                value,
                format!("{fiscal_year} {}", period_type.to_uppercase()),
            ))
        })
        .optional()?;
    Ok(row)
}

#[cfg(test)]
mod sector_tests {
    use crate::source_adapters::gpw_company_registry::GpwCompanyRegistryEntry;
    use crate::storage::{open_in_memory_database, AppState, NewCompany};

    fn cdr_entry(sector: Option<&str>) -> GpwCompanyRegistryEntry {
        GpwCompanyRegistryEntry {
            exchange: "GPW".to_owned(),
            ticker: "CDR".to_owned(),
            qualified_ticker: "GPW:CDR".to_owned(),
            display_name: "CD PROJEKT S.A.".to_owned(),
            isin: "PLOPTTC00011".to_owned(),
            source_url: "https://www.gpw.pl/spolka?isin=PLOPTTC00011".to_owned(),
            sector: sector.map(str::to_owned),
        }
    }

    fn cdr(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "gpw".to_owned(),
                ticker: "cdr".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    #[test]
    fn refresh_populates_sector_and_manual_override_is_never_clobbered() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let cid = cdr(&state);

        // A registry refresh carrying a sector propagates onto the tracked company.
        state
            .refresh_gpw_company_registry(&[cdr_entry(Some("Gry"))], "2026-05-31T12:00:00Z")
            .expect("refresh");
        assert_eq!(
            state.get_company_sector(&cid).expect("get"),
            (Some("Gry".to_owned()), Some("registry".to_owned()))
        );

        // A manual override wins...
        state
            .set_company_sector(&cid, Some("Rozrywka"))
            .expect("set");
        assert_eq!(
            state.get_company_sector(&cid).expect("get"),
            (Some("Rozrywka".to_owned()), Some("manual".to_owned()))
        );

        // ...and a later refresh must not clobber it.
        state
            .refresh_gpw_company_registry(&[cdr_entry(Some("Gry"))], "2026-06-30T12:00:00Z")
            .expect("refresh 2");
        assert_eq!(
            state.get_company_sector(&cid).expect("get"),
            (Some("Rozrywka".to_owned()), Some("manual".to_owned()))
        );
    }

    #[test]
    fn lists_the_distinct_registry_sector_taxonomy() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let mut bank = cdr_entry(Some("Banki"));
        bank.ticker = "PKO".to_owned();
        bank.qualified_ticker = "GPW:PKO".to_owned();
        bank.isin = "PLPKO0000016".to_owned();
        state
            .refresh_gpw_company_registry(&[cdr_entry(Some("Gry")), bank], "2026-05-31T12:00:00Z")
            .expect("refresh");

        let sectors = state
            .companies()
            .list_company_sectors()
            .expect("sectors should list");
        assert_eq!(sectors, vec!["Banki".to_owned(), "Gry".to_owned()]);
    }

    // The GPW and NewConnect taxonomies spell the same sector with different
    // casing ("usługi dla przedsiębiorstw" vs "Usługi dla Przedsiębiorstw");
    // the taxonomy command folds those into one entry, keeping the most
    // frequent spelling — a wall of near-duplicate suggestions is a UX defect
    // (owner report, 2026-07-14).
    #[test]
    fn sector_taxonomy_folds_case_variants_to_the_most_frequent_spelling() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let mut entries = Vec::new();
        for (i, sector) in ["usługi", "Usługi", "usługi", "Gry"].iter().enumerate() {
            let mut entry = cdr_entry(Some(sector));
            entry.ticker = format!("T{i:02}");
            entry.qualified_ticker = format!("GPW:T{i:02}");
            entry.isin = format!("PLTEST00{i:03}");
            entries.push(entry);
        }
        state
            .refresh_gpw_company_registry(&entries, "2026-05-31T12:00:00Z")
            .expect("refresh");

        let sectors = state
            .companies()
            .list_company_sectors()
            .expect("sectors should list");
        assert_eq!(sectors, vec!["Gry".to_owned(), "usługi".to_owned()]);
    }

    #[test]
    fn identical_refresh_does_not_churn_company_updated_at() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let cid = cdr(&state);
        state
            .refresh_gpw_company_registry(&[cdr_entry(Some("Gry"))], "2026-05-31T12:00:00Z")
            .expect("refresh 1");

        // Plant a sentinel updated_at; a byte-identical re-propagation must not touch it.
        let sentinel = "2020-01-01T00:00:00.000Z";
        {
            let raw = state.checkout_for_tests().expect("raw connection");
            raw.execute(
                "UPDATE companies SET updated_at = ?2 WHERE id = ?1",
                rusqlite::params![cid, sentinel],
            )
            .expect("plant sentinel");
        }

        state
            .refresh_gpw_company_registry(&[cdr_entry(Some("Gry"))], "2026-06-30T12:00:00Z")
            .expect("refresh 2");

        let raw = state.checkout_for_tests().expect("raw connection");
        let updated_at: String = raw
            .query_row(
                "SELECT updated_at FROM companies WHERE id = ?1",
                [&cid],
                |row| row.get(0),
            )
            .expect("read updated_at");
        assert_eq!(
            updated_at, sentinel,
            "an identical sector propagation must not bump updated_at"
        );
    }
}

#[cfg(test)]
mod ir_url_tests {
    use crate::storage::{open_in_memory_database, AppState, NewCompany};

    #[test]
    fn ir_reports_url_round_trips_and_clears() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company");

        assert_eq!(
            state.get_company_ir_reports_url(&company.id).expect("get"),
            None
        );

        let set = state
            .set_company_ir_reports_url(
                &company.id,
                Some("  https://www.cdprojekt.com/en/investors/  "),
            )
            .expect("set");
        assert_eq!(
            set.as_deref(),
            Some("https://www.cdprojekt.com/en/investors/")
        );
        assert_eq!(
            state.get_company_ir_reports_url(&company.id).expect("get"),
            Some("https://www.cdprojekt.com/en/investors/".to_owned())
        );

        // Empty clears it.
        state
            .set_company_ir_reports_url(&company.id, Some("   "))
            .expect("clear");
        assert_eq!(
            state.get_company_ir_reports_url(&company.id).expect("get"),
            None
        );
    }

    #[test]
    fn setting_ir_url_for_unknown_company_errors() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let error = state
            .set_company_ir_reports_url("company_missing", Some("https://example.com"))
            .expect_err("unknown company should error");
        assert!(error.to_string().contains("companies"));
    }
}
