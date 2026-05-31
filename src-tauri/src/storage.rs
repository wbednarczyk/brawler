use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::source_adapters::bankier_company::{
    BankierCompanyAttachment, BankierCompanyIdentifiers, BankierCompanyItem, BankierCompanyTarget,
    ADAPTER_ID as BANKIER_COMPANY_ADAPTER_ID, ATTRIBUTION as BANKIER_COMPANY_ATTRIBUTION,
    DISPLAY_NAME as BANKIER_COMPANY_DISPLAY_NAME, SOURCE_URL as BANKIER_COMPANY_SOURCE_URL,
};
use crate::source_adapters::bankier_rss::{
    BankierRssItem, ADAPTER_ID as BANKIER_RSS_ADAPTER_ID, ATTRIBUTION as BANKIER_RSS_ATTRIBUTION,
    DISPLAY_NAME as BANKIER_RSS_DISPLAY_NAME, SOURCE_URL as BANKIER_RSS_SOURCE_URL,
};
use crate::source_adapters::gpw_company_registry::{
    GpwCompanyRegistryEntry, ADAPTER_ID as GPW_REGISTRY_ADAPTER_ID,
    SOURCE_URL as GPW_REGISTRY_SOURCE_URL,
};
use crate::source_adapters::gpw_espi_ebi::{
    GpwReportAttachment, GpwReportListing, ADAPTER_ID, DISPLAY_NAME,
};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const PORTAL_ANALIZ_SOURCE_URL: &str = "https://portalanaliz.pl/";
const BANKIER_FIRMA_RSS_SOURCE_URL: &str = "https://www.bankier.pl/rss/firma.xml";
const BANKIER_WIADOMOSCI_RSS_SOURCE_URL: &str = "https://www.bankier.pl/rss/wiadomosci.xml";
#[cfg(test)]
const PORTAL_ANALIZ_ADAPTER_ID: &str = "portal-analiz";

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

#[derive(Clone)]
pub struct AppState {
    connection: Arc<Mutex<Connection>>,
}

impl AppState {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection: Arc::new(Mutex::new(connection)),
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
        let connection = self.connection.lock().expect("database mutex poisoned");

        lookup_company(&connection, input)
    }

    pub fn gpw_company_registry_needs_bootstrap_refresh(&self) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        gpw_company_registry_needs_bootstrap_refresh(&connection)
    }

    pub fn gpw_company_registry_is_stale(&self, stale_after_seconds: i64) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        gpw_company_registry_is_stale(&connection, stale_after_seconds)
    }

    pub fn refresh_gpw_company_registry(
        &self,
        entries: &[GpwCompanyRegistryEntry],
        fetched_at: &str,
    ) -> StorageResult<CompanyRegistryRefreshResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        refresh_gpw_company_registry(&mut connection, entries, fetched_at)
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

    pub fn ingest_bankier_rss_items(
        &self,
        items: &[BankierRssItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        ingest_bankier_rss_items(&mut connection, items)
    }

    pub fn list_bankier_company_targets(&self) -> StorageResult<Vec<BankierCompanyTarget>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_bankier_company_targets(&connection)
    }

    pub fn upsert_bankier_company_identifiers(
        &self,
        company_id: &str,
        identifiers: &BankierCompanyIdentifiers,
    ) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        upsert_bankier_company_identifiers(&connection, company_id, identifiers)
    }

    pub fn list_bankier_company_detail_cached_urls(&self) -> StorageResult<Vec<String>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_bankier_company_detail_cached_urls(&connection)
    }

    pub fn ingest_bankier_company_items(
        &self,
        items: &[BankierCompanyItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        ingest_bankier_company_items(&mut connection, items)
    }

    pub fn tracks_gpw_listing_company(&self, ticker: &str, isin: &str) -> StorageResult<bool> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        Ok(find_company_for_gpw_listing(&connection, ticker, isin)?.is_some())
    }

    pub fn update_feed_item_state(&self, input: FeedItemStateInput) -> StorageResult<FeedItem> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        update_feed_item_state(&connection, input)
    }

    pub fn prune_old_feed_items(&self, retention_days: i64) -> StorageResult<FeedPruneResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        prune_old_feed_items(&mut connection, retention_days)
    }

    pub fn delete_unsaved_feed_items(&self) -> StorageResult<FeedDeleteResult> {
        let mut connection = self.connection.lock().expect("database mutex poisoned");

        delete_unsaved_feed_items(&mut connection)
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

    pub fn list_company_registry_entries(&self) -> StorageResult<Vec<CompanyRegistryEntry>> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        list_company_registry_entries(&connection)
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

    pub fn record_source_adapter_state(
        &self,
        adapter_id: &str,
        key: &str,
        value: &str,
    ) -> StorageResult<()> {
        let connection = self.connection.lock().expect("database mutex poisoned");

        set_source_adapter_state(&connection, adapter_id, key, value)
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
    pub body_text: String,
    pub attachments: Vec<FeedItemAttachment>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedItemAttachment {
    pub id: String,
    pub label: String,
    pub url: String,
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
    pub detail_items_attempted: usize,
    pub detail_items_stored: usize,
    pub detail_items_failed: usize,
    pub fetched_at: Option<String>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeedPruneResult {
    pub retention_days: i64,
    pub items_deleted: usize,
    pub pruned_at: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FeedDeleteResult {
    pub items_deleted: usize,
    pub deleted_at: String,
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
    pub last_detail_items_attempted: Option<i64>,
    pub last_detail_items_stored: Option<i64>,
    pub last_detail_items_failed: Option<i64>,
    pub last_detail_warning: Option<String>,
    pub markets: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyRegistryEntry {
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: Option<String>,
    pub source_url: String,
    pub fetched_at: String,
    pub tracked: bool,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLookupInput {
    pub exchange: String,
    pub ticker: Option<String>,
    pub display_name: Option<String>,
    pub isin: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CompanyLookupResult {
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub isin: String,
    pub source: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompanyRegistryRefreshResult {
    pub adapter_id: String,
    pub entries_fetched: usize,
    pub entries_upserted: usize,
    pub entries_deactivated: usize,
    pub fetched_at: String,
}

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
    Migration {
        version: 5,
        name: "feed_item_attachments",
        sql: include_str!("../migrations/0005_feed_item_attachments.sql"),
    },
    Migration {
        version: 6,
        name: "company_registry",
        sql: include_str!("../migrations/0006_company_registry.sql"),
    },
    Migration {
        version: 7,
        name: "bankier_market_rss",
        sql: include_str!("../migrations/0007_bankier_market_rss.sql"),
    },
    Migration {
        version: 8,
        name: "portal_analiz_source_placeholder",
        sql: include_str!("../migrations/0008_portal_analiz_source_placeholder.sql"),
    },
    Migration {
        version: 9,
        name: "feed_item_duplicate_signatures",
        sql: include_str!("../migrations/0009_feed_item_duplicate_signatures.sql"),
    },
    Migration {
        version: 10,
        name: "bankier_company_komunikaty",
        sql: include_str!("../migrations/0010_bankier_company_komunikaty.sql"),
    },
    Migration {
        version: 11,
        name: "disable_gpw_espi_ebi",
        sql: include_str!("../migrations/0011_disable_gpw_espi_ebi.sql"),
    },
    Migration {
        version: 12,
        name: "bankier_reviewed_rss_placeholders",
        sql: include_str!("../migrations/0012_bankier_reviewed_rss_placeholders.sql"),
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

fn refresh_gpw_company_registry(
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

fn lookup_company(
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

fn lookup_company_registry(
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

fn gpw_company_registry_needs_bootstrap_refresh(connection: &Connection) -> StorageResult<bool> {
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

fn gpw_company_registry_is_stale(
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

fn registry_lookup_result(
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
            COALESCE(summary, '') AS summary,
            COALESCE(body_text, '') AS body_text,
            source_adapter_id
        FROM feed_items
        WHERE display_company IN (
            SELECT qualified_ticker FROM companies
        )
        ORDER BY COALESCE(published_at, fetched_at) DESC, fetched_at DESC, id
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok((
            feed_item_from_row(connection, row)?,
            row.get::<_, String>(15)?,
        ))
    })?;
    let listed_feed_items = rows.collect::<Result<Vec<_>, _>>()?;
    let feed_items = suppress_duplicate_bankier_company_items(listed_feed_items);

    Ok(feed_items)
}

fn suppress_duplicate_bankier_company_items(
    listed_feed_items: Vec<(FeedItem, String)>,
) -> Vec<FeedItem> {
    let gpw_titles = listed_feed_items
        .iter()
        .filter(|(_, adapter_id)| adapter_id == ADAPTER_ID)
        .map(|(item, _)| (item.company.clone(), comparable_official_title(&item.title)))
        .collect::<Vec<_>>();

    listed_feed_items
        .into_iter()
        .filter_map(|(item, adapter_id)| {
            let is_duplicate_bankier_company_item = adapter_id == BANKIER_COMPANY_ADAPTER_ID
                && gpw_titles.iter().any(|(company, title)| {
                    company == &item.company && title == &comparable_official_title(&item.title)
                });

            if is_duplicate_bankier_company_item {
                None
            } else {
                Some(item)
            }
        })
        .collect()
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
        let matched_company =
            find_company_for_gpw_listing(&transaction, &listing.company_ticker, &listing.isin)?;
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
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, 'pl', ?8, ?9, ?10, 'GPW', ?11)
            ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
                type = excluded.type,
                source_name = excluded.source_name,
                source_url = excluded.source_url,
                title = excluded.title,
                body_text = COALESCE(excluded.body_text, feed_items.body_text),
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
                listing.body_text.as_deref(),
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
                VALUES (?1, ?2, ?3)
                ",
                params![feed_item_id, company.id, company.match_type],
            )?;
            items_matched += 1;
        } else {
            items_unmatched += 1;
        }

        if listing.body_text.is_some() {
            replace_feed_item_attachments(&transaction, &feed_item_id, &listing.attachments)?;
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
        detail_items_attempted: 0,
        detail_items_stored: listings
            .iter()
            .filter(|listing| listing.body_text.is_some())
            .count(),
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

fn ingest_bankier_rss_items(
    connection: &mut Connection,
    items: &[BankierRssItem],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let tracked_companies = list_media_match_companies(&transaction)?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;
    let fetched_at = items
        .first()
        .map(|item| item.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| current_timestamp(&transaction))?;
    for item in items {
        let matched_companies = find_companies_for_media_item(&tracked_companies, item);
        let duplicate_signature = media_duplicate_signature(item, &matched_companies);
        let existing_feed_item_id = find_bankier_feed_item_by_source_url(&transaction, &item.link)?;
        let existing_duplicate_feed_item_id = if existing_feed_item_id.is_none() {
            find_media_feed_item_by_duplicate_signature(
                &transaction,
                duplicate_signature.as_deref(),
                BANKIER_RSS_ADAPTER_ID,
            )?
        } else {
            None
        };
        let feed_item_id = existing_feed_item_id
            .clone()
            .or(existing_duplicate_feed_item_id.clone())
            .unwrap_or_else(|| feed_item_id(&item.dedupe_key));
        let display_company = matched_companies
            .first()
            .map(|company| company.qualified_ticker.clone())
            .unwrap_or_else(|| BANKIER_RSS_ATTRIBUTION.to_owned());
        let existed = existing_feed_item_id.is_some()
            || existing_duplicate_feed_item_id.is_some()
            || feed_item_exists(&transaction, &feed_item_id)?;

        if existing_feed_item_id.is_some() {
            update_bankier_feed_item(
                &transaction,
                &feed_item_id,
                item,
                &display_company,
                duplicate_signature.as_deref(),
            )?;
        } else if existing_duplicate_feed_item_id.is_none() {
            insert_bankier_feed_item(
                &transaction,
                &feed_item_id,
                item,
                &display_company,
                duplicate_signature.as_deref(),
            )?;
        } else {
            record_media_duplicate_seen(&transaction, &feed_item_id, item)?;
        }

        if !existed {
            items_created += 1;
        }

        if existing_duplicate_feed_item_id.is_none() {
            transaction.execute(
                "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
                [&feed_item_id],
            )?;
        }

        if matched_companies.is_empty() {
            items_unmatched += 1;
        } else {
            items_matched += 1;
            for company in matched_companies {
                transaction.execute(
                    "
                    INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![
                        feed_item_id,
                        company.id,
                        if existing_duplicate_feed_item_id.is_some() {
                            "media_duplicate"
                        } else {
                            "media_signal"
                        },
                    ],
                )?;
            }
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
        params![&fetched_at, BANKIER_RSS_ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_fetched",
        &items.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_created",
        &items_created.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_unmatched",
        &items_unmatched.to_string(),
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: BANKIER_RSS_ADAPTER_ID.to_owned(),
        items_fetched: items.len(),
        items_created,
        items_matched,
        items_unmatched,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

fn list_bankier_company_targets(
    connection: &Connection,
) -> StorageResult<Vec<BankierCompanyTarget>> {
    let mut statement = connection.prepare(
        "
        SELECT
            companies.id,
            companies.ticker,
            companies.qualified_ticker,
            (
                SELECT source_value
                FROM company_source_ids
                WHERE company_id = companies.id
                    AND source_adapter_id = ?1
                    AND source_key = 'instrument_slug'
                LIMIT 1
            ) AS bankier_slug,
            (
                SELECT source_value
                FROM company_source_ids
                WHERE company_id = companies.id
                    AND source_adapter_id = ?1
                    AND source_key = 'tag_id'
                LIMIT 1
            ) AS bankier_tag_id
        FROM companies
        WHERE companies.exchange = 'GPW'
        ORDER BY companies.qualified_ticker
        ",
    )?;

    let rows = statement.query_map([BANKIER_COMPANY_ADAPTER_ID], |row| {
        Ok(BankierCompanyTarget {
            company_id: row.get(0)?,
            ticker: row.get(1)?,
            qualified_ticker: row.get(2)?,
            bankier_slug: row.get(3)?,
            bankier_tag_id: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn upsert_bankier_company_identifiers(
    connection: &Connection,
    company_id: &str,
    identifiers: &BankierCompanyIdentifiers,
) -> StorageResult<()> {
    upsert_company_source_id(
        connection,
        company_id,
        BANKIER_COMPANY_ADAPTER_ID,
        "instrument_slug",
        &identifiers.slug,
    )?;
    upsert_company_source_id(
        connection,
        company_id,
        BANKIER_COMPANY_ADAPTER_ID,
        "tag_id",
        &identifiers.tag_id,
    )?;

    Ok(())
}

fn upsert_company_source_id(
    connection: &Connection,
    company_id: &str,
    source_adapter_id: &str,
    source_key: &str,
    source_value: &str,
) -> StorageResult<()> {
    let id = format!(
        "company_source_{}_{}_{}",
        slug_part(company_id),
        slug_part(source_adapter_id),
        slug_part(source_key)
    );

    let updated = connection.execute(
        "
        UPDATE company_source_ids
        SET source_value = ?1
        WHERE company_id = ?2
            AND source_adapter_id = ?3
            AND source_key = ?4
        ",
        params![source_value, company_id, source_adapter_id, source_key],
    )?;

    if updated > 0 {
        return Ok(());
    }

    connection.execute(
        "
        INSERT INTO company_source_ids (
            id,
            company_id,
            source_adapter_id,
            source_key,
            source_value
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(source_adapter_id, source_key, source_value) DO UPDATE SET
            company_id = excluded.company_id
        ",
        params![id, company_id, source_adapter_id, source_key, source_value],
    )?;

    Ok(())
}

fn list_bankier_company_detail_cached_urls(connection: &Connection) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT source_url
        FROM feed_items
        WHERE source_adapter_id = ?1
            AND NULLIF(TRIM(COALESCE(body_text, '')), '') IS NOT NULL
        ",
    )?;
    let rows = statement.query_map([BANKIER_COMPANY_ADAPTER_ID], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn ingest_bankier_company_items(
    connection: &mut Connection,
    items: &[BankierCompanyItem],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;
    let fetched_at = items
        .first()
        .map(|item| item.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| current_timestamp(&transaction))?;
    let detail_items_attempted = items
        .iter()
        .filter(|item| item.detail_fetch_attempted)
        .count();
    let detail_items_stored = items.iter().filter(|item| item.body_text.is_some()).count();
    let detail_items_failed = detail_items_attempted.saturating_sub(detail_items_stored);

    for item in items {
        let existing_feed_item_id =
            find_bankier_company_feed_item_by_source_url(&transaction, &item.link)?;
        let existing_gpw_item_id = if existing_feed_item_id.is_none() {
            find_existing_gpw_report_for_bankier_company_item(&transaction, item)?
        } else {
            None
        };

        if let Some(feed_item_id) = existing_gpw_item_id {
            record_bankier_company_duplicate_seen(&transaction, &feed_item_id, item)?;
            items_matched += 1;
            transaction.execute(
                "
                INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    feed_item_id,
                    item.company_id,
                    "secondary_official_duplicate",
                ],
            )?;
        } else {
            let feed_item_id = existing_feed_item_id
                .clone()
                .unwrap_or_else(|| feed_item_id(&item.dedupe_key));
            let existed =
                existing_feed_item_id.is_some() || feed_item_exists(&transaction, &feed_item_id)?;

            upsert_bankier_company_feed_item(&transaction, &feed_item_id, item)?;
            transaction.execute(
                "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
                [&feed_item_id],
            )?;

            if !existed {
                items_created += 1;
            }

            if item.company_id.trim().is_empty() {
                items_unmatched += 1;
            } else {
                items_matched += 1;
                transaction.execute(
                    "
                    INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![feed_item_id, item.company_id, "bankier_tag_id"],
                )?;
            }
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
        params![&fetched_at, BANKIER_COMPANY_ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_fetched",
        &items.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_created",
        &items_created.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_unmatched",
        &items_unmatched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_detail_items_attempted",
        &detail_items_attempted.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_detail_items_stored",
        &detail_items_stored.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_detail_items_failed",
        &detail_items_failed.to_string(),
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: BANKIER_COMPANY_ADAPTER_ID.to_owned(),
        items_fetched: items.len(),
        items_created,
        items_matched,
        items_unmatched,
        detail_items_attempted,
        detail_items_stored,
        detail_items_failed,
        fetched_at: Some(fetched_at),
    })
}

fn find_bankier_company_feed_item_by_source_url(
    connection: &Connection,
    source_url: &str,
) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "
            SELECT id
            FROM feed_items
            WHERE source_adapter_id = ?1
                AND source_url = ?2
            ORDER BY updated_at DESC, id
            LIMIT 1
            ",
            params![BANKIER_COMPANY_ADAPTER_ID, source_url],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

fn find_existing_gpw_report_for_bankier_company_item(
    connection: &Connection,
    item: &BankierCompanyItem,
) -> StorageResult<Option<String>> {
    let mut statement = connection.prepare(
        "
        SELECT feed_items.id, feed_items.title
        FROM feed_items
        INNER JOIN feed_item_companies
            ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_item_companies.company_id = ?1
            AND feed_items.source_adapter_id = ?2
            AND feed_items.type = 'Official report'
        ORDER BY COALESCE(feed_items.published_at, feed_items.fetched_at) DESC,
            feed_items.updated_at DESC
        LIMIT 100
        ",
    )?;
    let rows = statement.query_map(params![&item.company_id, ADAPTER_ID], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let comparable_title = comparable_official_title(&item.title);

    for row in rows {
        let (feed_item_id, title) = row?;
        if comparable_official_title(&title) == comparable_title {
            return Ok(Some(feed_item_id));
        }
    }

    Ok(None)
}

fn upsert_bankier_company_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierCompanyItem,
) -> StorageResult<()> {
    connection.execute(
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
            display_company,
            duplicate_signature
        ) VALUES (?1, 'Official report', ?2, ?3, ?4, ?5, ?6, ?7, 'pl', ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
            type = excluded.type,
            source_name = excluded.source_name,
            source_url = excluded.source_url,
            title = excluded.title,
            summary = excluded.summary,
            body_text = COALESCE(excluded.body_text, feed_items.body_text),
            language = excluded.language,
            published_at = excluded.published_at,
            fetched_at = excluded.fetched_at,
            attribution = excluded.attribution,
            display_company = excluded.display_company,
            duplicate_signature = excluded.duplicate_signature,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            feed_item_id,
            BANKIER_COMPANY_ADAPTER_ID,
            BANKIER_COMPANY_DISPLAY_NAME,
            &item.link,
            &item.title,
            empty_string_to_none(Some(format_bankier_company_summary(item))),
            item.body_text.as_deref(),
            item.published_at.as_deref(),
            &item.fetched_at,
            &item.dedupe_key,
            BANKIER_COMPANY_ATTRIBUTION,
            &item.qualified_ticker,
            &item.duplicate_signature,
        ],
    )?;
    if item.body_text.is_some() {
        replace_bankier_company_feed_item_attachments(connection, feed_item_id, &item.attachments)?;
    }

    Ok(())
}

fn replace_bankier_company_feed_item_attachments(
    connection: &Connection,
    feed_item_id: &str,
    attachments: &[BankierCompanyAttachment],
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM feed_item_attachments WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;

    for (position, attachment) in attachments.iter().enumerate() {
        connection.execute(
            "
            INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                format!(
                    "feed_attachment_{}_{}",
                    slug_part(feed_item_id),
                    slug_part(&attachment.url)
                ),
                feed_item_id,
                attachment.label,
                attachment.url,
                position as i64,
            ],
        )?;
    }

    Ok(())
}

fn record_bankier_company_duplicate_seen(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierCompanyItem,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE feed_items
        SET fetched_at = CASE
                WHEN fetched_at < ?2 THEN ?2
                ELSE fetched_at
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![feed_item_id, &item.fetched_at],
    )?;

    Ok(())
}

fn format_bankier_company_summary(item: &BankierCompanyItem) -> String {
    let source_type = match item.pub_id {
        3 => "ESPI",
        379 => "EBI",
        _ => "Bankier komunikat",
    };

    if item
        .body_text
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        item.summary.trim().to_owned()
    } else {
        source_type.to_owned()
    }
}

fn comparable_official_title(value: &str) -> String {
    let title = value
        .split_once(':')
        .map(|(_, title)| title)
        .unwrap_or(value)
        .trim();

    normalize_media_match_text(title)
}

fn find_bankier_feed_item_by_source_url(
    connection: &Connection,
    source_url: &str,
) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "
            SELECT id
            FROM feed_items
            WHERE source_adapter_id = ?1
                AND source_url = ?2
            ORDER BY updated_at DESC, id
            LIMIT 1
            ",
            params![BANKIER_RSS_ADAPTER_ID, source_url],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

fn find_media_feed_item_by_duplicate_signature(
    connection: &Connection,
    duplicate_signature: Option<&str>,
    excluded_source_adapter_id: &str,
) -> StorageResult<Option<String>> {
    let Some(duplicate_signature) = duplicate_signature else {
        return Ok(None);
    };

    connection
        .query_row(
            "
            SELECT id
            FROM feed_items
            WHERE duplicate_signature = ?1
                AND source_adapter_id <> ?2
                AND type IN ('Public media', 'Analysis')
            ORDER BY COALESCE(published_at, fetched_at) DESC, updated_at DESC, id
            LIMIT 1
            ",
            params![duplicate_signature, excluded_source_adapter_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

fn insert_bankier_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
    display_company: &str,
    duplicate_signature: Option<&str>,
) -> StorageResult<()> {
    connection.execute(
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
            display_company,
            duplicate_signature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 'pl', ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
            type = excluded.type,
            source_name = excluded.source_name,
            source_url = excluded.source_url,
            title = excluded.title,
            summary = excluded.summary,
            language = excluded.language,
            published_at = excluded.published_at,
            fetched_at = excluded.fetched_at,
            attribution = excluded.attribution,
            display_company = excluded.display_company,
            duplicate_signature = excluded.duplicate_signature,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            feed_item_id,
            "Public media",
            BANKIER_RSS_ADAPTER_ID,
            BANKIER_RSS_DISPLAY_NAME,
            &item.link,
            &item.title,
            empty_string_to_none(Some(item.summary.clone())),
            item.published_at.as_deref(),
            &item.fetched_at,
            &item.dedupe_key,
            BANKIER_RSS_ATTRIBUTION,
            display_company,
            duplicate_signature,
        ],
    )?;

    Ok(())
}

fn update_bankier_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
    display_company: &str,
    duplicate_signature: Option<&str>,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE feed_items
        SET type = ?2,
            source_adapter_id = ?3,
            source_name = ?4,
            source_url = ?5,
            title = ?6,
            summary = ?7,
            body_text = NULL,
            language = 'pl',
            published_at = ?8,
            fetched_at = ?9,
            dedupe_key = ?10,
            attribution = ?11,
            display_company = ?12,
            duplicate_signature = ?13,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            feed_item_id,
            "Public media",
            BANKIER_RSS_ADAPTER_ID,
            BANKIER_RSS_DISPLAY_NAME,
            &item.link,
            &item.title,
            empty_string_to_none(Some(item.summary.clone())),
            item.published_at.as_deref(),
            &item.fetched_at,
            &item.dedupe_key,
            BANKIER_RSS_ATTRIBUTION,
            display_company,
            duplicate_signature,
        ],
    )?;

    Ok(())
}

fn record_media_duplicate_seen(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE feed_items
        SET fetched_at = CASE
                WHEN fetched_at < ?2 THEN ?2
                ELSE fetched_at
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![feed_item_id, &item.fetched_at],
    )?;

    Ok(())
}

#[derive(Debug, Clone)]
struct MediaMatchCompany {
    id: String,
    ticker: String,
    qualified_ticker: String,
    display_name: String,
}

fn list_media_match_companies(connection: &Connection) -> StorageResult<Vec<MediaMatchCompany>> {
    let mut statement = connection.prepare(
        "
        SELECT id, ticker, qualified_ticker, display_name
        FROM companies
        WHERE exchange = 'GPW'
        ORDER BY qualified_ticker
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(MediaMatchCompany {
            id: row.get(0)?,
            ticker: row.get(1)?,
            qualified_ticker: row.get(2)?,
            display_name: row.get(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn find_companies_for_media_item(
    companies: &[MediaMatchCompany],
    item: &BankierRssItem,
) -> Vec<MediaMatchCompany> {
    let haystack = normalize_media_match_text(&format!("{} {}", item.title, item.summary));
    let tokens = haystack.split_whitespace().collect::<Vec<_>>();

    companies
        .iter()
        .filter(|company| {
            let company_name = normalized_company_name_signal(&company.display_name);
            let ticker = company.ticker.to_uppercase();

            (!company_name.is_empty() && haystack.contains(&company_name))
                || (ticker.chars().count() >= 3 && tokens.iter().any(|token| *token == ticker))
        })
        .cloned()
        .collect()
}

fn media_duplicate_signature(
    item: &BankierRssItem,
    matched_companies: &[MediaMatchCompany],
) -> Option<String> {
    if matched_companies.is_empty() {
        return None;
    }

    let normalized_title = normalize_media_match_text(&item.title);
    if normalized_title.chars().count() < 12 {
        return None;
    }

    let mut companies = matched_companies
        .iter()
        .map(|company| company.qualified_ticker.as_str())
        .collect::<Vec<_>>();
    companies.sort_unstable();
    companies.dedup();

    Some(format!(
        "media:{}:{}",
        companies.join("+"),
        slug_part(&normalized_title)
    ))
}

fn normalized_company_name_signal(value: &str) -> String {
    let mut normalized = normalize_media_match_text(value);
    for suffix in [" SPOLKA AKCYJNA", " S A", " SA"] {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            normalized = stripped.trim().to_owned();
        }
    }

    if normalized.chars().count() < 4 {
        String::new()
    } else {
        normalized
    }
}

fn normalize_media_match_text(value: &str) -> String {
    value
        .chars()
        .map(normalize_media_character)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_media_character(character: char) -> char {
    match character {
        'ą' | 'Ą' => 'A',
        'ć' | 'Ć' => 'C',
        'ę' | 'Ę' => 'E',
        'ł' | 'Ł' => 'L',
        'ń' | 'Ń' => 'N',
        'ó' | 'Ó' => 'O',
        'ś' | 'Ś' => 'S',
        'ż' | 'Ż' | 'ź' | 'Ź' => 'Z',
        other => other.to_uppercase().next().unwrap_or(other),
    }
}

struct MatchedCompany {
    id: String,
    qualified_ticker: String,
    match_type: &'static str,
}

fn find_company_for_gpw_listing(
    connection: &Connection,
    ticker: &str,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    if let Some(company) = find_company_by_ticker(connection, "GPW", ticker)? {
        return Ok(Some(company));
    }

    if let Some(mapped_ticker) = gpw_registry_ticker_for_isin(connection, isin)? {
        if let Some(company) = find_company_by_ticker(connection, "GPW", &mapped_ticker)? {
            return Ok(Some(company));
        }
    }

    if let Some(company) = find_company_by_isin(connection, isin)? {
        return Ok(Some(company));
    }

    Ok(None)
}

fn find_company_by_ticker(
    connection: &Connection,
    exchange: &str,
    ticker: &str,
) -> StorageResult<Option<MatchedCompany>> {
    let ticker = ticker.trim();
    if ticker.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT id, qualified_ticker
            FROM companies
            WHERE exchange = ?1 AND ticker = ?2
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            params![exchange.trim().to_uppercase(), ticker.to_uppercase()],
            |row| {
                Ok(MatchedCompany {
                    id: row.get(0)?,
                    qualified_ticker: row.get(1)?,
                    match_type: "ticker",
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
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
                    match_type: "isin",
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

fn gpw_registry_ticker_for_isin(
    connection: &Connection,
    isin: &str,
) -> StorageResult<Option<String>> {
    let isin = isin.trim();
    if isin.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT ticker
            FROM company_registry_entries
            WHERE exchange = 'GPW'
                AND isin = ?1
                AND active = 1
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            [isin.to_uppercase()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

fn replace_feed_item_attachments(
    connection: &Connection,
    feed_item_id: &str,
    attachments: &[GpwReportAttachment],
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM feed_item_attachments WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;

    for (position, attachment) in attachments.iter().enumerate() {
        connection.execute(
            "
            INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(feed_item_id, url) DO UPDATE SET
                label = excluded.label,
                position = excluded.position
            ",
            params![
                feed_item_attachment_id(feed_item_id, &attachment.url),
                feed_item_id,
                attachment.label,
                attachment.url,
                position as i64,
            ],
        )?;
    }

    Ok(())
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
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN ?1
                WHEN 'bankier-market-rss' THEN ?2
                WHEN 'bankier-company-komunikaty' THEN ?3
                WHEN 'portal-analiz' THEN ?4
                WHEN 'bankier-firma-rss' THEN ?5
                WHEN 'bankier-wiadomosci-rss' THEN ?6
                ELSE 'https://www.gpw.pl/komunikaty'
            END AS source_url,
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN 'Manual refresh plus daily stale-cache scheduled refresh'
                WHEN 'bankier-market-rss' THEN 'Manual refresh plus normal in-app source scheduler; RSS feed only, no article crawling'
                WHEN 'bankier-company-komunikaty' THEN 'Manual refresh plus normal in-app source scheduler; tracked GPW companies only; cached Bankier tag ids; one listing page plus matched article pages per company'
                WHEN 'portal-analiz' THEN 'Late-v1 disabled placeholder; no automated access until the authenticated-source implementation is explicitly built'
                WHEN 'bankier-firma-rss' THEN 'Reviewed public RSS candidate; disabled until matching quality is proven against tracked GPW companies'
                WHEN 'bankier-wiadomosci-rss' THEN 'Reviewed public RSS candidate; disabled because expected listed-company signal is broad and noisy'
                ELSE 'Disabled while Bankier Company Komunikaty is the active official-report source'
            END AS rate_limit_policy,
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN 'Fetches the complete public GPW company list and caches ticker and ISIN metadata locally for lookup, autocomplete, and ticker-first matching.'
                WHEN 'bankier-market-rss' THEN 'Fetches Bankier.pl public Giełda RSS headlines as public media items; linked article pages are not crawled in this slice.'
                WHEN 'bankier-company-komunikaty' THEN 'Fetches Bankier.pl per-company public komunikaty JSON and article pages for tracked GPW companies only. Bankier is the active v1 official-report source while GPW ESPI/EBI is disabled.'
                WHEN 'portal-analiz' THEN 'Late-v1 planned authenticated private research adapter governed by ADR 0014. Credentials must use the OS keychain and no generic login or scraping subsystem is approved.'
                WHEN 'bankier-firma-rss' THEN 'Reviewed M8 follow-up candidate. Public and RSS-native, but broader business coverage needs matching-quality tests before runtime enablement.'
                WHEN 'bankier-wiadomosci-rss' THEN 'Reviewed M8 follow-up candidate. Public and RSS-native, but broad news coverage and stale backfill risk make it unsuitable for default v1 ingestion.'
                ELSE 'Registered for later revisit, but disabled because the global GPW listing slice missed tracked-company reports found by Bankier per-company komunikaty pages.'
            END AS policy_note,
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
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_detail_items_attempted'
            ) AS INTEGER) AS last_detail_items_attempted,
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_detail_items_stored'
            ) AS INTEGER) AS last_detail_items_stored,
            CAST((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_detail_items_failed'
            ) AS INTEGER) AS last_detail_items_failed,
            NULLIF((
                SELECT state_value
                FROM source_adapter_state
                WHERE source_adapter_id = source_adapters.id
                    AND state_key = 'last_detail_warning'
            ), '') AS last_detail_warning,
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

    let rows = statement.query_map(
        [
            GPW_REGISTRY_SOURCE_URL,
            BANKIER_RSS_SOURCE_URL,
            BANKIER_COMPANY_SOURCE_URL,
            PORTAL_ANALIZ_SOURCE_URL,
            BANKIER_FIRMA_RSS_SOURCE_URL,
            BANKIER_WIADOMOSCI_RSS_SOURCE_URL,
        ],
        |row| {
            let markets: String = row.get(22)?;

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
                last_detail_items_attempted: row.get(18)?,
                last_detail_items_stored: row.get(19)?,
                last_detail_items_failed: row.get(20)?,
                last_detail_warning: row.get(21)?,
                markets: markets
                    .split(',')
                    .filter(|market| !market.is_empty())
                    .map(str::to_owned)
                    .collect(),
            })
        },
    )?;

    let adapters = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(adapters)
}

fn list_company_registry_entries(
    connection: &Connection,
) -> StorageResult<Vec<CompanyRegistryEntry>> {
    let mut statement = connection.prepare(
        "
        SELECT
            registry.exchange,
            registry.ticker,
            registry.qualified_ticker,
            registry.display_name,
            registry.isin,
            registry.source_url,
            registry.fetched_at,
            EXISTS(
                SELECT 1
                FROM companies
                WHERE companies.exchange = registry.exchange
                    AND companies.ticker = registry.ticker
            ) AS tracked
        FROM company_registry_entries AS registry
        WHERE registry.source_adapter_id = ?1
            AND registry.active = 1
        ORDER BY registry.ticker
        ",
    )?;

    let rows = statement.query_map([GPW_REGISTRY_ADAPTER_ID], |row| {
        Ok(CompanyRegistryEntry {
            exchange: row.get(0)?,
            ticker: row.get(1)?,
            qualified_ticker: row.get(2)?,
            display_name: row.get(3)?,
            isin: row.get(4)?,
            source_url: row.get(5)?,
            fetched_at: row.get(6)?,
            tracked: row.get(7)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
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
        validate_allowed_setting_i64(
            "poll_interval_seconds",
            poll_interval_seconds,
            &[300, 900, 1800, 3600],
        )?;
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

fn validate_allowed_setting_i64(
    key: &'static str,
    value: i64,
    allowed: &[i64],
) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_string(),
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

fn prune_old_feed_items(
    connection: &mut Connection,
    retention_days: i64,
) -> StorageResult<FeedPruneResult> {
    let retention_days = retention_days.max(1);
    let retention_modifier = format!("-{retention_days} days");
    let transaction = connection.transaction()?;
    let pruned_at = current_timestamp(&transaction)?;

    let candidate_ids = old_unsaved_feed_item_ids(&transaction, &retention_modifier)?;

    delete_feed_items_by_id(&transaction, &candidate_ids)?;

    transaction.commit()?;

    Ok(FeedPruneResult {
        retention_days,
        items_deleted: candidate_ids.len(),
        pruned_at,
    })
}

fn delete_unsaved_feed_items(connection: &mut Connection) -> StorageResult<FeedDeleteResult> {
    let transaction = connection.transaction()?;
    let deleted_at = current_timestamp(&transaction)?;
    let candidate_ids = unsaved_feed_item_ids(&transaction)?;

    delete_feed_items_by_id(&transaction, &candidate_ids)?;

    transaction.commit()?;

    Ok(FeedDeleteResult {
        items_deleted: candidate_ids.len(),
        deleted_at,
    })
}

fn delete_feed_items_by_id(connection: &Connection, feed_item_ids: &[String]) -> StorageResult<()> {
    for feed_item_id in feed_item_ids {
        transaction_delete_feed_item(connection, feed_item_id)?;
    }

    Ok(())
}

fn transaction_delete_feed_item(connection: &Connection, feed_item_id: &str) -> StorageResult<()> {
    connection.execute(
        "
        DELETE FROM ai_analysis_source_references
        WHERE ai_analysis_result_id IN (
            SELECT id FROM ai_analysis_results WHERE feed_item_id = ?1
        )
        ",
        [feed_item_id],
    )?;
    connection.execute(
        "
        DELETE FROM ai_analysis_tags
        WHERE ai_analysis_result_id IN (
            SELECT id FROM ai_analysis_results WHERE feed_item_id = ?1
        )
        ",
        [feed_item_id],
    )?;
    connection.execute(
        "DELETE FROM ai_analysis_results WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;
    connection.execute(
        "DELETE FROM feed_item_attachments WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;
    connection.execute(
        "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;
    connection.execute("DELETE FROM feed_items WHERE id = ?1", [feed_item_id])?;

    Ok(())
}

fn old_unsaved_feed_item_ids(
    connection: &Connection,
    retention_modifier: &str,
) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT id
        FROM feed_items
        WHERE saved = 0
            AND datetime(COALESCE(published_at, fetched_at)) < datetime('now', ?1)
        ",
    )?;
    let rows = statement.query_map([retention_modifier], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

fn unsaved_feed_item_ids(connection: &Connection) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT id
        FROM feed_items
        WHERE saved = 0
        ",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
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
                COALESCE(summary, '') AS summary,
                COALESCE(body_text, '') AS body_text
            FROM feed_items
            WHERE id = ?1
            ",
            [feed_item_id],
            |row| feed_item_from_row(connection, row),
        )
        .map_err(StorageError::from)
}

fn feed_item_from_row(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<FeedItem> {
    let read: bool = row.get(6)?;
    let id: String = row.get(0)?;

    Ok(FeedItem {
        attachments: feed_item_attachments(connection, &id)?,
        id,
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
        body_text: row.get(14)?,
    })
}

fn feed_item_attachments(
    connection: &Connection,
    feed_item_id: &str,
) -> rusqlite::Result<Vec<FeedItemAttachment>> {
    let mut statement = connection.prepare(
        "
        SELECT id, label, url
        FROM feed_item_attachments
        WHERE feed_item_id = ?1
        ORDER BY position, id
        ",
    )?;
    let rows = statement.query_map([feed_item_id], |row| {
        Ok(FeedItemAttachment {
            id: row.get(0)?,
            label: row.get(1)?,
            url: row.get(2)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
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

fn feed_item_attachment_id(feed_item_id: &str, url: &str) -> String {
    format!(
        "feed_attachment_{}_{}",
        slug_part(feed_item_id),
        slug_part(url)
    )
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

fn company_registry_entry_id(exchange: &str, ticker: &str) -> String {
    format!(
        "company_registry_{}_{}",
        slug_part(exchange),
        slug_part(ticker)
    )
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

    fn registry_entry(ticker: &str, display_name: &str, isin: &str) -> GpwCompanyRegistryEntry {
        GpwCompanyRegistryEntry {
            exchange: "GPW".to_owned(),
            ticker: ticker.to_owned(),
            qualified_ticker: format!("GPW:{ticker}"),
            display_name: display_name.to_owned(),
            isin: isin.to_owned(),
            source_url: format!("https://www.gpw.pl/spolka?isin={isin}"),
        }
    }

    fn sample_cdr_listing() -> GpwReportListing {
        GpwReportListing {
            report_type: "Bieżący".to_owned(),
            system: "ESPI".to_owned(),
            report_number: "1/2026".to_owned(),
            company_ticker: "CDR".to_owned(),
            company_name: "CD PROJEKT S.A.".to_owned(),
            isin: "PLOPTTC00011".to_owned(),
            title: "Current report placeholder for tracked company".to_owned(),
            detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=111111".to_owned(),
            published_at: "2026-05-29T09:12:00Z".to_owned(),
            fetched_at: "2026-05-29T09:15:00Z".to_owned(),
            dedupe_key: "gpw-espi-ebi:test:PLOPTTC00011:1/2026:2026-05-29T09:12:00Z".to_owned(),
            body_text: None,
            attachments: Vec::new(),
        }
    }

    fn sample_bankier_items() -> Vec<BankierRssItem> {
        vec![
            BankierRssItem {
                title: "CD Projekt rośnie po komentarzu zarządu".to_owned(),
                link: "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html"
                    .to_owned(),
                summary: "Inwestorzy obserwują CD Projekt po nowych informacjach.".to_owned(),
                published_at: Some("2026-05-31T09:15:00+02:00".to_owned()),
                fetched_at: "2026-05-31T10:00:00Z".to_owned(),
                dedupe_key: "bankier-market-rss:bankier-900001".to_owned(),
            },
            BankierRssItem {
                title: "Rynek czeka na decyzje banków centralnych".to_owned(),
                link: "https://www.bankier.pl/wiadomosc/rynek-czeka-900002.html".to_owned(),
                summary: "Przegląd tematów na europejskich parkietach.".to_owned(),
                published_at: Some("2026-05-31T08:45:00+02:00".to_owned()),
                fetched_at: "2026-05-31T10:00:00Z".to_owned(),
                dedupe_key: "bankier-market-rss:bankier-900002".to_owned(),
            },
        ]
    }

    fn sample_bankier_company_items(company: &Company) -> Vec<BankierCompanyItem> {
        vec![BankierCompanyItem {
            company_id: company.id.clone(),
            qualified_ticker: company.qualified_ticker.clone(),
            title: "Wyniki finansowe QSr 1/2026".to_owned(),
            link: "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                .to_owned(),
            summary: "raporty okresowe: kwartalne, polroczne, roczne".to_owned(),
            published_at: Some("2026-05-28T17:33:09".to_owned()),
            fetched_at: "2026-05-31T10:00:00Z".to_owned(),
            article_id: "9141553".to_owned(),
            pub_id: 3,
            dedupe_key: "bankier-company-komunikaty:article:9141553".to_owned(),
            duplicate_signature:
                "official-secondary:GPW:CDR:wyniki-finansowe-qsr-1-2026:9141553".to_owned(),
            body_text: Some("Official Bankier report body from the article page.".to_owned()),
            attachments: vec![BankierCompanyAttachment {
                label: "report.xhtml".to_owned(),
                url: "https://bonnier.pl/report.xhtml".to_owned(),
            }],
            detail_fetch_attempted: true,
        }]
    }

    #[test]
    fn creates_clean_database_with_initial_schema() {
        let connection = open_in_memory_database().expect("database should initialize");

        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("schema_migrations should exist");

        assert_eq!(migration_count, 12);

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
    fn seeds_default_settings_and_source_adapters() {
        let connection = open_in_memory_database().expect("database should initialize");

        let theme: String = connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'theme'",
                [],
                |row| row.get(0),
            )
            .expect("theme setting should be seeded");

        let gpw_adapter: (String, bool) = connection
            .query_row(
                "SELECT display_name, enabled FROM source_adapters WHERE id = 'gpw-espi-ebi'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("GPW adapter should be seeded");
        let registry_adapter_name: String = connection
            .query_row(
                "SELECT display_name FROM source_adapters WHERE id = 'gpw-company-registry'",
                [],
                |row| row.get(0),
            )
            .expect("GPW registry adapter should be seeded");
        let bankier_adapter_name: String = connection
            .query_row(
                "SELECT display_name FROM source_adapters WHERE id = 'bankier-market-rss'",
                [],
                |row| row.get(0),
            )
            .expect("Bankier adapter should be seeded");
        let bankier_company_adapter_name: String = connection
            .query_row(
                "SELECT display_name FROM source_adapters WHERE id = 'bankier-company-komunikaty'",
                [],
                |row| row.get(0),
            )
            .expect("Bankier company adapter should be seeded");
        let portal_analiz_adapter: (String, bool) = connection
            .query_row(
                "SELECT display_name, enabled FROM source_adapters WHERE id = 'portal-analiz'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("Portal Analiz placeholder should be seeded");

        assert_eq!(theme, "dark");
        assert_eq!(gpw_adapter, ("GPW ESPI/EBI".to_owned(), false));
        assert_eq!(registry_adapter_name, "GPW Company Registry");
        assert_eq!(bankier_adapter_name, "Bankier Giełda RSS");
        assert_eq!(bankier_company_adapter_name, "Bankier Company Komunikaty");
        assert_eq!(portal_analiz_adapter, ("Portal Analiz".to_owned(), false));
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

        assert_eq!(status.applied_migrations, 12);
        assert_eq!(status.companies, 0);
        assert_eq!(status.source_adapters, 7);
        assert_eq!(status.settings, 7);
    }

    #[test]
    fn starts_without_seeded_feed_or_registry_rows() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let feed_items = state.list_feed_items().expect("feed items should list");
        let registry_entries = state
            .list_company_registry_entries()
            .expect("registry entries should list");

        assert!(feed_items.is_empty());
        assert!(registry_entries.is_empty());
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
            .expect("tracked company should create");

        state
            .ingest_gpw_report_listings(&[sample_cdr_listing()])
            .expect("test listing should ingest");
        let feed_item_id = state
            .list_feed_items()
            .expect("feed items should list")
            .first()
            .expect("test feed item should exist")
            .id
            .clone();

        let updated = state
            .update_feed_item_state(FeedItemStateInput {
                id: feed_item_id.clone(),
                read: Some(true),
                saved: Some(true),
            })
            .expect("feed item state should update");

        assert!(!updated.unread);
        assert!(updated.saved);

        let feed_items = state.list_feed_items().expect("feed items should list");
        let cdr = feed_items
            .iter()
            .find(|item| item.id == feed_item_id)
            .expect("CDR test item should remain present");

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
                    company_ticker: "NTC".to_owned(),
                    company_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
                    isin: "PLECMNG00019".to_owned(),
                    title: "Oświadczenie w sprawie formy przekazywania raportów kwartalnych."
                        .to_owned(),
                    detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=123456".to_owned(),
                    published_at: "2026-05-30T17:13:31+02:00".to_owned(),
                    fetched_at: "2026-05-30T17:30:00Z".to_owned(),
                    dedupe_key: "gpw-espi-ebi:espi:PLECMNG00019:7/2026:2026-05-30T17:13:31+02:00"
                        .to_owned(),
                    body_text: Some("Official report body from GPW detail page.".to_owned()),
                    attachments: vec![GpwReportAttachment {
                        label: "7_2026_oswiadczenie.pdf".to_owned(),
                        url: "https://www.gpw.pl/pub/GPW/ESPI/2026/7_2026_oswiadczenie.pdf"
                            .to_owned(),
                    }],
                },
                GpwReportListing {
                    report_type: "Bieżący".to_owned(),
                    system: "ESPI".to_owned(),
                    report_number: "9/2026".to_owned(),
                    company_ticker: "UNK".to_owned(),
                    company_name: "UNTRACKED S.A.".to_owned(),
                    isin: "PLUNTRK00001".to_owned(),
                    title: "Untracked company report".to_owned(),
                    detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=999999".to_owned(),
                    published_at: "2026-05-30T18:13:31+02:00".to_owned(),
                    fetched_at: "2026-05-30T18:30:00Z".to_owned(),
                    dedupe_key: "gpw-espi-ebi:espi:PLUNTRK00001:9/2026:2026-05-30T18:13:31+02:00"
                        .to_owned(),
                    body_text: None,
                    attachments: Vec::new(),
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
        assert_eq!(ntc.body_text, "Official report body from GPW detail page.");
        assert_eq!(ntc.attachments.len(), 1);
        assert_eq!(ntc.attachments[0].label, "7_2026_oswiadczenie.pdf");

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
    fn ingests_gpw_listing_by_registry_ticker_when_local_isin_is_missing() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "11B".to_owned(),
                display_name: "11 BIT STUDIOS S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("tracked company should create");
        state
            .refresh_gpw_company_registry(
                &[registry_entry(
                    "11B",
                    "11 BIT STUDIOS SPÓŁKA AKCYJNA",
                    "PL11BTS00015",
                )],
                "2026-05-31T12:00:00Z",
            )
            .expect("test registry should refresh");

        let result = state
            .ingest_gpw_report_listings(&[GpwReportListing {
                report_type: "Bieżący".to_owned(),
                system: "ESPI".to_owned(),
                report_number: "20/2026".to_owned(),
                company_ticker: String::new(),
                company_name: "11 BIT STUDIOS SPÓŁKA AKCYJNA".to_owned(),
                isin: "PL11BTS00015".to_owned(),
                title: "Informacja o zawarciu znaczącej umowy".to_owned(),
                detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=777777".to_owned(),
                published_at: "2026-05-30T17:13:31+02:00".to_owned(),
                fetched_at: "2026-05-30T17:30:00Z".to_owned(),
                dedupe_key: "gpw-espi-ebi:espi:PL11BTS00015:20/2026:2026-05-30T17:13:31+02:00"
                    .to_owned(),
                body_text: None,
                attachments: Vec::new(),
            }])
            .expect("listing should ingest");

        assert_eq!(result.items_matched, 1);
        assert_eq!(result.items_unmatched, 0);

        let visible_items = state.list_feed_items().expect("feed items should list");
        let item = visible_items
            .iter()
            .find(|item| item.company == "GPW:11B")
            .expect("ticker-registry matched listing should be visible");

        assert_eq!(item.title, "Informacja o zawarciu znaczącej umowy");
    }

    #[test]
    fn ingests_bankier_rss_items_and_matches_tracked_company_by_strong_signal() {
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
            .expect("company should create");

        let result = state
            .ingest_bankier_rss_items(&sample_bankier_items())
            .expect("RSS items should ingest");

        assert_eq!(result.adapter_id, BANKIER_RSS_ADAPTER_ID);
        assert_eq!(result.items_fetched, 2);
        assert_eq!(result.items_created, 2);
        assert_eq!(result.items_matched, 1);
        assert_eq!(result.items_unmatched, 1);

        let visible_items = state.list_feed_items().expect("feed items should list");

        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].company, "GPW:CDR");
        assert_eq!(visible_items[0].item_type, "Public media");
        assert_eq!(visible_items[0].source, "Bankier Giełda RSS");
        assert_eq!(visible_items[0].attribution, "Bankier.pl");
        assert_eq!(
            visible_items[0].summary,
            "Inwestorzy obserwują CD Projekt po nowych informacjach."
        );

        let unmatched = state
            .list_unmatched_source_items(BANKIER_RSS_ADAPTER_ID)
            .expect("unmatched RSS item should be diagnosable");

        assert_eq!(unmatched.len(), 1);
        assert_eq!(
            unmatched[0].title,
            "Rynek czeka na decyzje banków centralnych"
        );
    }

    #[test]
    fn bankier_rss_ingestion_updates_existing_item_by_source_url() {
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
            .expect("company should create");

        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            connection
                .execute(
                    "
                    INSERT INTO feed_items (
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
                        attribution,
                        display_company
                    ) VALUES (?1, 'Public media', ?2, ?3, ?4, ?5, '', 'pl', ?6, ?7, ?8, ?9, 'GPW:CDR')
                    ",
                    params![
                        "feed_bankier_old_title",
                        BANKIER_RSS_ADAPTER_ID,
                        BANKIER_RSS_DISPLAY_NAME,
                        "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html",
                        "&quot;Maluchy&quot; z nowym rekordem. CD Projekt rośnie po komentarzu zarządu",
                        "2026-05-31T09:15:00+02:00",
                        "2026-05-31T09:30:00Z",
                        "bankier-market-rss:old-title-derived-key",
                        BANKIER_RSS_ATTRIBUTION,
                    ],
                )
                .expect("old row should insert");
        }

        state
            .ingest_bankier_rss_items(&[BankierRssItem {
                title: "\"Maluchy\" z nowym rekordem. CD Projekt rośnie po komentarzu zarządu"
                    .to_owned(),
                link: "https://www.bankier.pl/wiadomosc/cd-projekt-komentarz-900001.html"
                    .to_owned(),
                summary: "Zdekodowany opis.".to_owned(),
                published_at: Some("2026-05-31T09:15:00+02:00".to_owned()),
                fetched_at: "2026-05-31T10:00:00Z".to_owned(),
                dedupe_key: "bankier-market-rss:bankier-900001".to_owned(),
            }])
            .expect("RSS item should update existing row");

        let visible_items = state.list_feed_items().expect("feed items should list");

        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].id, "feed_bankier_old_title");
        assert_eq!(
            visible_items[0].title,
            "\"Maluchy\" z nowym rekordem. CD Projekt rośnie po komentarzu zarządu"
        );
        assert_eq!(visible_items[0].summary, "Zdekodowany opis.");
    }

    #[test]
    fn bankier_rss_ingestion_skips_cross_source_media_duplicate() {
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
            .expect("company should create");
        let bankier_item = sample_bankier_items()
            .into_iter()
            .next()
            .expect("sample Bankier item should exist");
        let duplicate_signature = media_duplicate_signature(
            &bankier_item,
            &[MediaMatchCompany {
                id: company.id.clone(),
                ticker: "CDR".to_owned(),
                qualified_ticker: "GPW:CDR".to_owned(),
                display_name: "CD PROJEKT S.A.".to_owned(),
            }],
        )
        .expect("matched media item should have duplicate signature");

        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            connection
                .execute(
                    "
                    INSERT INTO source_adapters (
                        id,
                        display_name,
                        source_type,
                        fetch_mode,
                        enabled,
                        default_poll_interval_seconds
                    ) VALUES ('other-media-rss', 'Other Media RSS', 'public_media', 'rss', 1, 900)
                    ",
                    [],
                )
                .expect("other media adapter should insert");
            connection
                .execute(
                    "
                    INSERT INTO feed_items (
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
                        attribution,
                        display_company,
                        duplicate_signature
                    ) VALUES (?1, 'Public media', 'other-media-rss', 'Other Media RSS', ?2, ?3, ?4, 'pl', ?5, ?6, ?7, 'Other Media', 'GPW:CDR', ?8)
                    ",
                    params![
                        "feed_other_media_cdr",
                        "https://example.test/cd-projekt-komentarz",
                        &bankier_item.title,
                        &bankier_item.summary,
                        bankier_item.published_at.as_deref(),
                        &bankier_item.fetched_at,
                        "other-media-rss:cd-projekt-komentarz",
                        &duplicate_signature,
                    ],
                )
                .expect("other media item should insert");
            connection
                .execute(
                    "
                    INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                    VALUES ('feed_other_media_cdr', ?1, 'media_signal')
                    ",
                    [&company.id],
                )
                .expect("other media company match should insert");
        }

        let result = state
            .ingest_bankier_rss_items(&[bankier_item])
            .expect("Bankier duplicate should ingest");

        assert_eq!(result.items_fetched, 1);
        assert_eq!(result.items_created, 0);
        assert_eq!(result.items_matched, 1);
        assert_eq!(result.items_unmatched, 0);

        let visible_items = state.list_feed_items().expect("feed items should list");
        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].id, "feed_other_media_cdr");
        assert_eq!(visible_items[0].source, "Other Media RSS");
    }

    #[test]
    fn stores_bankier_company_identifiers_for_tracked_companies() {
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
            .expect("company should create");

        let targets_before = state
            .list_bankier_company_targets()
            .expect("targets should list");
        assert_eq!(targets_before.len(), 1);
        assert_eq!(targets_before[0].bankier_slug, None);
        assert_eq!(targets_before[0].bankier_tag_id, None);

        state
            .upsert_bankier_company_identifiers(
                &company.id,
                &BankierCompanyIdentifiers {
                    slug: "CDPROJEKT".to_owned(),
                    tag_id: "722".to_owned(),
                },
            )
            .expect("identifiers should store");

        let targets_after = state
            .list_bankier_company_targets()
            .expect("targets should list");
        assert_eq!(targets_after[0].bankier_slug.as_deref(), Some("CDPROJEKT"));
        assert_eq!(targets_after[0].bankier_tag_id.as_deref(), Some("722"));

        state
            .upsert_bankier_company_identifiers(
                &company.id,
                &BankierCompanyIdentifiers {
                    slug: "CDPROJEKT".to_owned(),
                    tag_id: "999".to_owned(),
                },
            )
            .expect("changed identifiers should update");

        let changed_targets = state
            .list_bankier_company_targets()
            .expect("targets should list");
        assert_eq!(changed_targets[0].bankier_tag_id.as_deref(), Some("999"));
    }

    #[test]
    fn ingests_bankier_company_items_for_tracked_company() {
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
            .expect("company should create");

        let result = state
            .ingest_bankier_company_items(&sample_bankier_company_items(&company))
            .expect("Bankier company items should ingest");

        assert_eq!(result.adapter_id, BANKIER_COMPANY_ADAPTER_ID);
        assert_eq!(result.items_fetched, 1);
        assert_eq!(result.items_created, 1);
        assert_eq!(result.items_matched, 1);
        assert_eq!(result.items_unmatched, 0);
        assert_eq!(result.detail_items_attempted, 1);
        assert_eq!(result.detail_items_stored, 1);
        assert_eq!(result.detail_items_failed, 0);

        let visible_items = state.list_feed_items().expect("feed items should list");
        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].company, "GPW:CDR");
        assert_eq!(visible_items[0].item_type, "Official report");
        assert_eq!(visible_items[0].source, BANKIER_COMPANY_DISPLAY_NAME);
        assert_eq!(visible_items[0].attribution, "Bankier.pl");
        assert_eq!(visible_items[0].summary, "ESPI");
        assert_eq!(
            visible_items[0].body_text,
            "Official Bankier report body from the article page."
        );
        assert_eq!(visible_items[0].attachments.len(), 1);
        assert_eq!(visible_items[0].attachments[0].label, "report.xhtml");
    }

    #[test]
    fn lists_bankier_company_detail_cached_urls() {
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
            .expect("company should create");

        state
            .ingest_bankier_company_items(&sample_bankier_company_items(&company))
            .expect("Bankier company item should ingest");

        let cached_urls = state
            .list_bankier_company_detail_cached_urls()
            .expect("cached URLs should list");

        assert_eq!(
            cached_urls,
            vec![
                "https://www.bankier.pl/wiadomosc/CD-PROJEKT-SA-Wyniki-finansowe-QSr-1-2026-9141553.html"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn does_not_prune_existing_bankier_company_items_during_ingestion() {
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
            .expect("company should create");
        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            connection
                .execute(
                    "
                    INSERT INTO feed_items (
                        id,
                        type,
                        source_adapter_id,
                        source_name,
                        source_url,
                        title,
                        language,
                        published_at,
                        fetched_at,
                        dedupe_key,
                        display_company
                    ) VALUES (?1, 'Official report', ?2, 'Bankier Company Komunikaty', ?3, ?4, 'pl', ?5, ?6, ?7, ?8)
                    ",
                    params![
                        "feed_bankier_company_komunikaty_legacy",
                        BANKIER_COMPANY_ADAPTER_ID,
                        "https://www.bankier.pl/wiadomosc/legacy.html",
                        "Legacy Bankier report",
                        "2026-05-20T10:00:00",
                        "2026-05-21T10:00:00Z",
                        "bankier-company-komunikaty:article:legacy",
                        company.qualified_ticker,
                    ],
                )
                .expect("legacy Bankier item should insert");
        }
        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            let legacy_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                    [BANKIER_COMPANY_ADAPTER_ID],
                    |row| row.get(0),
                )
                .expect("legacy item count should query");
            assert_eq!(legacy_count, 1);
        }

        state
            .ingest_bankier_company_items(&sample_bankier_company_items(&company))
            .expect("Bankier company refresh should not prune existing rows");

        let visible_items = state.list_feed_items().expect("feed items should list");
        assert_eq!(visible_items.len(), 2);
        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            let legacy_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                    [BANKIER_COMPANY_ADAPTER_ID],
                    |row| row.get(0),
                )
                .expect("legacy item count should query");
            assert_eq!(legacy_count, 2);
        }
    }

    #[test]
    fn bankier_company_items_do_not_duplicate_existing_gpw_report() {
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
            .expect("company should create");

        state
            .ingest_gpw_report_listings(&[GpwReportListing {
                report_type: "Okresowy".to_owned(),
                system: "ESPI".to_owned(),
                report_number: "QSr 1/2026".to_owned(),
                company_ticker: "CDR".to_owned(),
                company_name: "CD PROJEKT S.A.".to_owned(),
                isin: "PLOPTTC00011".to_owned(),
                title: "Wyniki finansowe QSr 1/2026".to_owned(),
                detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=9141553".to_owned(),
                published_at: "2026-05-28T17:33:09+02:00".to_owned(),
                fetched_at: "2026-05-28T17:40:00Z".to_owned(),
                dedupe_key: "gpw-espi-ebi:espi:PLOPTTC00011:QSr 1/2026:2026-05-28T17:33:09+02:00"
                    .to_owned(),
                body_text: None,
                attachments: Vec::new(),
            }])
            .expect("GPW report should ingest");

        let result = state
            .ingest_bankier_company_items(&sample_bankier_company_items(&company))
            .expect("Bankier company duplicate should ingest");

        assert_eq!(result.items_fetched, 1);
        assert_eq!(result.items_created, 0);
        assert_eq!(result.items_matched, 1);
        assert_eq!(result.items_unmatched, 0);

        let visible_items = state.list_feed_items().expect("feed items should list");
        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].source, "GPW ESPI/EBI");
        assert_eq!(visible_items[0].title, "Wyniki finansowe QSr 1/2026");
    }

    #[test]
    fn hides_bankier_company_item_after_matching_gpw_report_arrives() {
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
            .expect("company should create");

        state
            .ingest_bankier_company_items(&sample_bankier_company_items(&company))
            .expect("Bankier company item should ingest first");
        assert_eq!(
            state
                .list_feed_items()
                .expect("feed items should list")
                .len(),
            1
        );

        state
            .ingest_gpw_report_listings(&[GpwReportListing {
                report_type: "Okresowy".to_owned(),
                system: "ESPI".to_owned(),
                report_number: "QSr 1/2026".to_owned(),
                company_ticker: "CDR".to_owned(),
                company_name: "CD PROJEKT S.A.".to_owned(),
                isin: "PLOPTTC00011".to_owned(),
                title: "Wyniki finansowe QSr 1/2026".to_owned(),
                detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=9141553".to_owned(),
                published_at: "2026-05-28T17:33:09+02:00".to_owned(),
                fetched_at: "2026-05-28T17:40:00Z".to_owned(),
                dedupe_key: "gpw-espi-ebi:espi:PLOPTTC00011:QSr 1/2026:2026-05-28T17:33:09+02:00"
                    .to_owned(),
                body_text: Some("Official GPW body.".to_owned()),
                attachments: Vec::new(),
            }])
            .expect("GPW report should ingest");

        let visible_items = state.list_feed_items().expect("feed items should list");
        assert_eq!(visible_items.len(), 1);
        assert_eq!(visible_items[0].source, "GPW ESPI/EBI");
        assert_eq!(visible_items[0].body_text, "Official GPW body.");

        let stored_bankier_count: i64 = {
            let connection = state.connection.lock().expect("database mutex poisoned");
            connection
                .query_row(
                    "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                    [BANKIER_COMPANY_ADAPTER_ID],
                    |row| row.get(0),
                )
                .expect("stored Bankier count should query")
        };
        assert_eq!(stored_bankier_count, 1);
    }

    #[test]
    fn prunes_old_unsaved_feed_items_only_when_maintenance_runs() {
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
            .expect("company should create");

        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            for (id, published_at, saved) in [
                ("feed_old_unsaved", "2000-01-01T10:00:00Z", false),
                ("feed_old_saved", "2000-01-01T11:00:00Z", true),
                ("feed_recent_unsaved", "2999-05-31T10:00:00Z", false),
            ] {
                connection
                    .execute(
                        "
                        INSERT INTO feed_items (
                            id,
                            type,
                            source_adapter_id,
                            source_name,
                            source_url,
                            title,
                            language,
                            published_at,
                            fetched_at,
                            dedupe_key,
                            saved,
                            display_company
                        ) VALUES (?1, 'Public media', ?2, 'Bankier Giełda RSS', ?3, ?4, 'pl', ?5, ?5, ?6, ?7, ?8)
                        ",
                        params![
                            id,
                            BANKIER_RSS_ADAPTER_ID,
                            format!("https://www.bankier.pl/wiadomosc/{id}.html"),
                            id,
                            published_at,
                            id,
                            saved,
                            company.qualified_ticker,
                        ],
                    )
                    .expect("feed item should insert");
                connection
                    .execute(
                        "
                        INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                        VALUES (?1, ?2, 'test')
                        ",
                        params![id, company.id],
                    )
                    .expect("feed item company should insert");
            }
        }

        let result = state
            .prune_old_feed_items(30)
            .expect("old feed items should prune");

        assert_eq!(result.retention_days, 30);
        assert_eq!(result.items_deleted, 1);

        let remaining_ids = {
            let connection = state.connection.lock().expect("database mutex poisoned");
            let mut statement = connection
                .prepare("SELECT id FROM feed_items ORDER BY id")
                .expect("remaining feed query should prepare");
            statement
                .query_map([], |row| row.get::<_, String>(0))
                .expect("remaining feed query should run")
                .collect::<Result<Vec<_>, _>>()
                .expect("remaining feed ids should collect")
        };

        assert_eq!(
            remaining_ids,
            vec![
                "feed_old_saved".to_owned(),
                "feed_recent_unsaved".to_owned()
            ]
        );
    }

    #[test]
    fn deletes_all_unsaved_feed_items_when_requested() {
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
            .expect("company should create");

        {
            let connection = state.connection.lock().expect("database mutex poisoned");
            for (id, saved) in [
                ("feed_old_unsaved", false),
                ("feed_recent_unsaved", false),
                ("feed_saved", true),
            ] {
                connection
                    .execute(
                        "
                        INSERT INTO feed_items (
                            id,
                            type,
                            source_adapter_id,
                            source_name,
                            source_url,
                            title,
                            language,
                            published_at,
                            fetched_at,
                            dedupe_key,
                            saved,
                            display_company
                        ) VALUES (?1, 'Public media', ?2, 'Bankier Giełda RSS', ?3, ?4, 'pl', '2026-05-31T10:00:00Z', '2026-05-31T10:00:00Z', ?5, ?6, ?7)
                        ",
                        params![
                            id,
                            BANKIER_RSS_ADAPTER_ID,
                            format!("https://www.bankier.pl/wiadomosc/{id}.html"),
                            id,
                            id,
                            saved,
                            company.qualified_ticker,
                        ],
                    )
                    .expect("feed item should insert");
                connection
                    .execute(
                        "
                        INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                        VALUES (?1, ?2, 'test')
                        ",
                        params![id, company.id],
                    )
                    .expect("feed item company should insert");
                connection
                    .execute(
                        "
                        INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
                        VALUES (?1, ?2, 'report.pdf', ?3, 0)
                        ",
                        params![
                            format!("attachment_{id}"),
                            id,
                            format!("https://www.bankier.pl/{id}.pdf")
                        ],
                    )
                    .expect("feed item attachment should insert");
            }

            connection
                .execute(
                    "
                    INSERT INTO ai_analysis_results (
                        id,
                        feed_item_id,
                        provider_id,
                        model,
                        summary,
                        significance,
                        reasoning,
                        language
                    ) VALUES ('analysis_unsaved', 'feed_old_unsaved', 'local', 'test', 'summary', 'medium', 'reasoning', 'en')
                    ",
                    [],
                )
                .expect("analysis result should insert");
            connection
                .execute(
                    "
                    INSERT INTO ai_analysis_tags (ai_analysis_result_id, tag)
                    VALUES ('analysis_unsaved', 'important')
                    ",
                    [],
                )
                .expect("analysis tag should insert");
            connection
                .execute(
                    "
                    INSERT INTO ai_analysis_source_references (id, ai_analysis_result_id, source_url, label)
                    VALUES ('analysis_reference_unsaved', 'analysis_unsaved', 'https://example.local/report', 'Report')
                    ",
                    [],
                )
                .expect("analysis source reference should insert");
        }

        let result = state
            .delete_unsaved_feed_items()
            .expect("unsaved feed items should delete");

        assert_eq!(result.items_deleted, 2);

        let (remaining_ids, attachment_count, company_link_count, analysis_count) = {
            let connection = state.connection.lock().expect("database mutex poisoned");
            let mut statement = connection
                .prepare("SELECT id FROM feed_items ORDER BY id")
                .expect("remaining feed query should prepare");
            let remaining_ids = statement
                .query_map([], |row| row.get::<_, String>(0))
                .expect("remaining feed query should run")
                .collect::<Result<Vec<_>, _>>()
                .expect("remaining feed ids should collect");
            let attachment_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM feed_item_attachments", [], |row| {
                    row.get(0)
                })
                .expect("attachment count should query");
            let company_link_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM feed_item_companies", [], |row| {
                    row.get(0)
                })
                .expect("company link count should query");
            let analysis_count: i64 = connection
                .query_row("SELECT COUNT(*) FROM ai_analysis_results", [], |row| {
                    row.get(0)
                })
                .expect("analysis count should query");

            (
                remaining_ids,
                attachment_count,
                company_link_count,
                analysis_count,
            )
        };

        assert_eq!(remaining_ids, vec!["feed_saved".to_owned()]);
        assert_eq!(attachment_count, 1);
        assert_eq!(company_link_count, 1);
        assert_eq!(analysis_count, 0);
    }

    #[test]
    fn replaces_gpw_detail_attachments_when_accepted_detail_has_none() {
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

        let listing = GpwReportListing {
            report_type: "Bieżący".to_owned(),
            system: "ESPI".to_owned(),
            report_number: "7/2026".to_owned(),
            company_ticker: "NTC".to_owned(),
            company_name: "NEW TECH CAPITAL SPÓŁKA AKCYJNA".to_owned(),
            isin: "PLECMNG00019".to_owned(),
            title: "Oświadczenie w sprawie formy przekazywania raportów kwartalnych.".to_owned(),
            detail_url: "https://www.gpw.pl/komunikaty?ph_main_01_cmn_id=123456".to_owned(),
            published_at: "2026-05-30T17:13:31+02:00".to_owned(),
            fetched_at: "2026-05-30T17:30:00Z".to_owned(),
            dedupe_key: "gpw-espi-ebi:espi:PLECMNG00019:7/2026:2026-05-30T17:13:31+02:00"
                .to_owned(),
            body_text: Some("Official report body from GPW detail page.".to_owned()),
            attachments: vec![GpwReportAttachment {
                label: "7_2026_oswiadczenie.pdf".to_owned(),
                url: "https://www.gpw.pl/pub/GPW/ESPI/2026/7_2026_oswiadczenie.pdf".to_owned(),
            }],
        };

        state
            .ingest_gpw_report_listings(std::slice::from_ref(&listing))
            .expect("listing should ingest");

        let mut replacement = listing;
        replacement.body_text =
            Some("Updated official report body from GPW detail page.".to_owned());
        replacement.attachments = Vec::new();
        state
            .ingest_gpw_report_listings(&[replacement])
            .expect("replacement listing should ingest");

        let feed_items = state.list_feed_items().expect("feed items should list");
        let ntc = feed_items
            .iter()
            .find(|item| item.company == "GPW:NTC")
            .expect("matched GPW listing should be visible");

        assert!(ntc.attachments.is_empty());
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
                    source_id: Some("feed_sample_cdr_report".to_owned()),
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
            Some("feed_sample_cdr_report")
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

        assert_eq!(adapters.len(), 7);

        let report_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == "gpw-espi-ebi")
            .expect("GPW report adapter should exist");
        assert_eq!(report_adapter.display_name, "GPW ESPI/EBI");
        assert_eq!(report_adapter.markets, vec!["GPW".to_owned()]);
        assert!(!report_adapter.enabled);

        let registry_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == "gpw-company-registry")
            .expect("GPW registry adapter should exist");
        assert_eq!(registry_adapter.display_name, "GPW Company Registry");
        assert_eq!(registry_adapter.markets, vec!["GPW".to_owned()]);
        assert!(registry_adapter.enabled);

        let bankier_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == BANKIER_RSS_ADAPTER_ID)
            .expect("Bankier RSS adapter should exist");
        assert_eq!(bankier_adapter.display_name, "Bankier Giełda RSS");
        assert_eq!(bankier_adapter.source_type, "public_media");
        assert_eq!(bankier_adapter.fetch_mode, "rss");
        assert_eq!(bankier_adapter.source_url, BANKIER_RSS_SOURCE_URL);
        assert_eq!(bankier_adapter.markets, vec!["GPW".to_owned()]);
        assert!(bankier_adapter.enabled);

        let bankier_company_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == BANKIER_COMPANY_ADAPTER_ID)
            .expect("Bankier company adapter should exist");
        assert_eq!(
            bankier_company_adapter.display_name,
            "Bankier Company Komunikaty"
        );
        assert_eq!(bankier_company_adapter.source_type, "official_report");
        assert_eq!(bankier_company_adapter.fetch_mode, "public_json");
        assert_eq!(
            bankier_company_adapter.source_url,
            BANKIER_COMPANY_SOURCE_URL
        );
        assert_eq!(bankier_company_adapter.markets, vec!["GPW".to_owned()]);
        assert!(bankier_company_adapter.enabled);
        assert!(bankier_company_adapter
            .policy_note
            .contains("active v1 official-report source"));

        let bankier_firma_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == "bankier-firma-rss")
            .expect("Bankier Firma RSS placeholder should exist");
        assert_eq!(bankier_firma_adapter.display_name, "Bankier Firma RSS");
        assert_eq!(bankier_firma_adapter.source_type, "public_media");
        assert_eq!(bankier_firma_adapter.fetch_mode, "rss");
        assert_eq!(
            bankier_firma_adapter.source_url,
            BANKIER_FIRMA_RSS_SOURCE_URL
        );
        assert!(!bankier_firma_adapter.enabled);
        assert!(bankier_firma_adapter
            .policy_note
            .contains("matching-quality tests"));

        let bankier_wiadomosci_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == "bankier-wiadomosci-rss")
            .expect("Bankier Wiadomosci RSS placeholder should exist");
        assert_eq!(
            bankier_wiadomosci_adapter.display_name,
            "Bankier Wiadomosci RSS"
        );
        assert_eq!(bankier_wiadomosci_adapter.source_type, "public_media");
        assert_eq!(bankier_wiadomosci_adapter.fetch_mode, "rss");
        assert_eq!(
            bankier_wiadomosci_adapter.source_url,
            BANKIER_WIADOMOSCI_RSS_SOURCE_URL
        );
        assert!(!bankier_wiadomosci_adapter.enabled);
        assert!(bankier_wiadomosci_adapter
            .policy_note
            .contains("unsuitable for default v1 ingestion"));

        let portal_analiz_adapter = adapters
            .iter()
            .find(|adapter| adapter.id == PORTAL_ANALIZ_ADAPTER_ID)
            .expect("Portal Analiz placeholder should exist");
        assert_eq!(portal_analiz_adapter.display_name, "Portal Analiz");
        assert_eq!(portal_analiz_adapter.source_type, "authenticated_research");
        assert_eq!(portal_analiz_adapter.fetch_mode, "authenticated");
        assert_eq!(portal_analiz_adapter.source_url, PORTAL_ANALIZ_SOURCE_URL);
        assert_eq!(portal_analiz_adapter.markets, vec!["GPW".to_owned()]);
        assert!(!portal_analiz_adapter.enabled);
        assert!(portal_analiz_adapter
            .policy_note
            .contains("Late-v1 planned"));
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
                poll_interval_seconds: Some(1800),
                youtube_transcription_provider: None,
                general_analysis_provider: None,
                ai_analysis_mode: None,
            })
            .expect("settings should update");

        assert_eq!(settings.theme, "light");
        assert_eq!(settings.poll_interval_seconds, 1800);

        let persisted = state.get_settings().expect("settings should persist");

        assert_eq!(persisted.theme, "light");
        assert_eq!(persisted.poll_interval_seconds, 1800);
    }

    #[test]
    fn rejects_invalid_poll_interval_setting() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);

        let result = state.update_settings(SettingsUpdate {
            theme: None,
            poll_interval_seconds: Some(42),
            youtube_transcription_provider: None,
            general_analysis_provider: None,
            ai_analysis_mode: None,
        });

        assert!(result.is_err());
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
    fn looks_up_registry_company_by_ticker() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .refresh_gpw_company_registry(
                &[registry_entry("CDR", "CD PROJEKT S.A.", "PLOPTTC00011")],
                "2026-05-31T12:00:00Z",
            )
            .expect("test registry should refresh");

        let result = state
            .lookup_company(CompanyLookupInput {
                exchange: "gpw".to_owned(),
                ticker: Some("cdr".to_owned()),
                display_name: None,
                isin: None,
            })
            .expect("lookup should succeed")
            .expect("registry should match");

        assert_eq!(result.qualified_ticker, "GPW:CDR");
        assert_eq!(result.display_name, "CD PROJEKT S.A.");
        assert_eq!(result.isin, "PLOPTTC00011");
        assert_eq!(result.source, "gpw_registry");
    }

    #[test]
    fn looks_up_registry_company_by_isin() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        state
            .refresh_gpw_company_registry(
                &[registry_entry("PZU", "PZU S.A.", "PLPZU0000011")],
                "2026-05-31T12:00:00Z",
            )
            .expect("test registry should refresh");

        let result = state
            .lookup_company(CompanyLookupInput {
                exchange: "GPW".to_owned(),
                ticker: None,
                display_name: None,
                isin: Some("plpzu0000011".to_owned()),
            })
            .expect("lookup should succeed")
            .expect("registry should match");

        assert_eq!(result.ticker, "PZU");
        assert_eq!(result.display_name, "PZU S.A.");
        assert_eq!(result.source, "gpw_registry");
    }

    #[test]
    fn refreshes_gpw_company_registry_cache() {
        let connection = open_in_memory_database().expect("database should initialize");
        let state = AppState::new(connection);
        let entries = vec![GpwCompanyRegistryEntry {
            exchange: "GPW".to_owned(),
            ticker: "TST".to_owned(),
            qualified_ticker: "GPW:TST".to_owned(),
            display_name: "TEST COMPANY S.A.".to_owned(),
            isin: "PLTEST000001".to_owned(),
            source_url: "https://www.gpw.pl/spolka?isin=PLTEST000001".to_owned(),
        }];

        let result = state
            .refresh_gpw_company_registry(&entries, "2026-05-31T12:00:00Z")
            .expect("registry refresh should succeed");

        assert_eq!(result.adapter_id, GPW_REGISTRY_ADAPTER_ID);
        assert_eq!(result.entries_fetched, 1);
        assert_eq!(result.entries_upserted, 1);

        let lookup = state
            .lookup_company(CompanyLookupInput {
                exchange: "GPW".to_owned(),
                ticker: Some("tst".to_owned()),
                display_name: None,
                isin: None,
            })
            .expect("lookup should succeed")
            .expect("refreshed registry entry should match");

        assert_eq!(lookup.qualified_ticker, "GPW:TST");
        assert_eq!(lookup.source, "gpw_registry");
    }

    #[test]
    fn detects_stale_gpw_company_registry_cache() {
        let connection = open_in_memory_database().expect("database should initialize");

        assert!(gpw_company_registry_is_stale(&connection, 86_400)
            .expect("registry should report stale when never refreshed"));

        connection
            .execute(
                "
                UPDATE source_adapters
                SET last_success_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                WHERE id = ?1
                ",
                [GPW_REGISTRY_ADAPTER_ID],
            )
            .expect("registry adapter timestamp should update");

        assert!(!gpw_company_registry_is_stale(&connection, 86_400)
            .expect("fresh registry should not be stale"));

        connection
            .execute(
                "
                UPDATE source_adapters
                SET last_success_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-2 days')
                WHERE id = ?1
                ",
                [GPW_REGISTRY_ADAPTER_ID],
            )
            .expect("registry adapter timestamp should update");

        assert!(gpw_company_registry_is_stale(&connection, 86_400)
            .expect("old registry should be stale"));
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
