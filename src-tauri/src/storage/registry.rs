use super::*;
use crate::source_adapters::registry::{self as adapter_registry, SourceVisibility};

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
    // The static catalog metadata (display name, source URL, rate-limit policy,
    // policy note, visibility) comes from the source-adapter registry — the SSOT
    // (ADR 0050). This query reads only the mutable/structured runtime state from
    // the database; the row mapper merges in the descriptor. A drift-guard test
    // (`registry_matches_seeded_catalog`) binds the registry to the seed rows.
    let mut statement = connection.prepare(
        "
        SELECT
            source_adapters.id,
            source_adapters.display_name,
            source_adapters.source_type,
            source_adapters.fetch_mode,
            source_adapters.enabled,
            source_adapters.default_poll_interval_seconds,
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

    let rows = statement.query_map([], |row| {
        let id: String = row.get(0)?;
        let visibility = adapter_registry::source_visibility(&id);
        if visibility == SourceVisibility::Developer && !include_developer_only {
            return Ok(None);
        }

        let descriptor = adapter_registry::descriptor(&id);
        let db_display_name: String = row.get(1)?;
        let display_name = descriptor
            .map(|adapter| adapter.display_name.to_owned())
            .unwrap_or(db_display_name);
        let source_url = descriptor
            .map(|adapter| adapter.source_url.to_owned())
            .unwrap_or_default();
        let rate_limit_policy = descriptor
            .map(|adapter| adapter.rate_limit_policy.to_owned())
            .unwrap_or_default();
        let policy_note = descriptor
            .map(|adapter| adapter.policy_note.to_owned())
            .unwrap_or_default();

        let enabled: bool = row.get(4)?;
        let last_success_at: Option<String> = row.get(8)?;
        let last_error: Option<String> = row.get(10)?;
        let markets: String = row.get(19)?;

        Ok(Some(SourceAdapter {
            id,
            display_name,
            source_type: row.get(2)?,
            fetch_mode: row.get(3)?,
            visibility: visibility.as_str().to_owned(),
            user_configurable: visibility.user_configurable(),
            health_status: source_health_status(
                enabled,
                last_success_at.as_deref(),
                last_error.as_deref(),
            )
            .to_owned(),
            enabled,
            default_poll_interval_seconds: row.get(5)?,
            source_url,
            rate_limit_policy,
            policy_note,
            last_attempt_at: row.get(6)?,
            last_trigger: row.get(7)?,
            last_success_at,
            last_error_at: row.get(9)?,
            last_error,
            last_items_fetched: row.get(11)?,
            last_items_created: row.get(12)?,
            last_items_matched: row.get(13)?,
            last_items_unmatched: row.get(14)?,
            last_detail_items_attempted: row.get(15)?,
            last_detail_items_stored: row.get(16)?,
            last_detail_items_failed: row.get(17)?,
            last_detail_warning: row.get(18)?,
            markets: markets
                .split(',')
                .filter(|market| !market.is_empty())
                .map(str::to_owned)
                .collect(),
        }))
    })?;

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
    match adapter_registry::source_visibility(adapter_id) {
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

use super::database::Database;
/// registry domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::source_registry()`.
#[derive(Clone)]
pub struct SourceRegistryStore {
    db: Database,
}

impl SourceRegistryStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_source_adapters_with_developer(
        &self,
        include_developer_only: bool,
    ) -> StorageResult<Vec<SourceAdapter>> {
        let connection = self.db.checkout()?;

        list_source_adapters(&connection, include_developer_only)
    }

    pub fn set_source_adapter_enabled(
        &self,
        adapter_id: &str,
        enabled: bool,
    ) -> StorageResult<SourceAdapter> {
        let connection = self.db.checkout()?;

        set_source_adapter_enabled(&connection, adapter_id, enabled)
    }

    pub fn source_adapter_enabled(&self, adapter_id: &str) -> StorageResult<bool> {
        let connection = self.db.checkout()?;

        source_adapter_enabled(&connection, adapter_id)
    }

    pub fn list_company_registry_entries(&self) -> StorageResult<Vec<CompanyRegistryEntry>> {
        let connection = self.db.checkout()?;

        list_company_registry_entries(&connection)
    }

    pub fn record_source_adapter_error(&self, adapter_id: &str, error: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        record_source_adapter_error(&connection, adapter_id, error)
    }

    pub fn record_source_adapter_attempt(
        &self,
        adapter_id: &str,
        trigger: &str,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        record_source_adapter_attempt(&connection, adapter_id, trigger)
    }
}
