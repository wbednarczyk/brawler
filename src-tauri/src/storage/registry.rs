use super::*;

const REQUIRED_SOURCE_IDS: &[&str] = &[GPW_REGISTRY_ADAPTER_ID, NEWCONNECT_DIRECTORY_ADAPTER_ID];
const OPTIONAL_SOURCE_IDS: &[&str] = &[
    BANKIER_COMPANY_ADAPTER_ID,
    BANKIER_RSS_ADAPTER_ID,
    GPW_MARKET_EVENTS_ADAPTER_ID,
    BANKIER_CALENDAR_ADAPTER_ID,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SourceVisibility {
    Required,
    Optional,
    Developer,
}

impl SourceVisibility {
    fn as_str(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Optional => "optional",
            Self::Developer => "developer",
        }
    }
}

pub(super) fn source_visibility(adapter_id: &str) -> SourceVisibility {
    if REQUIRED_SOURCE_IDS.contains(&adapter_id) {
        SourceVisibility::Required
    } else if OPTIONAL_SOURCE_IDS.contains(&adapter_id) {
        SourceVisibility::Optional
    } else {
        SourceVisibility::Developer
    }
}

fn source_health_status(
    enabled: bool,
    last_success_at: Option<&str>,
    last_error: Option<&str>,
) -> &'static str {
    if !enabled {
        return "off";
    }

    if last_error.is_some_and(|value| !value.trim().is_empty()) {
        return "attention";
    }

    if last_success_at.is_some_and(|value| !value.trim().is_empty()) {
        "healthy"
    } else {
        "notRefreshed"
    }
}

pub(super) fn list_source_adapters(
    connection: &Connection,
    include_developer_only: bool,
) -> StorageResult<Vec<SourceAdapter>> {
    let mut statement = connection.prepare(
        "
        SELECT
            source_adapters.id,
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN 'GPW Company Directory'
                WHEN 'newconnect-company-directory' THEN 'NewConnect Company Directory'
                ELSE source_adapters.display_name
            END AS display_name,
            source_adapters.source_type,
            source_adapters.fetch_mode,
            source_adapters.enabled,
            source_adapters.default_poll_interval_seconds,
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN ?1
                WHEN 'newconnect-company-directory' THEN ?2
                WHEN 'bankier-market-rss' THEN ?3
                WHEN 'bankier-company-komunikaty' THEN ?4
                WHEN 'portal-analiz' THEN ?5
                WHEN 'bankier-firma-rss' THEN ?6
                WHEN 'bankier-wiadomosci-rss' THEN ?7
                WHEN 'gpw-market-events-rss' THEN ?8
                WHEN 'bankier-kalendarium-html' THEN ?9
                WHEN 'strefa-report-calendar' THEN ?10
                WHEN 'money-calendar' THEN ?11
                ELSE 'https://www.gpw.pl/komunikaty'
            END AS source_url,
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN 'Manual refresh plus daily stale-cache scheduled refresh'
                WHEN 'newconnect-company-directory' THEN 'Manual refresh plus daily stale-cache scheduled refresh'
                WHEN 'bankier-market-rss' THEN 'Manual refresh plus normal in-app source scheduler; RSS feed only, no article crawling'
                WHEN 'bankier-company-komunikaty' THEN 'Manual refresh plus normal in-app source scheduler; tracked GPW companies only; cached Bankier tag ids; one listing page plus matched article pages per company'
                WHEN 'portal-analiz' THEN 'Late-v1 disabled placeholder; no automated access until the authenticated-source implementation is explicitly built'
                WHEN 'bankier-firma-rss' THEN 'Reviewed public RSS candidate; disabled until matching quality is proven against tracked GPW companies'
                WHEN 'bankier-wiadomosci-rss' THEN 'Reviewed public RSS candidate; disabled because expected listed-company signal is broad and noisy'
                WHEN 'gpw-market-events-rss' THEN 'Manual refresh plus normal in-app source scheduler; official GPW market-events RSS; exact ticker matching only'
                WHEN 'bankier-kalendarium-html' THEN 'Manual refresh plus normal in-app source scheduler; one public calendar page; tracked GPW companies only; exact ticker matching'
                WHEN 'strefa-report-calendar' THEN 'Disabled event-source candidate; report-date extraction requires source-specific tests before runtime enablement'
                WHEN 'money-calendar' THEN 'Disabled event-source candidate; calendar extraction requires source-specific tests before runtime enablement'
                ELSE 'Disabled while Bankier Company Komunikaty is the active official-report source'
            END AS rate_limit_policy,
            CASE source_adapters.id
                WHEN 'gpw-company-registry' THEN 'Fetches the complete public GPW company list and caches ticker and ISIN metadata locally for lookup, autocomplete, and ticker-first matching.'
                WHEN 'newconnect-company-directory' THEN 'Fetches the complete public NewConnect company list and caches ticker and ISIN metadata for lookup, autocomplete, and ticker-first matching.'
                WHEN 'bankier-market-rss' THEN 'Fetches Bankier.pl public Giełda RSS headlines as public media items; linked article pages are not crawled in this slice.'
                WHEN 'bankier-company-komunikaty' THEN 'Fetches Bankier.pl per-company public komunikaty JSON and article pages for tracked GPW companies only. Bankier is the active v1 official-report source while GPW ESPI/EBI is disabled.'
                WHEN 'portal-analiz' THEN 'Late-v1 planned authenticated private research adapter governed by ADR 0014. Credentials must use the OS keychain and no generic login or scraping subsystem is approved.'
                WHEN 'bankier-firma-rss' THEN 'Reviewed M8 follow-up candidate. Public and RSS-native, but broader business coverage needs matching-quality tests before runtime enablement.'
                WHEN 'bankier-wiadomosci-rss' THEN 'Reviewed M8 follow-up candidate. Public and RSS-native, but broad news coverage and stale backfill risk make it unsuitable for default v1 ingestion.'
                WHEN 'gpw-market-events-rss' THEN 'Fetches GPW official market-events RSS for corporate-action and exchange calendar events. Creates company events only for tracked companies matched by exact ticker.'
                WHEN 'bankier-kalendarium-html' THEN 'Active M9 public calendar source for broader GPW event coverage. Creates company events only for tracked companies matched by exact ticker, while preserving Bankier attribution and source URLs.'
                WHEN 'strefa-report-calendar' THEN 'Fallback candidate for periodic-report publication dates. Disabled until source-specific sample parsing and attribution rules are accepted.'
                WHEN 'money-calendar' THEN 'Fallback/cross-check candidate for calendar and report-date coverage. Disabled until source-specific sample parsing and matching quality are accepted.'
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
            NEWCONNECT_DIRECTORY_SOURCE_URL,
            BANKIER_RSS_SOURCE_URL,
            BANKIER_COMPANY_SOURCE_URL,
            PORTAL_ANALIZ_SOURCE_URL,
            BANKIER_FIRMA_RSS_SOURCE_URL,
            BANKIER_WIADOMOSCI_RSS_SOURCE_URL,
            GPW_MARKET_EVENTS_SOURCE_URL,
            BANKIER_CALENDAR_SOURCE_URL,
            STREFA_REPORT_CALENDAR_SOURCE_URL,
            MONEY_CALENDAR_SOURCE_URL,
        ],
        |row| {
            let id: String = row.get(0)?;
            let enabled: bool = row.get(4)?;
            let last_success_at: Option<String> = row.get(11)?;
            let last_error: Option<String> = row.get(13)?;
            let visibility = source_visibility(&id);
            if visibility == SourceVisibility::Developer && !include_developer_only {
                return Ok(None);
            }
            let markets: String = row.get(22)?;

            Ok(Some(SourceAdapter {
                id,
                display_name: row.get(1)?,
                source_type: row.get(2)?,
                fetch_mode: row.get(3)?,
                visibility: visibility.as_str().to_owned(),
                user_configurable: visibility == SourceVisibility::Optional,
                health_status: source_health_status(
                    enabled,
                    last_success_at.as_deref(),
                    last_error.as_deref(),
                )
                .to_owned(),
                enabled,
                default_poll_interval_seconds: row.get(5)?,
                source_url: row.get(6)?,
                rate_limit_policy: row.get(7)?,
                policy_note: row.get(8)?,
                last_attempt_at: row.get(9)?,
                last_trigger: row.get(10)?,
                last_success_at,
                last_error_at: row.get(12)?,
                last_error,
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
            }))
        },
    )?;

    let adapters = rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(adapters)
}

pub(super) fn set_source_adapter_enabled(
    connection: &Connection,
    adapter_id: &str,
    enabled: bool,
) -> StorageResult<SourceAdapter> {
    match source_visibility(adapter_id) {
        SourceVisibility::Required => {
            if !enabled {
                return Err(StorageError::InvalidSourceValue {
                    key: "source",
                    value: format!("{adapter_id} is required"),
                });
            }
        }
        SourceVisibility::Optional => {}
        SourceVisibility::Developer => {
            return Err(StorageError::InvalidSourceValue {
                key: "source",
                value: format!("{adapter_id} is not user configurable"),
            });
        }
    }

    let updated = connection.execute(
        "
        UPDATE source_adapters
        SET enabled = ?1,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![enabled, adapter_id],
    )?;

    if updated == 0 {
        return Err(StorageError::InvalidSourceValue {
            key: "source",
            value: adapter_id.to_owned(),
        });
    }

    list_source_adapters(connection, true)?
        .into_iter()
        .find(|adapter| adapter.id == adapter_id)
        .ok_or_else(|| StorageError::InvalidSourceValue {
            key: "source",
            value: adapter_id.to_owned(),
        })
}

pub(super) fn source_adapter_enabled(
    connection: &Connection,
    adapter_id: &str,
) -> StorageResult<bool> {
    connection
        .query_row(
            "SELECT enabled FROM source_adapters WHERE id = ?1",
            [adapter_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| StorageError::InvalidSourceValue {
            key: "source",
            value: adapter_id.to_owned(),
        })
}

pub(super) fn list_company_registry_entries(
    connection: &Connection,
) -> StorageResult<Vec<CompanyRegistryEntry>> {
    let mut statement = connection.prepare(
        "
        SELECT
            registry.source_adapter_id,
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
        WHERE registry.active = 1
        ORDER BY registry.exchange, registry.ticker
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(CompanyRegistryEntry {
            source_adapter_id: row.get(0)?,
            exchange: row.get(1)?,
            ticker: row.get(2)?,
            qualified_ticker: row.get(3)?,
            display_name: row.get(4)?,
            isin: row.get(5)?,
            source_url: row.get(6)?,
            fetched_at: row.get(7)?,
            tracked: row.get(8)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn record_source_adapter_attempt(
    connection: &Connection,
    adapter_id: &str,
    trigger: &str,
) -> StorageResult<()> {
    let attempted_at = sources::current_timestamp(connection)?;
    sources::set_source_adapter_state(connection, adapter_id, "last_attempt_at", &attempted_at)?;
    sources::set_source_adapter_state(connection, adapter_id, "last_trigger", trigger)?;

    Ok(())
}

pub(super) fn record_source_adapter_error(
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
