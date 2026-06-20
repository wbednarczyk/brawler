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
    let transaction = connection.transaction()?;
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
                active
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)
            ON CONFLICT(exchange, ticker) DO UPDATE SET
                qualified_ticker = excluded.qualified_ticker,
                display_name = excluded.display_name,
                isin = excluded.isin,
                source_adapter_id = excluded.source_adapter_id,
                source_url = excluded.source_url,
                fetched_at = excluded.fetched_at,
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
            ],
        )?;
        entries_upserted += 1;
    }

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
