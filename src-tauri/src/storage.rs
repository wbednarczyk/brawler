use std::path::Path;
use std::sync::Mutex;

use crate::source_adapters::gpw_espi_ebi::{GpwReportListing, ADAPTER_ID, DISPLAY_NAME};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("invalid setting value for {key}: {value}")]
    InvalidSettingValue { key: &'static str, value: String },
    #[error("invalid notebook value for {key}: {value}")]
    InvalidNotebookValue { key: &'static str, value: String },
}

pub type StorageResult<T> = Result<T, StorageError>;

pub struct AppState {
    connection: Mutex<Connection>,
}

impl AppState {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection: Mutex::new(connection),
        }
    }

    pub fn database_status(&self) -> StorageResult<DatabaseStatus> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        database_status(&connection)
    }

    pub fn list_companies(&self) -> StorageResult<Vec<Company>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_companies(&connection)
    }

    pub fn create_company(&self, input: NewCompany) -> StorageResult<Company> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        create_company(&connection, input)
    }

    pub fn lookup_company(
        &self,
        input: CompanyLookupInput,
    ) -> StorageResult<Option<CompanyLookupResult>> {
        Ok(lookup_company(input))
    }

    pub fn delete_company(&self, company_id: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        delete_company(&connection, company_id)
    }

    pub fn list_watchlists(&self) -> StorageResult<Vec<Watchlist>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_watchlists(&connection)
    }

    pub fn list_watchlist_memberships(&self) -> StorageResult<Vec<WatchlistMembership>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_watchlist_memberships(&connection)
    }

    pub fn create_watchlist(&self, input: NewWatchlist) -> StorageResult<Watchlist> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        create_watchlist(&connection, input)
    }

    pub fn add_company_to_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        add_company_to_watchlist(&connection, input)
    }

    pub fn remove_company_from_watchlist(&self, input: WatchlistCompanyInput) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        remove_company_from_watchlist(&connection, input)
    }

    pub fn list_feed_items(&self) -> StorageResult<Vec<FeedItem>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_feed_items(&connection)
    }

    pub fn list_unmatched_source_items(
        &self,
        adapter_id: &str,
    ) -> StorageResult<Vec<UnmatchedSourceItem>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_unmatched_source_items(&connection, adapter_id)
    }

    pub fn ingest_gpw_report_listings(
        &self,
        listings: &[GpwReportListing],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        ingest_gpw_report_listings(&mut connection, listings)
    }

    pub fn update_feed_item_state(&self, input: FeedItemStateInput) -> StorageResult<FeedItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        update_feed_item_state(&connection, input)
    }

    pub fn list_notebook_entries(&self, company_id: &str) -> StorageResult<Vec<NotebookEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_notebook_entries(&connection, company_id)
    }

    pub fn create_notebook_entry(&self, input: NewNotebookEntry) -> StorageResult<NotebookEntry> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        create_notebook_entry(&connection, input)
    }

    pub fn update_notebook_entry(
        &self,
        input: NotebookEntryUpdate,
    ) -> StorageResult<NotebookEntry> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        update_notebook_entry(&connection, input)
    }

    pub fn list_source_adapters(&self) -> StorageResult<Vec<SourceAdapter>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_source_adapters(&connection)
    }

    pub fn record_source_adapter_error(&self, adapter_id: &str, error: &str) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        record_source_adapter_error(&connection, adapter_id, error)
    }

    pub fn record_source_adapter_attempt(
        &self,
        adapter_id: &str,
        trigger: &str,
    ) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        record_source_adapter_attempt(&connection, adapter_id, trigger)
    }

    pub fn get_settings(&self) -> StorageResult<UserSettings> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        get_settings(&connection)
    }

    pub fn update_settings(&self, input: SettingsUpdate) -> StorageResult<UserSettings> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        update_settings(&connection, input)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub applied_migrations: i64,
    pub companies: i64,
    pub source_adapters: i64,
    pub settings: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Company {
    pub id: String,
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: Option<String>,
    pub cik: Option<String>,
    pub lei: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewCompany {
    pub exchange: String,
    pub ticker: String,
    pub display_name: String,
    pub isin: Option<String>,
    pub cik: Option<String>,
    pub lei: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Watchlist {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub company_count: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistMembership {
    pub watchlist_id: String,
    pub watchlist_name: String,
    pub company_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewWatchlist {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistCompanyInput {
    pub watchlist_id: String,
    pub company_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItem {
    pub id: String,
    pub company: String,
    #[serde(rename = "type")]
    pub item_type: String,
    pub source: String,
    pub time: String,
    pub title: String,
    pub unread: bool,
    pub saved: bool,
    pub source_url: String,
    pub language: String,
    pub published_at: String,
    pub fetched_at: String,
    pub attribution: String,
    pub summary: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItemStateInput {
    pub id: String,
    pub read: Option<bool>,
    pub saved: Option<bool>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceIngestionResult {
    pub adapter_id: String,
    pub items_fetched: usize,
    pub items_created: usize,
    pub items_matched: usize,
    pub items_unmatched: usize,
    pub fetched_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnmatchedSourceItem {
    pub id: String,
    pub adapter_id: String,
    pub company_name: String,
    pub title: String,
    pub source_url: String,
    pub published_at: String,
    pub fetched_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookEntry {
    pub id: String,
    pub company_id: String,
    pub title: String,
    pub body: String,
    pub body_format: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub origins: Vec<NotebookOrigin>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookOrigin {
    pub id: String,
    pub source_type: String,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub label: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNotebookEntry {
    pub company_id: String,
    pub title: String,
    pub body: String,
    pub body_format: Option<String>,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
    pub origins: Vec<NewNotebookOrigin>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotebookEntryUpdate {
    pub id: String,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub kind: String,
    pub claim_status: Option<String>,
    pub event_date: Option<String>,
    pub follow_up_after: Option<String>,
    pub follow_up_date: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewNotebookOrigin {
    pub source_type: String,
    pub source_id: Option<String>,
    pub source_url: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceAdapter {
    pub id: String,
    pub display_name: String,
    pub source_type: String,
    pub fetch_mode: String,
    pub enabled: bool,
    pub default_poll_interval_seconds: i64,
    pub source_url: String,
    pub rate_limit_policy: String,
    pub policy_note: String,
    pub last_attempt_at: Option<String>,
    pub last_trigger: Option<String>,
    pub last_success_at: Option<String>,
    pub last_error_at: Option<String>,
    pub last_error: Option<String>,
    pub last_items_fetched: Option<i64>,
    pub last_items_created: Option<i64>,
    pub last_items_matched: Option<i64>,
    pub last_items_unmatched: Option<i64>,
    pub markets: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderSettings {
    pub youtube_transcription_provider: String,
    pub general_analysis_provider: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserSettings {
    pub theme: String,
    pub accent_palette: String,
    pub poll_interval_seconds: i64,
    pub settings_source: &'static str,
    pub settings_import_export_format: String,
    pub yaml_import_export_status: &'static str,
    pub ai_providers: AiProviderSettings,
    pub ai_analysis_mode: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub theme: Option<String>,
    pub poll_interval_seconds: Option<i64>,
    pub youtube_transcription_provider: Option<String>,
    pub general_analysis_provider: Option<String>,
    pub ai_analysis_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLookupInput {
    pub exchange: String,
    pub ticker: Option<String>,
    pub display_name: Option<String>,
    pub isin: Option<String>,
}

#[derive(Debug, Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLookupResult {
    pub exchange: &'static str,
    pub ticker: &'static str,
    pub qualified_ticker: &'static str,
    pub display_name: &'static str,
    pub isin: &'static str,
    pub source: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct CompanyFixture {
    exchange: &'static str,
    ticker: &'static str,
    qualified_ticker: &'static str,
    display_name: &'static str,
    isin: &'static str,
}

const COMPANY_FIXTURES: &[CompanyFixture] = &[
    CompanyFixture {
        exchange: "GPW",
        ticker: "CDR",
        qualified_ticker: "GPW:CDR",
        display_name: "CD PROJEKT S.A.",
        isin: "PLOPTTC00011",
    },
    CompanyFixture {
        exchange: "GPW",
        ticker: "PKN",
        qualified_ticker: "GPW:PKN",
        display_name: "ORLEN S.A.",
        isin: "PLPKN0000018",
    },
    CompanyFixture {
        exchange: "GPW",
        ticker: "KGH",
        qualified_ticker: "GPW:KGH",
        display_name: "KGHM POLSKA MIEDZ S.A.",
        isin: "PLKGHM000017",
    },
    CompanyFixture {
        exchange: "GPW",
        ticker: "PZU",
        qualified_ticker: "GPW:PZU",
        display_name: "PZU S.A.",
        isin: "PLPZU0000011",
    },
];

#[derive(Debug, Clone, Copy)]
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "initial_schema",
        sql: include_str!("../migrations/0001_initial.sql"),
    },
    Migration {
        version: 2,
        name: "feed_item_display_company",
        sql: include_str!("../migrations/0002_feed_item_display_company.sql"),
    },
    Migration {
        version: 3,
        name: "notebook_entry_origins",
        sql: include_str!("../migrations/0003_notebook_entry_origins.sql"),
    },
    Migration {
        version: 4,
        name: "notebook_follow_ups",
        sql: include_str!("../migrations/0004_notebook_follow_ups.sql"),
    },
];

pub fn open_database(path: impl AsRef<Path>) -> StorageResult<Connection> {
    let mut connection = Connection::open(path)?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

pub fn open_in_memory_database() -> StorageResult<Connection> {
    let mut connection = Connection::open_in_memory()?;
    apply_migrations(&mut connection)?;
    Ok(connection)
}

fn database_status(connection: &Connection) -> StorageResult<DatabaseStatus> {
    Ok(DatabaseStatus {
        applied_migrations: count_rows(connection, "schema_migrations")?,
        companies: count_rows(connection, "companies")?,
        source_adapters: count_rows(connection, "source_adapters")?,
        settings: count_rows(connection, "settings")?,
    })
}

fn list_companies(connection: &Connection) -> StorageResult<Vec<Company>> {
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

fn create_company(connection: &Connection, input: NewCompany) -> StorageResult<Company> {
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

fn lookup_company(input: CompanyLookupInput) -> Option<CompanyLookupResult> {
    let exchange = input.exchange.trim().to_uppercase();
    let ticker = input.ticker.as_deref().map(normalize_lookup_value);
    let isin = input.isin.as_deref().map(normalize_lookup_value);
    let display_name = input.display_name.as_deref().map(normalize_name_lookup);

    COMPANY_FIXTURES
        .iter()
        .find(|fixture| {
            if fixture.exchange != exchange {
                return false;
            }

            if let Some(ticker) = ticker.as_deref().filter(|value| !value.is_empty()) {
                return fixture.ticker == ticker;
            }

            if let Some(isin) = isin.as_deref().filter(|value| !value.is_empty()) {
                return fixture.isin == isin;
            }

            if let Some(display_name) = display_name.as_deref().filter(|value| value.len() >= 3) {
                return normalize_name_lookup(fixture.display_name).contains(display_name);
            }

            false
        })
        .map(|fixture| CompanyLookupResult {
            exchange: fixture.exchange,
            ticker: fixture.ticker,
            qualified_ticker: fixture.qualified_ticker,
            display_name: fixture.display_name,
            isin: fixture.isin,
            source: "local_fixture",
        })
}

fn delete_company(connection: &Connection, company_id: &str) -> StorageResult<()> {
    connection.execute("DELETE FROM companies WHERE id = ?1", [company_id])?;

    Ok(())
}

fn list_watchlists(connection: &Connection) -> StorageResult<Vec<Watchlist>> {
    let mut statement = connection.prepare(
        "
        SELECT
            watchlists.id,
            watchlists.name,
            watchlists.description,
            COUNT(watchlist_companies.company_id) AS company_count
        FROM watchlists
        LEFT JOIN watchlist_companies
            ON watchlist_companies.watchlist_id = watchlists.id
        GROUP BY watchlists.id, watchlists.name, watchlists.description
        ORDER BY watchlists.name
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(Watchlist {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            company_count: row.get(3)?,
        })
    })?;

    let watchlists = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(watchlists)
}

fn list_watchlist_memberships(connection: &Connection) -> StorageResult<Vec<WatchlistMembership>> {
    let mut statement = connection.prepare(
        "
        SELECT
            watchlists.id,
            watchlists.name,
            watchlist_companies.company_id
        FROM watchlist_companies
        INNER JOIN watchlists
            ON watchlists.id = watchlist_companies.watchlist_id
        ORDER BY watchlists.name, watchlist_companies.company_id
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(WatchlistMembership {
            watchlist_id: row.get(0)?,
            watchlist_name: row.get(1)?,
            company_id: row.get(2)?,
        })
    })?;

    let memberships = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(memberships)
}

fn create_watchlist(connection: &Connection, input: NewWatchlist) -> StorageResult<Watchlist> {
    let name = input.name.trim().to_owned();
    let id = watchlist_id(&name);
    let description = empty_string_to_none(input.description);

    connection.execute(
        "
        INSERT INTO watchlists (id, name, description)
        VALUES (?1, ?2, ?3)
        ",
        params![id, name, description],
    )?;

    connection
        .query_row(
            "
            SELECT id, name, description, 0 AS company_count
            FROM watchlists
            WHERE id = ?1
            ",
            [id],
            |row| {
                Ok(Watchlist {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    company_count: row.get(3)?,
                })
            },
        )
        .map_err(StorageError::from)
}

fn add_company_to_watchlist(
    connection: &Connection,
    input: WatchlistCompanyInput,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT OR IGNORE INTO watchlist_companies (watchlist_id, company_id)
        VALUES (?1, ?2)
        ",
        params![input.watchlist_id, input.company_id],
    )?;

    Ok(())
}

fn remove_company_from_watchlist(
    connection: &Connection,
    input: WatchlistCompanyInput,
) -> StorageResult<()> {
    connection.execute(
        "
        DELETE FROM watchlist_companies
        WHERE watchlist_id = ?1
            AND company_id = ?2
        ",
        params![input.watchlist_id, input.company_id],
    )?;

    Ok(())
}

fn list_feed_items(connection: &Connection) -> StorageResult<Vec<FeedItem>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            COALESCE(display_company, 'Unmatched') AS company,
            type,
            source_name,
            COALESCE(published_at, fetched_at) AS item_time,
            title,
            read,
            saved,
            source_url,
            COALESCE(language, 'unknown') AS language,
            COALESCE(published_at, '') AS published_at,
            fetched_at,
            COALESCE(attribution, source_name) AS attribution,
            COALESCE(summary, '') AS summary
        FROM feed_items
        WHERE display_company IN (
            SELECT qualified_ticker FROM companies
        )
        ORDER BY COALESCE(published_at, fetched_at) DESC, fetched_at DESC, id
        ",
    )?;

    let rows = statement.query_map([], feed_item_from_row)?;
    let feed_items = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(feed_items)
}

fn list_unmatched_source_items(
    connection: &Connection,
    adapter_id: &str,
) -> StorageResult<Vec<UnmatchedSourceItem>> {
    let mut statement = connection.prepare(
        "
        SELECT
            feed_items.id,
            feed_items.source_adapter_id,
            COALESCE(feed_items.display_company, 'Unmatched') AS company_name,
            feed_items.title,
            feed_items.source_url,
            COALESCE(feed_items.published_at, '') AS published_at,
            feed_items.fetched_at
        FROM feed_items
        LEFT JOIN feed_item_companies
            ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_items.source_adapter_id = ?1
            AND feed_item_companies.feed_item_id IS NULL
            AND COALESCE(feed_items.display_company, '') NOT IN (
                SELECT qualified_ticker FROM companies
            )
        ORDER BY COALESCE(feed_items.published_at, feed_items.fetched_at) DESC,
            feed_items.fetched_at DESC,
            feed_items.id
        LIMIT 20
        ",
    )?;

    let rows = statement.query_map([adapter_id], |row| {
        Ok(UnmatchedSourceItem {
            id: row.get(0)?,
            adapter_id: row.get(1)?,
            company_name: row.get(2)?,
            title: row.get(3)?,
            source_url: row.get(4)?,
            published_at: row.get(5)?,
            fetched_at: row.get(6)?,
        })
    })?;

    let unmatched_items = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(unmatched_items)
}

fn ingest_gpw_report_listings(
    connection: &mut Connection,
    listings: &[GpwReportListing],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;
    let fetched_at = listings
        .first()
        .map(|listing| listing.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| current_timestamp(&transaction))?;

    for listing in listings {
        let feed_item_id = feed_item_id(&listing.dedupe_key);
        let matched_company = find_company_by_isin(&transaction, &listing.isin)?;
        let display_company = matched_company
            .as_ref()
            .map(|company| company.qualified_ticker.clone())
            .unwrap_or_else(|| listing.company_name.clone());
        let existed = feed_item_exists(&transaction, &feed_item_id)?;

        transaction.execute(
            "
            INSERT INTO feed_items (
                id,
                type,
                source_adapter_id,
                source_name,
                source_url,
                title,
                summary,
                body_text,
                language,
                published_at,
                fetched_at,
                dedupe_key,
                attribution,
                display_company
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, 'pl', ?7, ?8, ?9, 'GPW', ?10)
            ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
                type = excluded.type,
                source_name = excluded.source_name,
                source_url = excluded.source_url,
                title = excluded.title,
                language = excluded.language,
                published_at = excluded.published_at,
                fetched_at = excluded.fetched_at,
                attribution = excluded.attribution,
                display_company = excluded.display_company,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                feed_item_id,
                "Official report",
                ADAPTER_ID,
                DISPLAY_NAME,
                listing.detail_url,
                listing.title,
                listing.published_at,
                listing.fetched_at,
                listing.dedupe_key,
                display_company,
            ],
        )?;

        if !existed {
            items_created += 1;
        }

        transaction.execute(
            "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
            [&feed_item_id],
        )?;

        if let Some(company) = matched_company {
            transaction.execute(
                "
                INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                VALUES (?1, ?2, 'isin')
                ",
                params![feed_item_id, company.id],
            )?;
            items_matched += 1;
        } else {
            items_unmatched += 1;
        }
    }

    transaction.execute(
        "
        UPDATE source_adapters
        SET last_success_at = ?1,
            last_error_at = NULL,
            last_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![&fetched_at, ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_fetched",
        &listings.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_created",
        &items_created.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_unmatched",
        &items_unmatched.to_string(),
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: ADAPTER_ID.to_owned(),
        items_fetched: listings.len(),
        items_created,
        items_matched,
        items_unmatched,
        fetched_at: Some(fetched_at),
    })
}

struct MatchedCompany {
    id: String,
    qualified_ticker: String,
}

fn find_company_by_isin(
    connection: &Connection,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    if isin.trim().is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT id, qualified_ticker
            FROM companies
            WHERE isin = ?1
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            [isin.trim()],
            |row| {
                Ok(MatchedCompany {
                    id: row.get(0)?,
                    qualified_ticker: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

fn feed_item_exists(connection: &Connection, feed_item_id: &str) -> StorageResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM feed_items WHERE id = ?1)",
            [feed_item_id],
            |row| row.get(0),
        )
        .map_err(StorageError::from)
}

fn list_notebook_entries(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<NotebookEntry>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            company_id,
            title,
            body,
            body_format,
            kind,
            claim_status,
            event_date,
            follow_up_after,
            follow_up_date,
            created_at,
            updated_at
        FROM notebook_entries
        WHERE company_id = ?1
        ORDER BY updated_at DESC, created_at DESC, id
        ",
    )?;

    let rows = statement.query_map([company_id], |row| notebook_entry_from_row(connection, row))?;
    let entries = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(entries)
}

fn create_notebook_entry(
    connection: &Connection,
    input: NewNotebookEntry,
) -> StorageResult<NotebookEntry> {
    let title = input.title.trim().to_owned();
    let body = input.body.trim().to_owned();
    let body_format = input
        .body_format
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("markdown")
        .to_owned();
    let kind = input.kind.trim().to_owned();
    let claim_status = empty_string_to_none(input.claim_status);
    let tags = normalize_tags(input.tags);
    let id = notebook_entry_id(connection, &input.company_id, &title)?;

    validate_allowed_notebook_value("body_format", &body_format, &["markdown"])?;
    validate_allowed_notebook_value(
        "kind",
        &kind,
        &["manual", "observation", "claim", "question", "follow_up"],
    )?;

    if let Some(status) = claim_status.as_deref() {
        validate_allowed_notebook_value(
            "claim_status",
            status,
            &[
                "open",
                "delivered",
                "partially_delivered",
                "missed",
                "unknown",
                "not_applicable",
            ],
        )?;
    }

    for origins in &input.origins {
        validate_allowed_notebook_value(
            "origins.source_type",
            origins.source_type.trim(),
            &[
                "feed_item",
                "transcript_segment",
                "ai_analysis",
                "manual",
                "external_url",
            ],
        )?;
    }

    connection.execute(
        "
        INSERT INTO notebook_entries (
            id,
            company_id,
            title,
            body,
            body_format,
            kind,
            claim_status,
            event_date,
            follow_up_after,
            follow_up_date
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ",
        params![
            id,
            input.company_id,
            title,
            body,
            body_format,
            kind,
            claim_status,
            empty_string_to_none(input.event_date),
            empty_string_to_none(input.follow_up_after),
            empty_string_to_none(input.follow_up_date),
        ],
    )?;

    for tag in tags {
        connection.execute(
            "
            INSERT OR IGNORE INTO notebook_entry_tags (notebook_entry_id, tag)
            VALUES (?1, ?2)
            ",
            params![&id, tag],
        )?;
    }

    for (index, origins) in input.origins.into_iter().enumerate() {
        let source_type = origins.source_type.trim().to_owned();

        connection.execute(
            "
            INSERT INTO notebook_entry_origins (
                id,
                notebook_entry_id,
                source_type,
                source_id,
                source_url,
                label
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                notebook_origin_id(&id, &source_type, index),
                id,
                source_type,
                empty_string_to_none(origins.source_id),
                empty_string_to_none(origins.source_url),
                empty_string_to_none(origins.label),
            ],
        )?;
    }

    get_notebook_entry(connection, &id)
}

fn update_notebook_entry(
    connection: &Connection,
    input: NotebookEntryUpdate,
) -> StorageResult<NotebookEntry> {
    let id = input.id;
    let title = input.title.trim().to_owned();
    let body = input.body.trim().to_owned();
    let kind = input.kind.trim().to_owned();
    let claim_status = empty_string_to_none(input.claim_status);
    let tags = normalize_tags(input.tags);

    validate_allowed_notebook_value(
        "kind",
        &kind,
        &["manual", "observation", "claim", "question", "follow_up"],
    )?;

    if let Some(status) = claim_status.as_deref() {
        validate_allowed_notebook_value(
            "claim_status",
            status,
            &[
                "open",
                "delivered",
                "partially_delivered",
                "missed",
                "unknown",
                "not_applicable",
            ],
        )?;
    }

    connection.execute(
        "
        UPDATE notebook_entries
        SET
            title = ?2,
            body = ?3,
            kind = ?4,
            claim_status = ?5,
            event_date = ?6,
            follow_up_after = ?7,
            follow_up_date = ?8,
            updated_at = datetime('now')
        WHERE id = ?1
        ",
        params![
            &id,
            title,
            body,
            kind,
            claim_status,
            empty_string_to_none(input.event_date),
            empty_string_to_none(input.follow_up_after),
            empty_string_to_none(input.follow_up_date),
        ],
    )?;

    connection.execute(
        "DELETE FROM notebook_entry_tags WHERE notebook_entry_id = ?1",
        [&id],
    )?;

    for tag in tags {
        connection.execute(
            "
            INSERT OR IGNORE INTO notebook_entry_tags (notebook_entry_id, tag)
            VALUES (?1, ?2)
            ",
            params![&id, tag],
        )?;
    }

    get_notebook_entry(connection, &id)
}

fn list_source_adapters(connection: &Connection) -> StorageResult<Vec<SourceAdapter>> {
    let mut statement = connection.prepare(
        "
        SELECT
            source_adapters.id,
            source_adapters.display_name,
            source_adapters.source_type,
            source_adapters.fetch_mode,
            source_adapters.enabled,
            source_adapters.default_poll_interval_seconds,
            'https://www.gpw.pl/komunikaty' AS source_url,
            'Serialized requests, default 15 minute poll interval' AS rate_limit_policy,
            'Uses the public GPW ESPI/EBI listing page. Paid processed GPW data products may be evaluated later.' AS policy_note,
            source_adapter_attempts.state_value AS last_attempt_at,
            source_adapter_triggers.state_value AS last_trigger,
            source_adapters.last_success_at,
            source_adapters.last_error_at,
            source_adapters.last_error,
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_items_fetched'
            ) AS INTEGER) AS last_items_fetched,
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_items_created'
            ) AS INTEGER) AS last_items_created,
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_items_matched'
            ) AS INTEGER) AS last_items_matched,
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_items_unmatched'
            ) AS INTEGER) AS last_items_unmatched,
            COALESCE(GROUP_CONCAT(source_adapter_markets.market, ','), '') AS markets
        FROM source_adapters
        LEFT JOIN source_adapter_state AS source_adapter_attempts
            ON source_adapter_attempts.source_adapter_id = source_adapters.id
            AND source_adapter_attempts.state_key = 'last_attempt_at'
        LEFT JOIN source_adapter_state AS source_adapter_triggers
            ON source_adapter_triggers.source_adapter_id = source_adapters.id
            AND source_adapter_triggers.state_key = 'last_trigger'
        LEFT JOIN source_adapter_markets
            ON source_adapter_markets.source_adapter_id = source_adapters.id
        GROUP BY
            source_adapters.id,
            source_adapters.display_name,
            source_adapters.source_type,
            source_adapters.fetch_mode,
            source_adapters.enabled,
            source_adapters.default_poll_interval_seconds,
            source_adapter_attempts.state_value,
            source_adapter_triggers.state_value,
            source_adapters.last_success_at,
            source_adapters.last_error_at,
            source_adapters.last_error
        ORDER BY source_adapters.display_name
        ",
    )?;

    let rows = statement.query_map([], |row| {
        let markets: String = row.get(18)?;

        Ok(SourceAdapter {
            id: row.get(0)?,
            display_name: row.get(1)?,
            source_type: row.get(2)?,
            fetch_mode: row.get(3)?,
            enabled: row.get(4)?,
            default_poll_interval_seconds: row.get(5)?,
            source_url: row.get(6)?,
            rate_limit_policy: row.get(7)?,
            policy_note: row.get(8)?,
            last_attempt_at: row.get(9)?,
            last_trigger: row.get(10)?,
            last_success_at: row.get(11)?,
            last_error_at: row.get(12)?,
            last_error: row.get(13)?,
            last_items_fetched: row.get(14)?,
            last_items_created: row.get(15)?,
            last_items_matched: row.get(16)?,
            last_items_unmatched: row.get(17)?,
            markets: markets
                .split(',')
                .filter(|market| !market.is_empty())
                .map(str::to_owned)
                .collect(),
        })
    })?;

    let adapters = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(adapters)
}

fn record_source_adapter_attempt(
    connection: &Connection,
    adapter_id: &str,
    trigger: &str,
) -> StorageResult<()> {
    let attempted_at = current_timestamp(connection)?;
    set_source_adapter_state(connection, adapter_id, "last_attempt_at", &attempted_at)?;
    set_source_adapter_state(connection, adapter_id, "last_trigger", trigger)?;

    Ok(())
}

fn set_source_adapter_state(
    connection: &Connection,
    adapter_id: &str,
    key: &str,
    value: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO source_adapter_state (source_adapter_id, state_key, state_value)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(source_adapter_id, state_key) DO UPDATE SET
            state_value = excluded.state_value,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![adapter_id, key, value],
    )?;

    Ok(())
}

fn current_timestamp(connection: &Connection) -> StorageResult<String> {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(StorageError::from)
}

fn record_source_adapter_error(
    connection: &Connection,
    adapter_id: &str,
    error: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE source_adapters
        SET last_error_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            last_error = ?1,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![error, adapter_id],
    )?;

    Ok(())
}

fn get_settings(connection: &Connection) -> StorageResult<UserSettings> {
    Ok(UserSettings {
        theme: setting_string(connection, "theme")?,
        accent_palette: setting_string(connection, "accent_palette")?,
        poll_interval_seconds: setting_i64(connection, "poll_interval_seconds")?,
        settings_source: "sqlite",
        settings_import_export_format: setting_string(connection, "settings_import_export_format")?,
        yaml_import_export_status: "accepted_deferred",
        ai_providers: AiProviderSettings {
            youtube_transcription_provider: setting_string(
                connection,
                "youtube_transcription_provider",
            )?,
            general_analysis_provider: empty_setting_to_none(setting_string(
                connection,
                "general_analysis_provider",
            )?),
        },
        ai_analysis_mode: setting_string(connection, "ai_analysis_mode")?,
    })
}

fn update_settings(connection: &Connection, input: SettingsUpdate) -> StorageResult<UserSettings> {
    if let Some(theme) = input.theme {
        validate_allowed_setting("theme", &theme, &["dark", "light", "system"])?;
        update_setting(connection, "theme", &theme)?;
    }

    if let Some(poll_interval_seconds) = input.poll_interval_seconds {
        update_setting(
            connection,
            "poll_interval_seconds",
            &poll_interval_seconds.to_string(),
        )?;
    }

    if let Some(youtube_transcription_provider) = input.youtube_transcription_provider {
        update_setting(
            connection,
            "youtube_transcription_provider",
            &youtube_transcription_provider,
        )?;
    }

    if let Some(general_analysis_provider) = input.general_analysis_provider {
        update_setting(
            connection,
            "general_analysis_provider",
            &general_analysis_provider,
        )?;
    }

    if let Some(ai_analysis_mode) = input.ai_analysis_mode {
        validate_allowed_setting(
            "ai_analysis_mode",
            &ai_analysis_mode,
            &["source_grounded", "opinionated"],
        )?;
        update_setting(connection, "ai_analysis_mode", &ai_analysis_mode)?;
    }

    get_settings(connection)
}

fn setting_string(connection: &Connection, key: &'static str) -> StorageResult<String> {
    connection
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
            row.get(0)
        })
        .map_err(StorageError::from)
}

fn setting_i64(connection: &Connection, key: &'static str) -> StorageResult<i64> {
    let value = setting_string(connection, key)?;

    value
        .parse::<i64>()
        .map_err(|_| StorageError::InvalidSettingValue { key, value })
}

fn update_setting(connection: &Connection, key: &'static str, value: &str) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE settings
        SET value = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE key = ?1
        ",
        params![key, value],
    )?;

    Ok(())
}

fn validate_allowed_setting(key: &'static str, value: &str, allowed: &[&str]) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn empty_setting_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}

fn update_feed_item_state(
    connection: &Connection,
    input: FeedItemStateInput,
) -> StorageResult<FeedItem> {
    if let Some(read) = input.read {
        connection.execute(
            "
            UPDATE feed_items
            SET read = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![input.id, read],
        )?;
    }

    if let Some(saved) = input.saved {
        connection.execute(
            "
            UPDATE feed_items
            SET saved = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![input.id, saved],
        )?;
    }

    get_feed_item(connection, &input.id)
}

fn get_feed_item(connection: &Connection, feed_item_id: &str) -> StorageResult<FeedItem> {
    connection
        .query_row(
            "
            SELECT
                id,
                COALESCE(display_company, 'Unmatched') AS company,
                type,
                source_name,
                COALESCE(published_at, fetched_at) AS item_time,
                title,
                read,
                saved,
                source_url,
                COALESCE(language, 'unknown') AS language,
                COALESCE(published_at, '') AS published_at,
                fetched_at,
                COALESCE(attribution, source_name) AS attribution,
                COALESCE(summary, '') AS summary
            FROM feed_items
            WHERE id = ?1
            ",
            [feed_item_id],
            feed_item_from_row,
        )
        .map_err(StorageError::from)
}

fn feed_item_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FeedItem> {
    let read: bool = row.get(6)?;

    Ok(FeedItem {
        id: row.get(0)?,
        company: row.get(1)?,
        item_type: row.get(2)?,
        source: row.get(3)?,
        time: row.get(4)?,
        title: row.get(5)?,
        unread: !read,
        saved: row.get(7)?,
        source_url: row.get(8)?,
        language: row.get(9)?,
        published_at: row.get(10)?,
        fetched_at: row.get(11)?,
        attribution: row.get(12)?,
        summary: row.get(13)?,
    })
}

fn get_notebook_entry(
    connection: &Connection,
    notebook_entry_id: &str,
) -> StorageResult<NotebookEntry> {
    connection
        .query_row(
            "
            SELECT
                id,
                company_id,
                title,
                body,
                body_format,
                kind,
                claim_status,
                event_date,
                follow_up_after,
                follow_up_date,
                created_at,
                updated_at
            FROM notebook_entries
            WHERE id = ?1
            ",
            [notebook_entry_id],
            |row| notebook_entry_from_row(connection, row),
        )
        .map_err(StorageError::from)
}

fn notebook_entry_from_row(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<NotebookEntry> {
    let id: String = row.get(0)?;

    Ok(NotebookEntry {
        tags: notebook_entry_tags(connection, &id)?,
        origins: notebook_entry_origins(connection, &id)?,
        id,
        company_id: row.get(1)?,
        title: row.get(2)?,
        body: row.get(3)?,
        body_format: row.get(4)?,
        kind: row.get(5)?,
        claim_status: row.get(6)?,
        event_date: row.get(7)?,
        follow_up_after: row.get(8)?,
        follow_up_date: row.get(9)?,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn notebook_entry_tags(
    connection: &Connection,
    notebook_entry_id: &str,
) -> rusqlite::Result<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT tag
        FROM notebook_entry_tags
        WHERE notebook_entry_id = ?1
        ORDER BY tag
        ",
    )?;
    let rows = statement.query_map([notebook_entry_id], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
}

fn notebook_entry_origins(
    connection: &Connection,
    notebook_entry_id: &str,
) -> rusqlite::Result<Vec<NotebookOrigin>> {
    let mut statement = connection.prepare(
        "
        SELECT id, source_type, source_id, source_url, label, created_at
        FROM notebook_entry_origins
        WHERE notebook_entry_id = ?1
        ORDER BY created_at, id
        ",
    )?;
    let rows = statement.query_map([notebook_entry_id], |row| {
        Ok(NotebookOrigin {
            id: row.get(0)?,
            source_type: row.get(1)?,
            source_id: row.get(2)?,
            source_url: row.get(3)?,
            label: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}

fn notebook_entry_id(
    connection: &Connection,
    company_id: &str,
    title: &str,
) -> StorageResult<String> {
    let base_id = format!("note_{}_{}", slug_part(company_id), slug_part(title));
    let existing_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM notebook_entries WHERE id = ?1 OR id LIKE ?2",
        params![&base_id, format!("{base_id}_%")],
        |row| row.get(0),
    )?;

    if existing_count == 0 {
        Ok(base_id)
    } else {
        Ok(format!("{base_id}_{}", existing_count + 1))
    }
}

fn notebook_origin_id(notebook_entry_id: &str, source_type: &str, index: usize) -> String {
    format!(
        "note_origin_{}_{}_{}",
        slug_part(notebook_entry_id),
        slug_part(source_type),
        index + 1
    )
}

fn feed_item_id(dedupe_key: &str) -> String {
    format!("feed_{}", slug_part(dedupe_key))
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    let mut normalized = tags
        .into_iter()
        .map(|tag| tag.trim().to_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect::<Vec<_>>();

    normalized.sort();
    normalized.dedup();
    normalized
}

fn validate_allowed_notebook_value(
    key: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidNotebookValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn seed_fixture_feed_items(connection: &Connection) -> StorageResult<()> {
    let feed_item_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM feed_items", [], |row| row.get(0))?;

    if feed_item_count > 0 {
        return Ok(());
    }

    let fixtures = [
        (
            "feed_fixture_cdr_report",
            "Official report",
            "GPW ESPI/EBI",
            "https://www.gpw.pl/komunikaty",
            "Current report placeholder for watchlist company",
            "Fixture official report used to validate feed filtering and detail rendering.",
            "pl",
            "2026-05-29T09:12:00Z",
            "2026-05-29T09:15:00Z",
            "gpw-espi-ebi:fixture:cdr-report",
            false,
            false,
            "GPW",
            "GPW:CDR",
        ),
        (
            "feed_fixture_pkn_news",
            "News",
            "Fixture feed",
            "https://example.local/fixture/pkn",
            "Fixture item proving the inbox layout can scan dense rows",
            "Saved fixture item used to validate the saved filter before real ingestion exists.",
            "en",
            "2026-05-28T16:00:00Z",
            "2026-05-28T16:03:00Z",
            "fixture:pkn-news",
            true,
            true,
            "Fixture",
            "GPW:PKN",
        ),
        (
            "feed_fixture_kgh_transcript",
            "Transcript",
            "Local fixture",
            "https://example.local/fixture/kgh-transcript",
            "Transcript-derived note candidate waits for future provider work",
            "Transcript placeholder for future video and notebook workflows.",
            "en",
            "2026-05-25T10:00:00Z",
            "2026-05-25T10:00:00Z",
            "fixture:kgh-transcript",
            true,
            false,
            "Fixture",
            "GPW:KGH",
        ),
        (
            "feed_fixture_pzu_report",
            "Official report",
            "GPW ESPI/EBI",
            "https://www.gpw.pl/komunikaty",
            "PZU governance report placeholder",
            "Fourth fixture item keeps the sample feed aligned with local GPW lookup companies.",
            "pl",
            "2026-05-24T12:00:00Z",
            "2026-05-24T12:05:00Z",
            "gpw-espi-ebi:fixture:pzu-report",
            true,
            false,
            "GPW",
            "GPW:PZU",
        ),
    ];

    for fixture in fixtures {
        connection.execute(
            "
            INSERT OR IGNORE INTO feed_items (
                id,
                type,
                source_adapter_id,
                source_name,
                source_url,
                title,
                summary,
                language,
                published_at,
                fetched_at,
                dedupe_key,
                read,
                saved,
                attribution,
                display_company
            ) VALUES (?1, ?2, 'gpw-espi-ebi', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            ",
            params![
                fixture.0, fixture.1, fixture.2, fixture.3, fixture.4, fixture.5, fixture.6,
                fixture.7, fixture.8, fixture.9, fixture.10, fixture.11, fixture.12, fixture.13
            ],
        )?;
    }

    Ok(())
}

fn apply_migrations(connection: &mut Connection) -> StorageResult<()> {
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        );
        ",
    )?;

    let transaction = connection.transaction()?;

    for migration in MIGRATIONS {
        let already_applied: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [migration.version],
            |row| row.get(0),
        )?;

        if already_applied {
            continue;
        }

        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO schema_migrations (version, name) VALUES (?1, ?2)",
            (migration.version, migration.name),
        )?;
    }

    transaction.commit()?;
    seed_fixture_feed_items(connection)?;
    Ok(())
}

fn count_rows(connection: &Connection, table_name: &str) -> StorageResult<i64> {
    let sql = format!("SELECT COUNT(*) FROM {table_name}");

    connection
        .query_row(&sql, [], |row| row.get(0))
        .map_err(StorageError::from)
}

fn company_id(exchange: &str, ticker: &str) -> String {
    format!("company_{}_{}", slug_part(exchange), slug_part(ticker))
}

fn watchlist_id(name: &str) -> String {
    format!("watchlist_{}", slug_part(name))
}

fn slug_part(value: &str) -> String {
    value
        .chars()
        .filter_map(|character| {
            if character.is_ascii_alphanumeric() {
                Some(character.to_ascii_lowercase())
            } else if character == '_' || character == '-' {
                Some('_')
            } else {
                None
            }
        })
        .collect()
}

fn empty_string_to_none(value: Option<String>) -> Option<String> {
    value.and_then(|inner| {
        let trimmed = inner.trim().to_owned();

        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_lookup_value(value: &str) -> String {
    value.trim().to_uppercase()
}

fn normalize_name_lookup(value: &str) -> String {
    value.trim().to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_clean_database_with_initial_schema() {
        let connection = open_in_memory_database().expect("database should initialize");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("schema_migrations should exist");

        assert_eq!(migration_count, 4);

        let company_table_exists: bool = connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM sqlite_master
                    WHERE type = 'table' AND name = 'companies'
                )",
                [],
                |row| row.get(0),
            )
            .expect("companies table lookup should work");

        assert!(company_table_exists);
    }

    #[test]
    fn seeds_default_settings_and_gpw_adapter() {
        let connection = open_in_memory_database().expect("database should initialize");

        let theme: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'theme'",
                [],
                |row| row.get(0),
            )
            .expect("theme setting should be seeded");

        let adapter_name: String = connection
            .query_row(
                "SELECT display_name FROM source_adapters WHERE id = 'gpw-espi-ebi'",
                [],
                |row| row.get(0),
            )
            .expect("GPW adapter should be seeded");

        assert_eq!(theme, "dark");
        assert_eq!(adapter_name, "GPW ESPI/EBI");
    }

    #[test]
    fn enforces_exchange_qualified_ticker_uniqueness() {
        let connection = open_in_memory_database().expect("database should initialize");

        connection
            .execute(
                "
                INSERT INTO companies (
                    id,
                    exchange,
                    ticker,
                    qualified_ticker,
                    display_name
                ) VALUES (?1, ?2, ?3, ?4, ?5)
                ",
                (
                    "company_gpw_cdr",
                    "GPW",
                    "CDR",
                    "GPW:CDR",
                    "CD PROJEKT S.A.",
                ),
            )
            .expect("first company insert should pass");

        let duplicate = connection.execute(
            "
            INSERT INTO companies (
                id,
                exchange,
                ticker,
                qualified_ticker,
                display_name
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            (
                "company_gpw_cdr_duplicate",
                "GPW",
                "CDR",
                "GPW:CDR",
                "Duplicate",
            ),
        );

        assert!(duplicate.is_err());
    }

    #[test]
    fn creates_and_lists_company_through_storage_api() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let created = state
            .create_company(NewCompany {
                exchange: "gpw".to_owned(),
                ticker: "cdr".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");

        let companies = state.list_companies().expect("companies should be listed");

        assert_eq!(created.id, "company_gpw_cdr");
        assert_eq!(created.qualified_ticker, "GPW:CDR");
        assert_eq!(companies.len(), 1);
        assert_eq!(companies[0].display_name, "CD PROJEKT S.A.");
    }

    #[test]
    fn reports_database_status() {
        let connection = open_in_memory_database().expect("database should initialize");
        let status = database_status(&connection).expect("status should be available");

        assert_eq!(status.applied_migrations, 4);
        assert_eq!(status.companies, 0);
        assert_eq!(status.source_adapters, 1);
        assert_eq!(status.settings, 7);
    }

    #[test]
    fn seeds_and_lists_fixture_feed_items() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("tracked fixture company should create");
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "PKN".to_owned(),
                display_name: "ORLEN S.A.".to_owned(),
                isin: Some("PLPKN0000018".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("tracked fixture company should create");
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "KGH".to_owned(),
                display_name: "KGHM POLSKA MIEDZ S.A.".to_owned(),
                isin: Some("PLKGHM000017".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("tracked fixture company should create");
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "PZU".to_owned(),
                display_name: "PZU S.A.".to_owned(),
                isin: Some("PLPZU0000011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("tracked fixture company should create");

        let feed_items = state.list_feed_items().expect("feed items should list");

        assert_eq!(feed_items.len(), 4);
        assert_eq!(feed_items[0].id, "feed_fixture_cdr_report");
        assert_eq!(feed_items[0].company, "GPW:CDR");
        assert!(feed_items[0].unread);
    }

    #[test]
    fn persists_feed_item_read_and_saved_state() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("tracked fixture company should create");

        let updated = state
            .update_feed_item_state(FeedItemStateInput {
                id: "feed_fixture_cdr_report".to_owned(),
                read: Some(true),
                saved: Some(true),
            })
            .expect("feed item state should update");

        assert!(!updated.unread);
        assert!(updated.saved);

        let feed_items = state.list_feed_items().expect("feed items should list");
        let cdr = feed_items
            .iter()
            .find(|item| item.id == "feed_fixture_cdr_report")
            .expect("CDR fixture should remain present");

        assert!(!cdr.unread);
        assert!(cdr.saved);
    }

    #[test]
    fn ingests_gpw_listings_and_matches_tracked_company_by_isin() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "NTC".to_owned(),
                display_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
                isin: Some("PLECMNG00019".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("tracked company should create");

        let result = state
            .ingest_gpw_report_listings(&[
                GpwReportListing {
                    report_type: "Bieżący".to_owned(),
                    system: "ESPI".to_owned(),
                    report_number: "7/2026".to_owned(),
                    company_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
                    isin: "PLECMNG00019".to_owned(),
                    title: "Oświadczenie w sprawie formy przekazywania raportów kwartalnych."
                        .to_owned(),
                    detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=123456".to_owned(),
                    published_at: "2026-05-30T17:13:31+02:00".to_owned(),
                    fetched_at: "2026-05-30T17:30:00Z".to_owned(),
                    dedupe_key: "gpw-espi-ebi:espi:PLECMNG00019:7/2026:2026-05-30T17:13:31+02:00"
                        .to_owned(),
                },
                GpwReportListing {
                    report_type: "Bieżący".to_owned(),
                    system: "ESPI".to_owned(),
                    report_number: "9/2026".to_owned(),
                    company_name: "UNTRACKED S.A.".to_owned(),
                    isin: "PLUNTRK00001".to_owned(),
                    title: "Untracked company report".to_owned(),
                    detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=999999".to_owned(),
                    published_at: "2026-05-30T18:13:31+02:00".to_owned(),
                    fetched_at: "2026-05-30T18:30:00Z".to_owned(),
                    dedupe_key: "gpw-espi-ebi:espi:PLUNTRK00001:9/2026:2026-05-30T18:13:31+02:00"
                        .to_owned(),
                },
            ])
            .expect("listings should ingest");

        assert_eq!(result.items_fetched, 2);
        assert_eq!(result.items_created, 2);
        assert_eq!(result.items_matched, 1);
        assert_eq!(result.items_unmatched, 1);

        let adapters = state
            .list_source_adapters()
            .expect("source adapters should list");
        let adapter = adapters
            .iter()
            .find(|adapter| adapter.id == ADAPTER_ID)
            .expect("GPW adapter should exist");

        assert_eq!(adapter.last_items_fetched, Some(2));
        assert_eq!(adapter.last_items_created, Some(2));
        assert_eq!(adapter.last_items_matched, Some(1));
        assert_eq!(adapter.last_items_unmatched, Some(1));

        let visible_items = state.list_feed_items().expect("feed items should list");
        let ntc = visible_items
            .iter()
            .find(|item| item.company == "GPW:NTC")
            .expect("matched GPW listing should be visible");

        assert_eq!(ntc.source, "GPW ESPI/EBI");
        assert_eq!(ntc.item_type, "Official report");
        assert_eq!(ntc.attribution, "GPW");
        assert_eq!(ntc.language, "pl");

        assert!(visible_items
            .iter()
            .all(|item| item.title != "Untracked company report"));

        let unmatched_items = state
            .list_unmatched_source_items(ADAPTER_ID)
            .expect("unmatched source diagnostics should list");
        let untracked = unmatched_items
            .iter()
            .find(|item| item.title == "Untracked company report")
            .expect("unmatched ingested listing should be diagnosable");

        assert_eq!(untracked.company_name, "UNTRACKED S.A.");
    }

    #[test]
    fn records_successful_zero_item_gpw_refresh() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let result = state
            .ingest_gpw_report_listings(&[])
            .expect("zero-item source refresh should record success");

        assert_eq!(result.items_fetched, 0);
        assert_eq!(result.items_created, 0);
        assert_eq!(result.items_matched, 0);
        assert_eq!(result.items_unmatched, 0);
        assert!(result.fetched_at.is_some());

        let adapters = state
            .list_source_adapters()
            .expect("source adapters should list");
        let adapter = adapters
            .iter()
            .find(|adapter| adapter.id == ADAPTER_ID)
            .expect("GPW adapter should exist");

        assert_eq!(adapter.last_success_at, result.fetched_at);
        assert!(adapter.last_error_at.is_none());
        assert!(adapter.last_error.is_none());
        assert_eq!(adapter.last_items_fetched, Some(0));
        assert_eq!(adapter.last_items_created, Some(0));
        assert_eq!(adapter.last_items_matched, Some(0));
        assert_eq!(adapter.last_items_unmatched, Some(0));
    }

    #[test]
    fn creates_and_lists_notebook_entries_for_company() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");

        let entry = state
            .create_notebook_entry(NewNotebookEntry {
                company_id: company.id.clone(),
                title: "Management claim about release schedule".to_owned(),
                body: "Management said the next milestone should happen in two quarters."
                    .to_owned(),
                body_format: None,
                tags: vec!["Product".to_owned(), " management-guidance ".to_owned()],
                kind: "claim".to_owned(),
                claim_status: Some("open".to_owned()),
                event_date: Some("2026-05-29".to_owned()),
                follow_up_after: Some("2026-Q4".to_owned()),
                follow_up_date: Some("2026-11-30".to_owned()),
                origins: vec![NewNotebookOrigin {
                    source_type: "feed_item".to_owned(),
                    source_id: Some("feed_fixture_cdr_report".to_owned()),
                    source_url: Some("https://www.gpw.pl/komunikaty".to_owned()),
                    label: Some("GPW report".to_owned()),
                }],
            })
            .expect("notebook entry should be created");

        let entries = state
            .list_notebook_entries(&company.id)
            .expect("notebook entries should list");

        assert_eq!(entry.body_format, "markdown");
        assert_eq!(entry.kind, "claim");
        assert_eq!(entry.claim_status.as_deref(), Some("open"));
        assert_eq!(entry.tags, vec!["management-guidance", "product"]);
        assert_eq!(entry.origins.len(), 1);
        assert_eq!(entry.origins[0].source_type, "feed_item");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, entry.id);

        let updated = state
            .update_notebook_entry(NotebookEntryUpdate {
                id: entry.id.clone(),
                title: "Updated release schedule claim".to_owned(),
                body: "Management clarified the next milestone date.".to_owned(),
                tags: vec!["product".to_owned(), "clarified".to_owned()],
                kind: "claim".to_owned(),
                claim_status: Some("unknown".to_owned()),
                event_date: Some("2026-05-29".to_owned()),
                follow_up_after: Some("2026-Q3".to_owned()),
                follow_up_date: None,
            })
            .expect("notebook entry should update");

        assert_eq!(updated.title, "Updated release schedule claim");
        assert_eq!(
            updated.body,
            "Management clarified the next milestone date."
        );
        assert_eq!(updated.claim_status.as_deref(), Some("unknown"));
        assert_eq!(updated.follow_up_after.as_deref(), Some("2026-Q3"));
        assert_eq!(updated.tags, vec!["clarified", "product"]);
        assert_eq!(updated.origins.len(), 1);
        assert_eq!(updated.origins[0].source_type, "feed_item");
        assert_eq!(
            updated.origins[0].source_id.as_deref(),
            Some("feed_fixture_cdr_report")
        );
        assert_eq!(
            updated.origins[0].source_url.as_deref(),
            Some("https://www.gpw.pl/komunikaty")
        );
        assert_eq!(updated.origins[0].label.as_deref(), Some("GPW report"));
    }

    #[test]
    fn lists_seeded_source_adapters() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let adapters = state
            .list_source_adapters()
            .expect("source adapters should list");

        assert_eq!(adapters.len(), 1);
        assert_eq!(adapters[0].id, "gpw-espi-ebi");
        assert_eq!(adapters[0].display_name, "GPW ESPI/EBI");
        assert_eq!(adapters[0].markets, vec!["GPW".to_owned()]);
        assert!(adapters[0].enabled);
    }

    #[test]
    fn records_source_adapter_error_state() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        state
            .record_source_adapter_error(ADAPTER_ID, "network timeout")
            .expect("source adapter error should record");

        let adapters = state
            .list_source_adapters()
            .expect("source adapters should list");
        let adapter = adapters
            .iter()
            .find(|adapter| adapter.id == ADAPTER_ID)
            .expect("GPW adapter should exist");

        assert_eq!(adapter.last_error.as_deref(), Some("network timeout"));
        assert!(adapter.last_error_at.is_some());
    }

    #[test]
    fn records_source_adapter_attempt_state() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        state
            .record_source_adapter_attempt(ADAPTER_ID, "scheduler")
            .expect("source adapter attempt should record");

        let adapters = state
            .list_source_adapters()
            .expect("source adapters should list");
        let adapter = adapters
            .iter()
            .find(|adapter| adapter.id == ADAPTER_ID)
            .expect("GPW adapter should exist");

        assert!(adapter.last_attempt_at.is_some());
        assert_eq!(adapter.last_trigger.as_deref(), Some("scheduler"));
    }

    #[test]
    fn reads_default_settings_from_sqlite() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let settings = state.get_settings().expect("settings should load");

        assert_eq!(settings.theme, "dark");
        assert_eq!(settings.accent_palette, "night-neon");
        assert_eq!(settings.poll_interval_seconds, 900);
        assert_eq!(settings.settings_source, "sqlite");
        assert_eq!(settings.settings_import_export_format, "yaml");
        assert_eq!(settings.yaml_import_export_status, "accepted_deferred");
        assert_eq!(
            settings.ai_providers.youtube_transcription_provider,
            "gemini"
        );
        assert!(settings.ai_providers.general_analysis_provider.is_none());
        assert_eq!(settings.ai_analysis_mode, "source_grounded");
    }

    #[test]
    fn updates_settings_through_storage_api() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let settings = state
            .update_settings(SettingsUpdate {
                theme: Some("light".to_owned()),
                poll_interval_seconds: Some(600),
                youtube_transcription_provider: None,
                general_analysis_provider: None,
                ai_analysis_mode: None,
            })
            .expect("settings should update");

        assert_eq!(settings.theme, "light");
        assert_eq!(settings.poll_interval_seconds, 600);

        let persisted = state.get_settings().expect("settings should persist");

        assert_eq!(persisted.theme, "light");
        assert_eq!(persisted.poll_interval_seconds, 600);
    }

    #[test]
    fn rejects_invalid_theme_setting() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let result = state.update_settings(SettingsUpdate {
            theme: Some("sepia".to_owned()),
            poll_interval_seconds: None,
            youtube_transcription_provider: None,
            general_analysis_provider: None,
            ai_analysis_mode: None,
        });

        assert!(result.is_err());
    }

    #[test]
    fn looks_up_company_fixture_by_ticker() {
        let result = lookup_company(CompanyLookupInput {
            exchange: "gpw".to_owned(),
            ticker: Some("cdr".to_owned()),
            display_name: None,
            isin: None,
        })
        .expect("fixture should match");

        assert_eq!(result.qualified_ticker, "GPW:CDR");
        assert_eq!(result.display_name, "CD PROJEKT S.A.");
        assert_eq!(result.isin, "PLOPTTC00011");
    }

    #[test]
    fn looks_up_company_fixture_by_isin() {
        let result = lookup_company(CompanyLookupInput {
            exchange: "GPW".to_owned(),
            ticker: None,
            display_name: None,
            isin: Some("plpzu0000011".to_owned()),
        })
        .expect("fixture should match");

        assert_eq!(result.ticker, "PZU");
        assert_eq!(result.display_name, "PZU S.A.");
    }

    #[test]
    fn deletes_company_through_storage_api() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let created = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");

        state
            .delete_company(&created.id)
            .expect("company should be deleted");

        let companies = state.list_companies().expect("companies should be listed");

        assert!(companies.is_empty());
    }

    #[test]
    fn creates_watchlist_and_assigns_company() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");

        let watchlist = state
            .create_watchlist(NewWatchlist {
                name: "Main GPW".to_owned(),
                description: Some("Primary Polish watchlist".to_owned()),
            })
            .expect("watchlist should be created");

        state
            .add_company_to_watchlist(WatchlistCompanyInput {
                watchlist_id: watchlist.id,
                company_id: company.id,
            })
            .expect("company should be assigned");

        let watchlists = state.list_watchlists().expect("watchlists should list");

        assert_eq!(watchlists.len(), 1);
        assert_eq!(watchlists[0].name, "Main GPW");
        assert_eq!(watchlists[0].company_count, 1);
    }

    #[test]
    fn lists_watchlist_memberships() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");

        let watchlist = state
            .create_watchlist(NewWatchlist {
                name: "Main GPW".to_owned(),
                description: None,
            })
            .expect("watchlist should be created");

        state
            .add_company_to_watchlist(WatchlistCompanyInput {
                watchlist_id: watchlist.id.clone(),
                company_id: company.id.clone(),
            })
            .expect("company should be assigned");

        let memberships = state
            .list_watchlist_memberships()
            .expect("memberships should list");

        assert_eq!(memberships.len(), 1);
        assert_eq!(memberships[0].watchlist_id, watchlist.id);
        assert_eq!(memberships[0].watchlist_name, "Main GPW");
        assert_eq!(memberships[0].company_id, company.id);
    }

    #[test]
    fn removes_company_from_watchlist() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
                isin: Some("PLOPTTC00011".to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company should be created");

        let watchlist = state
            .create_watchlist(NewWatchlist {
                name: "Main GPW".to_owned(),
                description: None,
            })
            .expect("watchlist should be created");

        state
            .add_company_to_watchlist(WatchlistCompanyInput {
                watchlist_id: watchlist.id.clone(),
                company_id: company.id.clone(),
            })
            .expect("company should be assigned");

        state
            .remove_company_from_watchlist(WatchlistCompanyInput {
                watchlist_id: watchlist.id,
                company_id: company.id,
            })
            .expect("company should be removed");

        let watchlists = state.list_watchlists().expect("watchlists should list");

        assert_eq!(watchlists[0].company_count, 0);
    }
}
