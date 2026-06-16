//! Storage for AI claim extraction (v0.42.0, epic cbf6999, ADR 0040).
//!
//! Extraction never writes a claim directly. The async job persists PROPOSALS;
//! only an explicit user confirmation materialises a `management_claims` row.
//! Confirmed proposals are retained as the provenance trail (which job/provider/
//! model/prompt produced the candidate, and the verbatim source snippet); rejected
//! proposals are kept so the same statement is not re-proposed without intent.
//! Mirrors the KPI extraction pattern (`kpi_extraction`).

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use super::management_claims::{create_management_claim, NewManagementClaim};
use super::{slug_part, ManagementClaim, StorageError, StorageResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimExtractionJob {
    pub id: String,
    pub company_id: String,
    pub source_type: String,
    pub source_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    pub status: String,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub proposals: Vec<ClaimExtractionProposal>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaimExtractionProposal {
    pub id: String,
    pub job_id: String,
    pub statement: String,
    pub due_fiscal_year: Option<i64>,
    pub due_period_type: Option<String>,
    pub target_metric_key: Option<String>,
    pub target_comparator: Option<String>,
    pub target_value_numeric: Option<String>,
    pub target_unit: Option<String>,
    pub confidence: Option<String>,
    pub source_snippet: Option<String>,
    pub source_evidence_type: Option<String>,
    pub source_evidence_id: Option<String>,
    pub status: String,
    pub claim_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewClaimExtractionJob {
    pub company_id: String,
    pub source_type: String,
    pub source_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
}

/// A single proposed claim the runner produces from the model output.
#[derive(Debug, Clone)]
pub struct NewClaimProposal {
    pub statement: String,
    pub due_fiscal_year: Option<i64>,
    pub due_period_type: Option<String>,
    pub target_metric_key: Option<String>,
    pub target_comparator: Option<String>,
    pub target_value_numeric: Option<String>,
    pub target_unit: Option<String>,
    pub confidence: Option<String>,
    pub source_snippet: Option<String>,
    pub source_evidence_type: Option<String>,
    pub source_evidence_id: Option<String>,
}

/// The runner's parsed extraction result, persisted in one transaction.
#[derive(Debug, Clone)]
pub struct CompletedClaimExtraction {
    pub job_id: String,
    pub proposals: Vec<NewClaimProposal>,
}

/// User overrides applied when confirming a proposal into a claim. Absent fields
/// keep the proposed value.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmClaimProposalInput {
    pub proposal_id: String,
    #[serde(default)]
    pub statement: Option<String>,
    #[serde(default)]
    pub due_fiscal_year: Option<i64>,
    #[serde(default)]
    pub due_period_type: Option<String>,
    #[serde(default)]
    pub target_metric_key: Option<String>,
    #[serde(default)]
    pub target_comparator: Option<String>,
    #[serde(default)]
    pub target_value_numeric: Option<String>,
    #[serde(default)]
    pub target_unit: Option<String>,
}

pub(crate) fn create_claim_extraction_job(
    connection: &Connection,
    input: NewClaimExtractionJob,
) -> StorageResult<ClaimExtractionJob> {
    require("company_id", &input.company_id)?;
    require("source_type", &input.source_type)?;
    require("source_id", &input.source_id)?;
    require("provider_id", &input.provider_id)?;
    require("model", &input.model)?;
    require("prompt_version", &input.prompt_version)?;
    if input.source_type != "report_document" && input.source_type != "transcript" {
        return Err(invalid("source_type", &input.source_type));
    }

    let id = job_id(&input.source_type, &input.source_id);
    connection.execute(
        "
        INSERT INTO claim_extraction_jobs (
            id, company_id, source_type, source_id, provider_id, model, prompt_version, status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued')
        ON CONFLICT(id) DO UPDATE SET
            provider_id = excluded.provider_id,
            model = excluded.model,
            prompt_version = excluded.prompt_version,
            status = 'queued',
            error_code = NULL,
            error = NULL,
            started_at = NULL,
            finished_at = NULL
        ",
        params![
            id,
            input.company_id,
            input.source_type,
            input.source_id,
            input.provider_id,
            input.model,
            input.prompt_version,
        ],
    )?;

    get_claim_extraction_job(connection, &id)
}

pub(crate) fn mark_claim_extraction_job_running(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ClaimExtractionJob> {
    connection.execute(
        "
        UPDATE claim_extraction_jobs
        SET status = 'running',
            started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;
    get_claim_extraction_job(connection, job_id)
}

pub(crate) fn mark_claim_extraction_job_failed(
    connection: &Connection,
    job_id: &str,
    error_code: &str,
    error: &str,
) -> StorageResult<ClaimExtractionJob> {
    require("error_code", error_code)?;
    require("error", error)?;
    connection.execute(
        "
        UPDATE claim_extraction_jobs
        SET status = 'failed',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = ?2,
            error = ?3
        WHERE id = ?1
        ",
        params![job_id, error_code, error],
    )?;
    get_claim_extraction_job(connection, job_id)
}

pub(crate) fn complete_claim_extraction_job(
    connection: &mut Connection,
    input: CompletedClaimExtraction,
) -> StorageResult<ClaimExtractionJob> {
    let job_id = input.job_id.clone();
    let transaction = connection.transaction()?;

    // Refresh pending/rejected proposals; never disturb confirmed ones (they own claims).
    transaction.execute(
        "DELETE FROM claim_extraction_proposals WHERE job_id = ?1 AND status != 'confirmed'",
        [&job_id],
    )?;

    for proposal in &input.proposals {
        let statement = proposal.statement.trim();
        if statement.is_empty() {
            continue;
        }
        let proposal_id = proposal_id(&job_id, statement);
        let confirmed: bool = transaction
            .query_row(
                "SELECT status = 'confirmed' FROM claim_extraction_proposals WHERE id = ?1",
                [&proposal_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);
        if confirmed {
            continue;
        }
        transaction.execute(
            "
            INSERT INTO claim_extraction_proposals (
                id, job_id, statement, due_fiscal_year, due_period_type, target_metric_key,
                target_comparator, target_value_numeric, target_unit, confidence, source_snippet,
                source_evidence_type, source_evidence_id, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending')
            ON CONFLICT(id) DO UPDATE SET
                due_fiscal_year = excluded.due_fiscal_year,
                due_period_type = excluded.due_period_type,
                target_metric_key = excluded.target_metric_key,
                target_comparator = excluded.target_comparator,
                target_value_numeric = excluded.target_value_numeric,
                target_unit = excluded.target_unit,
                confidence = excluded.confidence,
                source_snippet = excluded.source_snippet,
                source_evidence_type = excluded.source_evidence_type,
                source_evidence_id = excluded.source_evidence_id,
                status = 'pending',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                proposal_id,
                job_id,
                statement,
                proposal.due_fiscal_year,
                trimmed_ref(&proposal.due_period_type),
                trimmed_ref(&proposal.target_metric_key),
                trimmed_ref(&proposal.target_comparator),
                trimmed_ref(&proposal.target_value_numeric),
                trimmed_ref(&proposal.target_unit),
                trimmed_ref(&proposal.confidence),
                trimmed_ref(&proposal.source_snippet),
                trimmed_ref(&proposal.source_evidence_type),
                trimmed_ref(&proposal.source_evidence_id),
            ],
        )?;
    }

    transaction.execute(
        "
        UPDATE claim_extraction_jobs
        SET status = 'succeeded',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [&job_id],
    )?;

    transaction.commit()?;
    get_claim_extraction_job(connection, &job_id)
}

pub(crate) fn confirm_claim_proposal(
    connection: &Connection,
    input: ConfirmClaimProposalInput,
) -> StorageResult<ManagementClaim> {
    let proposal = get_proposal(connection, &input.proposal_id)?;
    if proposal.status == "confirmed" {
        return Err(invalid("proposal.status", "already confirmed"));
    }
    let company_id: String = connection.query_row(
        "SELECT company_id FROM claim_extraction_jobs WHERE id = ?1",
        [&proposal.job_id],
        |row| row.get(0),
    )?;

    let claim = create_management_claim(
        connection,
        NewManagementClaim {
            company_id,
            statement: input.statement.unwrap_or(proposal.statement),
            body: None,
            made_at: None,
            source_period_id: None,
            due_fiscal_year: input.due_fiscal_year.or(proposal.due_fiscal_year),
            due_period_type: input.due_period_type.or(proposal.due_period_type),
            status: None,
            source_evidence_type: proposal.source_evidence_type,
            source_evidence_id: proposal.source_evidence_id,
            extraction_proposal_id: Some(proposal.id.clone()),
            target_metric_key: input.target_metric_key.or(proposal.target_metric_key),
            target_comparator: input.target_comparator.or(proposal.target_comparator),
            target_value_numeric: input.target_value_numeric.or(proposal.target_value_numeric),
            target_unit: input.target_unit.or(proposal.target_unit),
        },
    )?;

    connection.execute(
        "
        UPDATE claim_extraction_proposals
        SET status = 'confirmed',
            claim_id = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![input.proposal_id, claim.id],
    )?;

    Ok(claim)
}

pub(crate) fn reject_claim_proposal(
    connection: &Connection,
    proposal_id: &str,
) -> StorageResult<ClaimExtractionJob> {
    let proposal = get_proposal(connection, proposal_id)?;
    connection.execute(
        "
        UPDATE claim_extraction_proposals
        SET status = 'rejected',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        [proposal_id],
    )?;
    get_claim_extraction_job(connection, &proposal.job_id)
}

pub(crate) fn list_claim_extraction_jobs_by_source(
    connection: &Connection,
    source_type: &str,
    source_id: &str,
) -> StorageResult<Vec<ClaimExtractionJob>> {
    let mut statement = connection.prepare(
        "
        SELECT id, company_id, source_type, source_id, provider_id, model, prompt_version,
               status, error_code, error, created_at, started_at, finished_at
        FROM claim_extraction_jobs
        WHERE source_type = ?1 AND source_id = ?2
        ORDER BY created_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map(params![source_type, source_id], job_from_row)?;
    let mut jobs = rows.collect::<Result<Vec<_>, _>>()?;
    for job in &mut jobs {
        job.proposals = list_proposals(connection, &job.id)?;
    }
    Ok(jobs)
}

pub(crate) fn get_claim_extraction_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<ClaimExtractionJob> {
    let mut job = connection.query_row(
        "
        SELECT id, company_id, source_type, source_id, provider_id, model, prompt_version,
               status, error_code, error, created_at, started_at, finished_at
        FROM claim_extraction_jobs
        WHERE id = ?1
        ",
        [job_id],
        job_from_row,
    )?;
    job.proposals = list_proposals(connection, &job.id)?;
    Ok(job)
}

fn list_proposals(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<Vec<ClaimExtractionProposal>> {
    let mut statement = connection.prepare(
        "
        SELECT id, job_id, statement, due_fiscal_year, due_period_type, target_metric_key,
               target_comparator, target_value_numeric, target_unit, confidence, source_snippet,
               source_evidence_type, source_evidence_id, status, claim_id, created_at, updated_at
        FROM claim_extraction_proposals
        WHERE job_id = ?1
        ORDER BY created_at ASC, id ASC
        ",
    )?;
    let rows = statement.query_map([job_id], proposal_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

fn get_proposal(
    connection: &Connection,
    proposal_id: &str,
) -> StorageResult<ClaimExtractionProposal> {
    connection
        .query_row(
            "
            SELECT id, job_id, statement, due_fiscal_year, due_period_type, target_metric_key,
                   target_comparator, target_value_numeric, target_unit, confidence, source_snippet,
                   source_evidence_type, source_evidence_id, status, claim_id, created_at, updated_at
            FROM claim_extraction_proposals
            WHERE id = ?1
            ",
            [proposal_id],
            proposal_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingClaimReference {
            table: "claim_extraction_proposals".to_owned(),
            id: proposal_id.to_owned(),
        })
}

fn job_from_row(row: &Row<'_>) -> rusqlite::Result<ClaimExtractionJob> {
    Ok(ClaimExtractionJob {
        id: row.get(0)?,
        company_id: row.get(1)?,
        source_type: row.get(2)?,
        source_id: row.get(3)?,
        provider_id: row.get(4)?,
        model: row.get(5)?,
        prompt_version: row.get(6)?,
        status: row.get(7)?,
        error_code: row.get(8)?,
        error: row.get(9)?,
        created_at: row.get(10)?,
        started_at: row.get(11)?,
        finished_at: row.get(12)?,
        proposals: Vec::new(),
    })
}

fn proposal_from_row(row: &Row<'_>) -> rusqlite::Result<ClaimExtractionProposal> {
    Ok(ClaimExtractionProposal {
        id: row.get(0)?,
        job_id: row.get(1)?,
        statement: row.get(2)?,
        due_fiscal_year: row.get(3)?,
        due_period_type: row.get(4)?,
        target_metric_key: row.get(5)?,
        target_comparator: row.get(6)?,
        target_value_numeric: row.get(7)?,
        target_unit: row.get(8)?,
        confidence: row.get(9)?,
        source_snippet: row.get(10)?,
        source_evidence_type: row.get(11)?,
        source_evidence_id: row.get(12)?,
        status: row.get(13)?,
        claim_id: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
    })
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

fn trimmed_ref(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn job_id(source_type: &str, source_id: &str) -> String {
    format!(
        "claim_ext_{}_{}",
        slug_part(source_type),
        slug_part(source_id)
    )
}

fn proposal_id(job_id: &str, statement: &str) -> String {
    format!("claim_prop_{}_{}", slug_part(job_id), slug_part(statement))
}
