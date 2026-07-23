//! Storage for first-class management claims (v0.42.0, epic cbf6999, ADR 0040).
//!
//! A claim is a tracked management promise with a normalized due period and a
//! user-set verdict (`status`). Verdicts are never assigned automatically. Claims
//! replace the legacy `notebook_entries(kind='claim')` model; migrated claims keep
//! their originating notebook-entry id so existing evidence links keep resolving.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::{slug_part, StorageError, StorageResult};

pub(super) const CLAIM_STATUSES: &[&str] = &[
    "pending",
    "delivered",
    "partially_delivered",
    "missed",
    "revised",
];

const SOURCE_EVIDENCE_TYPES: &[&str] = &[
    "report_document",
    "transcript_segment",
    "transcript",
    "feed_item",
    "manual",
];

const COMPARATORS: &[&str] = &["gte", "lte", "gt", "lt", "approx", "eq"];

const PERIOD_TYPES: &[&str] = &[
    "FY", "H1", "H2", "Q1", "Q2", "Q3", "Q4", "9M", "M01", "M02", "M03", "M04", "M05", "M06",
    "M07", "M08", "M09", "M10", "M11", "M12",
];

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ManagementClaim {
    pub id: String,
    pub company_id: String,
    pub statement: String,
    pub body: String,
    pub body_format: String,
    pub made_at: Option<String>,
    pub source_period_id: Option<String>,
    pub due_fiscal_year: Option<i64>,
    pub due_period_type: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(as = "crate::api_ts_unions::ClaimStatus"))]
    pub status: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::ClaimSourceEvidenceType")
    )]
    pub source_evidence_type: String,
    pub source_evidence_id: Option<String>,
    pub target_metric_key: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ClaimTargetComparator>")
    )]
    pub target_comparator: Option<String>,
    pub target_value_numeric: Option<String>,
    pub target_unit: Option<String>,
    pub verifying_fact_id: Option<String>,
    pub revises_claim_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "NewManagementClaimInput",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewManagementClaim {
    pub company_id: String,
    pub statement: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub made_at: Option<String>,
    #[serde(default)]
    pub source_period_id: Option<String>,
    #[serde(default)]
    pub due_fiscal_year: Option<i64>,
    #[serde(default)]
    pub due_period_type: Option<String>,
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ClaimStatus>")
    )]
    pub status: Option<String>,
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ClaimSourceEvidenceType>")
    )]
    pub source_evidence_type: Option<String>,
    #[serde(default)]
    pub source_evidence_id: Option<String>,
    #[serde(default)]
    pub target_metric_key: Option<String>,
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ClaimTargetComparator>")
    )]
    pub target_comparator: Option<String>,
    #[serde(default)]
    pub target_value_numeric: Option<String>,
    #[serde(default)]
    pub target_unit: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        rename = "UpdateManagementClaimInput",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct ManagementClaimUpdate {
    pub id: String,
    #[serde(default)]
    pub statement: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub made_at: Option<String>,
    #[serde(default)]
    pub due_fiscal_year: Option<i64>,
    #[serde(default)]
    pub due_period_type: Option<String>,
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ClaimSourceEvidenceType>")
    )]
    pub source_evidence_type: Option<String>,
    #[serde(default)]
    pub source_evidence_id: Option<String>,
    #[serde(default)]
    pub target_metric_key: Option<String>,
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "Option<crate::api_ts_unions::ClaimTargetComparator>")
    )]
    pub target_comparator: Option<String>,
    #[serde(default)]
    pub target_value_numeric: Option<String>,
    #[serde(default)]
    pub target_unit: Option<String>,
}

/// A confirmed financial fact that may verify a quantitative claim for its due period.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct VerifyingFactCandidate {
    pub fact_id: String,
    pub value_numeric: String,
}

/// One claim surfaced in the review queue, with the arrived period and (for
/// quantitative claims) the matching confirmed fact.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ClaimToVerify {
    pub claim: ManagementClaim,
    pub arrived_period_id: Option<String>,
    pub verifying_fact_candidate: Option<VerifyingFactCandidate>,
}

/// The due-period resurfacing read model (ADR 0040): open claims bucketed by
/// whether their due-period report has arrived.
#[derive(Debug, Clone, Default, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ClaimsToVerify {
    pub due: Vec<ClaimToVerify>,
    pub overdue: Vec<ClaimToVerify>,
    pub upcoming: Vec<ClaimToVerify>,
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
pub struct SetClaimVerdictInput {
    pub claim_id: String,
    #[cfg_attr(feature = "ts-export", ts(as = "crate::api_ts_unions::ClaimStatus"))]
    pub status: String,
    #[serde(default)]
    pub verifying_fact_id: Option<String>,
    /// Relation recorded in the evidence graph when a verifying fact is linked
    /// (`supports` | `contradicts`); applied by the verification slice.
    #[serde(default)]
    #[cfg_attr(
        feature = "ts-export",
        ts(optional, type = "\"supports\" | \"contradicts\" | null")
    )]
    pub verifying_relation: Option<String>,
    #[serde(default)]
    pub revises_claim_id: Option<String>,
}

pub(super) fn list_management_claims(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<ManagementClaim>> {
    let mut statement = connection.prepare(CLAIM_SELECT_BY_COMPANY)?;
    let rows = statement.query_map([company_id], claim_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(super) fn get_management_claim(
    connection: &Connection,
    claim_id: &str,
) -> StorageResult<ManagementClaim> {
    connection
        .query_row(CLAIM_SELECT_BY_ID, [claim_id], claim_from_row)
        .optional()?
        .ok_or_else(|| missing("management_claims", claim_id))
}

pub(super) fn create_management_claim(
    connection: &Connection,
    input: NewManagementClaim,
) -> StorageResult<ManagementClaim> {
    require("company_id", &input.company_id)?;
    require("statement", &input.statement)?;
    ensure_company_exists(connection, &input.company_id)?;

    let statement = input.statement.trim().to_owned();
    let body = input.body.unwrap_or_default().trim().to_owned();
    let status = trimmed_option(input.status).unwrap_or_else(|| "pending".to_owned());
    validate_allowed("status", &status, CLAIM_STATUSES)?;
    let source_evidence_type =
        trimmed_option(input.source_evidence_type).unwrap_or_else(|| "manual".to_owned());
    validate_allowed(
        "source_evidence_type",
        &source_evidence_type,
        SOURCE_EVIDENCE_TYPES,
    )?;
    let due_period_type = validated_period_type(input.due_period_type)?;
    let target_comparator = validated_comparator(input.target_comparator)?;

    let id = claim_id(connection, &input.company_id, &statement)?;

    connection.execute(
        "
        INSERT INTO management_claims (
            id, company_id, statement, body, made_at, source_period_id,
            due_fiscal_year, due_period_type, status, source_evidence_type,
            source_evidence_id, target_metric_key,
            target_comparator, target_value_numeric, target_unit
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ",
        params![
            id,
            input.company_id,
            statement,
            body,
            trimmed_option(input.made_at),
            trimmed_option(input.source_period_id),
            input.due_fiscal_year,
            due_period_type,
            status,
            source_evidence_type,
            trimmed_option(input.source_evidence_id),
            trimmed_option(input.target_metric_key),
            target_comparator,
            trimmed_option(input.target_value_numeric),
            trimmed_option(input.target_unit),
        ],
    )?;

    get_management_claim(connection, &id)
}

pub(super) fn update_management_claim(
    connection: &Connection,
    input: ManagementClaimUpdate,
) -> StorageResult<ManagementClaim> {
    let existing = get_management_claim(connection, &input.id)?;

    let statement = match trimmed_option(input.statement) {
        Some(value) => value,
        None => existing.statement,
    };
    let body = input
        .body
        .map(|b| b.trim().to_owned())
        .unwrap_or(existing.body);
    let due_period_type = match input.due_period_type {
        Some(_) => validated_period_type(input.due_period_type)?,
        None => existing.due_period_type,
    };
    let target_comparator = match input.target_comparator {
        Some(_) => validated_comparator(input.target_comparator)?,
        None => existing.target_comparator,
    };
    let source_evidence_type = match trimmed_option(input.source_evidence_type) {
        Some(value) => {
            validate_allowed("source_evidence_type", &value, SOURCE_EVIDENCE_TYPES)?;
            value
        }
        None => existing.source_evidence_type,
    };

    connection.execute(
        "
        UPDATE management_claims SET
            statement = ?2,
            body = ?3,
            made_at = ?4,
            due_fiscal_year = ?5,
            due_period_type = ?6,
            source_evidence_type = ?7,
            source_evidence_id = ?8,
            target_metric_key = ?9,
            target_comparator = ?10,
            target_value_numeric = ?11,
            target_unit = ?12,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            input.id,
            statement,
            body,
            input.made_at.map(none_if_blank).unwrap_or(existing.made_at),
            input.due_fiscal_year.or(existing.due_fiscal_year),
            due_period_type,
            source_evidence_type,
            input
                .source_evidence_id
                .map(none_if_blank)
                .unwrap_or(existing.source_evidence_id),
            input
                .target_metric_key
                .map(none_if_blank)
                .unwrap_or(existing.target_metric_key),
            target_comparator,
            input
                .target_value_numeric
                .map(none_if_blank)
                .unwrap_or(existing.target_value_numeric),
            input
                .target_unit
                .map(none_if_blank)
                .unwrap_or(existing.target_unit),
        ],
    )?;

    get_management_claim(connection, &input.id)
}

pub(super) fn set_claim_verdict(
    connection: &Connection,
    input: SetClaimVerdictInput,
) -> StorageResult<ManagementClaim> {
    let _ = get_management_claim(connection, &input.claim_id)?;
    validate_allowed("status", &input.status, CLAIM_STATUSES)?;

    let verifying_fact_id = trimmed_option(input.verifying_fact_id);
    if let Some(fact_id) = verifying_fact_id.as_deref() {
        ensure_fact_exists(connection, fact_id)?;
    }
    let revises_claim_id = trimmed_option(input.revises_claim_id);

    connection.execute(
        "
        UPDATE management_claims SET
            status = ?2,
            verifying_fact_id = COALESCE(?3, verifying_fact_id),
            revises_claim_id = COALESCE(?4, revises_claim_id),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            input.claim_id,
            input.status,
            verifying_fact_id,
            revises_claim_id,
        ],
    )?;

    get_management_claim(connection, &input.claim_id)
}

/// The "claims to verify" read model. An open (`pending`) claim with a due period
/// resurfaces here once the due-period report arrives (its `financial_period` is
/// created). Buckets:
/// - `due`: the exact due-period report has arrived;
/// - `overdue`: a later fiscal year's report has arrived but the claim is still pending;
/// - `upcoming`: the due period has not yet arrived.
///
/// Within-year period ordering is deliberately not attempted (period types overlap);
/// `overdue` keys off a strictly later fiscal year, which is unambiguous.
pub(super) fn list_claims_to_verify(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<ClaimsToVerify> {
    let mut statement = connection.prepare(
        "
        SELECT id, company_id, statement, body, body_format, made_at, source_period_id,
               due_fiscal_year, due_period_type, status, source_evidence_type, source_evidence_id,
               target_metric_key, target_comparator, target_value_numeric,
               target_unit, verifying_fact_id, revises_claim_id, created_at, updated_at
        FROM management_claims
        WHERE company_id = ?1
          AND status = 'pending'
          AND due_fiscal_year IS NOT NULL
          AND due_period_type IS NOT NULL
        ORDER BY due_fiscal_year ASC, due_period_type ASC, id ASC
        ",
    )?;
    let claims = statement
        .query_map([company_id], claim_from_row)?
        .collect::<Result<Vec<_>, _>>()?;

    let mut result = ClaimsToVerify::default();
    for claim in claims {
        let due_year = claim.due_fiscal_year.unwrap_or_default();
        let due_type = claim.due_period_type.clone().unwrap_or_default();

        let arrived_period_id: Option<String> = connection
            .query_row(
                "SELECT id FROM financial_periods
                 WHERE company_id = ?1 AND fiscal_year = ?2 AND period_type = ?3
                 LIMIT 1",
                params![company_id, due_year, due_type],
                |row| row.get(0),
            )
            .optional()?;

        let later_year_arrived: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM financial_periods
             WHERE company_id = ?1 AND fiscal_year > ?2)",
            params![company_id, due_year],
            |row| row.get(0),
        )?;

        let verifying_fact_candidate = match (&claim.target_metric_key, &arrived_period_id) {
            (Some(metric_key), Some(period_id)) => connection
                .query_row(
                    "SELECT f.id, f.value_numeric
                     FROM financial_facts f
                     JOIN kpi_definitions d ON f.definition_id = d.id
                     WHERE f.period_id = ?1 AND d.metric_key = ?2
                       AND f.confirmation_state = 'confirmed'
                     LIMIT 1",
                    params![period_id, metric_key],
                    |row| {
                        Ok(VerifyingFactCandidate {
                            fact_id: row.get(0)?,
                            value_numeric: row.get(1)?,
                        })
                    },
                )
                .optional()?,
            _ => None,
        };

        let entry = ClaimToVerify {
            claim,
            arrived_period_id: arrived_period_id.clone(),
            verifying_fact_candidate,
        };

        if later_year_arrived {
            result.overdue.push(entry);
        } else if arrived_period_id.is_some() {
            result.due.push(entry);
        } else {
            result.upcoming.push(entry);
        }
    }

    Ok(result)
}

pub(super) fn delete_management_claim(
    connection: &Connection,
    claim_id: &str,
) -> StorageResult<()> {
    let affected = connection.execute("DELETE FROM management_claims WHERE id = ?1", [claim_id])?;
    if affected == 0 {
        return Err(missing("management_claims", claim_id));
    }
    Ok(())
}

const CLAIM_SELECT_BY_COMPANY: &str = "
    SELECT id, company_id, statement, body, body_format, made_at, source_period_id,
           due_fiscal_year, due_period_type, status, source_evidence_type, source_evidence_id,
           target_metric_key, target_comparator, target_value_numeric,
           target_unit, verifying_fact_id, revises_claim_id, created_at, updated_at
    FROM management_claims
    WHERE company_id = ?1
    ORDER BY updated_at DESC, id DESC
";

const CLAIM_SELECT_BY_ID: &str = "
    SELECT id, company_id, statement, body, body_format, made_at, source_period_id,
           due_fiscal_year, due_period_type, status, source_evidence_type, source_evidence_id,
           target_metric_key, target_comparator, target_value_numeric,
           target_unit, verifying_fact_id, revises_claim_id, created_at, updated_at
    FROM management_claims
    WHERE id = ?1
";

fn claim_from_row(row: &Row<'_>) -> rusqlite::Result<ManagementClaim> {
    Ok(ManagementClaim {
        id: row.get(0)?,
        company_id: row.get(1)?,
        statement: row.get(2)?,
        body: row.get(3)?,
        body_format: row.get(4)?,
        made_at: row.get(5)?,
        source_period_id: row.get(6)?,
        due_fiscal_year: row.get(7)?,
        due_period_type: row.get(8)?,
        status: row.get(9)?,
        source_evidence_type: row.get(10)?,
        source_evidence_id: row.get(11)?,
        target_metric_key: row.get(12)?,
        target_comparator: row.get(13)?,
        target_value_numeric: row.get(14)?,
        target_unit: row.get(15)?,
        verifying_fact_id: row.get(16)?,
        revises_claim_id: row.get(17)?,
        created_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
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
        Err(StorageError::MissingClaimReference {
            table: "companies".to_owned(),
            id: company_id.to_owned(),
        })
    }
}

fn ensure_fact_exists(connection: &Connection, fact_id: &str) -> StorageResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM financial_facts WHERE id = ?1)",
        [fact_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::MissingClaimReference {
            table: "financial_facts".to_owned(),
            id: fact_id.to_owned(),
        })
    }
}

fn claim_id(connection: &Connection, company_id: &str, statement: &str) -> StorageResult<String> {
    let base_id = format!("claim_{}_{}", slug_part(company_id), slug_part(statement));
    let existing_count: i64 = connection.query_row(
        "SELECT COUNT(*) FROM management_claims WHERE id = ?1 OR id LIKE ?2",
        params![&base_id, format!("{base_id}_%")],
        |row| row.get(0),
    )?;
    if existing_count == 0 {
        Ok(base_id)
    } else {
        Ok(format!("{base_id}_{}", existing_count + 1))
    }
}

fn validated_period_type(value: Option<String>) -> StorageResult<Option<String>> {
    match trimmed_option(value) {
        None => Ok(None),
        Some(value) => {
            let upper = value.to_uppercase();
            validate_allowed("due_period_type", &upper, PERIOD_TYPES)?;
            Ok(Some(upper))
        }
    }
}

fn validated_comparator(value: Option<String>) -> StorageResult<Option<String>> {
    match trimmed_option(value) {
        None => Ok(None),
        Some(value) => {
            let lower = value.to_lowercase();
            validate_allowed("target_comparator", &lower, COMPARATORS)?;
            Ok(Some(lower))
        }
    }
}

fn validate_allowed(key: &'static str, value: &str, allowed: &[&str]) -> StorageResult<()> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(invalid(key, value))
    }
}

fn require(key: &'static str, value: &str) -> StorageResult<()> {
    if value.trim().is_empty() {
        Err(invalid(key, value))
    } else {
        Ok(())
    }
}

fn invalid(key: &'static str, value: &str) -> StorageError {
    StorageError::InvalidClaimValue {
        key,
        value: value.to_owned(),
    }
}

fn missing(table: &str, id: &str) -> StorageError {
    StorageError::MissingClaimReference {
        table: table.to_owned(),
        id: id.to_owned(),
    }
}

fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn none_if_blank(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

use super::database::Database;
/// management_claims domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::management_claims()`.
#[derive(Clone)]
pub struct ManagementClaimStore {
    db: Database,
}

impl ManagementClaimStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_management_claims(&self, company_id: &str) -> StorageResult<Vec<ManagementClaim>> {
        let connection = self.db.checkout()?;

        list_management_claims(&connection, company_id)
    }

    pub fn create_management_claim(
        &self,
        input: NewManagementClaim,
    ) -> StorageResult<ManagementClaim> {
        let connection = self.db.checkout()?;

        create_management_claim(&connection, input)
    }

    pub fn update_management_claim(
        &self,
        input: ManagementClaimUpdate,
    ) -> StorageResult<ManagementClaim> {
        let connection = self.db.checkout()?;

        update_management_claim(&connection, input)
    }

    pub fn set_claim_verdict(&self, input: SetClaimVerdictInput) -> StorageResult<ManagementClaim> {
        let connection = self.db.checkout()?;

        set_claim_verdict(&connection, input)
    }

    pub fn delete_management_claim(&self, claim_id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        delete_management_claim(&connection, claim_id)
    }

    pub fn list_claims_to_verify(&self, company_id: &str) -> StorageResult<ClaimsToVerify> {
        let connection = self.db.checkout()?;

        list_claims_to_verify(&connection, company_id)
    }
}
