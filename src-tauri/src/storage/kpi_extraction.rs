//! Storage for AI KPI extraction (v0.36.0, epic 9879941).
//!
//! Extraction never writes facts directly. The async job persists PROPOSALS; only
//! an explicit user confirmation materialises a `financial_fact`. Confirmed
//! proposals are retained as the provenance trail (which job/provider/model/prompt
//! produced the value, and the verbatim source snippet).

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::financials::{
    create_or_reobserve_financial_fact, update_financial_fact, FactWriteOutcome,
    UpdateFinancialFact,
};
use super::fundamentals_provenance::fact_source_tier;
use super::{slug_part, FinancialFact, NewFinancialFact, StorageResult};

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
/// Auto-confirm a proposal on the autopilot path (North Star, v0.49.0 / ADR 0055):
/// commit the model-detected value as a fact in the **`auto_unreviewed`**
/// provenance state — cited, flagged, and reversible — using the job's detected
/// period (no user overrides). The global confirm-before-commit default is
/// unchanged; this only runs for a company explicitly opted into `autopilot`.
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
    /// A HIGHER tier took over a lower-tier slot (ADR 0086 decision 3): the
    /// provenance (and, on a disagreement, the value) now belong to the incoming
    /// tier. `previous_value` is `Some` when the stored value was overwritten,
    /// `None` when the tiers agreed and only the label/evidence moved.
    Upgraded {
        fact_id: String,
        previous_value: Option<String>,
        /// The tier the slot carried before the takeover (for upgrade evidence).
        previous_tier: String,
    },
    Divergent {
        fact_id: String,
        metric_key: String,
        existing: String,
        incoming: String,
    },
    NoDefinition,
}

/// The STORED fact's tier when the incoming tier strictly outranks it (ADR 0086
/// decision 3), `None` otherwise. A fact with no provenance row is a manual
/// entry — untouchable by every automatic path — and an unparsable stored tier
/// is treated the same.
fn outranked_stored_tier_of(
    connection: &Connection,
    fact_id: &str,
    incoming_tier: &str,
) -> StorageResult<Option<String>> {
    use crate::fundamentals::extraction::SourceTier;
    let Some(stored) = fact_source_tier(connection, fact_id)? else {
        return Ok(None);
    };
    let (Some(stored_tier), Some(incoming)) =
        (SourceTier::parse(&stored), SourceTier::parse(incoming_tier))
    else {
        return Ok(None);
    };
    Ok(incoming.outranks(stored_tier).then_some(stored))
}

/// Shared prologue for both fact-recording paths ([`record_structured_fact`] and
/// [`record_aggregator_fact`]): ensure the period, resolve the catalog definition,
/// build the ~20-field [`NewFinancialFact`] identically, and run the slot-aware
/// write. `Ok(None)` means the metric has no catalog definition (a defensive skip
/// the caller maps to its own `NoDefinition` variant). Everything downstream —
/// the divergent precedence policy — is the caller's, so this is the ONE place the
/// two paths share their fact construction (byte-identical before this extraction).
fn prepare_fact_write(
    connection: &Connection,
    input: &StructuredFactInput<'_>,
) -> StorageResult<Option<FactWriteOutcome>> {
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
        // Not a catalog metric — both pipelines only emit canonical keys, so this
        // is a defensive skip, never a silent bad write.
        return Ok(None);
    };

    // Slot-aware write: a re-extraction of a period whose facts already landed
    // re-observes each slot instead of raising its UNIQUE violation (owner T7).
    let outcome = create_or_reobserve_financial_fact(
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
            // The specific deterministic mechanism (`api` vs `html_positional`) is
            // the caller's — never an AI read.
            extraction_method: Some(input.extraction_method.to_owned()),
            confidence: None,
            confirmation_state: Some(input.confirmation_state.to_owned()),
            supersedes_id: None,
            source_document_ref: Some(input.report_document_id.to_owned()),
        },
    )?;
    Ok(Some(outcome))
}

pub(super) fn record_structured_fact(
    connection: &Connection,
    input: StructuredFactInput<'_>,
) -> StorageResult<StructuredFactCommit> {
    let Some(outcome) = prepare_fact_write(connection, &input)? else {
        return Ok(StructuredFactCommit::NoDefinition);
    };

    match outcome {
        FactWriteOutcome::Created(fact) => {
            write_fact_provenance(connection, &fact.id, &input)?;
            Ok(StructuredFactCommit::Created(fact.id))
        }
        // Same slot, same value. A HIGHER tier re-observing a lower-tier slot
        // takes the label/evidence over (ADR 0086 decision 3 — the fact is now
        // the issuer's filing, not the third party that agreed with it);
        // otherwise an idempotent re-observation leaves provenance untouched.
        FactWriteOutcome::Reobserved(existing) => {
            if let Some(previous_tier) =
                outranked_stored_tier_of(connection, &existing.id, input.source_tier)?
            {
                write_fact_provenance(connection, &existing.id, &input)?;
                return Ok(StructuredFactCommit::Upgraded {
                    fact_id: existing.id,
                    previous_value: None,
                    previous_tier,
                });
            }
            Ok(StructuredFactCommit::Reobserved(existing.id))
        }
        // Same slot, different value. A HIGHER tier overwrites a lower-tier slot
        // (ADR 0086 decision 3 — the issuer's number wins its own slot); between
        // peers, or against a manual/no-provenance fact, the stored value is
        // never silently overwritten — skip + report the divergence.
        FactWriteOutcome::Divergent { existing, incoming } => {
            if let Some(previous_tier) =
                outranked_stored_tier_of(connection, &existing.id, input.source_tier)?
            {
                update_financial_fact(
                    connection,
                    UpdateFinancialFact {
                        id: existing.id.clone(),
                        value_numeric: Some(incoming),
                        currency: input.currency.map(str::to_owned),
                        data_quality: None,
                        confirmation_state: Some(input.confirmation_state.to_owned()),
                        supersedes_id: None,
                        source_document_ref: Some(input.report_document_id.to_owned()),
                    },
                )?;
                write_fact_provenance(connection, &existing.id, &input)?;
                return Ok(StructuredFactCommit::Upgraded {
                    fact_id: existing.id,
                    previous_value: Some(existing.value_numeric),
                    previous_tier,
                });
            }
            Ok(StructuredFactCommit::Divergent {
                fact_id: existing.id,
                metric_key: input.metric_key.to_owned(),
                existing: existing.value_numeric,
                incoming,
            })
        }
    }
}

/// Outcome of committing one BiznesRadar-primary aggregator fact into its slot,
/// applying the ADR 0086 decision 3 precedence: `manual` > `esef` >
/// `espi_cover_note` > positional > `html_aggregator`. The aggregator only ever
/// overwrites its OWN (`html_aggregator`, non-manual) slot and NEVER a manual or
/// higher-tier fact.
#[derive(Debug, Clone)]
pub enum AggregatorFactCommit {
    /// The slot was empty — a new aggregator fact was written.
    Created(String),
    /// The slot held an aggregator fact with a different value — overwritten in
    /// place with the fresh aggregator value.
    Updated(String),
    /// The slot already held this exact value (aggregator's own or an agreeing
    /// higher tier) — no write, no change.
    Reobserved(String),
    /// The slot is held by a higher-precedence fact (manual / esef /
    /// structured_xhtml / espi_cover_note / positional) — left untouched. The
    /// caller decides whether the divergence warrants an informational
    /// `witness_disagreement` outcome (issuer tiers only).
    SkippedHigherTier {
        fact_id: String,
        /// The provenance `source_tier` of the holding fact, or `manual` when it
        /// has no provenance row (a hand-entered fact).
        existing_tier: String,
        existing_method: String,
        existing_value: String,
    },
    /// The metric has no catalog definition — a defensive skip.
    NoDefinition,
}

/// Persists one BiznesRadar-primary fact under the ADR 0086 tier precedence. The
/// `input` must carry `source_tier = html_aggregator`, `extraction_method = api`
/// and `confirmation_state = confirmed`. Unlike [`record_structured_fact`], this
/// OVERWRITES the aggregator's own occupied slot (BR re-observing its own figure
/// with a fresh value), while never touching a manual or higher-tier fact.
pub(super) fn record_aggregator_fact(
    connection: &Connection,
    input: StructuredFactInput<'_>,
) -> StorageResult<AggregatorFactCommit> {
    let Some(outcome) = prepare_fact_write(connection, &input)? else {
        return Ok(AggregatorFactCommit::NoDefinition);
    };
    apply_aggregator_precedence(connection, &input, outcome)
}

/// Applies the ADR 0086 decision-3 aggregator precedence to a slot-write outcome —
/// the divergent policy that is the aggregator path's own (the structured path's
/// `outranked_stored_tier_of` policy is the mirror). Shared by the single-fact
/// [`record_aggregator_fact`] and the batched [`record_aggregator_facts`] so the
/// precedence lives in exactly one place.
fn apply_aggregator_precedence(
    connection: &Connection,
    input: &StructuredFactInput<'_>,
    outcome: FactWriteOutcome,
) -> StorageResult<AggregatorFactCommit> {
    match outcome {
        FactWriteOutcome::Created(fact) => {
            write_fact_provenance(connection, &fact.id, input)?;
            Ok(AggregatorFactCommit::Created(fact.id))
        }
        // Slot holds this exact value already — aggregator's own row, or a higher
        // tier that happens to agree. Either way nothing to write and no conflict.
        FactWriteOutcome::Reobserved(existing) => Ok(AggregatorFactCommit::Reobserved(existing.id)),
        FactWriteOutcome::Divergent { existing, incoming } => {
            let existing_tier = fact_source_tier(connection, &existing.id)?;
            if aggregator_owns_slot(existing_tier.as_deref(), &existing.extraction_method) {
                // BR overwrites its OWN slot with the fresh aggregator value.
                update_financial_fact(
                    connection,
                    UpdateFinancialFact {
                        id: existing.id.clone(),
                        value_numeric: Some(incoming),
                        currency: input.currency.map(str::to_owned),
                        data_quality: None,
                        confirmation_state: Some(input.confirmation_state.to_owned()),
                        supersedes_id: None,
                        source_document_ref: Some(input.report_document_id.to_owned()),
                    },
                )?;
                write_fact_provenance(connection, &existing.id, input)?;
                Ok(AggregatorFactCommit::Updated(existing.id))
            } else {
                Ok(AggregatorFactCommit::SkippedHigherTier {
                    fact_id: existing.id,
                    existing_tier: existing_tier.unwrap_or_else(|| "manual".to_owned()),
                    existing_method: existing.extraction_method,
                    existing_value: existing.value_numeric,
                })
            }
        }
    }
}

/// Batched BiznesRadar-primary write for one `(company, page)`: the whole page's
/// facts under ONE transaction (the caller opens it), with `ensure_period`
/// resolved once per distinct period and a `metric_key → definition_id` cache
/// instead of a per-fact SELECT — the daily pull writes ~9k facts, so a
/// per-fact checkout+IMMEDIATE transaction was ~9k fsyncs a run.
///
/// Returns one [`AggregatorFactCommit`] per input, in order — **byte-identical**
/// to calling [`record_aggregator_fact`] per input (idempotent `ensure_period`
/// and a pure definition read make the caching invisible to the outcome). The
/// caller applies the reversed-witnessing / summary bookkeeping OUTSIDE this
/// transaction, exactly as the per-fact path did.
pub(super) fn record_aggregator_facts(
    connection: &Connection,
    inputs: &[StructuredFactInput<'_>],
) -> StorageResult<Vec<AggregatorFactCommit>> {
    // Resolve each distinct `(fiscal_year, period_type)` period once, and each
    // distinct `metric_key` definition once — the loop-invariant work
    // `prepare_fact_write` otherwise repeats per fact.
    use std::collections::HashMap;
    let mut period_by_key: HashMap<(i64, String), String> = HashMap::new();
    let mut definition_by_metric: HashMap<String, Option<String>> = HashMap::new();

    let mut commits = Vec::with_capacity(inputs.len());
    for input in inputs {
        let definition_id = match definition_by_metric.get(input.metric_key) {
            Some(cached) => cached.clone(),
            None => {
                let resolved = resolve_definition_by_metric_key(
                    connection,
                    input.company_id,
                    input.metric_key,
                )?;
                definition_by_metric.insert(input.metric_key.to_owned(), resolved.clone());
                resolved
            }
        };
        let Some(definition_id) = definition_id else {
            commits.push(AggregatorFactCommit::NoDefinition);
            continue;
        };

        let period_key = (input.fiscal_year, input.period_type.to_owned());
        let period_id = match period_by_key.get(&period_key) {
            Some(cached) => cached.clone(),
            None => {
                let resolved = ensure_period(
                    connection,
                    input.company_id,
                    input.fiscal_year,
                    input.period_type,
                    input.period_end,
                    input.report_document_id,
                )?;
                period_by_key.insert(period_key, resolved.clone());
                resolved
            }
        };

        let outcome = create_or_reobserve_financial_fact(
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
                extraction_method: Some(input.extraction_method.to_owned()),
                confidence: None,
                confirmation_state: Some(input.confirmation_state.to_owned()),
                supersedes_id: None,
                source_document_ref: Some(input.report_document_id.to_owned()),
            },
        )?;
        commits.push(apply_aggregator_precedence(connection, input, outcome)?);
    }
    Ok(commits)
}

/// Whether a slot's holding fact is the aggregator's OWN — i.e. the aggregator
/// may overwrite it. True only for a non-manual `html_aggregator` fact; a manual
/// fact (no provenance row → `None` tier, `extraction_method = 'manual'`) and
/// every issuer tier are untouchable (ADR 0086 decision 3).
fn aggregator_owns_slot(source_tier: Option<&str>, extraction_method: &str) -> bool {
    use crate::fundamentals::extraction::SourceTier;
    source_tier.and_then(SourceTier::parse) == Some(SourceTier::HtmlAggregator)
        && extraction_method != "manual"
}

/// Upserts the provenance row (tier + validation verdict + drift + citation) for
/// a fact — the ONE writer shared by both the structured
/// ([`record_structured_fact`]) and aggregator ([`record_aggregator_fact`]) paths
/// (they wrote byte-identical rows before this consolidation).
fn write_fact_provenance(
    connection: &Connection,
    fact_id: &str,
    input: &StructuredFactInput<'_>,
) -> StorageResult<()> {
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
            fact_id,
            input.source_tier,
            input.validation_status,
            input.drift_json,
            input.citation,
        ],
    )?;
    Ok(())
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

fn period_id(company_id: &str, fiscal_year: i64, period_type: &str) -> String {
    format!(
        "finper_{}_{}_{}",
        slug_part(company_id),
        fiscal_year,
        slug_part(period_type)
    )
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

    /// Persists one deterministically-extracted fact (ADR 0061): ensures the
    /// period, resolves the canonical KPI definition, writes the fact with the
    /// given confirmation state, and records its structured provenance (source
    /// tier + validation verdict + citation) — all in one transaction. Returns a
    /// [`StructuredFactCommit`]: `Created` for a new value, `Reobserved`/`Divergent`
    /// when the slot is already occupied (idempotent re-extraction, never a UNIQUE
    /// violation), or `NoDefinition` when the metric has no catalog definition.
    ///
    /// This is all that remains of the module: the KPI proposal/job ledger went
    /// with the in-app AI layer (ADR 0084 decision 5 — `kpi_extraction_jobs` and
    /// `kpi_extraction_proposals` are dropped), leaving only the deterministic
    /// fact-recording path the structured pipeline uses.
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

    /// Persists one BiznesRadar-primary aggregator fact under the ADR 0086 tier
    /// precedence (see [`record_aggregator_fact`]): writes an empty slot,
    /// overwrites the aggregator's OWN slot with a fresh value, and never touches
    /// a manual or higher-tier fact (the divergence is reported for the caller to
    /// log as an informational `witness_disagreement`).
    pub fn record_aggregator_fact(
        &self,
        input: StructuredFactInput<'_>,
    ) -> StorageResult<AggregatorFactCommit> {
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = record_aggregator_fact(&tx, input)?;
        tx.commit()?;
        Ok(result)
    }

    /// Persist a whole `(company, page)` batch of BiznesRadar-primary facts under
    /// ONE `IMMEDIATE` transaction (see [`record_aggregator_facts`]) — one
    /// checkout+commit for the page instead of one per fact. Returns one
    /// [`AggregatorFactCommit`] per input, in order; the daily pull applies the
    /// reversed-witnessing / summary bookkeeping to them outside this transaction.
    pub fn record_aggregator_facts(
        &self,
        inputs: &[StructuredFactInput<'_>],
    ) -> StorageResult<Vec<AggregatorFactCommit>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let mut connection = self.db.checkout()?;
        let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = record_aggregator_facts(&tx, inputs)?;
        tx.commit()?;
        Ok(result)
    }
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
    /// Seeds a pending proposal for an arbitrary canonical `metric_key`/`value`
    /// (2025 FY period), so the T4.4 confirm-validation tests can drive a
    /// balance-sheet total through the real proposal→fact→validate plumbing.
    #[allow(clippy::too_many_arguments)]
    /// Seeds an already-confirmed fact for `metric_key` in the 2025 FY period,
    /// so a subsequent confirm assembles a multi-fact period the balance-sheet
    /// identity can actually evaluate.
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
    /// G-1 class-closer (ADR 0077): exercise every production write path in one
    /// DB and assert not a single provenance row carries the retired
    /// `validation_status='none'`. Post-ADR-0084 the surviving write paths are
    /// the deterministic ones — the manual-confirm and autopilot auto-confirm
    /// proposal paths went with the KPI staging ledger — so this now pins the
    /// structured tier and the tier-3b positional tier.
    #[test]
    fn no_production_path_writes_validation_status_none() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        // Structured-pipeline emission.
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

    /// ADR 0086 decision 3: an issuer tier re-observing an aggregator-held slot
    /// takes the slot's LABEL over — the fact's evidence becomes the issuer's
    /// filing, not the third-party page that happened to agree with it.
    #[test]
    fn an_issuer_reobservation_upgrades_an_aggregator_slot_label() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
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
                source_tier: "html_aggregator",
                extraction_method: "api",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("https://biznesradar.example/page | Przychody"),
            },
        )
        .expect("aggregator write");

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
                source_tier: "esef",
                extraction_method: "api",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Revenue"),
            },
        )
        .expect("issuer re-observation");

        let (tier, citation): (String, String) = connection
            .query_row(
                "SELECT p.source_tier, p.citation FROM financial_fact_provenance p",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("provenance row");
        assert_eq!(
            tier, "esef",
            "the issuer tier must take over the slot label"
        );
        assert_eq!(citation, "Revenue", "the evidence must point at the filing");
    }

    /// ADR 0086 decision 3: an issuer tier DISAGREEING with an aggregator-held
    /// slot overwrites it — the issuer's number wins its own slot (the reverse
    /// of the old T7 stored-wins rule, which now applies only between peers).
    #[test]
    fn an_issuer_divergence_overwrites_an_aggregator_slot() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2024,
                period_type: "FY",
                period_end: Some("2024-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "999000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "html_aggregator",
                extraction_method: "api",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("https://biznesradar.example/page | Przychody"),
            },
        )
        .expect("aggregator write");

        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2024,
                period_type: "FY",
                period_end: Some("2024-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "1000123",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "esef",
                extraction_method: "api",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Revenue"),
            },
        )
        .expect("issuer divergence");

        let (value, tier): (String, String) = connection
            .query_row(
                "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
                 JOIN financial_fact_provenance p ON p.fact_id = f.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("fact row");
        assert_eq!(value, "1000123", "the issuer's number must win its slot");
        assert_eq!(tier, "esef");
    }

    /// A fact with NO provenance row is a manual entry — untouchable by every
    /// automatic path (ADR 0086 decision 3), including an issuer-tier write.
    #[test]
    fn a_manual_slot_is_never_upgraded_or_overwritten() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2024,
                period_type: "FY",
                period_end: Some("2024-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "555000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "html_aggregator",
                extraction_method: "manual",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("hand-entered"),
            },
        )
        .expect("seed");
        // Strip the provenance row — a hand-entered fact never gets one.
        connection
            .execute("DELETE FROM financial_fact_provenance", [])
            .expect("strip provenance");

        let commit = record_structured_fact(
            &connection,
            StructuredFactInput {
                company_id: &company_id,
                fiscal_year: 2024,
                period_type: "FY",
                period_end: Some("2024-12-31"),
                report_document_id: &document_id,
                metric_key: "revenue",
                value_numeric: "560000",
                currency: Some("PLN"),
                confirmation_state: "confirmed",
                source_tier: "esef",
                extraction_method: "api",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Revenue"),
            },
        )
        .expect("issuer write against a manual slot");

        assert!(
            matches!(commit, StructuredFactCommit::Divergent { .. }),
            "a manual slot is reported as a divergence, never overwritten: {commit:?}"
        );
        let value: String = connection
            .query_row("SELECT value_numeric FROM financial_facts", [], |row| {
                row.get(0)
            })
            .expect("fact");
        assert_eq!(value, "555000", "the hand-entered value must survive");
    }

    /// F4 (ADR 0086 perf): a multi-fact page writes through the ONE batched call,
    /// resolving each period once and returning a commit per input in order. The
    /// second identical batch re-observes every catalog slot — proof the batch
    /// wrote real rows as one logical unit (not per-fact best-effort).
    #[test]
    fn record_aggregator_facts_batches_a_page_in_order() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let make = |value: &'static str, metric: &'static str| StructuredFactInput {
            company_id: &company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: &document_id,
            metric_key: metric,
            value_numeric: value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "html_aggregator",
            extraction_method: "api",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("https://biznesradar.example/page | Przychody"),
        };
        // Two catalog facts sharing ONE period + a non-catalog key (order matters).
        let inputs = vec![
            make("1000000", "revenue"),
            make("2000000", "total_assets"),
            make("3", "definitely_not_a_catalog_metric"),
        ];

        let commits = record_aggregator_facts(&connection, &inputs).expect("batch write");
        assert_eq!(commits.len(), 3, "one commit per input, in order");
        assert!(matches!(commits[0], AggregatorFactCommit::Created(_)));
        assert!(matches!(commits[1], AggregatorFactCommit::Created(_)));
        assert!(matches!(commits[2], AggregatorFactCommit::NoDefinition));

        // Both catalog facts actually landed (the batch committed them together).
        let fact_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
            .expect("count");
        assert_eq!(fact_count, 2);

        let again = record_aggregator_facts(&connection, &inputs).expect("re-batch");
        assert!(matches!(again[0], AggregatorFactCommit::Reobserved(_)));
        assert!(matches!(again[1], AggregatorFactCommit::Reobserved(_)));
        assert!(matches!(again[2], AggregatorFactCommit::NoDefinition));
    }
}
