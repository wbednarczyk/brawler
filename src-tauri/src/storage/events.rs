use super::feed_matching::normalize_media_character;
use super::*;
use super::{companies, sources, watchlists};

pub(super) fn list_company_events(
    connection: &Connection,
    input: CompanyEventListInput,
) -> StorageResult<Vec<CompanyEvent>> {
    let today = connection.query_row("SELECT date('now')", [], |row| row.get::<_, String>(0))?;
    let mode = input
        .mode
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("upcoming")
        .to_owned();

    validate_allowed_company_event_value("mode", &mode, &["upcoming", "historical", "all"])?;

    let mut statement = connection.prepare(
        "
        SELECT
            company_events.id,
            company_events.company_id,
            companies.qualified_ticker,
            companies.display_name,
            company_events.event_type,
            company_events.title,
            company_events.event_date,
            company_events.event_time,
            company_events.status,
            company_events.source_type,
            company_events.source_adapter_id,
            company_events.source_event_key,
            company_events.source_url,
            company_events.attribution,
            company_events.fetched_at,
            company_events.manual,
            company_events.created_at,
            company_events.updated_at
        FROM company_events
        JOIN companies ON companies.id = company_events.company_id
        ORDER BY company_events.event_date ASC, company_events.event_time ASC, company_events.title ASC
        ",
    )?;

    let rows = statement.query_map([], company_event_from_row)?;
    let events = rows.collect::<Result<Vec<_>, _>>()?;

    let filtered = events
        .into_iter()
        .filter(|event| {
            input
                .company_id
                .as_deref()
                .map(|company_id| event.company_id == company_id)
                .unwrap_or(true)
        })
        .filter(|event| {
            input
                .event_type
                .as_deref()
                .map(|event_type| event.event_type == event_type)
                .unwrap_or(true)
        })
        .filter(|event| {
            input
                .status
                .as_deref()
                .map(|status| event.status == status)
                .unwrap_or(true)
        })
        .filter(|event| {
            input
                .date_from
                .as_deref()
                .map(|date_from| event.event_date.as_str() >= date_from)
                .unwrap_or(true)
        })
        .filter(|event| {
            input
                .date_to
                .as_deref()
                .map(|date_to| event.event_date.as_str() <= date_to)
                .unwrap_or(true)
        })
        .filter(|event| {
            if input.date_from.is_some() || input.date_to.is_some() {
                return true;
            }

            match mode.as_str() {
                "upcoming" => event.event_date.as_str() >= today.as_str(),
                "historical" => event.event_date.as_str() < today.as_str(),
                _ => true,
            }
        })
        .filter(|event| {
            if let Some(watchlist_id) = input.watchlist_id.as_deref() {
                watchlists::company_is_in_watchlist(connection, watchlist_id, &event.company_id)
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect();

    Ok(filtered)
}

pub(super) fn create_company_event(
    connection: &Connection,
    input: NewCompanyEvent,
) -> StorageResult<CompanyEvent> {
    let event_type = input.event_type.trim().to_owned();
    let title = input.title.trim().to_owned();
    let event_date = input.event_date.trim().to_owned();
    let status = input
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("scheduled")
        .to_owned();
    let source_type = input
        .source_type
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("manual")
        .to_owned();
    let source_adapter_id = empty_string_to_none(input.source_adapter_id);
    let source_event_key = empty_string_to_none(input.source_event_key);
    let manual = source_type == "manual";
    let id = if let (Some(adapter_id), Some(source_key)) =
        (source_adapter_id.as_deref(), source_event_key.as_deref())
    {
        company_event_source_id(adapter_id, source_key)
    } else {
        company_event_id(&input.company_id, &event_type, &event_date, &title)
    };

    validate_allowed_company_event_value(
        "event_type",
        &event_type,
        &[
            "periodic_report",
            "corporate_action",
            "dividend",
            "shareholder_meeting",
            "conference_call",
            "investor_conference",
            "market_making",
            "listing_change",
            "other_market_event",
            "custom",
        ],
    )?;
    validate_allowed_company_event_value(
        "status",
        &status,
        &[
            "scheduled",
            "confirmed",
            "tentative",
            "changed",
            "cancelled",
            "completed",
            // Derived calendar events await user confirmation before they appear on the
            // calendar (ADR 0036).
            "proposed",
        ],
    )?;
    validate_allowed_company_event_value(
        "source_type",
        &source_type,
        &[
            "manual",
            "official_calendar",
            "official_report",
            "public_media",
            "notebook_entry",
            "feed_item",
            // Calendar event derived from a confirmed dividend/general-meeting signal (ADR 0036).
            "derived_signal",
        ],
    )?;
    connection.execute(
        "
        INSERT INTO company_events (
            id,
            company_id,
            event_type,
            title,
            event_date,
            event_time,
            status,
            source_type,
            source_adapter_id,
            source_event_key,
            source_url,
            attribution,
            fetched_at,
            manual
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
        ON CONFLICT(id) DO UPDATE SET
            company_id = excluded.company_id,
            event_type = excluded.event_type,
            title = excluded.title,
            event_date = excluded.event_date,
            event_time = excluded.event_time,
            status = excluded.status,
            source_type = excluded.source_type,
            source_adapter_id = excluded.source_adapter_id,
            source_event_key = excluded.source_event_key,
            source_url = excluded.source_url,
            attribution = excluded.attribution,
            fetched_at = excluded.fetched_at,
            manual = excluded.manual,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE excluded.source_adapter_id IS NOT NULL
            AND excluded.source_event_key IS NOT NULL
        ",
        params![
            id,
            input.company_id,
            event_type,
            title,
            event_date,
            empty_string_to_none(input.event_time),
            status,
            source_type,
            source_adapter_id,
            source_event_key,
            empty_string_to_none(input.source_url),
            empty_string_to_none(input.attribution),
            empty_string_to_none(input.fetched_at),
            manual,
        ],
    )?;

    get_company_event(connection, &id)
}

pub(super) fn ingest_gpw_market_event_items(
    connection: &mut Connection,
    items: &[GpwMarketEventItem],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let tracked_companies = companies::list_companies(&transaction)?;
    let fetched_at = items
        .first()
        .map(|item| item.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| sources::current_timestamp(&transaction))?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;

    for item in items {
        let Some(company) = tracked_companies
            .iter()
            .find(|company| company.exchange == "GPW" && company.ticker == item.ticker)
        else {
            items_unmatched += 1;
            continue;
        };

        items_matched += 1;
        let event_id =
            company_event_source_id(GPW_MARKET_EVENTS_ADAPTER_ID, &item.source_event_key);
        let already_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM company_events WHERE id = ?1)",
            [&event_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "
            INSERT INTO company_events (
                id,
                company_id,
                event_type,
                title,
                event_date,
                event_time,
                status,
                source_type,
                source_adapter_id,
                source_event_key,
                source_url,
                attribution,
                fetched_at,
                manual
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'scheduled', 'official_calendar', ?6, ?7, ?8, ?9, ?10, 0)
            ON CONFLICT(id) DO UPDATE SET
                company_id = excluded.company_id,
                event_type = excluded.event_type,
                title = excluded.title,
                event_date = excluded.event_date,
                event_time = excluded.event_time,
                status = excluded.status,
                source_type = excluded.source_type,
                source_adapter_id = excluded.source_adapter_id,
                source_event_key = excluded.source_event_key,
                source_url = excluded.source_url,
                attribution = excluded.attribution,
                fetched_at = excluded.fetched_at,
                manual = excluded.manual,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                event_id,
                company.id,
                item.event_type,
                item.title,
                item.event_date,
                GPW_MARKET_EVENTS_ADAPTER_ID,
                item.source_event_key,
                item.link,
                GPW_MARKET_EVENTS_ATTRIBUTION,
                item.fetched_at,
            ],
        )?;

        if !already_exists {
            items_created += 1;
        }
    }

    super::ingestion::record_source_outcome(
        &transaction,
        GPW_MARKET_EVENTS_ADAPTER_ID,
        &fetched_at,
        items.len(),
        items_created,
        items_matched,
        items_unmatched,
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: GPW_MARKET_EVENTS_ADAPTER_ID.to_owned(),
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

pub(super) fn ingest_bankier_calendar_event_items(
    connection: &mut Connection,
    items: &[BankierCalendarEventItem],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let tracked_companies = companies::list_companies(&transaction)?;
    let fetched_at = items
        .first()
        .map(|item| item.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| sources::current_timestamp(&transaction))?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;

    for item in items {
        let Some(company) = tracked_companies
            .iter()
            .find(|company| company.exchange == "GPW" && company.ticker == item.ticker)
            .cloned()
            .or_else(|| {
                find_company_for_bankier_calendar_symbol(&transaction, &item.ticker)
                    .ok()
                    .flatten()
            })
            .or_else(|| {
                tracked_companies
                    .iter()
                    .find(|company| {
                        company.exchange == "GPW"
                            && bankier_calendar_symbol_matches_company_name(
                                &item.ticker,
                                &company.display_name,
                            )
                    })
                    .cloned()
            })
        else {
            items_unmatched += 1;
            continue;
        };

        items_matched += 1;
        let event_id = company_event_source_id(BANKIER_CALENDAR_ADAPTER_ID, &item.source_event_key);
        let already_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM company_events WHERE id = ?1)",
            [&event_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "
            INSERT INTO company_events (
                id,
                company_id,
                event_type,
                title,
                event_date,
                event_time,
                status,
                source_type,
                source_adapter_id,
                source_event_key,
                source_url,
                attribution,
                fetched_at,
                manual
            ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'scheduled', 'public_calendar', ?6, ?7, ?8, ?9, ?10, 0)
            ON CONFLICT(id) DO UPDATE SET
                company_id = excluded.company_id,
                event_type = excluded.event_type,
                title = excluded.title,
                event_date = excluded.event_date,
                event_time = excluded.event_time,
                status = excluded.status,
                source_type = excluded.source_type,
                source_adapter_id = excluded.source_adapter_id,
                source_event_key = excluded.source_event_key,
                source_url = excluded.source_url,
                attribution = excluded.attribution,
                fetched_at = excluded.fetched_at,
                manual = excluded.manual,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                event_id,
                company.id,
                item.event_type,
                item.title,
                item.event_date,
                BANKIER_CALENDAR_ADAPTER_ID,
                item.source_event_key,
                item.link,
                BANKIER_CALENDAR_ATTRIBUTION,
                item.fetched_at,
            ],
        )?;

        if !already_exists {
            items_created += 1;
        }
    }

    super::ingestion::record_source_outcome(
        &transaction,
        BANKIER_CALENDAR_ADAPTER_ID,
        &fetched_at,
        items.len(),
        items_created,
        items_matched,
        items_unmatched,
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: BANKIER_CALENDAR_ADAPTER_ID.to_owned(),
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

pub(super) fn find_company_for_bankier_calendar_symbol(
    connection: &Connection,
    symbol: &str,
) -> StorageResult<Option<Company>> {
    let symbol = symbol.trim();
    if symbol.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT
                companies.id,
                companies.exchange,
                companies.ticker,
                companies.qualified_ticker,
                companies.display_name,
                companies.isin,
                companies.cik,
                companies.lei
            FROM companies
            INNER JOIN company_source_ids
                ON company_source_ids.company_id = companies.id
            WHERE companies.exchange = 'GPW'
                AND company_source_ids.source_adapter_id = ?1
                AND company_source_ids.source_key = 'instrument_slug'
                AND UPPER(company_source_ids.source_value) = ?2
            ORDER BY companies.qualified_ticker
            LIMIT 1
            ",
            params![BANKIER_COMPANY_ADAPTER_ID, symbol.to_uppercase()],
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
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn bankier_calendar_symbol_matches_company_name(
    symbol: &str,
    display_name: &str,
) -> bool {
    let symbol = normalize_calendar_match_text(symbol);
    let display_name = normalize_calendar_match_text(display_name);

    !symbol.is_empty()
        && symbol.chars().count() >= 3
        && (display_name == symbol || display_name.starts_with(&format!("{symbol} ")))
}

pub(super) fn normalize_calendar_match_text(value: &str) -> String {
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

pub(super) fn company_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CompanyEvent> {
    Ok(CompanyEvent {
        id: row.get(0)?,
        company_id: row.get(1)?,
        company: row.get(2)?,
        company_name: row.get(3)?,
        event_type: row.get(4)?,
        title: row.get(5)?,
        event_date: row.get(6)?,
        event_time: row.get(7)?,
        status: row.get(8)?,
        source_type: row.get(9)?,
        source_adapter_id: row.get(10)?,
        source_event_key: row.get(11)?,
        source_url: row.get(12)?,
        attribution: row.get(13)?,
        fetched_at: row.get(14)?,
        manual: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

pub(super) fn get_company_event(connection: &Connection, id: &str) -> StorageResult<CompanyEvent> {
    connection
        .query_row(
            "
            SELECT
                company_events.id,
                company_events.company_id,
                companies.qualified_ticker,
                companies.display_name,
                company_events.event_type,
                company_events.title,
                company_events.event_date,
                company_events.event_time,
                company_events.status,
                company_events.source_type,
                company_events.source_adapter_id,
                company_events.source_event_key,
                company_events.source_url,
                company_events.attribution,
                company_events.fetched_at,
                company_events.manual,
        company_events.created_at,
        company_events.updated_at
            FROM company_events
            JOIN companies ON companies.id = company_events.company_id
            WHERE company_events.id = ?1
            ",
            [id],
            company_event_from_row,
        )
        .map_err(StorageError::from)
}

use super::database::Database;
/// events domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::events()`.
#[derive(Clone)]
pub struct EventStore {
    db: Database,
}

impl EventStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn ingest_gpw_market_event_items(
        &self,
        items: &[GpwMarketEventItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;

        ingest_gpw_market_event_items(&mut connection, items)
    }

    pub fn ingest_bankier_calendar_event_items(
        &self,
        items: &[BankierCalendarEventItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;

        ingest_bankier_calendar_event_items(&mut connection, items)
    }

    pub fn list_company_events(
        &self,
        input: CompanyEventListInput,
    ) -> StorageResult<Vec<CompanyEvent>> {
        let connection = self.db.checkout()?;

        list_company_events(&connection, input)
    }

    pub fn create_company_event(&self, input: NewCompanyEvent) -> StorageResult<CompanyEvent> {
        let connection = self.db.checkout()?;

        create_company_event(&connection, input)
    }
}
