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

    // ADR 0061 guardrail: no fact without a validation status. `ai` (not
    // `ai_text`) because this path sends the native document to the model
    // today, not extracted text; an AI-confirmed fact is never validated, so
    // it always lands `none`. Inline SQL (mirrors `record_structured_fact`
    // below) because only a `&Connection` is held here.
    connection.execute(
        "
        INSERT INTO financial_fact_provenance
            (fact_id, source_tier, validation_status, drift_json, citation)
        VALUES (?1, 'ai', 'none', NULL, ?2)
        ON CONFLICT(fact_id) DO UPDATE SET
            source_tier = excluded.source_tier,
            validation_status = excluded.validation_status,
            citation = excluded.citation,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![fact.id, proposal.label],
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

/// Input for [`record_structured_fact`]: one metric produced by the
/// deterministic pipeline (ADR 0061).
pub struct StructuredFactInput<'a> {
    pub company_id: &'a str,
    pub fiscal_year: i64,
    pub period_type: &'a str,
    pub period_end: Option<&'a str>,
    pub report_document_id: &'a str,
    pub metric_key: &'a str,
    pub value_numeric: &'a str,
    pub currency: Option<&'a str>,
    /// `pending` (assist) | `auto_unreviewed` (autopilot) | `confirmed`.
    pub confirmation_state: &'a str,
    /// `esef` | `pdf` | `html_aggregator` | …
    pub source_tier: &'a str,
    /// `passed` | `witness_confirmed` | `unreviewed` | `flagged`.
    pub validation_status: &'a str,
    /// Serialized `DriftReport` JSON when the pipeline detected a layout drift
    /// for this outcome (PDF tier only); `None` on a clean/no-profile parse.
    pub drift_json: Option<&'a str>,
    pub citation: Option<&'a str>,
}

pub(crate) fn record_structured_fact(
    connection: &Connection,
    input: StructuredFactInput<'_>,
) -> StorageResult<Option<String>> {
    let period_id = ensure_period(
        connection,
        input.company_id,
        input.fiscal_year,
        input.period_type,
        input.period_end,
        input.report_document_id,
    )?;
    let Some(definition_id) =
        resolve_definition_by_metric_key(connection, input.company_id, input.metric_key)?
    else {
        // Not a catalog metric — the structured pipeline only emits canonical
        // keys, so this is a defensive skip, never a silent bad write.
        return Ok(None);
    };

    let fact = create_financial_fact(
        connection,
        NewFinancialFact {
            company_id: input.company_id.to_owned(),
            period_id,
            definition_id,
            value_numeric: input.value_numeric.to_owned(),
            currency: input.currency.map(str::to_owned),
            statement_basis: None,
            attribution: None,
            variant: None,
            measure_window: None,
            data_quality: None,
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            // Deterministic structured extraction, not an AI read.
            extraction_method: Some("api".to_owned()),
            confidence: None,
            confirmation_state: Some(input.confirmation_state.to_owned()),
            supersedes_id: None,
            source_document_ref: Some(input.report_document_id.to_owned()),
        },
    )?;

    connection.execute(
        "
        INSERT INTO financial_fact_provenance
            (fact_id, source_tier, validation_status, drift_json, citation)
        VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(fact_id) DO UPDATE SET
            source_tier = excluded.source_tier,
            validation_status = excluded.validation_status,
            drift_json = excluded.drift_json,
            citation = excluded.citation,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            fact.id,
            input.source_tier,
            input.validation_status,
            input.drift_json,
            input.citation,
        ],
    )?;

    Ok(Some(fact.id))
}

/// Resolves a canonical/company KPI definition by metric key, without creating
/// one (the structured pipeline only emits seeded catalog metrics).
fn resolve_definition_by_metric_key(
    connection: &Connection,
    company_id: &str,
    metric_key: &str,
) -> StorageResult<Option<String>> {
    let existing: Option<String> = connection
        .query_row(
            "
            SELECT id FROM kpi_definitions
            WHERE metric_key = ?1 AND (company_id = ?2 OR company_id IS NULL)
            ORDER BY (company_id IS NULL)
            LIMIT 1
            ",
            params![metric_key.trim(), company_id],
            |row| row.get(0),
        )
        .optional()?;
    Ok(existing)
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

    /// Persists one deterministically-extracted fact (ADR 0061): ensures the
    /// period, resolves the canonical KPI definition, writes the fact with the
    /// given confirmation state, and records its structured provenance (source
    /// tier + validation verdict + citation) — all in one transaction. Returns
    /// the fact id, or `None` when the metric has no catalog definition (a
    /// non-canonical key the structured pipeline should not emit).
    pub fn record_structured_fact(
        &self,
        input: StructuredFactInput<'_>,
    ) -> StorageResult<Option<String>> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction()?;
        let result = record_structured_fact(&tx, input)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn reject_kpi_proposal(&self, proposal_id: &str) -> StorageResult<KpiExtractionProposal> {
        let connection = self.db.checkout()?;

        reject_kpi_proposal(&connection, proposal_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    fn seed_company_and_document(connection: &Connection) -> (String, String) {
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c1', 'gpw', 'ABC', 'GPW:ABC', 'ABC SA')",
                [],
            )
            .expect("company");
        connection
            .execute(
                "INSERT INTO report_documents (id, company_id, source_type, url, fetch_status)
                 VALUES ('doc1', 'c1', 'espi_attachment', 'https://x/doc1.pdf', 'fetched')",
                [],
            )
            .expect("document");
        ("c1".to_owned(), "doc1".to_owned())
    }

    /// Runs a job through the same completion path the runner uses and returns
    /// the resulting `revenue` proposal (a seeded canonical metric key), so
    /// confirm/auto-confirm tests exercise the real proposal->fact plumbing
    /// without depending on the AI provider job runner (which lives outside
    /// this module).
    fn seed_pending_revenue_proposal(
        connection: &mut Connection,
        company_id: &str,
        document_id: &str,
    ) -> KpiExtractionProposal {
        let job = create_kpi_extraction_job(
            connection,
            NewKpiExtractionJob {
                company_id: company_id.to_owned(),
                report_document_id: document_id.to_owned(),
                provider_id: "test-sample".to_owned(),
                model: "test-sample-model".to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            },
        )
        .expect("extraction job created");

        let job = complete_kpi_extraction_job(
            connection,
            CompletedKpiExtraction {
                job_id: job.id,
                detected_fiscal_year: Some(2025),
                detected_period_type: Some("FY".to_owned()),
                detected_period_end_date: Some("2025-12-31".to_owned()),
                detected_currency: Some("PLN".to_owned()),
                detected_language: Some("pl".to_owned()),
                proposals: vec![NewKpiProposal {
                    metric_key: "revenue".to_owned(),
                    label: "Revenue".to_owned(),
                    value_numeric: "1000000".to_owned(),
                    unit: None,
                    currency: Some("PLN".to_owned()),
                    as_reported_value: Some("1,000,000".to_owned()),
                    as_reported_scale: Some("units".to_owned()),
                    measure_window: None,
                    confidence: Some("high".to_owned()),
                    source_snippet: Some("Revenue for FY2025 was PLN 1,000,000.".to_owned()),
                    is_proposed_kpi: false,
                }],
            },
        )
        .expect("job completed");

        job.proposals
            .into_iter()
            .find(|p| p.metric_key == "revenue")
            .expect("revenue proposal")
    }

    fn fact_provenance_row(
        connection: &Connection,
        fact_id: &str,
    ) -> Option<(String, String, Option<String>, Option<String>)> {
        connection
            .query_row(
                "SELECT source_tier, validation_status, drift_json, citation
                 FROM financial_fact_provenance WHERE fact_id = ?1",
                [fact_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .expect("provenance query")
    }

    /// ADR 0061 guardrail: no fact without a validation status. The manual
    /// confirm path sends the native PDF to the model today (not extracted
    /// text), so its provenance tier is the honest `ai` — not `ai_text`.
    #[test]
    fn confirming_a_proposal_records_ai_provenance() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let proposal = seed_pending_revenue_proposal(&mut connection, &company_id, &document_id);

        let fact = confirm_kpi_proposal(
            &connection,
            ConfirmKpiProposalInput {
                proposal_id: proposal.id.clone(),
                value_numeric: None,
                currency: None,
                fiscal_year: None,
                period_type: None,
                period_end_date: None,
                accept_as_new_kpi: false,
            },
        )
        .expect("confirm proposal");

        let (source_tier, validation_status, drift_json, citation) =
            fact_provenance_row(&connection, &fact.id)
                .expect("a confirmed AI proposal must carry a provenance row");
        assert_eq!(source_tier, "ai");
        assert_eq!(validation_status, "none");
        assert_eq!(drift_json, None);
        assert_eq!(citation.as_deref(), Some(proposal.label.as_str()));
    }

    /// Same guardrail on the autopilot auto-confirm path (ADR 0055): the
    /// `auto_unreviewed` confirmation state must not skip provenance either.
    #[test]
    fn auto_confirming_a_proposal_records_ai_provenance() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let proposal = seed_pending_revenue_proposal(&mut connection, &company_id, &document_id);

        let fact =
            auto_confirm_kpi_proposal(&connection, &proposal.id).expect("auto-confirm proposal");

        let (source_tier, validation_status, drift_json, citation) =
            fact_provenance_row(&connection, &fact.id)
                .expect("an auto-confirmed AI proposal must carry a provenance row");
        assert_eq!(source_tier, "ai");
        assert_eq!(validation_status, "none");
        assert_eq!(drift_json, None);
        assert_eq!(citation.as_deref(), Some(proposal.label.as_str()));
    }

    /// ADR 0061: the structured pipeline persists its per-outcome drift
    /// alongside the fact, not just returns it for the caller to drop —
    /// `record_structured_fact`'s INSERT used to hardcode `drift_json = NULL`.
    #[test]
    fn structured_fact_persists_drift_json_when_present() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let drift =
            r#"{"addedLabels":[],"removedLabels":["total equity line"],"unitChanged":null}"#;

        let id = record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: Some("2025-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "1000000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "pdf",
                validation_status: "flagged",
                drift_json: Some(drift),
                citation: Some("Przychody netto ze sprzedazy"),
            },
        )
        .expect("record structured fact")
        .expect("revenue is a catalog metric");

        let (source_tier, validation_status, stored_drift, citation) =
            fact_provenance_row(&connection, &id).expect("a structured fact must carry provenance");
        assert_eq!(source_tier, "pdf");
        assert_eq!(validation_status, "flagged");
        assert_eq!(stored_drift.as_deref(), Some(drift));
        assert_eq!(citation.as_deref(), Some("Przychody netto ze sprzedazy"));
    }

    /// The provenance `ON CONFLICT(fact_id)` clause must refresh `drift_json`
    /// too (not just the columns it already updated) — proven directly against
    /// the table rather than via two `record_structured_fact` calls: a second
    /// call for the *same* period+metric hits `financial_facts`' own
    /// `UNIQUE(period_id, definition_id, ...)` constraint before it would ever
    /// reach a repeated `fact_id` (facts are not upserted, only inserted), so
    /// that branch is unreached via this function today — this pins the SQL
    /// behavior itself so a future caller that *does* reuse a `fact_id` (e.g. a
    /// correction/re-provenance path) can rely on it.
    #[test]
    fn structured_fact_provenance_on_conflict_refreshes_drift_json() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        let id = record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2025,
                period_type: "FY",
                period_end: Some("2025-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "1000000",
                currency: Some("PLN"),
                confirmation_state: "flagged",
                source_tier: "pdf",
                validation_status: "flagged",
                drift_json: Some(r#"{"addedLabels":[],"removedLabels":["x"],"unitChanged":null}"#),
                citation: Some("Przychody"),
            },
        )
        .expect("record structured fact")
        .expect("revenue is a catalog metric");

        // Re-provenance the same fact (the same `ON CONFLICT(fact_id)` upsert
        // `record_structured_fact` issues) with a resolved, drift-free outcome.
        connection
            .execute(
                "
                INSERT INTO financial_fact_provenance
                    (fact_id, source_tier, validation_status, drift_json, citation)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT(fact_id) DO UPDATE SET
                    source_tier = excluded.source_tier,
                    validation_status = excluded.validation_status,
                    drift_json = excluded.drift_json,
                    citation = excluded.citation,
                    updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                ",
                params![id, "pdf", "passed", Option::<&str>::None, "Przychody"],
            )
            .expect("re-provenance the same fact");

        let (_, validation_status, drift_json, _) =
            fact_provenance_row(&connection, &id).expect("a structured fact must carry provenance");
        assert_eq!(validation_status, "passed");
        assert_eq!(
            drift_json, None,
            "the upsert must clear a stale drift flag, not just leave it in place"
        );
    }
}
