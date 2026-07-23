use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::{slug_part, StorageError, StorageResult};

const REMINDER_KINDS: &[&str] = &[
    "claim_follow_up",
    "event_review",
    "question_review",
    "manual_research",
    "digest_review",
    "signal_review",
];
const REMINDER_STATUSES: &[&str] = &["open", "completed", "dismissed"];

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReminderListInput {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchReminderStatus>")
    )]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReminder {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub company_id: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReminderKind")
    )]
    pub reminder_kind: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchEvidenceType>")
    )]
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub title: String,
    pub body: String,
    pub due_at: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReminderStatus")
    )]
    pub status: String,
    pub snoozed_until: Option<String>,
    pub completed_at: Option<String>,
    pub dismissed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewResearchReminder {
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReviewScopeType")
    )]
    pub scope_type: String,
    pub scope_id: String,
    pub company_id: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ResearchReminderKind")
    )]
    pub reminder_kind: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchEvidenceType>")
    )]
    pub source_type: Option<String>,
    pub source_id: Option<String>,
    pub title: String,
    pub body: Option<String>,
    pub due_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ResearchReminderUpdate {
    pub id: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ResearchReminderStatus>")
    )]
    pub status: Option<String>,
    pub due_at: Option<String>,
    pub snoozed_until: Option<String>,
}

pub(super) fn list_research_reminders(
    connection: &Connection,
    input: ResearchReminderListInput,
) -> StorageResult<Vec<ResearchReminder>> {
    validate_scope(connection, &input.scope_type, &input.scope_id)?;
    if let Some(status) = input.status.as_deref() {
        validate_allowed("reminder_status", status, REMINDER_STATUSES)?;
    }
    sync_derived_reminders(connection, &input.scope_type, &input.scope_id)?;

    let mut statement = connection.prepare(
        "
        SELECT
            id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id,
            title, body, due_at, status, snoozed_until, completed_at, dismissed_at,
            created_at, updated_at
        FROM research_reminders
        WHERE (?1 != 'company' OR scope_type = 'company' AND scope_id = ?2)
            AND (
                ?1 != 'watchlist'
                OR scope_type = 'watchlist' AND scope_id = ?2
                OR company_id IN (
                    SELECT company_id
                    FROM watchlist_companies
                    WHERE watchlist_id = ?2
                )
            )
            AND (?3 IS NULL OR status = ?3)
        ORDER BY
            CASE WHEN due_at IS NULL THEN 1 ELSE 0 END,
            datetime(COALESCE(snoozed_until, due_at, updated_at)) ASC,
            updated_at DESC,
            id
        ",
    )?;
    let rows = statement.query_map(
        params![input.scope_type, input.scope_id, input.status],
        research_reminder_from_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn create_research_reminder(
    connection: &Connection,
    input: NewResearchReminder,
) -> StorageResult<ResearchReminder> {
    validate_scope(connection, &input.scope_type, &input.scope_id)?;
    validate_allowed("reminder_kind", &input.reminder_kind, REMINDER_KINDS)?;
    let title = input.title.trim();
    if title.is_empty() {
        return Err(StorageError::InvalidSettingValue {
            key: "title",
            value: input.title,
        });
    }
    let id = next_research_reminder_id(connection, &input.scope_type, &input.scope_id)?;
    connection.execute(
        "
        INSERT INTO research_reminders (
            id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id,
            title, body, due_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ",
        params![
            id,
            input.scope_type,
            input.scope_id,
            input.company_id,
            input.reminder_kind,
            input.source_type,
            input.source_id,
            title,
            input.body.unwrap_or_default(),
            input.due_at
        ],
    )?;
    get_research_reminder(connection, &id)
}

pub(super) fn update_research_reminder(
    connection: &Connection,
    input: ResearchReminderUpdate,
) -> StorageResult<ResearchReminder> {
    let current = get_research_reminder(connection, &input.id)?;
    let status = input.status.unwrap_or(current.status);
    validate_allowed("reminder_status", &status, REMINDER_STATUSES)?;
    connection.execute(
        "
        UPDATE research_reminders
        SET
            status = ?2,
            due_at = COALESCE(?3, due_at),
            snoozed_until = ?4,
            completed_at = CASE WHEN ?2 = 'completed' THEN COALESCE(completed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE NULL END,
            dismissed_at = CASE WHEN ?2 = 'dismissed' THEN COALESCE(dismissed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')) ELSE NULL END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![input.id, status, input.due_at, input.snoozed_until],
    )?;
    get_research_reminder(connection, &input.id)
}

pub(super) fn delete_research_reminder(connection: &Connection, id: &str) -> StorageResult<()> {
    let affected = connection.execute("DELETE FROM research_reminders WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(StorageError::MissingResearchReference {
            table: "research_reminders".to_owned(),
            id: id.to_owned(),
        });
    }
    Ok(())
}

fn sync_derived_reminders(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<()> {
    match scope_type {
        "company" => {
            sync_company_derived_reminders(connection, scope_id)?;
        }
        "watchlist" => {
            let mut statement = connection
                .prepare("SELECT company_id FROM watchlist_companies WHERE watchlist_id = ?1")?;
            let company_ids = statement
                .query_map([scope_id], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for company_id in company_ids {
                sync_company_derived_reminders(connection, &company_id)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn sync_company_derived_reminders(connection: &Connection, company_id: &str) -> StorageResult<()> {
    connection.execute(
        "
        INSERT OR IGNORE INTO research_reminders (
            id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id,
            title, body, due_at
        )
        SELECT
            'reminder_claim_' || id,
            'company',
            company_id,
            company_id,
            'claim_follow_up',
            'claim',
            id,
            title,
            body,
            COALESCE(follow_up_date, event_date, updated_at)
        FROM notebook_entries
        WHERE company_id = ?1
            AND (kind = 'claim' OR claim_status IS NOT NULL)
            AND COALESCE(claim_status, 'open') != 'closed'
        ",
        [company_id],
    )?;
    connection.execute(
        "
        INSERT OR IGNORE INTO research_reminders (
            id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id,
            title, body, due_at
        )
        SELECT
            'reminder_event_' || id,
            'company',
            company_id,
            company_id,
            'event_review',
            'company_event',
            id,
            title,
            COALESCE(event_type, ''),
            event_date
        FROM company_events
        WHERE company_id = ?1
            AND COALESCE(status, 'scheduled') != 'cancelled'
        ",
        [company_id],
    )?;
    connection.execute(
        "
        INSERT OR IGNORE INTO research_reminders (
            id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id,
            title, body, due_at
        )
        SELECT
            'reminder_question_' || id,
            'company',
            scope_id,
            scope_id,
            'question_review',
            'research_question',
            id,
            title,
            body,
            updated_at
        FROM research_questions
        WHERE scope_type = 'company'
            AND scope_id = ?1
            AND status = 'open'
        ",
        [company_id],
    )?;
    Ok(())
}

fn get_research_reminder(connection: &Connection, id: &str) -> StorageResult<ResearchReminder> {
    connection
        .query_row(
            "
            SELECT
                id, scope_type, scope_id, company_id, reminder_kind, source_type, source_id,
                title, body, due_at, status, snoozed_until, completed_at, dismissed_at,
                created_at, updated_at
            FROM research_reminders
            WHERE id = ?1
            ",
            [id],
            research_reminder_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingResearchReference {
            table: "research_reminders".to_owned(),
            id: id.to_owned(),
        })
}

fn research_reminder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ResearchReminder> {
    Ok(ResearchReminder {
        id: row.get(0)?,
        scope_type: row.get(1)?,
        scope_id: row.get(2)?,
        company_id: row.get(3)?,
        reminder_kind: row.get(4)?,
        source_type: row.get(5)?,
        source_id: row.get(6)?,
        title: row.get(7)?,
        body: row.get(8)?,
        due_at: row.get(9)?,
        status: row.get(10)?,
        snoozed_until: row.get(11)?,
        completed_at: row.get(12)?,
        dismissed_at: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn validate_scope(connection: &Connection, scope_type: &str, scope_id: &str) -> StorageResult<()> {
    let table_name = match scope_type {
        "company" => "companies",
        "watchlist" => "watchlists",
        _ => {
            return Err(StorageError::InvalidSettingValue {
                key: "scope_type",
                value: scope_type.to_owned(),
            })
        }
    };
    let exists: bool = connection.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)"),
        [scope_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(StorageError::MissingResearchReference {
            table: table_name.to_owned(),
            id: scope_id.to_owned(),
        });
    }
    Ok(())
}

fn validate_allowed(key: &'static str, value: &str, allowed: &[&str]) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(StorageError::InvalidSettingValue {
            key,
            value: value.to_owned(),
        })
    }
}

fn next_research_reminder_id(
    connection: &Connection,
    scope_type: &str,
    scope_id: &str,
) -> StorageResult<String> {
    let prefix = format!(
        "research_reminder_{}_{}",
        slug_part(scope_type),
        slug_part(scope_id)
    );
    let count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM research_reminders WHERE id LIKE ?1",
        [format!("{prefix}_%")],
        |row| row.get(0),
    )?;
    Ok(format!("{prefix}_{:03}", count + 1))
}

use super::database::Database;
/// research_reminders domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::research_reminders()`.
#[derive(Clone)]
pub struct ResearchReminderStore {
    db: Database,
}

impl ResearchReminderStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_research_reminders(
        &self,
        input: ResearchReminderListInput,
    ) -> StorageResult<Vec<ResearchReminder>> {
        let connection = self.db.checkout()?;

        list_research_reminders(&connection, input)
    }

    pub fn create_research_reminder(
        &self,
        input: NewResearchReminder,
    ) -> StorageResult<ResearchReminder> {
        let connection = self.db.checkout()?;

        create_research_reminder(&connection, input)
    }

    pub fn update_research_reminder(
        &self,
        input: ResearchReminderUpdate,
    ) -> StorageResult<ResearchReminder> {
        let connection = self.db.checkout()?;

        update_research_reminder(&connection, input)
    }

    pub fn delete_research_reminder(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        delete_research_reminder(&connection, id)
    }
}
