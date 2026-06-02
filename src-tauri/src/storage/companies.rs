use super::sources::set_source_adapter_state;
use super::*;

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

pub(super) fn refresh_gpw_company_registry(
    connection: &mut Connection,
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
        [GPW_REGISTRY_ADAPTER_ID],
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
                GPW_REGISTRY_ADAPTER_ID,
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
        params![GPW_REGISTRY_ADAPTER_ID, fetched_at],
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
        params![fetched_at, GPW_REGISTRY_ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        GPW_REGISTRY_ADAPTER_ID,
        "last_items_fetched",
        &entries.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        GPW_REGISTRY_ADAPTER_ID,
        "last_items_created",
        &entries_upserted.to_string(),
    )?;

    transaction.commit()?;

    Ok(CompanyRegistryRefreshResult {
        adapter_id: GPW_REGISTRY_ADAPTER_ID.to_owned(),
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
                WHERE exchange = ?1
                    AND ticker = ?2
                    AND active = 1
                ORDER BY qualified_ticker
                LIMIT 1
                ",
                params![exchange, ticker],
                |row| registry_lookup_result(row, "gpw_registry"),
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
                WHERE exchange = ?1
                    AND isin = ?2
                    AND active = 1
                ORDER BY qualified_ticker
                LIMIT 1
                ",
                params![exchange, isin],
                |row| registry_lookup_result(row, "gpw_registry"),
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
                WHERE exchange = ?1
                    AND UPPER(display_name) LIKE '%' || ?2 || '%'
                    AND active = 1
                ORDER BY qualified_ticker
                LIMIT 1
                ",
                params![exchange, display_name],
                |row| registry_lookup_result(row, "gpw_registry"),
            )
            .optional()
            .map_err(StorageError::from);
    }

    Ok(None)
}

pub(super) fn gpw_company_registry_needs_bootstrap_refresh(
    connection: &Connection,
) -> StorageResult<bool> {
    let active_count: i64 = connection.query_row(
        "
        SELECT COUNT(*)
        FROM company_registry_entries
        WHERE source_adapter_id = ?1
            AND active = 1
        ",
        [GPW_REGISTRY_ADAPTER_ID],
        |row| row.get(0),
    )?;

    Ok(active_count == 0)
}

pub(super) fn gpw_company_registry_is_stale(
    connection: &Connection,
    stale_after_seconds: i64,
) -> StorageResult<bool> {
    let stale_after_seconds = stale_after_seconds.max(60);
    let is_stale: bool = connection.query_row(
        "
        SELECT COALESCE(
            (
                SELECT
                    last_success_at IS NULL
                    OR COALESCE(
                        ((julianday('now') - julianday(last_success_at)) * 86400.0) >= ?1,
                        1
                    )
                FROM source_adapters
                WHERE id = ?2
            ),
            1
        )
        ",
        params![stale_after_seconds, GPW_REGISTRY_ADAPTER_ID],
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
