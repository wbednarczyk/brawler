//! Report-season cockpit read models and preparation workflow (ADR 0044).
//!
//! The calendar and pre-report card are backend-owned read models assembled from
//! the canonical domains — `company_events`, `research_questions`,
//! `management_claims`, `financial_facts`/`financial_periods`, and the
//! research-timeline read model — with no stored projection and no duplicated
//! domain logic. The only persisted state is the per-occurrence prepare->process
//! workflow status in `report_preparations`.

use super::*;
use std::collections::HashMap;

const REPORT_EVENT_TYPE: &str = "periodic_report";
const PREPARATION_STATUSES: &[&str] = &["upcoming", "prepared", "processed"];

#[derive(Debug, Default, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "ListReportSeasonInput"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ReportSeasonInput {
    pub watchlist_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportSeasonEntry {
    pub company_id: String,
    pub qualified_ticker: String,
    pub display_name: String,
    pub event_key: String,
    pub event_date: String,
    pub event_time: Option<String>,
    pub title: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ReportPreparationStatus")
    )]
    pub preparation_status: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CalendarFreshness {
    pub last_fetched_at: Option<String>,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportSeasonResult {
    pub upcoming: Vec<ReportSeasonEntry>,
    pub past: Vec<ReportSeasonEntry>,
    pub calendar_freshness: CalendarFreshness,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreReportCardInput {
    pub company_id: String,
    pub event_key: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PreReportKpi {
    pub period_id: String,
    pub metric_key: String,
    pub label: String,
    pub unit: Option<String>,
    pub value_numeric: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PreReportCard {
    pub company_id: String,
    pub event_key: String,
    pub event_date: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ReportPreparationStatus")
    )]
    pub preparation_status: String,
    pub linked_report_document_id: Option<String>,
    pub open_questions: Vec<ResearchQuestion>,
    pub unresolved_claims: ClaimsToVerify,
    pub last_period_kpis: Vec<PreReportKpi>,
    pub recent_evidence: Vec<ResearchEvidenceItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkReportPreparedInput {
    pub company_id: String,
    pub event_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkReportProcessedInput {
    pub company_id: String,
    pub event_key: String,
    #[serde(default)]
    pub linked_report_document_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportPreparation {
    pub company_id: String,
    pub event_key: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ReportPreparationStatus")
    )]
    pub status: String,
    pub prepared_at: Option<String>,
    pub processed_at: Option<String>,
    pub linked_report_document_id: Option<String>,
}

/// One stored preparation row, defaulting to `upcoming` when absent.
struct StoredPreparation {
    status: String,
    prepared_at: Option<String>,
    processed_at: Option<String>,
    linked_report_document_id: Option<String>,
}

impl Default for StoredPreparation {
    fn default() -> Self {
        Self {
            status: "upcoming".to_owned(),
            prepared_at: None,
            processed_at: None,
            linked_report_document_id: None,
        }
    }
}

fn report_preparation_id(company_id: &str, event_key: &str) -> String {
    format!(
        "report_prep_{}_{}",
        slug_part(company_id),
        slug_part(event_key)
    )
}

/// The stable key for a report occurrence: the calendar `source_event_key` when
/// present, otherwise the event row id (manual events have no source key).
fn entry_event_key(event: &CompanyEvent) -> String {
    event
        .source_event_key
        .clone()
        .filter(|key| !key.trim().is_empty())
        .unwrap_or_else(|| event.id.clone())
}

fn load_preparation(
    connection: &Connection,
    company_id: &str,
    event_key: &str,
) -> StorageResult<StoredPreparation> {
    connection
        .query_row(
            "SELECT status, prepared_at, processed_at, linked_report_document_id
             FROM report_preparations
             WHERE company_id = ?1 AND event_key = ?2",
            params![company_id, event_key],
            |row| {
                Ok(StoredPreparation {
                    status: row.get(0)?,
                    prepared_at: row.get(1)?,
                    processed_at: row.get(2)?,
                    linked_report_document_id: row.get(3)?,
                })
            },
        )
        .optional()
        .map(Option::unwrap_or_default)
        .map_err(StorageError::from)
}

pub(super) fn list_report_season(
    connection: &Connection,
    input: ReportSeasonInput,
) -> StorageResult<ReportSeasonResult> {
    if let Some(watchlist_id) = input.watchlist_id.as_deref() {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM watchlists WHERE id = ?1)",
            [watchlist_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StorageError::MissingReportSeasonReference {
                table: "watchlists".to_owned(),
                id: watchlist_id.to_owned(),
            });
        }
    }

    // Reuse the canonical calendar query path; no duplicated event logic.
    let events = events::list_company_events(
        connection,
        CompanyEventListInput {
            mode: Some("all".to_owned()),
            event_type: Some(REPORT_EVENT_TYPE.to_owned()),
            watchlist_id: input.watchlist_id.clone(),
            ..CompanyEventListInput::default()
        },
    )?;

    let (today, stale_threshold) =
        connection.query_row("SELECT date('now'), date('now', '-2 days')", [], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

    // One pass over stored preparation rows, keyed by (company_id, event_key).
    let mut preparations: HashMap<(String, String), String> = HashMap::new();
    {
        let mut statement =
            connection.prepare("SELECT company_id, event_key, status FROM report_preparations")?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (company_id, event_key, status) = row?;
            preparations.insert((company_id, event_key), status);
        }
    }

    let mut upcoming = Vec::new();
    let mut past = Vec::new();
    let mut last_fetched_at: Option<String> = None;

    for event in events {
        if let Some(fetched_at) = event.fetched_at.as_deref() {
            if last_fetched_at
                .as_deref()
                .is_none_or(|current| fetched_at > current)
            {
                last_fetched_at = Some(fetched_at.to_owned());
            }
        }

        let event_key = entry_event_key(&event);
        let preparation_status = preparations
            .get(&(event.company_id.clone(), event_key.clone()))
            .cloned()
            .unwrap_or_else(|| "upcoming".to_owned());

        let entry = ReportSeasonEntry {
            company_id: event.company_id,
            qualified_ticker: event.company,
            display_name: event.company_name,
            event_key,
            event_date: event.event_date.clone(),
            event_time: event.event_time,
            title: event.title,
            preparation_status,
        };

        if entry.event_date.as_str() >= today.as_str() {
            upcoming.push(entry);
        } else {
            past.push(entry);
        }
    }

    // Upcoming earliest-first; past most-recent-first.
    past.reverse();

    let stale = match last_fetched_at.as_deref() {
        Some(fetched_at) => fetched_at.get(0..10).unwrap_or(fetched_at) < stale_threshold.as_str(),
        None => true,
    };

    Ok(ReportSeasonResult {
        upcoming,
        past,
        calendar_freshness: CalendarFreshness {
            last_fetched_at,
            stale,
        },
    })
}

pub(super) fn get_pre_report_card(
    connection: &Connection,
    input: PreReportCardInput,
) -> StorageResult<PreReportCard> {
    ensure_company_exists(connection, &input.company_id)?;

    let preparation = load_preparation(connection, &input.company_id, &input.event_key)?;

    let event_date: Option<String> = connection
        .query_row(
            "SELECT event_date FROM company_events
             WHERE company_id = ?1
               AND event_type = ?2
               AND (source_event_key = ?3 OR id = ?3)
             ORDER BY event_date ASC
             LIMIT 1",
            params![input.company_id, REPORT_EVENT_TYPE, input.event_key],
            |row| row.get(0),
        )
        .optional()?;

    let open_questions = research::list_research_questions(
        connection,
        ResearchQuestionListInput {
            scope_type: Some("company".to_owned()),
            scope_id: Some(input.company_id.clone()),
            status: Some("open".to_owned()),
        },
    )?;

    let unresolved_claims =
        management_claims::list_claims_to_verify(connection, &input.company_id)?;

    let last_period_kpis = list_last_period_kpis(connection, &input.company_id)?;

    let recent_evidence = research::list_research_evidence(
        connection,
        ResearchEvidenceInput {
            company_id: Some(input.company_id.clone()),
            watchlist_id: None,
            evidence_types: None,
            changed_since_review_only: None,
            limit: Some(5),
        },
    )?
    .items;

    Ok(PreReportCard {
        company_id: input.company_id,
        event_key: input.event_key,
        event_date,
        preparation_status: preparation.status,
        linked_report_document_id: preparation.linked_report_document_id,
        open_questions,
        unresolved_claims,
        last_period_kpis,
        recent_evidence,
    })
}

fn list_last_period_kpis(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<PreReportKpi>> {
    let mut statement = connection.prepare(
        "
        SELECT f.period_id, d.metric_key, d.label, d.unit, f.value_numeric
        FROM financial_facts f
        JOIN financial_periods p ON p.id = f.period_id
        JOIN kpi_definitions d ON d.id = f.definition_id
        WHERE f.company_id = ?1
          AND f.confirmation_state = 'confirmed'
          AND p.id = (
              SELECT id FROM financial_periods
              WHERE company_id = ?1
              ORDER BY COALESCE(period_end_date, '') DESC, fiscal_year DESC, period_type DESC
              LIMIT 1
          )
        ORDER BY d.metric_key COLLATE NOCASE
        ",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok(PreReportKpi {
            period_id: row.get(0)?,
            metric_key: row.get(1)?,
            label: row.get(2)?,
            unit: row.get(3)?,
            value_numeric: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn mark_report_prepared(
    connection: &Connection,
    input: MarkReportPreparedInput,
) -> StorageResult<ReportPreparation> {
    ensure_company_exists(connection, &input.company_id)?;
    let event_key = validate_event_key(&input.event_key)?;

    let id = report_preparation_id(&input.company_id, &event_key);
    connection.execute(
        "
        INSERT INTO report_preparations (id, company_id, event_key, status, prepared_at)
        VALUES (?1, ?2, ?3, 'prepared', strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ON CONFLICT (company_id, event_key) DO UPDATE SET
            status = 'prepared',
            prepared_at = COALESCE(report_preparations.prepared_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![id, input.company_id, event_key],
    )?;

    read_preparation(connection, &input.company_id, &event_key)
}

pub(super) fn mark_report_processed(
    connection: &Connection,
    input: MarkReportProcessedInput,
) -> StorageResult<ReportPreparation> {
    ensure_company_exists(connection, &input.company_id)?;
    let event_key = validate_event_key(&input.event_key)?;
    let linked = empty_string_to_none(input.linked_report_document_id);

    let id = report_preparation_id(&input.company_id, &event_key);
    connection.execute(
        "
        INSERT INTO report_preparations
            (id, company_id, event_key, status, prepared_at, processed_at, linked_report_document_id)
        VALUES (?1, ?2, ?3, 'processed',
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                ?4)
        ON CONFLICT (company_id, event_key) DO UPDATE SET
            status = 'processed',
            prepared_at = COALESCE(report_preparations.prepared_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            processed_at = COALESCE(report_preparations.processed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            linked_report_document_id = COALESCE(?4, report_preparations.linked_report_document_id),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![id, input.company_id, event_key, linked],
    )?;

    read_preparation(connection, &input.company_id, &event_key)
}

fn read_preparation(
    connection: &Connection,
    company_id: &str,
    event_key: &str,
) -> StorageResult<ReportPreparation> {
    let stored = load_preparation(connection, company_id, event_key)?;
    Ok(ReportPreparation {
        company_id: company_id.to_owned(),
        event_key: event_key.to_owned(),
        status: stored.status,
        prepared_at: stored.prepared_at,
        processed_at: stored.processed_at,
        linked_report_document_id: stored.linked_report_document_id,
    })
}

fn validate_event_key(event_key: &str) -> StorageResult<String> {
    let trimmed = event_key.trim();
    if trimmed.is_empty() {
        return Err(StorageError::InvalidReportSeasonValue {
            key: "eventKey",
            value: event_key.to_owned(),
        });
    }
    Ok(trimmed.to_owned())
}

fn ensure_company_exists(connection: &Connection, company_id: &str) -> StorageResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM companies WHERE id = ?1)",
        [company_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::MissingReportSeasonReference {
            table: "companies".to_owned(),
            id: company_id.to_owned(),
        })
    }
}

/// Exposed for the storage boundary's allowed-value validation; the status
/// itself is only ever set by the workflow commands, never by free user input.
#[allow(dead_code)]
pub(super) fn validate_preparation_status(status: &str) -> StorageResult<()> {
    if PREPARATION_STATUSES.contains(&status) {
        Ok(())
    } else {
        Err(StorageError::InvalidReportSeasonValue {
            key: "status",
            value: status.to_owned(),
        })
    }
}

use super::database::Database;
/// report_season domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::report_season()`.
#[derive(Clone)]
pub struct ReportSeasonStore {
    db: Database,
}

impl ReportSeasonStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_report_season(
        &self,
        input: ReportSeasonInput,
    ) -> StorageResult<ReportSeasonResult> {
        let connection = self.db.checkout()?;

        list_report_season(&connection, input)
    }

    pub fn get_pre_report_card(&self, input: PreReportCardInput) -> StorageResult<PreReportCard> {
        let connection = self.db.checkout()?;

        get_pre_report_card(&connection, input)
    }

    pub fn mark_report_prepared(
        &self,
        input: MarkReportPreparedInput,
    ) -> StorageResult<ReportPreparation> {
        let connection = self.db.checkout()?;

        mark_report_prepared(&connection, input)
    }

    pub fn mark_report_processed(
        &self,
        input: MarkReportProcessedInput,
    ) -> StorageResult<ReportPreparation> {
        let connection = self.db.checkout()?;

        mark_report_processed(&connection, input)
    }
}
