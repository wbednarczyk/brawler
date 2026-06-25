//! Storage for AI KPI extraction (v0.36.0, epic 9879941).
//!
//! Extraction never writes facts directly. The async job persists PROPOSALS; only
//! an explicit user confirmation materialises a `financial_fact`. Confirmed
//! proposals are retained as the provenance trail (which job/provider/model/prompt
//! produced the value, and the verbatim source snippet).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::financials::create_financial_fact;
use super::{slug_part, FinancialFact, NewFinancialFact, StorageError, StorageResult};

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiExtractionJob {
    pub id: String,
    pub company_id: String,
    pub report_document_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    pub period_hint: Option<String>,
    pub status: String,
    pub error_code: Option<String>,
    pub error: Option<String>,
    pub detected_fiscal_year: Option<i64>,
    pub detected_period_type: Option<String>,
    pub detected_period_end_date: Option<String>,
    pub detected_currency: Option<String>,
    pub detected_language: Option<String>,
    pub created_at: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub proposals: Vec<KpiExtractionProposal>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiExtractionProposal {
    pub id: String,
    pub job_id: String,
    pub metric_key: String,
    pub label: String,
    pub value_numeric: String,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub as_reported_value: Option<String>,
    pub as_reported_scale: Option<String>,
    pub measure_window: Option<String>,
    pub confidence: Option<String>,
    pub source_snippet: Option<String>,
    pub is_proposed_kpi: bool,
    pub status: String,
    pub fact_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewKpiExtractionJob {
    pub company_id: String,
    pub report_document_id: String,
    pub provider_id: String,
    pub model: String,
    pub prompt_version: String,
    pub period_hint: Option<String>,
}

/// A single proposed value the runner produces from the model output.
#[derive(Debug, Clone)]
pub struct NewKpiProposal {
    pub metric_key: String,
    pub label: String,
    pub value_numeric: String,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub as_reported_value: Option<String>,
    pub as_reported_scale: Option<String>,
    pub measure_window: Option<String>,
    pub confidence: Option<String>,
    pub source_snippet: Option<String>,
    pub is_proposed_kpi: bool,
}

/// The runner's parsed extraction result, persisted in one transaction.
#[derive(Debug, Clone)]
pub struct CompletedKpiExtraction {
    pub job_id: String,
    pub detected_fiscal_year: Option<i64>,
    pub detected_period_type: Option<String>,
    pub detected_period_end_date: Option<String>,
    pub detected_currency: Option<String>,
    pub detected_language: Option<String>,
    pub proposals: Vec<NewKpiProposal>,
}

/// User overrides applied when confirming a proposal into a fact. Period fields
/// default to the job's detected period; the model-detected period is confirmed,
/// not trusted blindly.
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmKpiProposalInput {
    pub proposal_id: String,
    pub value_numeric: Option<String>,
    pub currency: Option<String>,
    pub fiscal_year: Option<i64>,
    pub period_type: Option<String>,
    pub period_end_date: Option<String>,
    /// When the proposal is a model-suggested KPI beyond the taxonomy, create a
    /// company-scoped definition for it before committing the fact.
    #[serde(default)]
    #[cfg_attr(feature = "ts-export", ts(optional, as = "Option<bool>"))]
    pub accept_as_new_kpi: bool,
}

pub(crate) fn create_kpi_extraction_job(
    connection: &Connection,
    input: NewKpiExtractionJob,
) -> StorageResult<KpiExtractionJob> {
    require("company_id", &input.company_id)?;
    require("report_document_id", &input.report_document_id)?;
    require("provider_id", &input.provider_id)?;
    require("model", &input.model)?;
    require("prompt_version", &input.prompt_version)?;

    let period_hint = trimmed_option(input.period_hint);
    let id = job_id(&input.report_document_id);

    // Re-running on the same document re-queues the job; confirmed proposals are
    // preserved (a re-run refreshes only pending/rejected ones).
    connection.execute(
        "
        INSERT INTO kpi_extraction_jobs (
            id, company_id, report_document_id, provider_id, model, prompt_version, period_hint, status
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'queued')
        ON CONFLICT(id) DO UPDATE SET
            provider_id = excluded.provider_id,
            model = excluded.model,
            prompt_version = excluded.prompt_version,
            period_hint = excluded.period_hint,
            status = 'queued',
            error_code = NULL,
            error = NULL,
            started_at = NULL,
            finished_at = NULL
        ",
        params![
            id,
            input.company_id,
            input.report_document_id,
            input.provider_id,
            input.model,
            input.prompt_version,
            period_hint
        ],
    )?;

    get_kpi_extraction_job(connection, &id)
}

pub(crate) fn mark_kpi_extraction_job_running(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<KpiExtractionJob> {
    connection.execute(
        "
        UPDATE kpi_extraction_jobs
        SET status = 'running',
            started_at = COALESCE(started_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
            error_code = NULL,
            error = NULL
        WHERE id = ?1
        ",
        [job_id],
    )?;
    get_kpi_extraction_job(connection, job_id)
}

pub(crate) fn mark_kpi_extraction_job_failed(
    connection: &Connection,
    job_id: &str,
    error_code: &str,
    error: &str,
) -> StorageResult<KpiExtractionJob> {
    require("error_code", error_code)?;
    require("error", error)?;
    connection.execute(
        "
        UPDATE kpi_extraction_jobs
        SET status = 'failed',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = ?2,
            error = ?3
        WHERE id = ?1
        ",
        params![job_id, error_code, error],
    )?;
    get_kpi_extraction_job(connection, job_id)
}

pub(crate) fn complete_kpi_extraction_job(
    connection: &mut Connection,
    input: CompletedKpiExtraction,
) -> StorageResult<KpiExtractionJob> {
    let job_id = input.job_id.clone();
    let transaction = connection.transaction()?;

    // Refresh pending/rejected proposals; never disturb confirmed ones (they own facts).
    transaction.execute(
        "DELETE FROM kpi_extraction_proposals WHERE job_id = ?1 AND status != 'confirmed'",
        [&job_id],
    )?;

    for proposal in &input.proposals {
        let metric_key = proposal.metric_key.trim();
        if metric_key.is_empty() || proposal.value_numeric.trim().is_empty() {
            continue;
        }
        let proposal_id = proposal_id(&job_id, metric_key);
        // Skip a metric already confirmed for this job.
        let confirmed: bool = transaction
            .query_row(
                "SELECT status = 'confirmed' FROM kpi_extraction_proposals WHERE id = ?1",
                [&proposal_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(false);
        if confirmed {
            continue;
        }

        let label = if proposal.label.trim().is_empty() {
            metric_key
        } else {
            proposal.label.trim()
        };
        transaction.execute(
            "
            INSERT INTO kpi_extraction_proposals (
                id, job_id, metric_key, label, value_numeric, unit, currency,
                as_reported_value, as_reported_scale, measure_window, confidence,
                source_snippet, is_proposed_kpi, status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'pending')
            ON CONFLICT(id) DO UPDATE SET
                label = excluded.label,
                value_numeric = excluded.value_numeric,
                unit = excluded.unit,
                currency = excluded.currency,
                as_reported_value = excluded.as_reported_value,
                as_reported_scale = excluded.as_reported_scale,
                measure_window = excluded.measure_window,
                confidence = excluded.confidence,
                source_snippet = excluded.source_snippet,
                is_proposed_kpi = excluded.is_proposed_kpi,
                status = 'pending',
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                proposal_id,
                job_id,
                metric_key,
                label,
                proposal.value_numeric.trim(),
                trimmed_ref(&proposal.unit),
                trimmed_ref(&proposal.currency),
                trimmed_ref(&proposal.as_reported_value),
                trimmed_ref(&proposal.as_reported_scale),
                trimmed_ref(&proposal.measure_window),
                trimmed_ref(&proposal.confidence),
                trimmed_ref(&proposal.source_snippet),
                proposal.is_proposed_kpi as i64,
            ],
        )?;
    }

    transaction.execute(
        "
        UPDATE kpi_extraction_jobs
        SET status = 'succeeded',
            finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
            error_code = NULL,
            error = NULL,
            detected_fiscal_year = ?2,
            detected_period_type = ?3,
            detected_period_end_date = ?4,
            detected_currency = ?5,
            detected_language = ?6
        WHERE id = ?1
        ",
        params![
            job_id,
            input.detected_fiscal_year,
            trimmed_option(input.detected_period_type),
            trimmed_option(input.detected_period_end_date),
            trimmed_option(input.detected_currency),
            trimmed_option(input.detected_language),
        ],
    )?;

    transaction.commit()?;
    get_kpi_extraction_job(connection, &job_id)
}

pub(crate) fn list_kpi_extraction_jobs_by_document(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Vec<KpiExtractionJob>> {
    let mut statement = connection.prepare(
        "
        SELECT id, company_id, report_document_id, provider_id, model, prompt_version, period_hint,
               status, error_code, error, detected_fiscal_year, detected_period_type,
               detected_period_end_date, detected_currency, detected_language,
               created_at, started_at, finished_at
        FROM kpi_extraction_jobs
        WHERE report_document_id = ?1
        ORDER BY created_at DESC, id DESC
        ",
    )?;
    let rows = statement.query_map([report_document_id], job_from_row)?;
    let mut jobs = rows.collect::<Result<Vec<_>, _>>()?;
    for job in &mut jobs {
        job.proposals = list_proposals(connection, &job.id)?;
    }
    Ok(jobs)
}

pub(crate) fn get_kpi_extraction_job(
    connection: &Connection,
    job_id: &str,
) -> StorageResult<KpiExtractionJob> {
    let mut job = connection.query_row(
        "
        SELECT id, company_id, report_document_id, provider_id, model, prompt_version, period_hint,
               status, error_code, error, detected_fiscal_year, detected_period_type,
               detected_period_end_date, detected_currency, detected_language,
               created_at, started_at, finished_at
        FROM kpi_extraction_jobs
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
) -> StorageResult<Vec<KpiExtractionProposal>> {
    let mut statement = connection.prepare(
        "
        SELECT id, job_id, metric_key, label, value_numeric, unit, currency,
               as_reported_value, as_reported_scale, measure_window, confidence,
               source_snippet, is_proposed_kpi, status, fact_id, created_at, updated_at
        FROM kpi_extraction_proposals
        WHERE job_id = ?1
        ORDER BY is_proposed_kpi, metric_key COLLATE NOCASE
        ",
    )?;
    let rows = statement.query_map([job_id], proposal_from_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(crate) fn reject_kpi_proposal(
    connection: &Connection,
    proposal_id: &str,
) -> StorageResult<KpiExtractionProposal> {
    connection.execute(
        "
        UPDATE kpi_extraction_proposals
        SET status = 'rejected',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1 AND status != 'confirmed'
        ",
        [proposal_id],
    )?;
    get_proposal(connection, proposal_id)
}

/// Confirm a proposal into a committed financial fact. Ensures the period exists,
/// resolves (or, for accepted suggestions, creates) the KPI definition, writes the
/// fact, and records the proposal as confirmed with the new fact id.
pub(crate) fn confirm_kpi_proposal(
    connection: &Connection,
    input: ConfirmKpiProposalInput,
) -> StorageResult<FinancialFact> {
    confirm_kpi_proposal_with_state(connection, input, "confirmed")
}

/// Auto-confirm a proposal on the autopilot path (North Star, v0.49.0 / ADR 0055):
/// commit the model-detected value as a fact in the **`auto_unreviewed`**
/// provenance state — cited, flagged, and reversible — using the job's detected
/// period (no user overrides). The global confirm-before-commit default is
/// unchanged; this only runs for a company explicitly opted into `autopilot`.
pub(crate) fn auto_confirm_kpi_proposal(
    connection: &Connection,
    proposal_id: &str,
) -> StorageResult<FinancialFact> {
    confirm_kpi_proposal_with_state(
        connection,
        ConfirmKpiProposalInput {
            proposal_id: proposal_id.to_owned(),
            value_numeric: None,
            currency: None,
            fiscal_year: None,
            period_type: None,
            period_end_date: None,
            accept_as_new_kpi: false,
        },
        "auto_unreviewed",
    )
}

fn confirm_kpi_proposal_with_state(
    connection: &Connection,
    input: ConfirmKpiProposalInput,
    confirmation_state: &str,
) -> StorageResult<FinancialFact> {
    let proposal = get_proposal(connection, &input.proposal_id)?;
    if proposal.status == "confirmed" {
        return Err(invalid("status", "already confirmed"));
    }
    let job = get_kpi_extraction_job(connection, &proposal.job_id)?;

    let fiscal_year = input
        .fiscal_year
        .or(job.detected_fiscal_year)
        .ok_or_else(|| invalid("fiscal_year", "missing"))?;
    let period_type = trimmed_option(input.period_type)
        .or_else(|| job.detected_period_type.clone())
        .ok_or_else(|| invalid("period_type", "missing"))?;
    let period_end_date =
        trimmed_option(input.period_end_date).or_else(|| job.detected_period_end_date.clone());

    let period_id = ensure_period(
        connection,
        &job.company_id,
        fiscal_year,
        &period_type,
        period_end_date.as_deref(),
        &job.report_document_id,
    )?;
    let definition_id = resolve_definition(
        connection,
        &job.company_id,
        &proposal,
        input.accept_as_new_kpi,
    )?;

    let value_numeric = trimmed_option(input.value_numeric).unwrap_or(proposal.value_numeric);
    let currency = trimmed_option(input.currency).or(proposal.currency);

    let fact = create_financial_fact(
        connection,
        NewFinancialFact {
            company_id: job.company_id.clone(),
            period_id,
            definition_id,
            value_numeric,
            currency,
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: proposal.as_reported_value,
            as_reported_scale: proposal.as_reported_scale,
            reporting_standard: None,
            extraction_method: Some("ai".to_owned()),
            confidence: proposal.confidence,
            confirmation_state: Some(confirmation_state.to_owned()),
            supersedes_id: None,
            source_document_ref: Some(job.report_document_id.clone()),
        },
    )?;

    connection.execute(
        "
        UPDATE kpi_extraction_proposals
        SET status = 'confirmed',
            fact_id = ?2,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![input.proposal_id, fact.id],
    )?;

    Ok(fact)
}

fn ensure_period(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    period_end_date: Option<&str>,
    report_evidence_ref: &str,
) -> StorageResult<String> {
    // UNIQUE(company_id, fiscal_year, period_type) makes this an idempotent upsert,
    // sharing one period row with manual entry regardless of the generated id.
    let id = period_id(company_id, fiscal_year, period_type);
    connection.execute(
        "
        INSERT OR IGNORE INTO financial_periods (
            id, company_id, fiscal_year, period_type, period_end_date, report_evidence_ref
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            id,
            company_id,
            fiscal_year,
            period_type,
            period_end_date,
            report_evidence_ref
        ],
    )?;
    let resolved: String = connection.query_row(
        "SELECT id FROM financial_periods WHERE company_id = ?1 AND fiscal_year = ?2 AND period_type = ?3",
        params![company_id, fiscal_year, period_type],
        |row| row.get(0),
    )?;
    Ok(resolved)
}

fn resolve_definition(
    connection: &Connection,
    company_id: &str,
    proposal: &KpiExtractionProposal,
    accept_as_new_kpi: bool,
) -> StorageResult<String> {
    let metric_key = proposal.metric_key.trim();
    // Prefer a company-scoped definition, then any global/sector definition.
    let existing: Option<String> = connection
        .query_row(
            "
            SELECT id FROM kpi_definitions
            WHERE metric_key = ?1 AND (company_id = ?2 OR company_id IS NULL)
            ORDER BY (company_id IS NULL)
            LIMIT 1
            ",
            params![metric_key, company_id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        return Ok(id);
    }

    if !accept_as_new_kpi {
        return Err(invalid(
            "metric_key",
            "no KPI definition; accept it as a new custom KPI to confirm",
        ));
    }

    // Create a company-scoped definition for the accepted suggestion.
    let id = company_definition_id(company_id, metric_key);
    connection.execute(
        "
        INSERT OR IGNORE INTO kpi_definitions (
            id, scope, company_id, metric_key, label, value_kind, unit, computation
        ) VALUES (?1, 'company', ?2, ?3, ?4, 'monetary', ?5, 'reported')
        ",
        params![
            id,
            company_id,
            metric_key,
            proposal.label.trim(),
            proposal.unit.as_deref()
        ],
    )?;
    let resolved: String = connection.query_row(
        "SELECT id FROM kpi_definitions WHERE metric_key = ?1 AND scope = 'company' AND company_id = ?2",
        params![metric_key, company_id],
        |row| row.get(0),
    )?;
    Ok(resolved)
}

fn get_proposal(
    connection: &Connection,
    proposal_id: &str,
) -> StorageResult<KpiExtractionProposal> {
    connection
        .query_row(
            "
            SELECT id, job_id, metric_key, label, value_numeric, unit, currency,
                   as_reported_value, as_reported_scale, measure_window, confidence,
                   source_snippet, is_proposed_kpi, status, fact_id, created_at, updated_at
            FROM kpi_extraction_proposals
            WHERE id = ?1
            ",
            [proposal_id],
            proposal_from_row,
        )
        .map_err(StorageError::from)
}

fn job_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KpiExtractionJob> {
    Ok(KpiExtractionJob {
        id: row.get(0)?,
        company_id: row.get(1)?,
        report_document_id: row.get(2)?,
        provider_id: row.get(3)?,
        model: row.get(4)?,
        prompt_version: row.get(5)?,
        period_hint: row.get(6)?,
        status: row.get(7)?,
        error_code: row.get(8)?,
        error: row.get(9)?,
        detected_fiscal_year: row.get(10)?,
        detected_period_type: row.get(11)?,
        detected_period_end_date: row.get(12)?,
        detected_currency: row.get(13)?,
        detected_language: row.get(14)?,
        created_at: row.get(15)?,
        started_at: row.get(16)?,
        finished_at: row.get(17)?,
        proposals: Vec::new(),
    })
}

fn proposal_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KpiExtractionProposal> {
    Ok(KpiExtractionProposal {
        id: row.get(0)?,
        job_id: row.get(1)?,
        metric_key: row.get(2)?,
        label: row.get(3)?,
        value_numeric: row.get(4)?,
        unit: row.get(5)?,
        currency: row.get(6)?,
        as_reported_value: row.get(7)?,
        as_reported_scale: row.get(8)?,
        measure_window: row.get(9)?,
        confidence: row.get(10)?,
        source_snippet: row.get(11)?,
        is_proposed_kpi: row.get::<_, i64>(12)? != 0,
        status: row.get(13)?,
        fact_id: row.get(14)?,
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
    StorageError::InvalidFinancialsValue {
        key,
        value: value.to_owned(),
    }
}

fn trimmed_option(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn trimmed_ref(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn job_id(report_document_id: &str) -> String {
    format!("kpi_ext_{}", slug_part(report_document_id))
}

fn proposal_id(job_id: &str, metric_key: &str) -> String {
    format!("kpi_prop_{}_{}", slug_part(job_id), slug_part(metric_key))
}

fn period_id(company_id: &str, fiscal_year: i64, period_type: &str) -> String {
    format!(
        "finper_{}_{}_{}",
        slug_part(company_id),
        fiscal_year,
        slug_part(period_type)
    )
}

fn company_definition_id(company_id: &str, metric_key: &str) -> String {
    format!("kpidef_{}_{}", slug_part(company_id), slug_part(metric_key))
}

use super::database::Database;
/// kpi_extraction domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::kpi_extraction()`.
#[derive(Clone)]
pub struct KpiExtractionStore {
    db: Database,
}

impl KpiExtractionStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create_kpi_extraction_job(
        &self,
        input: NewKpiExtractionJob,
    ) -> StorageResult<KpiExtractionJob> {
        let connection = self.db.checkout()?;

        create_kpi_extraction_job(&connection, input)
    }

    pub fn get_kpi_extraction_job(&self, job_id: &str) -> StorageResult<KpiExtractionJob> {
        let connection = self.db.checkout()?;

        get_kpi_extraction_job(&connection, job_id)
    }

    pub fn list_kpi_extraction_jobs_by_document(
        &self,
        report_document_id: &str,
    ) -> StorageResult<Vec<KpiExtractionJob>> {
        let connection = self.db.checkout()?;

        list_kpi_extraction_jobs_by_document(&connection, report_document_id)
    }

    pub fn mark_kpi_extraction_job_running(&self, job_id: &str) -> StorageResult<KpiExtractionJob> {
        let connection = self.db.checkout()?;

        mark_kpi_extraction_job_running(&connection, job_id)
    }

    pub fn mark_kpi_extraction_job_failed(
        &self,
        job_id: &str,
        error_code: &str,
        error: &str,
    ) -> StorageResult<KpiExtractionJob> {
        let connection = self.db.checkout()?;

        mark_kpi_extraction_job_failed(&connection, job_id, error_code, error)
    }

    pub fn complete_kpi_extraction_job(
        &self,
        input: CompletedKpiExtraction,
    ) -> StorageResult<KpiExtractionJob> {
        let mut connection = self.db.checkout()?;

        complete_kpi_extraction_job(&mut connection, input)
    }

    pub fn confirm_kpi_proposal(
        &self,
        input: ConfirmKpiProposalInput,
    ) -> StorageResult<FinancialFact> {
        let connection = self.db.checkout()?;

        confirm_kpi_proposal(&connection, input)
    }

    /// Auto-confirm a proposal as an `auto_unreviewed` fact (autopilot path, ADR 0055).
    pub fn auto_confirm_kpi_proposal(&self, proposal_id: &str) -> StorageResult<FinancialFact> {
        let connection = self.db.checkout()?;

        auto_confirm_kpi_proposal(&connection, proposal_id)
    }

    pub fn reject_kpi_proposal(&self, proposal_id: &str) -> StorageResult<KpiExtractionProposal> {
        let connection = self.db.checkout()?;

        reject_kpi_proposal(&connection, proposal_id)
    }
}
