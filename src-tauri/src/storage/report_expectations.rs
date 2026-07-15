//! Pre-report expectations storage (ADR 0071, v0.52.0): the user's stance and
//! optional per-metric expectations written down BEFORE a report lands, keyed
//! by the same stable occurrence key as `report_preparations`
//! (`company_id` + the report occurrence's `company_events.source_event_key`),
//! resolved at creation to the `(fiscal_year, period_type)` the report covers.
//!
//! Freeze rule (hindsight-bias check): an expectation is editable until the
//! resolved period's facts are recorded. Every update runs in a transaction
//! that first checks the facts coverage (`facts_coverage_by_period`, the same
//! read model the coverage panel uses); once facts exist the update is refused
//! with [`StorageError::ReportExpectationFrozen`] and `frozen_at` is stamped on
//! first observation (reads also stamp it — freeze-on-read). The user's own
//! `resolution_note_md` verdict stays recordable after the freeze; the app
//! never scores judgment automatically (ADR 0042 posture).

use rusqlite::{params, Connection, OptionalExtension};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::financials::{facts_coverage_by_period, normalize_period_type_label};
use super::{slug_part, StorageError, StorageResult};

pub const EXPECTATION_COMPARATORS: &[&str] = &["lt", "lte", "eq", "gte", "gt"];

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReportExpectation {
    pub id: String,
    pub company_id: String,
    pub event_key: String,
    /// The fiscal period the report occurrence covers — what the freeze rule
    /// joins against the facts coverage.
    pub fiscal_year: i64,
    pub period_type: String,
    pub stance_md: String,
    pub frozen_at: Option<String>,
    pub resolution_note_md: Option<String>,
    pub resolved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub metrics: Vec<ExpectationMetric>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ExpectationMetric {
    pub id: String,
    pub expectation_id: String,
    pub metric_key: String,
    pub comparator: String,
    /// Decimal-exact text in base units, matching `financial_facts.value_numeric`.
    pub expected_value: String,
    pub unit: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewReportExpectation {
    pub company_id: String,
    pub event_key: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub stance_md: String,
    #[serde(default)]
    pub metrics: Vec<NewExpectationMetric>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewExpectationMetric {
    pub metric_key: String,
    pub comparator: String,
    pub expected_value: String,
    #[serde(default)]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateReportExpectation {
    pub company_id: String,
    pub event_key: String,
    /// `None` keeps the stored stance.
    #[serde(default)]
    pub stance_md: Option<String>,
    /// `Some(rows)` replaces the metric set wholesale; `None` keeps it.
    #[serde(default)]
    pub metrics: Option<Vec<NewExpectationMetric>>,
}

#[derive(Debug, Default, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ListReportExpectationsInput {
    /// Restrict to one company; `None` lists all.
    #[serde(default)]
    pub company_id: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct RecordExpectationResolutionInput {
    pub company_id: String,
    pub event_key: String,
    pub resolution_note_md: String,
}

/// Occurrence key for the expectation-vs-actual review command (ADR 0071).
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ExpectationReviewInput {
    pub company_id: String,
    pub event_key: String,
}

/// Outcome of one metric expectation against an actual value. `Unknown` covers
/// everything that does not parse as a decimal (NaN/inf/empty/garbage) and any
/// comparator outside the closed set — the evaluator mirrors, it never guesses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "snake_case")]
pub enum MetricExpectationOutcome {
    Met,
    Missed,
    Unknown,
}

/// Expectation-vs-actual read model (ADR 0071): the frozen expectation joined
/// with the occurrence's confirmed facts. Composed on read — NO stored
/// projection — so it mirrors the current facts without grading the user's
/// judgment (ADR 0042). The user's own verdict stays in `resolution_note_md`.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ExpectationReview {
    pub company_id: String,
    pub event_key: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub stance_md: String,
    pub frozen_at: Option<String>,
    /// True once ANY facts exist for the resolved period — the freeze condition
    /// (`facts_coverage_by_period`), distinct from a per-metric confirmed value.
    pub facts_available: bool,
    pub resolution_note_md: Option<String>,
    pub resolved_at: Option<String>,
    pub metrics: Vec<MetricExpectationReview>,
}

/// One metric expectation composed against its confirmed actual. `actual_value`
/// is `None` when no confirmed fact for the metric+period is recorded, in which
/// case `outcome` is [`MetricExpectationOutcome::Unknown`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct MetricExpectationReview {
    pub metric_key: String,
    pub comparator: String,
    pub expected_value: String,
    pub unit: Option<String>,
    pub actual_value: Option<String>,
    pub outcome: MetricExpectationOutcome,
}

/// Pure, total comparator evaluation: was the expectation met by the actual
/// value? Values are decimal-exact text (`financial_facts.value_numeric`
/// convention); anything unparseable yields `Unknown` instead of panicking.
pub fn evaluate_metric_expectation(
    comparator: &str,
    expected_value: &str,
    actual_value: &str,
) -> MetricExpectationOutcome {
    let (Ok(expected), Ok(actual)) = (
        Decimal::from_str(expected_value.trim()),
        Decimal::from_str(actual_value.trim()),
    ) else {
        return MetricExpectationOutcome::Unknown;
    };
    let met = match comparator {
        "lt" => actual < expected,
        "lte" => actual <= expected,
        "eq" => actual == expected,
        "gte" => actual >= expected,
        "gt" => actual > expected,
        _ => return MetricExpectationOutcome::Unknown,
    };
    if met {
        MetricExpectationOutcome::Met
    } else {
        MetricExpectationOutcome::Missed
    }
}

pub(super) fn create_report_expectation(
    connection: &mut Connection,
    input: NewReportExpectation,
) -> StorageResult<ReportExpectation> {
    ensure_company_exists(connection, &input.company_id)?;
    let event_key = require("eventKey", &input.event_key)?;
    let stance_md = require("stanceMd", &input.stance_md)?;
    let period_type =
        normalize_period_type_label(require("periodType", &input.period_type)?.as_str());
    if input.fiscal_year <= 0 {
        return Err(invalid("fiscalYear", &input.fiscal_year.to_string()));
    }
    for metric in &input.metrics {
        validate_metric(metric)?;
    }

    let id = report_expectation_id(&input.company_id, &event_key);
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    transaction.execute(
        "
        INSERT INTO report_expectations
            (id, company_id, event_key, fiscal_year, period_type, stance_md)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            id,
            input.company_id,
            event_key,
            input.fiscal_year,
            period_type,
            stance_md,
        ],
    )?;
    insert_metrics(&transaction, &id, &input.metrics)?;
    transaction.commit()?;

    get_report_expectation(connection, &input.company_id, &event_key)
}

pub(super) fn update_report_expectation(
    connection: &mut Connection,
    input: UpdateReportExpectation,
) -> StorageResult<ReportExpectation> {
    let stance_md = match input.stance_md.as_deref() {
        Some(value) => Some(require("stanceMd", value)?),
        None => None,
    };
    if let Some(metrics) = input.metrics.as_deref() {
        for metric in metrics {
            validate_metric(metric)?;
        }
    }

    // The freeze check and the write share ONE transaction: facts confirmed
    // between the check and the write cannot slip an edit through.
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let header = load_header(&transaction, &input.company_id, &input.event_key)?;
    if facts_recorded_for_period(
        &transaction,
        &header.company_id,
        header.fiscal_year,
        &header.period_type,
    )? {
        stamp_frozen_at(&transaction, &header.id)?;
        transaction.commit()?;
        return Err(StorageError::ReportExpectationFrozen {
            event_key: input.event_key,
        });
    }

    if let Some(stance_md) = stance_md {
        transaction.execute(
            "UPDATE report_expectations SET stance_md = ?2 WHERE id = ?1",
            params![header.id, stance_md],
        )?;
    }
    if let Some(metrics) = input.metrics.as_deref() {
        transaction.execute(
            "DELETE FROM report_expectation_metrics WHERE expectation_id = ?1",
            [&header.id],
        )?;
        insert_metrics(&transaction, &header.id, metrics)?;
    }
    transaction.execute(
        "UPDATE report_expectations
         SET updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        [&header.id],
    )?;
    transaction.commit()?;

    get_report_expectation(connection, &input.company_id, &input.event_key)
}

pub(super) fn list_report_expectations(
    connection: &Connection,
    input: ListReportExpectationsInput,
) -> StorageResult<Vec<ReportExpectation>> {
    // Freeze-on-read: stamp frozen_at for any not-yet-frozen expectation whose
    // period facts have arrived, so the freeze moment is durably observed.
    let unfrozen: Vec<(String, String, i64, String)> = {
        let mut statement = connection.prepare(
            "SELECT id, company_id, fiscal_year, period_type
             FROM report_expectations
             WHERE frozen_at IS NULL AND (?1 IS NULL OR company_id = ?1)",
        )?;
        let rows = statement.query_map([&input.company_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, company_id, fiscal_year, period_type) in unfrozen {
        if facts_recorded_for_period(connection, &company_id, fiscal_year, &period_type)? {
            stamp_frozen_at(connection, &id)?;
        }
    }

    let mut statement = connection.prepare(
        "
        SELECT id, company_id, event_key, fiscal_year, period_type, stance_md,
               frozen_at, resolution_note_md, resolved_at, created_at, updated_at
        FROM report_expectations
        WHERE (?1 IS NULL OR company_id = ?1)
        ORDER BY created_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map([&input.company_id], expectation_from_row)?;
    let mut expectations = rows.collect::<Result<Vec<_>, _>>()?;
    for expectation in &mut expectations {
        expectation.metrics = list_metrics(connection, &expectation.id)?;
    }
    Ok(expectations)
}

pub(super) fn record_expectation_resolution(
    connection: &Connection,
    input: RecordExpectationResolutionInput,
) -> StorageResult<ReportExpectation> {
    let note = require("resolutionNoteMd", &input.resolution_note_md)?;
    let header = load_header(connection, &input.company_id, &input.event_key)?;
    connection.execute(
        "
        UPDATE report_expectations
        SET resolution_note_md = ?2,
            resolved_at = COALESCE(resolved_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![header.id, note],
    )?;
    get_report_expectation(connection, &input.company_id, &input.event_key)
}

pub(super) fn expectation_review(
    connection: &Connection,
    company_id: &str,
    event_key: &str,
) -> StorageResult<ExpectationReview> {
    let header = load_header(connection, company_id, event_key)?;
    // Freeze-on-read: the review is a durable observation of the occurrence, so
    // stamp the freeze if the period's facts have landed (idempotent), exactly
    // like the list path.
    let facts_available = facts_recorded_for_period(
        connection,
        &header.company_id,
        header.fiscal_year,
        &header.period_type,
    )?;
    if facts_available {
        stamp_frozen_at(connection, &header.id)?;
    }
    // Re-read the full row so `frozen_at` reflects any stamp just written.
    let expectation = get_report_expectation(connection, company_id, event_key)?;

    let mut metrics = Vec::with_capacity(expectation.metrics.len());
    for metric in &expectation.metrics {
        let actual_value = actual_confirmed_value(
            connection,
            &expectation.company_id,
            expectation.fiscal_year,
            &expectation.period_type,
            &metric.metric_key,
        )?;
        let outcome = match actual_value.as_deref() {
            Some(actual) => {
                evaluate_metric_expectation(&metric.comparator, &metric.expected_value, actual)
            }
            None => MetricExpectationOutcome::Unknown,
        };
        metrics.push(MetricExpectationReview {
            metric_key: metric.metric_key.clone(),
            comparator: metric.comparator.clone(),
            expected_value: metric.expected_value.clone(),
            unit: metric.unit.clone(),
            actual_value,
            outcome,
        });
    }

    Ok(ExpectationReview {
        company_id: expectation.company_id,
        event_key: expectation.event_key,
        fiscal_year: expectation.fiscal_year,
        period_type: expectation.period_type,
        stance_md: expectation.stance_md,
        frozen_at: expectation.frozen_at,
        facts_available,
        resolution_note_md: expectation.resolution_note_md,
        resolved_at: expectation.resolved_at,
        metrics,
    })
}

/// The latest CONFIRMED actual for a metric in the occurrence's period, joined
/// through `kpi_definitions.metric_key` (the same convention expectations key
/// on) and `financial_periods`. `None` when no confirmed fact is recorded. The
/// `period_type` match is exact, mirroring the freeze check's coverage join.
fn actual_confirmed_value(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    metric_key: &str,
) -> StorageResult<Option<String>> {
    let value: Option<String> = connection
        .query_row(
            "SELECT f.value_numeric
             FROM financial_facts f
             JOIN financial_periods p ON p.id = f.period_id
             JOIN kpi_definitions k ON k.id = f.definition_id
             WHERE p.company_id = ?1
               AND p.fiscal_year = ?2
               AND p.period_type = ?3
               AND k.metric_key = ?4
               AND f.confirmation_state = 'confirmed'
             ORDER BY f.created_at DESC, f.id DESC
             LIMIT 1",
            params![company_id, fiscal_year, period_type, metric_key],
            |row| row.get(0),
        )
        .optional()?;
    Ok(value)
}

/// Minimal stored identity of one expectation for the freeze/update paths.
struct ExpectationHeader {
    id: String,
    company_id: String,
    fiscal_year: i64,
    period_type: String,
}

fn load_header(
    connection: &Connection,
    company_id: &str,
    event_key: &str,
) -> StorageResult<ExpectationHeader> {
    connection
        .query_row(
            "SELECT id, company_id, fiscal_year, period_type
             FROM report_expectations
             WHERE company_id = ?1 AND event_key = ?2",
            params![company_id, event_key],
            |row| {
                Ok(ExpectationHeader {
                    id: row.get(0)?,
                    company_id: row.get(1)?,
                    fiscal_year: row.get(2)?,
                    period_type: row.get(3)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingReportSeasonReference {
            table: "report_expectations".to_owned(),
            id: format!("{company_id}/{event_key}"),
        })
}

/// Have any facts been recorded for the occurrence's resolved period? Joined
/// through the same coverage read model the fundamentals panel uses
/// (`facts_coverage_by_period`, ADR 0077 §2).
fn facts_recorded_for_period(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
) -> StorageResult<bool> {
    let coverage = facts_coverage_by_period(connection, company_id)?;
    Ok(coverage.iter().any(|bucket| {
        bucket.fiscal_year == fiscal_year && bucket.period_type == period_type && bucket.total > 0
    }))
}

/// Stamp `frozen_at` exactly once (idempotent: only when still NULL).
fn stamp_frozen_at(connection: &Connection, expectation_id: &str) -> StorageResult<()> {
    connection.execute(
        "UPDATE report_expectations
         SET frozen_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1 AND frozen_at IS NULL",
        [expectation_id],
    )?;
    Ok(())
}

fn get_report_expectation(
    connection: &Connection,
    company_id: &str,
    event_key: &str,
) -> StorageResult<ReportExpectation> {
    let mut expectation = connection
        .query_row(
            "
            SELECT id, company_id, event_key, fiscal_year, period_type, stance_md,
                   frozen_at, resolution_note_md, resolved_at, created_at, updated_at
            FROM report_expectations
            WHERE company_id = ?1 AND event_key = ?2
            ",
            params![company_id, event_key],
            expectation_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingReportSeasonReference {
            table: "report_expectations".to_owned(),
            id: format!("{company_id}/{event_key}"),
        })?;
    expectation.metrics = list_metrics(connection, &expectation.id)?;
    Ok(expectation)
}

fn list_metrics(
    connection: &Connection,
    expectation_id: &str,
) -> StorageResult<Vec<ExpectationMetric>> {
    let mut statement = connection.prepare(
        "
        SELECT id, expectation_id, metric_key, comparator, expected_value, unit, created_at
        FROM report_expectation_metrics
        WHERE expectation_id = ?1
        ORDER BY id
        ",
    )?;
    let rows = statement.query_map([expectation_id], |row| {
        Ok(ExpectationMetric {
            id: row.get(0)?,
            expectation_id: row.get(1)?,
            metric_key: row.get(2)?,
            comparator: row.get(3)?,
            expected_value: row.get(4)?,
            unit: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn insert_metrics(
    connection: &Connection,
    expectation_id: &str,
    metrics: &[NewExpectationMetric],
) -> StorageResult<()> {
    for (index, metric) in metrics.iter().enumerate() {
        connection.execute(
            "
            INSERT INTO report_expectation_metrics
                (id, expectation_id, metric_key, comparator, expected_value, unit)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                format!("{expectation_id}_metric_{}", index + 1),
                expectation_id,
                metric.metric_key.trim(),
                metric.comparator,
                metric.expected_value.trim(),
                metric
                    .unit
                    .as_deref()
                    .map(str::trim)
                    .filter(|unit| !unit.is_empty()),
            ],
        )?;
    }
    Ok(())
}

fn validate_metric(metric: &NewExpectationMetric) -> StorageResult<()> {
    require("metricKey", &metric.metric_key)?;
    if !EXPECTATION_COMPARATORS.contains(&metric.comparator.as_str()) {
        return Err(invalid("comparator", &metric.comparator));
    }
    let expected = require("expectedValue", &metric.expected_value)?;
    if Decimal::from_str(&expected).is_err() {
        return Err(invalid("expectedValue", &metric.expected_value));
    }
    Ok(())
}

fn expectation_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ReportExpectation> {
    Ok(ReportExpectation {
        id: row.get(0)?,
        company_id: row.get(1)?,
        event_key: row.get(2)?,
        fiscal_year: row.get(3)?,
        period_type: row.get(4)?,
        stance_md: row.get(5)?,
        frozen_at: row.get(6)?,
        resolution_note_md: row.get(7)?,
        resolved_at: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
        metrics: Vec::new(),
    })
}

fn report_expectation_id(company_id: &str, event_key: &str) -> String {
    format!(
        "report_expectation_{}_{}",
        slug_part(company_id),
        slug_part(event_key)
    )
}

fn require(key: &'static str, value: &str) -> StorageResult<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(invalid(key, value))
    } else {
        Ok(trimmed.to_owned())
    }
}

fn invalid(key: &'static str, value: &str) -> StorageError {
    StorageError::InvalidReportSeasonValue {
        key,
        value: value.to_owned(),
    }
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

use super::database::Database;
/// report_expectations domain store (Architecture v2 / ADR 0050). Owns a
/// [`Database`] and exposes only this domain's operations. Reach it via
/// `AppState::report_expectations()`.
#[derive(Clone)]
pub struct ReportExpectationStore {
    db: Database,
}

impl ReportExpectationStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_report_expectation(
        &self,
        input: NewReportExpectation,
    ) -> StorageResult<ReportExpectation> {
        let mut connection = self.db.checkout()?;

        create_report_expectation(&mut connection, input)
    }

    pub fn update_report_expectation(
        &self,
        input: UpdateReportExpectation,
    ) -> StorageResult<ReportExpectation> {
        let mut connection = self.db.checkout()?;

        update_report_expectation(&mut connection, input)
    }

    pub fn list_report_expectations(
        &self,
        input: ListReportExpectationsInput,
    ) -> StorageResult<Vec<ReportExpectation>> {
        let connection = self.db.checkout()?;

        list_report_expectations(&connection, input)
    }

    pub fn record_expectation_resolution(
        &self,
        input: RecordExpectationResolutionInput,
    ) -> StorageResult<ReportExpectation> {
        let connection = self.db.checkout()?;

        record_expectation_resolution(&connection, input)
    }

    /// Expectation-vs-actual review read model (ADR 0071): composes the frozen
    /// expectation with the occurrence's confirmed facts. No stored projection.
    pub fn expectation_review(
        &self,
        company_id: &str,
        event_key: &str,
    ) -> StorageResult<ExpectationReview> {
        let connection = self.db.checkout()?;

        expectation_review(&connection, company_id, event_key)
    }
}
