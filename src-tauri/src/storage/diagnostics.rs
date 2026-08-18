use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::observability::redact_metadata;

use super::{StorageError, StorageResult};

const RETENTION_EVENT_LIMIT: i64 = 1_000;
const RETENTION_DAYS: i64 = 7;

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub id: String,
    pub occurred_at: String,
    pub module: String,
    pub scope: Option<DiagnosticScope>,
    pub stage: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"debug\" | \"info\" | \"warning\" | \"error\"")
    )]
    pub severity: String,
    pub message: String,
    #[cfg_attr(feature = "ts-export", ts(type = "Record<string, unknown>"))]
    pub metadata: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticScope {
    #[serde(rename = "type")]
    pub scope_type: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewDiagnosticEvent {
    pub occurred_at: Option<String>,
    pub module: String,
    pub scope: Option<DiagnosticScope>,
    pub stage: String,
    pub severity: String,
    pub message: String,
    pub metadata: Option<Value>,
}

pub(crate) fn record_diagnostic_event(
    connection: &mut Connection,
    input: NewDiagnosticEvent,
) -> StorageResult<Option<DiagnosticEvent>> {
    if !developer_mode_enabled(connection)? {
        return Ok(None);
    }
    record_diagnostic_event_ungated(connection, input)
}

/// [`record_diagnostic_event`] WITHOUT the developer-mode gate — for
/// integrity events that must reach the owner in ordinary operation (sol
/// review round 3, finding 2): a swallowed best-effort failure recorded only
/// behind a settings flag is a log line wearing a diagnostic's clothes. The
/// gate exists to keep chatty dev-time telemetry out of a normal profile,
/// never to hide data-loss evidence.
pub(crate) fn record_integrity_event(
    connection: &mut Connection,
    input: NewDiagnosticEvent,
) -> StorageResult<Option<DiagnosticEvent>> {
    record_diagnostic_event_ungated(connection, input)
}

fn record_diagnostic_event_ungated(
    connection: &mut Connection,
    input: NewDiagnosticEvent,
) -> StorageResult<Option<DiagnosticEvent>> {
    validate_identifier("module", &input.module)?;
    validate_identifier("stage", &input.stage)?;
    validate_allowed_severity(&input.severity)?;
    validate_required_value("message", &input.message)?;

    if let Some(scope) = input.scope.as_ref() {
        validate_identifier("scope_type", &scope.scope_type)?;
    }

    let metadata = redact_metadata(input.metadata.unwrap_or_else(|| Value::Object(Map::new())));
    let metadata_json = serde_json::to_string(&metadata)?;
    let scope_type = input.scope.as_ref().map(|scope| scope.scope_type.as_str());
    let scope_id = input
        .scope
        .as_ref()
        .and_then(|scope| scope.id.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    connection.execute(
        "
        INSERT INTO diagnostic_events (
            occurred_at,
            module,
            scope_type,
            scope_id,
            stage,
            severity,
            message,
            metadata_json
        ) VALUES (
            COALESCE(?1, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            ?2,
            ?3,
            ?4,
            ?5,
            ?6,
            ?7,
            ?8
        )
        ",
        params![
            input.occurred_at,
            input.module,
            scope_type,
            scope_id,
            input.stage,
            input.severity,
            input.message.trim(),
            metadata_json
        ],
    )?;

    let event = connection.query_row(
        "
        SELECT id,
               occurred_at,
               module,
               scope_type,
               scope_id,
               stage,
               severity,
               message,
               metadata_json,
               created_at
        FROM diagnostic_events
        WHERE rowid = last_insert_rowid()
        ",
        [],
        diagnostic_event_from_row,
    )?;

    trim_diagnostic_events(connection)?;

    Ok(Some(event))
}

pub(crate) fn list_diagnostic_events(
    connection: &Connection,
    limit: i64,
) -> StorageResult<Vec<DiagnosticEvent>> {
    let bounded_limit = limit.clamp(1, RETENTION_EVENT_LIMIT);
    let mut statement = connection.prepare(
        "
        SELECT id,
               occurred_at,
               module,
               scope_type,
               scope_id,
               stage,
               severity,
               message,
               metadata_json,
               created_at
        FROM diagnostic_events
        ORDER BY occurred_at DESC, created_at DESC, rowid DESC
        LIMIT ?1
        ",
    )?;

    let events = statement
        .query_map([bounded_limit], diagnostic_event_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(events)
}

pub(crate) fn clear_diagnostic_events(connection: &Connection) -> StorageResult<usize> {
    connection
        .execute("DELETE FROM diagnostic_events", [])
        .map_err(StorageError::from)
}

fn trim_diagnostic_events(connection: &Connection) -> StorageResult<()> {
    connection.execute(
        "
        DELETE FROM diagnostic_events
        WHERE occurred_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)
        ",
        [format!("-{RETENTION_DAYS} days")],
    )?;

    connection.execute(
        "
        DELETE FROM diagnostic_events
        WHERE rowid NOT IN (
            SELECT rowid
            FROM diagnostic_events
            ORDER BY occurred_at DESC, created_at DESC, rowid DESC
            LIMIT ?1
        )
        ",
        [RETENTION_EVENT_LIMIT],
    )?;

    Ok(())
}

fn diagnostic_event_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<DiagnosticEvent> {
    let scope_type: Option<String> = row.get(3)?;
    let scope_id: Option<String> = row.get(4)?;
    let metadata_json: String = row.get(8)?;

    let metadata = serde_json::from_str(&metadata_json).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(8, rusqlite::types::Type::Text, Box::new(error))
    })?;

    Ok(DiagnosticEvent {
        id: row.get(0)?,
        occurred_at: row.get(1)?,
        module: row.get(2)?,
        scope: scope_type.map(|scope_type| DiagnosticScope {
            scope_type,
            id: scope_id,
        }),
        stage: row.get(5)?,
        severity: row.get(6)?,
        message: row.get(7)?,
        metadata,
        created_at: row.get(9)?,
    })
}

fn developer_mode_enabled(connection: &Connection) -> StorageResult<bool> {
    connection
        .query_row(
            "SELECT value FROM settings WHERE key = 'developer_mode'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|value| value == "true")
        .ok_or(StorageError::InvalidSettingValue {
            key: "developer_mode",
            value: "missing".to_owned(),
        })
}

fn validate_required_value(key: &'static str, value: &str) -> StorageResult<()> {
    if value.trim().is_empty() {
        Err(StorageError::InvalidDiagnosticValue {
            key,
            value: value.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_identifier(key: &'static str, value: &str) -> StorageResult<()> {
    validate_required_value(key, value)?;

    if value.chars().all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
    }) {
        Ok(())
    } else {
        Err(StorageError::InvalidDiagnosticValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn validate_allowed_severity(value: &str) -> StorageResult<()> {
    if matches!(value, "debug" | "info" | "warning" | "error") {
        Ok(())
    } else {
        Err(StorageError::InvalidDiagnosticValue {
            key: "severity",
            value: value.to_owned(),
        })
    }
}

use super::database::Database;
/// diagnostics domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::diagnostics()`.
#[derive(Clone)]
pub struct DiagnosticsStore {
    db: Database,
}

impl DiagnosticsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn record_diagnostic_event(
        &self,
        input: NewDiagnosticEvent,
    ) -> StorageResult<Option<DiagnosticEvent>> {
        let mut connection = self.db.checkout()?;

        record_diagnostic_event(&mut connection, input)
    }

    /// Ungated integrity event — see [`record_integrity_event`] (free fn).
    pub fn record_integrity_event(
        &self,
        input: NewDiagnosticEvent,
    ) -> StorageResult<Option<DiagnosticEvent>> {
        let mut connection = self.db.checkout()?;

        record_integrity_event(&mut connection, input)
    }

    pub fn list_diagnostic_events(&self, limit: i64) -> StorageResult<Vec<DiagnosticEvent>> {
        let connection = self.db.checkout()?;

        list_diagnostic_events(&connection, limit)
    }

    pub fn clear_diagnostic_events(&self) -> StorageResult<usize> {
        let connection = self.db.checkout()?;

        clear_diagnostic_events(&connection)
    }
}
