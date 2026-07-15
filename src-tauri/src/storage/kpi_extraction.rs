//! Storage for AI KPI extraction (v0.36.0, epic 9879941).
//!
//! Extraction never writes facts directly. The async job persists PROPOSALS; only
//! an explicit user confirmation materialises a `financial_fact`. Confirmed
//! proposals are retained as the provenance trail (which job/provider/model/prompt
//! produced the value, and the verbatim source snippet).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::financials::{create_or_reobserve_financial_fact, stored_fact_set, FactWriteOutcome};
use super::{slug_part, FinancialFact, NewFinancialFact, StorageError, StorageResult};
use crate::fundamentals::validation::{validate, Status, Tolerance};

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
    /// How many validated facts this run committed directly (ADR 0077 §4 tier-4:
    /// a confirmed OCR profile parses to facts, not proposals). `0` for the
    /// classic proposals-only path, so the panel reads an honest outcome.
    pub committed_fact_count: i64,
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

/// One pending proposal with the period + document context the review queue
/// panel (F5, ADR 0077 §4/§5) renders: the proposed metric/value, the job's
/// detected period (the grouping axis), the source document it came from, and
/// the `source_snippet` whose `ocr_bootstrap` / `ocr_pending_profile` /
/// `ocr_flagged` prefix the UI derives the source chip from. Only `pending`
/// proposals appear — confirmed ones own facts, rejected ones are discarded.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct PendingKpiProposal {
    pub id: String,
    pub job_id: String,
    pub metric_key: String,
    pub label: String,
    pub value_numeric: String,
    pub unit: Option<String>,
    pub currency: Option<String>,
    pub source_snippet: Option<String>,
    /// Always `"pending"` (the read model filters to it); kept explicit so the
    /// DTO reads honestly and the fidelity corpus can pin its absence.
    pub status: String,
    /// The job's detected fiscal year — the review queue groups by period, so a
    /// job with no detected period yields `None` (an ungrouped bucket).
    pub fiscal_year: Option<i64>,
    pub period_type: Option<String>,
    pub document_id: String,
    pub document_title: Option<String>,
    pub document_url: String,
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
    /// Validated facts committed directly by this run (tier-4 profile path, ADR
    /// 0077 §4). `0` for the proposals-only path.
    pub committed_fact_count: i64,
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
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

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
            detected_language = ?6,
            committed_fact_count = ?7
        WHERE id = ?1
        ",
        params![
            job_id,
            input.detected_fiscal_year,
            trimmed_option(input.detected_period_type),
            trimmed_option(input.detected_period_end_date),
            trimmed_option(input.detected_currency),
            trimmed_option(input.detected_language),
            input.committed_fact_count,
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
               created_at, started_at, finished_at, committed_fact_count
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
               created_at, started_at, finished_at, committed_fact_count
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

/// The outcome of confirming a KPI proposal (ADR 0077 T4.4): the committed fact
/// plus the validation status recorded on its provenance row. The confirmed
/// value always persists (the user explicitly confirmed it) — the status
/// records what `validate` *saw* over the period's fact set, so the UI can
/// surface a `flagged` contradiction without the confirm being blocked.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmedKpiFact {
    pub fact: FinancialFact,
    /// `passed` | `flagged` | `unreviewed` — the deterministic validation
    /// verdict over the period's fact set (the retired `none` is never written).
    pub validation_status: String,
}

/// Confirm a proposal into a committed financial fact. Ensures the period exists,
/// resolves (or, for accepted suggestions, creates) the KPI definition, writes the
/// fact, and records the proposal as confirmed with the new fact id.
pub(crate) fn confirm_kpi_proposal(
    connection: &Connection,
    input: ConfirmKpiProposalInput,
) -> StorageResult<ConfirmedKpiFact> {
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
    .map(|confirmed| confirmed.fact)
}

fn confirm_kpi_proposal_with_state(
    connection: &Connection,
    input: ConfirmKpiProposalInput,
    confirmation_state: &str,
) -> StorageResult<ConfirmedKpiFact> {
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

    // Slot-aware confirm (owner live find, 2026-07-10): a leftover proposal may
    // target a slot a later run already committed. Resolve the uniqueness slot
    // before inserting instead of raising its raw sqlite UNIQUE violation —
    // mirrors the structured pipeline's re-observation model (`record_structured_fact`).
    let (fact, reobserved) = match create_or_reobserve_financial_fact(
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
    )? {
        // Fresh value → committed as usual.
        FactWriteOutcome::Created(fact) => (fact, false),
        // Same slot, same value → re-observed: link the proposal to the EXISTING
        // fact (no dupe row), keeping its original provenance untouched. The
        // confirm still validates + confirms the OCR profile below.
        FactWriteOutcome::Reobserved(existing) => (existing, true),
        // Same slot, different value → never silently overwrite a stored
        // (possibly confirmed) fact and never mutate the proposal. Surface a typed
        // conflict carrying both values so the user decides (keep/reject).
        FactWriteOutcome::Divergent { existing, incoming } => {
            return Err(invalid(
                "value_conflict",
                &format!(
                    "stored={} proposed={}",
                    existing.value_numeric.trim(),
                    incoming.trim()
                ),
            ));
        }
    };

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

    // ADR 0077 T4.4: the AI-confirm path validates like every other fact source
    // — the retired `'none'` is never written. Assemble the period's fact set
    // (the freshly-confirmed value is already stored, plus any facts already in
    // the period) via the same storage read the structured pipeline's
    // cross-check uses, and run the deterministic identity gate. The confirmed
    // fact persists regardless (the user explicitly confirmed it) — the status
    // records only what validation *saw*, it does not block the confirm.
    let current = stored_fact_set(connection, &job.company_id, fiscal_year, &period_type)?
        .unwrap_or_default();
    let prior = stored_fact_set(connection, &job.company_id, fiscal_year - 1, &period_type)?;
    let report = validate(&current, prior.as_ref(), None, None, &Tolerance::default());
    let validation_status = match report.status {
        Status::Passed => "passed",
        Status::Failed => "flagged",
        Status::Inconclusive => "unreviewed",
    };

    // `ai` (not `ai_text`) because this path sends the native document to the
    // model today, not extracted text. Inline SQL (mirrors
    // `record_structured_fact` below) because only a `&Connection` is held here.
    // Skipped when re-observing an existing fact: that fact already carries the
    // provenance of the source that actually produced it, and a re-observation
    // must not repaint it as `ai` (mirrors the structured path's `Reobserved`).
    if !reobserved {
        connection.execute(
            "
            INSERT INTO financial_fact_provenance
                (fact_id, source_tier, validation_status, drift_json, citation)
            VALUES (?1, 'ai', ?2, NULL, ?3)
            ON CONFLICT(fact_id) DO UPDATE SET
                source_tier = excluded.source_tier,
                validation_status = excluded.validation_status,
                citation = excluded.citation,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![fact.id, validation_status, proposal.label],
        )?;
    }

    // ADR 0077 §4 kickoff decision 3: confirming a tier-4 OCR proposal is what
    // CONFIRMS the company's bootstrap profile — the user has reviewed values
    // parsed through the proposed map, so the version-1 profile bumps to 2 and
    // tier-4 may start committing validated facts. Idempotent: only a v1 row
    // bumps; later confirms are no-ops. The snippet prefix is the marker the
    // tier-4 proposal writer stamps (`ocr_bootstrap` / `ocr_pending_profile`).
    let is_ocr_proposal = proposal
        .source_snippet
        .as_deref()
        .is_some_and(|s| s.starts_with("ocr_bootstrap") || s.starts_with("ocr_pending_profile"));
    if is_ocr_proposal {
        confirm_ocr_profile_if_pending(connection, &job.company_id)?;
    }

    Ok(ConfirmedKpiFact {
        fact,
        validation_status: validation_status.to_owned(),
    })
}

/// Bump a company's version-1 (unconfirmed bootstrap) OCR profile to its
/// confirmed version, keeping the serialized `profile_json` in sync with the
/// `version` column. A missing row or an already-confirmed profile is a no-op.
fn confirm_ocr_profile_if_pending(connection: &Connection, company_id: &str) -> StorageResult<()> {
    use crate::fundamentals::extraction::ocr::OcrExtractionProfile;
    let row: Option<(i64, String)> = connection
        .query_row(
            "SELECT version, profile_json FROM company_ocr_extraction_profile
             WHERE company_id = ?1",
            [company_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((1, json)) = row else {
        return Ok(());
    };
    let Ok(stored) = serde_json::from_str::<OcrExtractionProfile>(&json) else {
        // A corrupt profile row must not fail the confirm; the profile simply
        // stays unconfirmed (tier-4 keeps landing proposals — the safe side).
        return Ok(());
    };
    // Re-affirm the same layout: confirmation reviews the map, it does not edit it.
    let confirmed = stored.clone().confirm(
        stored.scale,
        stored.label_map.clone(),
        stored.value_column,
        stored.skip_columns.clone(),
        stored.strip_enumerators,
    );
    connection.execute(
        "UPDATE company_ocr_extraction_profile
         SET version = ?2,
             profile_json = ?3,
             template_hash = ?4,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE company_id = ?1 AND version = 1",
        params![
            company_id,
            confirmed.version,
            serde_json::to_string(&confirmed).map_err(StorageError::Json)?,
            confirmed.template_hash,
        ],
    )?;
    Ok(())
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
    /// The `financial_facts.extraction_method` marker distinguishing sub-tiers that
    /// share a `source_tier`: `api` (the deterministic ESEF/PDF/xHTML tiers) |
    /// `html_positional` (the tier-3b pdf2htmlEX positional parser, persisted under
    /// `source_tier='pdf'` — ADR 0077 T-B2). Never trust-bearing on its own; the
    /// `source_tier` + `validation_status` remain the trust signals.
    pub extraction_method: &'a str,
    /// `passed` | `witness_confirmed` | `unreviewed` | `flagged`.
    pub validation_status: &'a str,
    /// Serialized `DriftReport` JSON when the pipeline detected a layout drift
    /// for this outcome (PDF tier only); `None` on a clean/no-profile parse.
    pub drift_json: Option<&'a str>,
    pub citation: Option<&'a str>,
}

/// Outcome of committing one deterministically-extracted fact (ADR 0061) into
/// its uniqueness slot. `Created` is a genuinely new value; `Reobserved`
/// re-confirms an identical already-committed value (idempotent re-extraction —
/// counted as skipped, never produced); `Divergent` re-observes the slot with a
/// *different* value than the stored one (never silently overwritten — reported
/// for ratification); `NoDefinition` is a non-catalog key the pipeline should not
/// emit (defensive skip).
#[derive(Debug, Clone)]
pub enum StructuredFactCommit {
    Created(String),
    Reobserved(String),
    Divergent {
        fact_id: String,
        metric_key: String,
        existing: String,
        incoming: String,
    },
    NoDefinition,
}

pub(crate) fn record_structured_fact(
    connection: &Connection,
    input: StructuredFactInput<'_>,
) -> StorageResult<StructuredFactCommit> {
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
        return Ok(StructuredFactCommit::NoDefinition);
    };

    // Slot-aware write: a re-extraction of a period whose facts already landed
    // re-observes each slot instead of raising its UNIQUE violation (owner T7).
    let fact = match create_or_reobserve_financial_fact(
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
            // Deterministic structured extraction, not an AI read. The specific
            // deterministic mechanism (`api` vs `html_positional`) is the caller's.
            extraction_method: Some(input.extraction_method.to_owned()),
            confidence: None,
            confirmation_state: Some(input.confirmation_state.to_owned()),
            supersedes_id: None,
            source_document_ref: Some(input.report_document_id.to_owned()),
        },
    )? {
        FactWriteOutcome::Created(fact) => fact,
        // Same slot, same value — idempotent re-observation. Provenance already
        // records the original tier/validation; leave it untouched.
        FactWriteOutcome::Reobserved(existing) => {
            return Ok(StructuredFactCommit::Reobserved(existing.id));
        }
        // Same slot, different value — never silently overwrite a stored
        // (possibly confirmed) fact. Skip + report for ratification.
        FactWriteOutcome::Divergent { existing, incoming } => {
            return Ok(StructuredFactCommit::Divergent {
                fact_id: existing.id,
                metric_key: input.metric_key.to_owned(),
                existing: existing.value_numeric,
                incoming,
            });
        }
    };

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

    Ok(StructuredFactCommit::Created(fact.id))
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
        committed_fact_count: row.get(18)?,
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
    ) -> StorageResult<ConfirmedKpiFact> {
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
    /// tier + validation verdict + citation) — all in one transaction. Returns a
    /// [`StructuredFactCommit`]: `Created` for a new value, `Reobserved`/`Divergent`
    /// when the slot is already occupied (idempotent re-extraction, never a UNIQUE
    /// violation), or `NoDefinition` when the metric has no catalog definition.
    pub fn record_structured_fact(
        &self,
        input: StructuredFactInput<'_>,
    ) -> StorageResult<StructuredFactCommit> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = record_structured_fact(&tx, input)?;
        tx.commit()?;
        Ok(result)
    }

    pub fn reject_kpi_proposal(&self, proposal_id: &str) -> StorageResult<KpiExtractionProposal> {
        let connection = self.db.checkout()?;

        reject_kpi_proposal(&connection, proposal_id)
    }

    /// Pending-proposal counts per detected `(fiscal_year, period_type)` for a
    /// company — the review axis of the coverage read model (ADR 0077 §2). Only
    /// `pending` proposals count, and only jobs whose document detected a period
    /// (both `detected_fiscal_year` and `detected_period_type` present) — a job
    /// with no detected period cannot be keyed to a coverage row.
    pub fn pending_proposals_by_period(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<PeriodPendingProposals>> {
        let connection = self.db.checkout()?;

        pending_proposals_by_period(&connection, company_id)
    }

    /// Every `pending` proposal for a company with its period + document context,
    /// newest period first — the F5 review-queue read model (ADR 0077 §4/§5).
    pub fn list_pending_kpi_proposals(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<PendingKpiProposal>> {
        let connection = self.db.checkout()?;

        list_pending_kpi_proposals(&connection, company_id)
    }
}

/// One `(fiscal_year, period_type)` bucket of pending-proposal counts for the
/// coverage read model. Not an IPC DTO — the coverage command folds it into
/// `CoverageReviewCell.pending_proposals`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodPendingProposals {
    pub fiscal_year: i64,
    pub period_type: String,
    pub pending: i64,
}

fn pending_proposals_by_period(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<PeriodPendingProposals>> {
    let mut statement = connection.prepare(
        "SELECT j.detected_fiscal_year, j.detected_period_type, COUNT(*) AS pending
         FROM kpi_extraction_proposals pr
         JOIN kpi_extraction_jobs j ON j.id = pr.job_id
         WHERE j.company_id = ?1
           AND pr.status = 'pending'
           AND j.detected_fiscal_year IS NOT NULL
           AND j.detected_period_type IS NOT NULL
         GROUP BY j.detected_fiscal_year, j.detected_period_type",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok(PeriodPendingProposals {
            fiscal_year: row.get(0)?,
            period_type: row.get(1)?,
            pending: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Every `pending` proposal for a company joined to its job's detected period
/// and its source document (id/title/url) — the F5 review-queue read model
/// (ADR 0077 §4/§5). Newest period first; jobs with no detected period sort
/// last (they cannot key to a period group). Confirmed/rejected proposals are
/// excluded (confirmed ones own facts, rejected ones are discarded).
fn list_pending_kpi_proposals(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<PendingKpiProposal>> {
    let mut statement = connection.prepare(
        "SELECT pr.id, pr.job_id, pr.metric_key, pr.label, pr.value_numeric, pr.unit,
                pr.currency, pr.source_snippet, pr.status,
                j.detected_fiscal_year, j.detected_period_type,
                j.report_document_id, d.title, d.url,
                pr.created_at, pr.updated_at
         FROM kpi_extraction_proposals pr
         JOIN kpi_extraction_jobs j ON j.id = pr.job_id
         JOIN report_documents d ON d.id = j.report_document_id
         WHERE j.company_id = ?1 AND pr.status = 'pending'
         ORDER BY j.detected_fiscal_year DESC, pr.metric_key COLLATE NOCASE",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok(PendingKpiProposal {
            id: row.get(0)?,
            job_id: row.get(1)?,
            metric_key: row.get(2)?,
            label: row.get(3)?,
            value_numeric: row.get(4)?,
            unit: row.get(5)?,
            currency: row.get(6)?,
            source_snippet: row.get(7)?,
            status: row.get(8)?,
            fiscal_year: row.get(9)?,
            period_type: row.get(10)?,
            document_id: row.get(11)?,
            document_title: row.get(12)?,
            document_url: row.get(13)?,
            created_at: row.get(14)?,
            updated_at: row.get(15)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::open_in_memory_database;

    /// The id of a freshly-created fact, asserting the commit was a new write
    /// (not a re-observation) — the shape these unit tests exercise.
    fn created_id(commit: StructuredFactCommit) -> String {
        match commit {
            StructuredFactCommit::Created(id) => id,
            other => panic!("expected a newly created fact, got {other:?}"),
        }
    }

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
                committed_fact_count: 0,
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

    /// Seeds a pending proposal for an arbitrary canonical `metric_key`/`value`
    /// (2025 FY period), so the T4.4 confirm-validation tests can drive a
    /// balance-sheet total through the real proposal→fact→validate plumbing.
    fn seed_pending_proposal(
        connection: &mut Connection,
        company_id: &str,
        document_id: &str,
        metric_key: &str,
        label: &str,
        value: &str,
    ) -> KpiExtractionProposal {
        seed_pending_proposal_with_snippet(
            connection,
            company_id,
            document_id,
            metric_key,
            label,
            value,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn seed_pending_proposal_with_snippet(
        connection: &mut Connection,
        company_id: &str,
        document_id: &str,
        metric_key: &str,
        label: &str,
        value: &str,
        source_snippet: Option<&str>,
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
                committed_fact_count: 0,
                proposals: vec![NewKpiProposal {
                    metric_key: metric_key.to_owned(),
                    label: label.to_owned(),
                    value_numeric: value.to_owned(),
                    unit: None,
                    currency: Some("PLN".to_owned()),
                    as_reported_value: None,
                    as_reported_scale: None,
                    measure_window: None,
                    confidence: Some("high".to_owned()),
                    source_snippet: source_snippet.map(str::to_owned),
                    is_proposed_kpi: false,
                }],
            },
        )
        .expect("job completed");
        job.proposals
            .into_iter()
            .find(|p| p.metric_key == metric_key)
            .unwrap_or_else(|| panic!("{metric_key} proposal"))
    }

    /// Seeds an already-confirmed fact for `metric_key` in the 2025 FY period,
    /// so a subsequent confirm assembles a multi-fact period the balance-sheet
    /// identity can actually evaluate.
    fn seed_period_fact(
        connection: &Connection,
        company_id: &str,
        document_id: &str,
        metric_key: &str,
        value: &str,
    ) {
        let period_id = ensure_period(
            connection,
            company_id,
            2025,
            "FY",
            Some("2025-12-31"),
            document_id,
        )
        .expect("ensure period");
        let definition_id = resolve_definition_by_metric_key(connection, company_id, metric_key)
            .expect("definition query")
            .unwrap_or_else(|| panic!("{metric_key} must exist in the canonical catalog"));
        create_or_reobserve_financial_fact(
            connection,
            NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id,
                definition_id,
                value_numeric: value.to_owned(),
                currency: Some("PLN".to_owned()),
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: Some("confirmed".to_owned()),
                supersedes_id: None,
                source_document_ref: None,
            },
        )
        .expect("seed period fact");
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
    /// ADR 0077 T4.4 (approved contract change): the confirm path now *validates*
    /// — a lone revenue value has no identity to check, so the status is the
    /// honest `unreviewed`, never the retired `none`.
    #[test]
    fn confirming_a_proposal_records_ai_provenance() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let proposal = seed_pending_revenue_proposal(&mut connection, &company_id, &document_id);

        let confirmed = confirm_kpi_proposal(
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

        assert_eq!(confirmed.validation_status, "unreviewed");
        let (source_tier, validation_status, drift_json, citation) =
            fact_provenance_row(&connection, &confirmed.fact.id)
                .expect("a confirmed AI proposal must carry a provenance row");
        assert_eq!(source_tier, "ai");
        assert_eq!(validation_status, "unreviewed");
        assert_eq!(drift_json, None);
        assert_eq!(citation.as_deref(), Some(proposal.label.as_str()));
    }

    /// Same guardrail on the autopilot auto-confirm path (ADR 0055): the
    /// `auto_unreviewed` confirmation state must not skip provenance either.
    /// ADR 0077 T4.4: it validates identically — a lone value is `unreviewed`.
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
        assert_eq!(validation_status, "unreviewed");
        assert_eq!(drift_json, None);
        assert_eq!(citation.as_deref(), Some(proposal.label.as_str()));
    }

    /// ADR 0077 T4.4: confirming a total that satisfies the balance-sheet
    /// identity over the period's fact set records a real `passed` status —
    /// AI-confirmed facts are validated like every other source.
    #[test]
    fn confirming_a_balance_sheet_consistent_total_records_passed() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        // Liabilities + equity already stored for the same (2025, FY) period.
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_liabilities",
            "100000000",
        );
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_equity",
            "200000000",
        );
        // Confirm the matching total: 300 == 100 + 200 → balance sheet passes.
        let proposal = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Total assets",
            "300000000",
        );

        let confirmed = confirm_kpi_proposal(
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

        assert_eq!(confirmed.validation_status, "passed");
        let (_, validation_status, _, _) = fact_provenance_row(&connection, &confirmed.fact.id)
            .expect("provenance row must exist");
        assert_eq!(validation_status, "passed");
    }

    /// ADR 0077 T4.4 (the spike's repainted-total case): confirming a total that
    /// violates the balance-sheet identity records `flagged`, and the flag is
    /// surfaced in the command's return value. The fact still persists — the
    /// user explicitly confirmed it; the status records what validation *saw*.
    #[test]
    fn confirming_a_balance_sheet_violating_total_records_flagged() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_liabilities",
            "100000000",
        );
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_equity",
            "200000000",
        );
        // 999 != 100 + 200 → balance-sheet identity fails.
        let proposal = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Total assets",
            "999000000",
        );

        let confirmed = confirm_kpi_proposal(
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

        assert_eq!(confirmed.validation_status, "flagged");
        // The fact persists regardless — the confirm is never blocked.
        assert_eq!(confirmed.fact.value_numeric.trim(), "999000000");
        let (_, validation_status, _, _) = fact_provenance_row(&connection, &confirmed.fact.id)
            .expect("provenance row must exist");
        assert_eq!(validation_status, "flagged");
    }

    /// ADR 0077 T4.4: a confirm with no other facts in the period has no
    /// identity to evaluate → `unreviewed` (never `none`).
    #[test]
    fn confirming_with_no_other_facts_records_unreviewed() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let proposal = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Total assets",
            "300000000",
        );

        let confirmed = confirm_kpi_proposal(
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

        assert_eq!(confirmed.validation_status, "unreviewed");
    }

    /// ADR 0077 T4.4: the autopilot auto-confirm path maps the verdict the same
    /// way — a balance-sheet-consistent total records `passed`.
    #[test]
    fn auto_confirm_maps_validation_status_from_the_verdict() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_liabilities",
            "100000000",
        );
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_equity",
            "200000000",
        );
        let proposal = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Total assets",
            "300000000",
        );

        let fact = auto_confirm_kpi_proposal(&connection, &proposal.id).expect("auto-confirm");
        let (_, validation_status, _, _) =
            fact_provenance_row(&connection, &fact.id).expect("provenance row must exist");
        assert_eq!(validation_status, "passed");
    }

    /// ADR 0077 §4 kickoff decision 3: confirming a tier-4 OCR proposal (the
    /// `ocr_bootstrap`/`ocr_pending_profile` snippet marker) is what CONFIRMS
    /// the company's bootstrap profile — the version-1 row bumps to 2 (both the
    /// column and the serialized profile_json), and only then does tier-4 start
    /// committing facts. A non-OCR proposal must not touch the profile.
    #[test]
    fn confirming_an_ocr_bootstrap_proposal_confirms_the_profile() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let profile = crate::fundamentals::extraction::ocr::OcrExtractionProfile::bootstrap(
            &company_id,
            crate::fundamentals::extraction::pdf::UnitScale::Thousands,
            std::collections::BTreeMap::from([(
                "aktywa razem".to_owned(),
                "total_assets".to_owned(),
            )]),
            crate::fundamentals::extraction::ocr::ValueColumnLayout::CurrentPeriodFirst,
            Vec::new(),
            false,
        );
        connection
            .execute(
                "INSERT INTO company_ocr_extraction_profile
                    (company_id, template_hash, scale, profile_json, version)
                 VALUES (?1, ?2, 'Thousands', ?3, 1)",
                params![
                    company_id,
                    profile.template_hash,
                    serde_json::to_string(&profile).expect("profile json"),
                ],
            )
            .expect("seed v1 profile");

        // A non-OCR confirm first: the profile must stay v1.
        let plain = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "revenue",
            "Przychody",
            "1000000",
        );
        confirm_kpi_proposal(
            &connection,
            ConfirmKpiProposalInput {
                proposal_id: plain.id,
                value_numeric: None,
                currency: None,
                fiscal_year: None,
                period_type: None,
                period_end_date: None,
                accept_as_new_kpi: false,
            },
        )
        .expect("confirm plain proposal");
        let version: i64 = connection
            .query_row(
                "SELECT version FROM company_ocr_extraction_profile WHERE company_id = ?1",
                [&company_id],
                |row| row.get(0),
            )
            .expect("profile row");
        assert_eq!(version, 1, "a non-OCR confirm must not confirm the profile");

        // Confirming an OCR-bootstrap proposal confirms the profile.
        let ocr = seed_pending_proposal_with_snippet(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Aktywa razem",
            "45000000",
            Some("ocr_bootstrap: Aktywa razem"),
        );
        confirm_kpi_proposal(
            &connection,
            ConfirmKpiProposalInput {
                proposal_id: ocr.id,
                value_numeric: None,
                currency: None,
                fiscal_year: None,
                period_type: None,
                period_end_date: None,
                accept_as_new_kpi: false,
            },
        )
        .expect("confirm ocr proposal");

        let (version, json): (i64, String) = connection
            .query_row(
                "SELECT version, profile_json FROM company_ocr_extraction_profile
                 WHERE company_id = ?1",
                [&company_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("profile row");
        assert_eq!(version, 2, "the OCR confirm must bump the profile version");
        let stored: crate::fundamentals::extraction::ocr::OcrExtractionProfile =
            serde_json::from_str(&json).expect("stored profile parses");
        assert_eq!(stored.version, 2, "profile_json must stay in sync");
    }

    /// G-1 class-closer (ADR 0077): exercise every production write path —
    /// manual confirm, autopilot auto-confirm, and a structured-pipeline
    /// emission — in one DB and assert not a single provenance row carries the
    /// retired `validation_status='none'`.
    #[test]
    fn no_production_path_writes_validation_status_none() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        // Manual confirm.
        let manual = seed_pending_revenue_proposal(&mut connection, &company_id, &document_id);
        confirm_kpi_proposal(
            &connection,
            ConfirmKpiProposalInput {
                proposal_id: manual.id,
                value_numeric: None,
                currency: None,
                fiscal_year: None,
                period_type: None,
                period_end_date: None,
                accept_as_new_kpi: false,
            },
        )
        .expect("manual confirm");

        // Autopilot auto-confirm.
        let auto = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Total assets",
            "300000000",
        );
        auto_confirm_kpi_proposal(&connection, &auto.id).expect("auto-confirm");

        // Structured-pipeline emission (a different period, avoiding slot reuse).
        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2024,
                period_type: "FY",
                period_end: Some("2024-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "1000000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "pdf",
                extraction_method: "api",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Revenue"),
            },
        )
        .expect("structured emit");

        // Tier-3b positional emission (ADR 0077 T-B2): the pdf2htmlEX render path
        // persists under `source_tier='pdf'` with `extraction_method='html_positional'`
        // and a real validation status — it must clear the G-1 no-`none` guard too.
        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2023,
                period_type: "FY",
                period_end: Some("2023-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "900000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "pdf",
                extraction_method: "html_positional",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Sales revenue"),
            },
        )
        .expect("positional emit");

        // The positional path's provenance is identifiable and honest.
        let positional_method: String = connection
            .query_row(
                "SELECT f.extraction_method FROM financial_facts f \
                 JOIN financial_fact_provenance p ON p.fact_id = f.id \
                 WHERE p.source_tier = 'pdf' AND f.extraction_method = 'html_positional'",
                [],
                |row| row.get(0),
            )
            .expect("the positional fact carries an identifiable extraction_method");
        assert_eq!(positional_method, "html_positional");

        let none_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM financial_fact_provenance WHERE validation_status = 'none'",
                [],
                |row| row.get(0),
            )
            .expect("count query");
        assert_eq!(
            none_count, 0,
            "no production path may write validation_status='none' (ADR 0077 G-1)"
        );
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

        let id = created_id(
            record_structured_fact(
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
                    extraction_method: "api",
                    validation_status: "flagged",
                    drift_json: Some(drift),
                    citation: Some("Przychody netto ze sprzedazy"),
                },
            )
            .expect("record structured fact"),
        );

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

        let id = created_id(
            record_structured_fact(
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
                    extraction_method: "api",
                    validation_status: "flagged",
                    drift_json: Some(
                        r#"{"addedLabels":[],"removedLabels":["x"],"unitChanged":null}"#,
                    ),
                    citation: Some("Przychody"),
                },
            )
            .expect("record structured fact"),
        );

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

    /// F5 review-queue read model (ADR 0077 §4/§5, T5.3b): a company's job with a
    /// detected period carries 4 proposals — 2 pending, 1 confirmed, 1 rejected.
    /// `list_pending_kpi_proposals` returns exactly the 2 pending ones, each
    /// stamped with the job's detected period, the source document's id/title/url,
    /// and its `source_snippet` verbatim (the UI derives the source chip from it).
    #[test]
    fn list_pending_kpi_proposals_returns_only_pending_with_period_and_document_context() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        // Give the document a title so the read model can pass it through.
        connection
            .execute(
                "UPDATE report_documents SET title = ?2 WHERE id = ?1",
                params![document_id, "CBF 2025 Q3 raport kwartalny"],
            )
            .expect("set document title");

        let job = create_kpi_extraction_job(
            &connection,
            NewKpiExtractionJob {
                company_id: company_id.clone(),
                report_document_id: document_id.clone(),
                provider_id: "test-sample".to_owned(),
                model: "test-sample-model".to_owned(),
                prompt_version: "kpi-extraction.v1".to_owned(),
                period_hint: None,
            },
        )
        .expect("job");

        let proposal = |metric: &str, label: &str, snippet: &str| NewKpiProposal {
            metric_key: metric.to_owned(),
            label: label.to_owned(),
            value_numeric: "120400000".to_owned(),
            unit: None,
            currency: Some("PLN".to_owned()),
            as_reported_value: None,
            as_reported_scale: None,
            measure_window: None,
            confidence: Some("high".to_owned()),
            source_snippet: Some(snippet.to_owned()),
            is_proposed_kpi: false,
        };

        let job = complete_kpi_extraction_job(
            &mut connection,
            CompletedKpiExtraction {
                job_id: job.id,
                detected_fiscal_year: Some(2025),
                detected_period_type: Some("Q3".to_owned()),
                detected_period_end_date: Some("2025-09-30".to_owned()),
                detected_currency: Some("PLN".to_owned()),
                detected_language: Some("pl".to_owned()),
                committed_fact_count: 0,
                proposals: vec![
                    proposal(
                        "revenue",
                        "Przychody ze sprzedaży",
                        "ocr_bootstrap: przychody",
                    ),
                    proposal("net_profit", "Zysk netto", "ocr_bootstrap: zysk netto"),
                    proposal(
                        "total_liabilities",
                        "Zobowiązania razem",
                        "ocr_flagged: zobowiązania",
                    ),
                    proposal("ebitda", "EBITDA", "ebitda"),
                ],
            },
        )
        .expect("complete job");

        // Reject one, confirm another (direct status update to isolate the read
        // model's status filter from the confirm-validation machinery). Reference
        // by metric_key — `complete_kpi_extraction_job` returns proposals sorted.
        let by_metric = |metric: &str| {
            job.proposals
                .iter()
                .find(|p| p.metric_key == metric)
                .unwrap_or_else(|| panic!("{metric} proposal"))
                .id
                .clone()
        };
        reject_kpi_proposal(&connection, &by_metric("total_liabilities")).expect("reject");
        connection
            .execute(
                "UPDATE kpi_extraction_proposals SET status = 'confirmed' WHERE id = ?1",
                [&by_metric("ebitda")],
            )
            .expect("confirm one");

        let pending = list_pending_kpi_proposals(&connection, &company_id).expect("read model");
        assert_eq!(pending.len(), 2, "only the 2 still-pending proposals");
        let metrics: Vec<&str> = pending.iter().map(|p| p.metric_key.as_str()).collect();
        assert!(metrics.contains(&"revenue"));
        assert!(metrics.contains(&"net_profit"));

        let revenue = pending
            .iter()
            .find(|p| p.metric_key == "revenue")
            .expect("revenue proposal");
        assert_eq!(revenue.status, "pending");
        assert_eq!(revenue.fiscal_year, Some(2025));
        assert_eq!(revenue.period_type.as_deref(), Some("Q3"));
        assert_eq!(revenue.document_id, document_id);
        assert_eq!(
            revenue.document_title.as_deref(),
            Some("CBF 2025 Q3 raport kwartalny")
        );
        assert_eq!(revenue.document_url, "https://x/doc1.pdf");
        assert_eq!(
            revenue.source_snippet.as_deref(),
            Some("ocr_bootstrap: przychody")
        );
    }

    /// The defect fix (owner live find, 2026-07-10): confirming a proposal whose
    /// slot is ALREADY occupied by an identical fact re-observes it — the proposal
    /// links to the EXISTING fact id, no new `financial_facts` row is written, the
    /// real validation status is returned, and (for an OCR-bootstrap proposal) the
    /// profile is still confirmed. Mirrors the extraction path's re-observation.
    #[test]
    fn confirming_a_reobserved_identical_fact_links_the_existing_row() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        // A v1 (unconfirmed bootstrap) OCR profile, so the OCR-bootstrap confirm
        // has a profile to bump.
        let profile = crate::fundamentals::extraction::ocr::OcrExtractionProfile::bootstrap(
            &company_id,
            crate::fundamentals::extraction::pdf::UnitScale::Thousands,
            std::collections::BTreeMap::from([(
                "aktywa razem".to_owned(),
                "total_assets".to_owned(),
            )]),
            crate::fundamentals::extraction::ocr::ValueColumnLayout::CurrentPeriodFirst,
            Vec::new(),
            false,
        );
        connection
            .execute(
                "INSERT INTO company_ocr_extraction_profile
                    (company_id, template_hash, scale, profile_json, version)
                 VALUES (?1, ?2, 'Thousands', ?3, 1)",
                params![
                    company_id,
                    profile.template_hash,
                    serde_json::to_string(&profile).expect("profile json"),
                ],
            )
            .expect("seed v1 profile");

        // A later tier-4 profile run already committed total_assets for 2025 FY.
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_assets",
            "45000000",
        );
        let existing_id: String = connection
            .query_row("SELECT id FROM financial_facts", [], |row| row.get(0))
            .expect("existing fact id");
        let facts_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
            .expect("count facts");

        // A leftover OCR-bootstrap proposal for the same slot, same value.
        let proposal = seed_pending_proposal_with_snippet(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Aktywa razem",
            "45000000",
            Some("ocr_bootstrap: Aktywa razem"),
        );

        let confirmed = confirm_kpi_proposal(
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
        .expect("confirm re-observed proposal");

        // No new row; the confirmed fact IS the existing one, with its real status.
        let facts_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
            .expect("count facts");
        assert_eq!(
            facts_after, facts_before,
            "re-observation must not add a row"
        );
        assert_eq!(confirmed.fact.id, existing_id, "links the existing fact id");
        assert_eq!(confirmed.fact.value_numeric.trim(), "45000000");
        // Honest validation over the period's fact set (a lone total → unreviewed).
        assert_eq!(confirmed.validation_status, "unreviewed");

        // The proposal is confirmed and points at the existing fact.
        let stored = get_proposal(&connection, &proposal.id).expect("proposal");
        assert_eq!(stored.status, "confirmed");
        assert_eq!(stored.fact_id.as_deref(), Some(existing_id.as_str()));

        // The OCR-bootstrap confirm still bumps the profile to v2 — re-observing a
        // committed value is still a human confirmation of the profile.
        let version: i64 = connection
            .query_row(
                "SELECT version FROM company_ocr_extraction_profile WHERE company_id = ?1",
                [&company_id],
                |row| row.get(0),
            )
            .expect("profile row");
        assert_eq!(
            version, 2,
            "a re-observed OCR confirm still confirms the profile"
        );
    }

    /// The defect fix: confirming a proposal whose slot already holds a DIFFERENT
    /// value returns a typed `value_conflict` error carrying both values, writes
    /// nothing, and leaves the proposal pending — the user decides. Never a raw
    /// sqlite UNIQUE error, never a silent overwrite of possibly-confirmed data.
    #[test]
    fn confirming_a_conflicting_fact_errors_without_writing() {
        let mut connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        seed_period_fact(
            &connection,
            &company_id,
            &document_id,
            "total_assets",
            "300000000",
        );
        let facts_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
            .expect("count facts");

        let proposal = seed_pending_proposal(
            &mut connection,
            &company_id,
            &document_id,
            "total_assets",
            "Total assets",
            "999000000",
        );

        let err = confirm_kpi_proposal(
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
        .expect_err("a slot conflict must be a typed error, not a raw sqlite panic");
        match err {
            StorageError::InvalidFinancialsValue { key, value } => {
                assert_eq!(key, "value_conflict");
                assert!(
                    value.contains("300000000"),
                    "carries the stored value: {value}"
                );
                assert!(
                    value.contains("999000000"),
                    "carries the proposed value: {value}"
                );
            }
            other => panic!("expected a value_conflict error, got {other:?}"),
        }

        // Nothing written; the proposal stays pending; the stored value is intact.
        let facts_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
            .expect("count facts");
        assert_eq!(
            facts_after, facts_before,
            "a conflict must not write a fact"
        );
        let stored = get_proposal(&connection, &proposal.id).expect("proposal");
        assert_eq!(
            stored.status, "pending",
            "a conflicting proposal stays pending"
        );
        assert_eq!(stored.fact_id, None);
        let value: String = connection
            .query_row("SELECT value_numeric FROM financial_facts", [], |row| {
                row.get(0)
            })
            .expect("stored value");
        assert_eq!(
            value.trim(),
            "300000000",
            "the stored fact is never overwritten"
        );
    }
}
