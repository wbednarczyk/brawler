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
pub(super) fn ensure_period(
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
    /// Slot dimension `total` (default) | `owners_of_parent` | `nci` (ADR 0093
    /// epic #285 T7). Every writer before the MCP agent batch tool passes
    /// `None` — `slot_dims` already defaults it to `total`, unchanged behavior.
    pub attribution: Option<&'a str>,
    /// Slot dimension `flow` (default) | `stock`. Every writer before the MCP
    /// agent batch tool passes `None` — `slot_dims` already defaults it to
    /// `flow`, unchanged behavior.
    pub measure_window: Option<&'a str>,
    /// `final` (default, `None` normalizes to it) | `preliminary` | `estimated`
    /// (ADR 0093 decision 2), normalized at the storage write boundary
    /// (`normalize_data_quality`). Every writer before the MCP agent tool (T7)
    /// passes `None` — no behavior change for the existing pipeline.
    pub data_quality: Option<&'a str>,
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

    // Every writer keeps the default basis (`None` → `slot_dims`'s
    // `or_default` resolves to 'consolidated') — ADR 0095.
    let statement_basis = None;

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
            statement_basis,
            attribution: input.attribution.map(str::to_owned),
            variant: None,
            measure_window: input.measure_window.map(str::to_owned),
            data_quality: input.data_quality.map(str::to_owned),
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
            annotation: None,
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
    apply_structured_precedence(
        connection,
        outcome,
        StructuredPrecedenceFields {
            metric_key: input.metric_key,
            currency: input.currency,
            confirmation_state: input.confirmation_state,
            source_tier: input.source_tier,
            extraction_method: input.extraction_method,
            validation_status: input.validation_status,
            drift_json: input.drift_json,
            citation: input.citation,
            report_document_id: input.report_document_id,
        },
    )
}

/// The fields [`apply_structured_precedence`] needs beyond the raw
/// `create_or_reobserve_financial_fact` outcome — everything [`StructuredFactInput`]
/// and #362's pinned primitive both carry, so the ladder itself stays input-shape-agnostic.
struct StructuredPrecedenceFields<'a> {
    metric_key: &'a str,
    currency: Option<&'a str>,
    confirmation_state: &'a str,
    source_tier: &'a str,
    extraction_method: &'a str,
    validation_status: &'a str,
    drift_json: Option<&'a str>,
    citation: Option<&'a str>,
    report_document_id: &'a str,
}

/// The shared post-resolver core (#362 F2 sol): given a slot-write outcome,
/// applies the structured-path precedence ladder (`outranked_stored_tier_of`
/// — manual > esef/espi_cover_note > agent > html_aggregator, ADR 0086
/// decision 3/ADR 0098 dec. 7) and stamps provenance. [`record_structured_fact`]
/// (public [`StructuredFactInput`]) and `record_pinned_fact` (#362's
/// manifest-pinned-definition primitive) share this ONE ladder — byte-identical
/// to the pre-#362 inline match, verified by the untouched `record_structured_fact`
/// suites.
fn apply_structured_precedence(
    connection: &Connection,
    outcome: FactWriteOutcome,
    fields: StructuredPrecedenceFields<'_>,
) -> StorageResult<StructuredFactCommit> {
    match outcome {
        FactWriteOutcome::Created(fact) => {
            write_fact_provenance_fields(
                connection,
                &fact.id,
                fields.source_tier,
                fields.extraction_method,
                fields.validation_status,
                fields.drift_json,
                fields.citation,
            )?;
            Ok(StructuredFactCommit::Created(fact.id))
        }
        // Same slot, same value. A HIGHER tier re-observing a lower-tier slot
        // takes the label/evidence over (ADR 0086 decision 3 — the fact is now
        // the issuer's filing, not the third party that agreed with it);
        // otherwise an idempotent re-observation leaves provenance untouched.
        FactWriteOutcome::Reobserved(existing) => {
            if let Some(previous_tier) =
                outranked_stored_tier_of(connection, &existing.id, fields.source_tier)?
            {
                // The VALUE already agreed
                // (that is what `Reobserved` means) but the lower tier's
                // write may have left metadata gaps a higher tier's write
                // now closes — merge SAFELY, never clobber. `currency` fills
                // in ONLY when the stored slot has none: only value_numeric
                // agreement is actually proven here, so an already-set
                // currency is left alone rather than overwritten by a value
                // that was never cross-checked. `source_document_ref` always
                // repoints to the new tier's own document — it is now this
                // slot's authoritative evidence, same as a value-changing
                // upgrade already does below.
                update_financial_fact(
                    connection,
                    UpdateFinancialFact {
                        id: existing.id.clone(),
                        value_numeric: None,
                        currency: existing
                            .currency
                            .is_none()
                            .then(|| fields.currency.map(str::to_owned))
                            .flatten(),
                        data_quality: None,
                        confirmation_state: None,
                        supersedes_id: None,
                        source_document_ref: Some(fields.report_document_id.to_owned()),
                        annotation: None,
                    },
                )?;
                // write_fact_provenance_fields syncs financial_facts.extraction_method
                // to fields.extraction_method in the SAME call (bug #324
                // class) — no separate sync needed.
                write_fact_provenance_fields(
                    connection,
                    &existing.id,
                    fields.source_tier,
                    fields.extraction_method,
                    fields.validation_status,
                    fields.drift_json,
                    fields.citation,
                )?;
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
                outranked_stored_tier_of(connection, &existing.id, fields.source_tier)?
            {
                update_financial_fact(
                    connection,
                    UpdateFinancialFact {
                        id: existing.id.clone(),
                        value_numeric: Some(incoming),
                        currency: fields.currency.map(str::to_owned),
                        data_quality: None,
                        confirmation_state: Some(fields.confirmation_state.to_owned()),
                        supersedes_id: None,
                        source_document_ref: Some(fields.report_document_id.to_owned()),
                        annotation: None,
                    },
                )?;
                // write_fact_provenance_fields syncs financial_facts.extraction_method
                // to fields.extraction_method in the SAME call (bug #324
                // class) — no separate sync needed.
                write_fact_provenance_fields(
                    connection,
                    &existing.id,
                    fields.source_tier,
                    fields.extraction_method,
                    fields.validation_status,
                    fields.drift_json,
                    fields.citation,
                )?;
                return Ok(StructuredFactCommit::Upgraded {
                    fact_id: existing.id,
                    previous_value: Some(existing.value_numeric),
                    previous_tier,
                });
            }
            Ok(StructuredFactCommit::Divergent {
                fact_id: existing.id,
                metric_key: fields.metric_key.to_owned(),
                existing: existing.value_numeric,
                incoming,
            })
        }
    }
}

/// Input for `record_pinned_fact` (#362): one manifest observation the commit
/// transaction consumes — the definition is PINNED (`manifest.definitionId`),
/// never re-resolved, unlike [`StructuredFactInput`]'s metric-key resolution.
pub(super) struct PinnedFactInput<'a> {
    /// The run id, carried only for error context (`PinnedDefinitionMissing`/
    /// `CorruptStoredManifest`).
    pub run_id: &'a str,
    pub company_id: &'a str,
    /// Resolved by the commit transaction's period step (#362 step 4) —
    /// never re-derived here.
    pub period_id: &'a str,
    pub definition_id: &'a str,
    /// The manifest observation's raw (untrimmed) `metricKey` candidate —
    /// compared trimmed against the pinned definition's own `metric_key`.
    pub metric_key: &'a str,
    pub value_numeric: &'a str,
    pub currency: Option<&'a str>,
    /// `obs.scope || run.scope` (ADR 0095 slot dimension) — the caller
    /// resolves the fallback; this primitive only validates the vocabulary.
    pub statement_basis: &'a str,
    pub attribution: &'a str,
    pub measure_window: Option<&'a str>,
    pub data_quality: &'a str,
    pub report_document_id: &'a str,
    /// `passed` | `unreviewed` (derived by the caller from the observation's
    /// `validationState` — a ready manifest never carries `flagged`).
    pub validation_status: &'a str,
    /// Canonical structural-locator JSON (`{"page":…,"table":…,"row":…,"quote":…}`).
    pub citation: Option<&'a str>,
}

/// Writes one manifest-pinned observation into its uniqueness slot (#362 step
/// 5): validates the pinned definition still exists, matches the manifest's
/// `metricKey`, and is still ELIGIBLE for `company_id` (mirrors
/// [`resolve_kpi_definition`]'s own WHERE acceptance — company-scoped must
/// name this company, sector-scoped must match either eligibility axis via
/// [`sector_definition_matches`], everything else must be company-unscoped —
/// without re-resolving), then
/// validates `statement_basis` vocabulary (unreachable from a real validator;
/// a raw-tampered stored manifest is the only path here — same defensive
/// class as [`SealedManifest::seal`]'s F4 finding). Shares
/// [`apply_structured_precedence`] with [`record_structured_fact`] — the ONE
/// ladder both inputs go through. Provenance is HARDCODED here, never
/// caller-supplied (ADR 0098 dec. 7, mirrors `jobs::record_financial_facts`'s
/// `agent`/`mcp_agent`/`confirmed` triple): `source_tier="agent"`,
/// `extraction_method="mcp_agent"`, `confirmation_state="confirmed"`,
/// `drift_json=None` (a manifest carries no drift signal).
pub(super) fn record_pinned_fact(
    connection: &Connection,
    input: PinnedFactInput<'_>,
) -> StorageResult<StructuredFactCommit> {
    let missing = || StorageError::PinnedDefinitionMissing {
        run: input.run_id.to_owned(),
        definition: input.definition_id.to_owned(),
    };
    let definition: Option<(String, Option<String>, Option<String>, String)> = connection
        .query_row(
            "SELECT scope, company_id, sector, metric_key FROM kpi_definitions WHERE id = ?1",
            [input.definition_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((scope, definition_company_id, definition_sector, metric_key)) = definition else {
        return Err(missing());
    };
    if metric_key != input.metric_key.trim() {
        return Err(missing());
    }
    let (company_sector, _source) =
        super::companies::get_company_sector(connection, input.company_id)?;
    let statement_type = super::companies::get_statement_type(connection, input.company_id)?;
    let eligible = match scope.as_str() {
        "company" => definition_company_id.as_deref() == Some(input.company_id),
        "sector" => sector_definition_matches(
            definition_sector.as_deref(),
            company_sector.as_deref(),
            &statement_type,
        ),
        _ => definition_company_id.is_none(),
    };
    if !eligible {
        return Err(missing());
    }
    if !matches!(input.statement_basis, "standalone" | "consolidated") {
        return Err(StorageError::CorruptStoredManifest {
            run: input.run_id.to_owned(),
        });
    }

    let outcome = create_or_reobserve_financial_fact(
        connection,
        NewFinancialFact {
            company_id: input.company_id.to_owned(),
            period_id: input.period_id.to_owned(),
            definition_id: input.definition_id.to_owned(),
            value_numeric: input.value_numeric.to_owned(),
            currency: input.currency.map(str::to_owned),
            statement_basis: Some(input.statement_basis.to_owned()),
            attribution: Some(input.attribution.to_owned()),
            variant: None,
            measure_window: input.measure_window.map(str::to_owned),
            data_quality: Some(input.data_quality.to_owned()),
            as_reported_value: None,
            as_reported_scale: None,
            reporting_standard: None,
            extraction_method: Some("mcp_agent".to_owned()),
            confidence: None,
            confirmation_state: Some("confirmed".to_owned()),
            supersedes_id: None,
            source_document_ref: Some(input.report_document_id.to_owned()),
            annotation: None,
        },
    )?;
    apply_structured_precedence(
        connection,
        outcome,
        StructuredPrecedenceFields {
            metric_key: input.metric_key,
            currency: input.currency,
            confirmation_state: "confirmed",
            source_tier: "agent",
            extraction_method: "mcp_agent",
            validation_status: input.validation_status,
            drift_json: None,
            citation: input.citation,
            report_document_id: input.report_document_id,
        },
    )
}

/// Outcome of committing one BiznesRadar-primary aggregator fact into its slot,
/// applying the ADR 0086 decision 3 precedence (`agent` added by ADR 0093
/// decision 1; positional retired by ADR 0095): `manual` > `esef` >
/// `espi_cover_note` > `agent` >
/// `html_aggregator`. The aggregator only ever overwrites its OWN
/// (`html_aggregator`, non-manual) slot and NEVER a manual or higher-tier fact.
#[derive(Debug, Clone)]
pub enum AggregatorFactCommit {
    /// The slot was empty — a new aggregator fact was written.
    Created(String),
    /// The slot held an aggregator fact with a different value — overwritten in
    /// place with the fresh aggregator value.
    Updated(String),
    /// The slot already held this exact value (aggregator's own or an agreeing
    /// higher tier) — no write, no change. The holder's tier/method travel with
    /// it because exact agreement with an ISSUER or MANUAL slot is the positive
    /// half of reversed witnessing (ADR 0086 dec. 4, epic #229 T5): the caller
    /// corroborates that slot, and must not corroborate the aggregator's own.
    Reobserved {
        fact_id: String,
        /// The holder's provenance `source_tier`; `None` for a fact with no
        /// provenance row (a hand-entered value).
        existing_tier: Option<String>,
        existing_method: String,
    },
    /// The slot is held by a higher-precedence fact (manual / esef /
    /// structured_xhtml / espi_cover_note / agent) — left
    /// untouched. The caller decides whether the divergence warrants an informational
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
        // tier that happens to agree. Either way nothing to write and no conflict;
        // the holder's identity travels out so the caller can tell "my own value
        // again" (no self-witnessing) from "the issuer/user agrees" (corroboration).
        FactWriteOutcome::Reobserved(existing) => {
            let existing_tier = fact_source_tier(connection, &existing.id)?;
            Ok(AggregatorFactCommit::Reobserved {
                fact_id: existing.id,
                existing_tier,
                existing_method: existing.extraction_method,
            })
        }
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
                        annotation: None,
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
                annotation: None,
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
///
/// The witness-corroboration stamp (migration `0122`) is CLEARED here: this
/// writer runs only where the fact's value was just written or overwritten, and
/// a stamp recorded against the previous value would otherwise be read as a live
/// agreement with the new one.
fn write_fact_provenance(
    connection: &Connection,
    fact_id: &str,
    input: &StructuredFactInput<'_>,
) -> StorageResult<()> {
    write_fact_provenance_fields(
        connection,
        fact_id,
        input.source_tier,
        input.extraction_method,
        input.validation_status,
        input.drift_json,
        input.citation,
    )
}

/// The field-level primitive [`write_fact_provenance`] delegates to — the ONE
/// upsert every provenance writer shares, including the legacy single-fact MCP
/// `create_financial_fact`/`update_financial_fact` act path (ADR 0093
/// decision 1 honesty rule, epic #285 T9), which has no natural
/// [`StructuredFactInput`] to build (it writes by already-resolved
/// `period_id`/`definition_id`, never by `metric_key`).
///
/// Bug #324 class: `source_tier` and
/// `financial_facts.extraction_method` are TWO writes of one fact about a
/// slot's origin, and a caller writing one without the other is exactly what
/// produced 7 tier/method-incoherent rows on the maintainer's DB (an issuer
/// tier taking over a positional slot rewrote the provenance tier but left
/// the stale `extraction_method='html_positional'` on the fact row) — and,
/// separately, what let the MCP takeover path (`update_financial_fact_handler`)
/// stamp `source_tier='agent'` onto a fact whose `extraction_method` stayed
/// whatever the ORIGINAL writer used. This function closes both classes at
/// once by construction: `extraction_method` is a REQUIRED parameter here
/// (never an afterthought synced separately), the two are validated for
/// coherence before either is written, and both are written together in the
/// same call — so no caller of this shared primitive can produce an
/// incoherent pair, or forget to sync the fact row at all. A release build
/// enforces this exactly like a debug build: a `debug_assert!` silently
/// vanishes in release and would not have caught the takeover path (it calls
/// this primitive directly, with no `StructuredFactInput` to assert against),
/// so the check is a real, always-on typed error instead.
///
/// `manual` and any extraction_method this map does not recognize are exempt
/// (`SourceTier::matches_extraction_method` fails OPEN there) — this can
/// never block a legitimate new writer, only the enumerated known-incoherent
/// pairs.
fn write_fact_provenance_fields(
    connection: &Connection,
    fact_id: &str,
    source_tier: &str,
    extraction_method: &str,
    validation_status: &str,
    drift_json: Option<&str>,
    citation: Option<&str>,
) -> StorageResult<()> {
    // ADR 0095: `pdf` (the html_positional tier's storage marker) is
    // retired. `SourceTier::Pdf` stays in the enum only as a legacy READ
    // value; no NEW write may ever produce it — a runtime refusal, not a
    // debug_assert, so a release build enforces this exactly like a debug
    // build.
    // `structured_xhtml` joined `pdf` as a legacy read-only tier (ADR 0098
    // dec. 7, #365): no live producer constructs it and new provenance writes
    // are refused by the same mechanism.
    if source_tier == "pdf" || source_tier == "structured_xhtml" {
        return Err(StorageError::RetiredSourceTier {
            fact_id: fact_id.to_owned(),
            source_tier: source_tier.to_owned(),
        });
    }
    if !crate::fundamentals::extraction::SourceTier::parse(source_tier)
        .map(|tier| tier.matches_extraction_method(extraction_method))
        .unwrap_or(true)
    {
        return Err(StorageError::IncoherentFactProvenance {
            fact_id: fact_id.to_owned(),
            source_tier: source_tier.to_owned(),
            extraction_method: extraction_method.to_owned(),
        });
    }

    // Sync the fact row's extraction_method FIRST — see the function doc:
    // this is what makes an incoherent pair structurally impossible rather
    // than merely asserted against — then delegate the actual upsert to the
    // shared `set_fact_provenance` seam, so the codebase has exactly ONE
    // provenance upsert; its stored-method coherence re-check sees the
    // just-synced value.
    connection.execute(
        "UPDATE financial_facts \
         SET extraction_method = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') \
         WHERE id = ?1",
        params![fact_id, extraction_method],
    )?;
    super::fundamentals_provenance::set_fact_provenance(
        connection,
        super::NewFactProvenance {
            fact_id,
            source_tier,
            validation_status,
            drift_json,
            citation,
        },
    )?;
    Ok(())
}

/// A resolved KPI definition's identity + shape — everything a caller needs
/// beyond the bare id (#361's manifest builder needs `value_kind` too, for
/// `unit.currency_*`/`period.window_kind_mismatch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKpiDefinition {
    pub definition_id: String,
    pub metric_key: String,
    pub value_kind: String,
}

/// One sector-scoped definition eligibility rule, shared in spirit with the
/// resolver's SQL predicate below (the truth-table test drives IDENTICAL cases
/// through both): a sector-scoped definition matches on TWO axes — the raw
/// directory `companies.sector` (runtime-created definitions, e.g. "Gry"), OR
/// the derived `statement_type` (the seeded statement packs, data-model.md
/// §731) guarded to non-`industrial` so the classification default never
/// matches anything the old NULL-sector behavior did not.
fn sector_definition_matches(
    definition_sector: Option<&str>,
    company_sector: Option<&str>,
    statement_type: &str,
) -> bool {
    match definition_sector {
        None => false,
        Some(definition_sector) => {
            company_sector == Some(definition_sector)
                || (statement_type != "industrial" && definition_sector == statement_type)
        }
    }
}

/// Resolves a KPI definition by metric key, without creating one (the
/// structured pipeline only emits seeded catalog metrics) — the SAME
/// sector-aware precedence #361's manifest validator resolves observations
/// against (data-model.md § Rejestr kodów / resolver). Sector-scoped
/// definitions match on TWO axes (see [`sector_definition_matches`]): the raw
/// `companies.sector` OR the non-`industrial` `statement_type`; full
/// precedence company-scoped > canonical > raw-sector match > statement-type
/// match > every remaining global non-sector definition, lexicographic by id
/// within a rank. A sector-scoped definition matching NEITHER axis is
/// excluded entirely, not merely deprioritized (a bank must never resolve an
/// industrial sector pack); a company with no raw sector and the default
/// `'industrial'` classification never gets a sector-scoped row (`sector =
/// ?3` against a bound `NULL` matches nothing in SQL; the `!= 'industrial'`
/// guard closes the statement axis).
fn resolve_kpi_definition(
    connection: &Connection,
    company_id: &str,
    metric_key: &str,
) -> StorageResult<Option<ResolvedKpiDefinition>> {
    let (sector, _source) = super::companies::get_company_sector(connection, company_id)?;
    let statement_type = super::companies::get_statement_type(connection, company_id)?;
    let existing: Option<(String, String, String)> = connection
        .query_row(
            "
            SELECT id, metric_key, value_kind FROM kpi_definitions
            WHERE metric_key = ?1
              AND (
                    (scope = 'company' AND company_id = ?2)
                 OR scope = 'canonical'
                 OR (scope = 'sector'
                     AND (sector = ?3 OR (?4 != 'industrial' AND sector = ?4)))
                 OR (scope NOT IN ('company', 'sector') AND company_id IS NULL)
              )
            ORDER BY
              CASE
                WHEN scope = 'company' AND company_id = ?2 THEN 0
                WHEN scope = 'canonical' THEN 1
                WHEN scope = 'sector' AND sector = ?3 THEN 2
                WHEN scope = 'sector' AND sector = ?4 THEN 3
                ELSE 4
              END,
              id
            LIMIT 1
            ",
            params![metric_key.trim(), company_id, sector, statement_type],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    Ok(existing.map(
        |(definition_id, metric_key, value_kind)| ResolvedKpiDefinition {
            definition_id,
            metric_key,
            value_kind,
        },
    ))
}

/// [`resolve_kpi_definition`], bare id only — the shape
/// [`prepare_fact_write`]/`record_aggregator_facts` need.
fn resolve_definition_by_metric_key(
    connection: &Connection,
    company_id: &str,
    metric_key: &str,
) -> StorageResult<Option<String>> {
    Ok(resolve_kpi_definition(connection, company_id, metric_key)?.map(|d| d.definition_id))
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

    /// Stamps honest MCP-agent provenance on a fact written through the
    /// LEGACY single-fact `create_financial_fact`/`update_financial_fact` MCP
    /// act path (ADR 0093 decision 1 honesty rule, epic #285 T9):
    /// `source_tier='agent'`, `extraction_method='mcp_agent'`,
    /// `validation_status='unreviewed'` (no validation gate runs on this
    /// single-fact path — the honest label; `record_financial_facts`/T7 is
    /// the validated batch path), no drift. `update_financial_fact_handler`
    /// (the TAKEOVER path) calls this on a fact another writer may have
    /// created, so `extraction_method` is corrected here too — never left at
    /// whatever the original writer used, which would otherwise leave
    /// `source_tier='agent'` paired with a stale, incoherent method (e.g.
    /// `'api'`) on the fact row. Reuses the exact upsert
    /// `record_structured_fact`/`record_aggregator_fact` share, so a slot
    /// this stamps is a real rung on the trust ladder: a later issuer-tier
    /// write still upgrades it in place.
    pub fn stamp_agent_fact_provenance(
        &self,
        fact_id: &str,
        citation: Option<&str>,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        // extraction_method='mcp_agent' is written in the SAME call as
        // source_tier='agent': this is the MCP takeover path
        // (`update_financial_fact_handler` calls this on a
        // fact another writer originally created), so the fact row's
        // extraction_method must be corrected here too, not left at whatever
        // the original writer used — `write_fact_provenance_fields` refuses
        // any other pairing as incoherent.
        write_fact_provenance_fields(
            &connection,
            fact_id,
            "agent",
            "mcp_agent",
            "unreviewed",
            None,
            citation,
        )
    }

    /// The sector-aware definition resolver (#361), read-only — no fact
    /// write, no definition creation. `jobs::kpi_ingest_validation` calls
    /// this per staged observation to build the manifest's `definitionId`
    /// and `value_kind`; #362 never re-resolves, it consumes the manifest's
    /// pinned `definitionId` (data-model.md § resolver).
    pub fn resolve_kpi_definition(
        &self,
        company_id: &str,
        metric_key: &str,
    ) -> StorageResult<Option<ResolvedKpiDefinition>> {
        let connection = self.db.checkout()?;
        resolve_kpi_definition(&connection, company_id, metric_key)
    }

    /// Idempotently ensures a fiscal period exists and returns its id (ADR 0093
    /// decision 6): the MCP batch fact tool needs the `finper_` id up front so
    /// its response always carries a `periodId`, even when every submitted fact
    /// is skipped (no_definition/implausible/identity_violation) and
    /// [`record_structured_fact`] never runs. Shares the same idempotent upsert
    /// every structured write uses — never `create_financial_period` (that
    /// manual-entry path stays unexposed over MCP).
    pub fn ensure_financial_period(
        &self,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
        period_end: Option<&str>,
        report_evidence_ref: &str,
    ) -> StorageResult<String> {
        let connection = self.db.checkout()?;
        ensure_period(
            &connection,
            company_id,
            fiscal_year,
            period_type,
            period_end,
            report_evidence_ref,
        )
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
    use crate::storage::{open_in_memory_database, StorageError};

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

    /// ADR 0077 G-1 class-closer: exercise every production write path in one
    /// DB and assert not a single provenance row carries the retired
    /// `validation_status='none'`. Pins the structured (ESEF) and ESPI
    /// cover-note tiers — the surviving deterministic write paths.
    #[test]
    fn no_production_path_writes_validation_status_none() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        // Structured-pipeline (ESEF) emission.
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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("structured emit");

        // ESPI cover-note tier emission: its own identifiable
        // extraction_method marker — must clear the G-1 no-`none` guard too.
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
                source_tier: "espi_cover_note",
                extraction_method: "espi_cover_note",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Sales revenue"),
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("cover-note emit");

        // The cover-note path's provenance is identifiable and honest.
        let cover_note_method: String = connection
            .query_row(
                "SELECT f.extraction_method FROM financial_facts f \
                 JOIN financial_fact_provenance p ON p.fact_id = f.id \
                 WHERE p.source_tier = 'espi_cover_note' AND f.extraction_method = 'espi_cover_note'",
                [],
                |row| row.get(0),
            )
            .expect("the cover-note fact carries an identifiable extraction_method");
        assert_eq!(cover_note_method, "espi_cover_note");

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
    /// alongside the fact, not just returns it for the caller to drop.
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
                    source_tier: "esef",
                    extraction_method: "api",
                    validation_status: "flagged",
                    drift_json: Some(drift),
                    citation: Some("Przychody netto ze sprzedazy"),
                    attribution: None,
                    measure_window: None,
                    data_quality: None,
                },
            )
            .expect("record structured fact"),
        );

        let (source_tier, validation_status, stored_drift, citation) =
            fact_provenance_row(&connection, &id).expect("a structured fact must carry provenance");
        assert_eq!(source_tier, "esef");
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
                    source_tier: "esef",
                    extraction_method: "api",
                    validation_status: "flagged",
                    drift_json: Some(
                        r#"{"addedLabels":[],"removedLabels":["x"],"unitChanged":null}"#,
                    ),
                    citation: Some("Przychody"),
                    attribution: None,
                    measure_window: None,
                    data_quality: None,
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
                attribution: None,
                measure_window: None,
                data_quality: None,
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
                attribution: None,
                measure_window: None,
                data_quality: None,
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
    /// slot overwrites it — the issuer's number wins its own slot (T7's
    /// stored-wins rule now applies only between peers).
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
                attribution: None,
                measure_window: None,
                data_quality: None,
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
                attribution: None,
                measure_window: None,
                data_quality: None,
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

    /// Bug #324 (7 real rows on the maintainer's DB): an issuer tier
    /// RE-OBSERVING (same value) a lower-tier slot must upgrade BOTH halves
    /// of the provenance together — `financial_fact_provenance.source_tier`
    /// to the issuer tier AND `financial_facts.extraction_method` to the
    /// issuer's own marker (`api`), never leaving the stale lower tier's
    /// method label stamped on a slot it no longer belongs to. Uses the ESPI
    /// cover-note tier as the lower tier — its own identifiable
    /// `extraction_method='espi_cover_note'` marker is distinct from the
    /// generic `api` the issuer tier writes.
    #[test]
    fn an_issuer_reobservation_upgrades_a_lower_tier_slots_extraction_method_too() {
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
                source_tier: "espi_cover_note",
                extraction_method: "espi_cover_note",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Sales revenue"),
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("cover-note write");

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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("issuer re-observation");

        let (tier, method): (String, String) = connection
            .query_row(
                "SELECT p.source_tier, f.extraction_method \
                 FROM financial_facts f JOIN financial_fact_provenance p ON p.fact_id = f.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("fact + provenance row");
        assert_eq!(tier, "esef");
        assert_eq!(
            method, "api",
            "extraction_method must move with the tier upgrade, never stay stale"
        );
    }

    /// An issuer tier RE-OBSERVING (same value) a lower-tier slot that was
    /// written with NO currency must fill in the gap from the incoming ESEF
    /// write, which does carry one — never leave the fact permanently
    /// currency-less just because the value happened to already agree. Uses
    /// the aggregator tier as the lower tier.
    #[test]
    fn an_issuer_reobservation_fills_a_currency_gap_left_by_a_lower_tier() {
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
                currency: None,
                confirmation_state: "confirmed",
                source_tier: "html_aggregator",
                extraction_method: "api",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("https://biznesradar.example/page | Przychody"),
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("aggregator write with no currency");

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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("issuer re-observation");

        let currency: Option<String> = connection
            .query_row("SELECT currency FROM financial_facts", [], |row| row.get(0))
            .expect("fact row");
        assert_eq!(
            currency.as_deref(),
            Some("PLN"),
            "a same-value tier upgrade must fill a currency gap the lower tier left, \
             not leave the fact permanently currency-less"
        );
    }

    /// Companion to the currency-gap test: a same NUMBER in a CONTRADICTING
    /// currency is not an agreement — EUR 1 000 000 and PLN 1 000 000 are
    /// different figures sharing digits. It takes the Divergent path: the
    /// HIGHER tier corrects the slot under its own currency and evidence.
    #[test]
    fn a_same_number_in_a_different_currency_is_a_divergence_the_higher_tier_corrects() {
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
                currency: Some("EUR"),
                confirmation_state: "confirmed",
                source_tier: "html_aggregator",
                extraction_method: "api",
                validation_status: "unreviewed",
                drift_json: None,
                citation: Some("https://biznesradar.example/page | Przychody"),
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("aggregator write with EUR");

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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("issuer re-observation");

        let (currency, tier): (Option<String>, String) = connection
            .query_row(
                "SELECT f.currency, p.source_tier
                 FROM financial_facts f
                 JOIN financial_fact_provenance p ON p.fact_id = f.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("fact row");
        assert_eq!(
            currency.as_deref(),
            Some("PLN"),
            "a same-number observation in a contradicting currency is a \
             divergence: the higher tier corrects the slot under its OWN \
             currency instead of relabeling the row around the aggregator's EUR"
        );
        assert_eq!(
            tier, "esef",
            "the correcting tier owns the slot's provenance"
        );
    }

    /// Bug #324, the DIVERGENT-value half: an issuer tier OVERWRITING a
    /// lower-tier slot with a different value must sync `extraction_method`
    /// alongside the value + tier — the exact `StructuredFactCommit::Upgraded`
    /// path whose `update_financial_fact` call never carried an
    /// `extraction_method` field. Uses the ESPI cover-note tier as the lower
    /// tier — its own identifiable `extraction_method='espi_cover_note'`
    /// marker is distinct from the generic `api` the issuer tier writes.
    #[test]
    fn an_issuer_divergence_upgrades_a_lower_tier_slots_extraction_method_too() {
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
                source_tier: "espi_cover_note",
                extraction_method: "espi_cover_note",
                validation_status: "passed",
                drift_json: None,
                citation: Some("Sales revenue"),
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("cover-note write");

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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("issuer divergence");

        let (value, tier, method): (String, String, String) = connection
            .query_row(
                "SELECT f.value_numeric, p.source_tier, f.extraction_method \
                 FROM financial_facts f JOIN financial_fact_provenance p ON p.fact_id = f.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("fact + provenance row");
        assert_eq!(value, "1000123");
        assert_eq!(tier, "esef");
        assert_eq!(
            method, "api",
            "extraction_method must move with the tier upgrade, never stay stale"
        );
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
                attribution: None,
                measure_window: None,
                data_quality: None,
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
                attribution: None,
                measure_window: None,
                data_quality: None,
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
            attribution: None,
            measure_window: None,
            data_quality: None,
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
        assert!(matches!(again[0], AggregatorFactCommit::Reobserved { .. }));
        assert!(matches!(again[1], AggregatorFactCommit::Reobserved { .. }));
        assert!(matches!(again[2], AggregatorFactCommit::NoDefinition));
    }

    // --- ADR 0093 decision 1: `SourceTier::Agent` threaded through the trust
    // ladder — ranked below every issuer tier and above `html_aggregator`. ---

    fn agent_input<'a>(
        company_id: &'a str,
        document_id: &'a str,
        value: &'a str,
    ) -> StructuredFactInput<'a> {
        StructuredFactInput {
            company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: document_id,
            metric_key: "revenue",
            value_numeric: value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "agent",
            extraction_method: "mcp_agent",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("XTB RB 18/2026 | Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        }
    }

    fn issuer_input<'a>(
        company_id: &'a str,
        document_id: &'a str,
        value: &'a str,
    ) -> StructuredFactInput<'a> {
        StructuredFactInput {
            company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: document_id,
            metric_key: "revenue",
            value_numeric: value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "esef",
            extraction_method: "api",
            validation_status: "passed",
            drift_json: None,
            citation: Some("Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: None,
        }
    }

    /// (a) ADR 0093 decision 1: an issuer tier RE-OBSERVING an agent-held slot
    /// takes the slot's LABEL over — mirrors
    /// `an_issuer_reobservation_upgrades_an_aggregator_slot_label`, agent instead
    /// of the aggregator.
    #[test]
    fn an_issuer_reobservation_upgrades_an_agent_slot_label() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "1000000"),
        )
        .expect("agent write");

        record_structured_fact(
            &connection,
            issuer_input(&company_id, &document_id, "1000000"),
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
            "the issuer tier must take over the agent slot label"
        );
        assert_eq!(citation, "Revenue", "the evidence must point at the filing");
    }

    /// (a) ADR 0093 decision 1: an issuer tier DISAGREEING with an agent-held
    /// slot overwrites it — a mis-extracted agent figure can never block the
    /// audited correction.
    #[test]
    fn an_issuer_divergence_overwrites_an_agent_slot() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "999000"),
        )
        .expect("agent write");

        let commit = record_structured_fact(
            &connection,
            issuer_input(&company_id, &document_id, "1000123"),
        )
        .expect("issuer divergence");
        assert!(
            matches!(
                &commit,
                StructuredFactCommit::Upgraded {
                    previous_tier,
                    previous_value: Some(v),
                    ..
                } if previous_tier == "agent" && v == "999000"
            ),
            "the issuer's number must upgrade the agent slot: {commit:?}"
        );

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

    /// (b) ADR 0093 decision 1: the agent tier never overwrites an issuer-held
    /// slot — a disagreement is a `Divergent` outcome, reported, never resolved
    /// silently.
    #[test]
    fn an_agent_divergence_against_an_issuer_slot_is_never_applied() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        record_structured_fact(
            &connection,
            issuer_input(&company_id, &document_id, "1000123"),
        )
        .expect("issuer write");

        let commit = record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "999000"),
        )
        .expect("agent divergence against an issuer slot");
        assert!(
            matches!(commit, StructuredFactCommit::Divergent { .. }),
            "an agent write against an issuer slot is reported, never overwritten: {commit:?}"
        );

        let (value, tier): (String, String) = connection
            .query_row(
                "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
                 JOIN financial_fact_provenance p ON p.fact_id = f.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("fact row");
        assert_eq!(value, "1000123", "the issuer's value must survive");
        assert_eq!(tier, "esef");
    }

    /// (b) ADR 0093 decision 1: the agent tier never overwrites a manual
    /// (no-provenance) slot either.
    #[test]
    fn an_agent_write_against_a_manual_slot_is_never_applied() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "555000"),
        )
        .expect("seed");
        // Strip the provenance row — a hand-entered fact never gets one.
        connection
            .execute("DELETE FROM financial_fact_provenance", [])
            .expect("strip provenance");

        let commit = record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "560000"),
        )
        .expect("agent write against a manual slot");
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

    /// (c) ADR 0093 decision 1: the agent tier fills over `html_aggregator` — an
    /// agent reads the issuer's own document, the aggregator is third-party.
    /// Agreement branch: the slot's LABEL takes the agent's over.
    #[test]
    fn an_agent_reobservation_upgrades_an_html_aggregator_slot_label() {
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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("aggregator write");

        record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "1000000"),
        )
        .expect("agent re-observation");

        let tier: String = connection
            .query_row(
                "SELECT source_tier FROM financial_fact_provenance",
                [],
                |row| row.get(0),
            )
            .expect("provenance row");
        assert_eq!(
            tier, "agent",
            "the agent tier must take over the aggregator slot label"
        );
    }

    /// (c) ADR 0093 decision 1: the agent tier OVERWRITES a divergent
    /// `html_aggregator` slot (outranks it).
    #[test]
    fn an_agent_divergence_overwrites_an_html_aggregator_slot() {
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
                attribution: None,
                measure_window: None,
                data_quality: None,
            },
        )
        .expect("aggregator write");

        record_structured_fact(
            &connection,
            agent_input(&company_id, &document_id, "1000123"),
        )
        .expect("agent divergence");

        let (value, tier): (String, String) = connection
            .query_row(
                "SELECT f.value_numeric, p.source_tier FROM financial_facts f \
                 JOIN financial_fact_provenance p ON p.fact_id = f.id",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("fact row");
        assert_eq!(
            value, "1000123",
            "the agent's number must win the aggregator's slot"
        );
        assert_eq!(tier, "agent");
    }

    /// (2) `outranked_stored_tier_of` explicit precedence pins for the agent
    /// tier — no assumption that parse/outranks compose correctly.
    #[test]
    fn outranked_stored_tier_of_places_agent_between_pdf_and_html_aggregator() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let fact_id = created_id(
            record_structured_fact(&connection, agent_input(&company_id, &document_id, "1"))
                .expect("agent seed"),
        );

        assert_eq!(
            outranked_stored_tier_of(&connection, &fact_id, "esef").expect("esef outranks"),
            Some("agent".to_owned())
        );
        assert_eq!(
            outranked_stored_tier_of(&connection, &fact_id, "structured_xhtml")
                .expect("structured_xhtml outranks"),
            Some("agent".to_owned())
        );
        assert_eq!(
            outranked_stored_tier_of(&connection, &fact_id, "espi_cover_note")
                .expect("espi_cover_note outranks"),
            Some("agent".to_owned())
        );
        assert_eq!(
            outranked_stored_tier_of(&connection, &fact_id, "pdf").expect("pdf outranks"),
            Some("agent".to_owned())
        );
        assert_eq!(
            outranked_stored_tier_of(&connection, &fact_id, "html_aggregator")
                .expect("html_aggregator does not outrank agent"),
            None
        );
    }

    // --- ADR 0093 decision 2: `data_quality` canonical vocabulary + the
    // preliminary-data lifecycle — coexistence and write-path supersession,
    // exercised through the structured path (`record_structured_fact` /
    // `create_or_reobserve_financial_fact`). The plain `create_financial_fact`
    // path (MCP/UI manual writes) is covered directly in
    // `storage/tests/financials.rs`, since both share the same
    // `create_financial_fact` stamping logic (T3). ---

    fn quality_input<'a>(
        company_id: &'a str,
        document_id: &'a str,
        value: &'a str,
        data_quality: &'a str,
    ) -> StructuredFactInput<'a> {
        StructuredFactInput {
            company_id,
            fiscal_year: 2024,
            period_type: "FY",
            period_end: Some("2024-12-31"),
            report_document_id: document_id,
            metric_key: "revenue",
            value_numeric: value,
            currency: Some("PLN"),
            confirmation_state: "confirmed",
            source_tier: "agent",
            extraction_method: "mcp_agent",
            validation_status: "unreviewed",
            drift_json: None,
            citation: Some("XTB RB 18/2026 | Revenue"),
            attribution: None,
            measure_window: None,
            data_quality: Some(data_quality),
        }
    }

    /// `record_structured_fact` rejects an unknown `data_quality` token as a
    /// typed error rather than silently minting a phantom uniqueness slot
    /// (`normalize_data_quality`, `storage/financials.rs`).
    #[test]
    fn record_structured_fact_rejects_an_unknown_data_quality_token() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        let error = record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "1", "garbage"),
        )
        .expect_err("an unknown data_quality token must never be silently slotted");
        assert!(
            matches!(
                error,
                StorageError::InvalidFinancialsValue {
                    key: "data_quality",
                    ref value
                } if value == "garbage"
            ),
            "expected a typed invalid-data_quality error, got {error:?}"
        );
    }

    /// `record_structured_fact` (and every other caller funnelled through
    /// `write_fact_provenance_fields`) must
    /// REFUSE an explicit source_tier/extraction_method pair the tier's own
    /// allowlist (`SourceTier::matches_extraction_method`) does not
    /// recognize as coherent — a typed error at RUNTIME, not a
    /// `debug_assert!` that silently vanishes in a release build (bug #324's
    /// 7 real incoherent rows were never caught by anything before this).
    #[test]
    fn record_structured_fact_refuses_an_incoherent_tier_method_pair() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        // `esef` paired with `mcp_agent` is incoherent (the agent marker
        // pairs only with tier `agent`). Bug #324's original shape —
        // `esef` + `html_positional` — is now refused EARLIER by the ADR
        // 0095 `RetiredExtractionMethod` guard on the create path, so this
        // test exercises the coherence guard with a live incoherent pair.
        let mut input = quality_input(&company_id, &document_id, "1", "final");
        input.source_tier = "esef";
        input.extraction_method = "mcp_agent";

        let error = record_structured_fact(&connection, input)
            .expect_err("an incoherent source_tier/extraction_method pair must be refused");
        assert!(
            matches!(
                error,
                StorageError::IncoherentFactProvenance {
                    ref source_tier,
                    ref extraction_method,
                    ..
                } if source_tier == "esef" && extraction_method == "mcp_agent"
            ),
            "expected a typed IncoherentFactProvenance error, got {error:?}"
        );

        // No provenance row is left behind by the rejected write (the
        // production write path wraps this in a transaction the caller rolls
        // back on error — `KpiExtractionStore::record_structured_fact`).
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM financial_fact_provenance",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .expect("count"),
            0,
            "a refused write must leave no provenance row"
        );
    }

    /// ADR 0095 scope expansion: `source_tier='pdf'` is retired outright —
    /// migration 0135 deleted every stored `pdf`-tier fact, and no NEW write
    /// may ever produce it again, regardless of which extraction_method
    /// accompanies it (a runtime refusal in the shared writer, not a
    /// `debug_assert` that vanishes in a release build).
    #[test]
    fn record_structured_fact_refuses_a_retired_pdf_source_tier() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        let mut input = quality_input(&company_id, &document_id, "1", "final");
        input.source_tier = "pdf";
        input.extraction_method = "api";

        let error = record_structured_fact(&connection, input)
            .expect_err("a write naming the retired pdf tier must be refused");
        assert!(
            matches!(
                error,
                StorageError::RetiredSourceTier { ref source_tier, .. } if source_tier == "pdf"
            ),
            "expected a typed RetiredSourceTier error naming the tier, got {error:?}"
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM financial_fact_provenance",
                    [],
                    |row| row.get::<_, i64>(0)
                )
                .expect("count"),
            0,
            "a refused write must leave no provenance row"
        );
    }

    /// ADR 0098 dec. 7 (#365): `structured_xhtml` joined `pdf` as a legacy
    /// read-only tier. Driven through the PUBLIC store wrapper (its
    /// transaction is what rolls the whole write back), asserting BOTH the
    /// fact row and the provenance row are gone — the private fn creates the
    /// fact before provenance, so a provenance-only assertion would pass even
    /// if the fact leaked.
    #[test]
    fn record_structured_fact_refuses_a_retired_structured_xhtml_source_tier() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);
        let state = crate::storage::AppState::new(connection);

        let mut input = quality_input(&company_id, &document_id, "1", "final");
        input.source_tier = "structured_xhtml";
        input.extraction_method = "api";

        let error = state
            .kpi_extraction()
            .record_structured_fact(input)
            .expect_err("a write naming the retired structured_xhtml tier must be refused");
        assert!(
            matches!(
                error,
                StorageError::RetiredSourceTier { ref source_tier, .. }
                    if source_tier == "structured_xhtml"
            ),
            "expected a typed RetiredSourceTier error naming the tier, got {error:?}"
        );
        let raw = state.checkout_for_tests().expect("raw");
        for (table, label) in [
            ("financial_facts", "fact"),
            ("financial_fact_provenance", "provenance"),
        ] {
            assert_eq!(
                raw.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                    .expect("count"),
                0,
                "a refused write must leave no {label} row"
            );
        }
    }

    /// A `preliminary` fact and a `final` fact for the same metric/period are
    /// two DIFFERENT rows in the uniqueness slot (`data_quality` is a slot
    /// dimension, 0034) — never a UNIQUE violation, never a silent overwrite.
    #[test]
    fn preliminary_and_final_coexist_in_the_same_slot() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        let preliminary_id = created_id(
            record_structured_fact(
                &connection,
                quality_input(&company_id, &document_id, "492200000", "preliminary"),
            )
            .expect("preliminary write"),
        );
        let final_id = created_id(
            record_structured_fact(
                &connection,
                quality_input(&company_id, &document_id, "495000000", "final"),
            )
            .expect("final write"),
        );

        assert_ne!(preliminary_id, final_id, "distinct rows, same slot");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM financial_facts", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 2, "both quality variants persist");
    }

    /// A `final` fact created into a slot whose sibling is `preliminary` stamps
    /// `supersedes_id` at it (ADR 0093 decision 2), via the structured path.
    #[test]
    fn final_created_next_to_preliminary_stamps_supersedes_id_via_structured_path() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        let preliminary_id = created_id(
            record_structured_fact(
                &connection,
                quality_input(&company_id, &document_id, "492200000", "preliminary"),
            )
            .expect("preliminary write"),
        );
        let final_id = created_id(
            record_structured_fact(
                &connection,
                quality_input(&company_id, &document_id, "495000000", "final"),
            )
            .expect("final write"),
        );

        let supersedes_id: Option<String> = connection
            .query_row(
                "SELECT supersedes_id FROM financial_facts WHERE id = ?1",
                [&final_id],
                |row| row.get(0),
            )
            .expect("final row");
        assert_eq!(supersedes_id, Some(preliminary_id));
    }

    /// When BOTH a `preliminary` and an `estimated` sibling occupy the slot, a
    /// later `final` fact supersedes the `preliminary` one — the issuer's own
    /// preliminary release outranks a third-party estimate (ADR 0093 decision 2).
    #[test]
    fn final_prefers_the_preliminary_sibling_over_an_estimated_one() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, document_id) = seed_company_and_document(&connection);

        record_structured_fact(
            &connection,
            quality_input(&company_id, &document_id, "480000000", "estimated"),
        )
        .expect("estimated write");
        let preliminary_id = created_id(
            record_structured_fact(
                &connection,
                quality_input(&company_id, &document_id, "492200000", "preliminary"),
            )
            .expect("preliminary write"),
        );
        let final_id = created_id(
            record_structured_fact(
                &connection,
                quality_input(&company_id, &document_id, "495000000", "final"),
            )
            .expect("final write"),
        );

        let supersedes_id: Option<String> = connection
            .query_row(
                "SELECT supersedes_id FROM financial_facts WHERE id = ?1",
                [&final_id],
                |row| row.get(0),
            )
            .expect("final row");
        assert_eq!(
            supersedes_id,
            Some(preliminary_id),
            "the issuer-published preliminary must win over a third-party estimate"
        );
    }

    // -----------------------------------------------------------------
    // resolve_definition_by_metric_key (#361): sector-aware deterministic
    // precedence, shared with the manifest validator's resolver.
    // -----------------------------------------------------------------

    fn insert_definition(
        connection: &Connection,
        id: &str,
        scope: &str,
        company_id: Option<&str>,
        sector: Option<&str>,
        metric_key: &str,
    ) {
        connection
            .execute(
                "INSERT INTO kpi_definitions (id, scope, company_id, sector, metric_key, label, value_kind)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?5, 'monetary')",
                params![id, scope, company_id, sector, metric_key],
            )
            .expect("kpi_definitions insert");
    }

    #[test]
    fn resolver_prefers_company_scoped_over_canonical_and_sector() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
            .expect("set sector");
        insert_definition(
            &connection,
            "kpidef_revenue__s_banking",
            "sector",
            None,
            Some("banking"),
            "revenue",
        );
        insert_definition(
            &connection,
            "kpidef_revenue__c_c1",
            "company",
            Some(&company_id),
            None,
            "revenue",
        );

        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
        assert_eq!(resolved.as_deref(), Some("kpidef_revenue__c_c1"));
    }

    #[test]
    fn resolver_prefers_canonical_over_matching_sector() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
            .expect("set sector");
        insert_definition(
            &connection,
            "kpidef_revenue__s_banking",
            "sector",
            None,
            Some("banking"),
            "revenue",
        );

        // "revenue" is already seeded canonically by migration 0034.
        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
        assert_eq!(resolved.as_deref(), Some("kpidef_revenue"));
    }

    #[test]
    fn resolver_picks_matching_sector_and_excludes_non_matching_sector() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
            .expect("set sector");
        // A company-specific metric key with NO canonical twin, so only the
        // sector rows compete.
        insert_definition(
            &connection,
            "kpidef_nim__s_banking",
            "sector",
            None,
            Some("banking"),
            "net_interest_margin",
        );
        insert_definition(
            &connection,
            "kpidef_nim__s_industrial",
            "sector",
            None,
            Some("industrial"),
            "net_interest_margin",
        );

        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "net_interest_margin")
                .expect("resolve");
        assert_eq!(
            resolved.as_deref(),
            Some("kpidef_nim__s_banking"),
            "the industrial sector row must be excluded, not merely deprioritized"
        );
    }

    #[test]
    fn resolver_company_without_sector_never_gets_a_sector_scoped_definition() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        // No sector set on the company at all.
        insert_definition(
            &connection,
            "kpidef_nim__s_banking",
            "sector",
            None,
            Some("banking"),
            "net_interest_margin",
        );

        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "net_interest_margin")
                .expect("resolve");
        assert_eq!(resolved, None);
    }

    /// Dual-axis regression (a): a runtime definition scoped to a RAW
    /// directory sector ("Gry") keeps resolving exactly as before the
    /// statement-type axis existed.
    #[test]
    fn resolver_keeps_matching_a_raw_directory_sector_definition() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        super::super::companies::set_company_sector(&connection, &company_id, Some("Gry"))
            .expect("set sector");
        insert_definition(
            &connection,
            "kpidef_arpu__s_gry",
            "sector",
            None,
            Some("Gry"),
            "arpu",
        );

        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "arpu").expect("resolve");
        assert_eq!(resolved.as_deref(), Some("kpidef_arpu__s_gry"));
    }

    /// Dual-axis regression (b): the `'industrial'` classification default
    /// never opens the statement axis — a company with no raw sector still
    /// matches nothing, even against a runtime `sector='industrial'` row.
    #[test]
    fn resolver_never_matches_an_industrial_statement_type_definition() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        insert_definition(
            &connection,
            "kpidef_widget__s_industrial",
            "sector",
            None,
            Some("industrial"),
            "widget_output",
        );

        let resolved = resolve_definition_by_metric_key(&connection, &company_id, "widget_output")
            .expect("resolve");
        assert_eq!(resolved, None);
    }

    /// Dual-axis (c) — red before the fix: the seeded statement packs use the
    /// `statement_type` vocabulary, so a classified issuer resolves its pack
    /// even when its raw directory sector says something else (or nothing).
    #[test]
    fn resolver_matches_the_statement_type_axis_for_a_classified_issuer() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        connection
            .execute(
                "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
                [&company_id],
            )
            .expect("classify");

        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "net_interest_income")
                .expect("resolve");
        assert_eq!(resolved.as_deref(), Some("kpidef_bank_net_interest_income"));
    }

    /// Dual-axis collision (d): when a raw-sector definition and a
    /// statement-type definition both carry the same metric key, the raw
    /// match wins by RANK — even when its id sorts lexicographically last.
    #[test]
    fn a_raw_sector_definition_outranks_the_statement_type_axis() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        super::super::companies::set_company_sector(
            &connection,
            &company_id,
            Some("banki komercyjne"),
        )
        .expect("set sector");
        connection
            .execute(
                "UPDATE companies SET statement_type = 'banking' WHERE id = ?1",
                [&company_id],
            )
            .expect("classify");
        // Sorts AFTER the seeded kpidef_bank_net_interest_income — only rank
        // can make it win.
        insert_definition(
            &connection,
            "kpidef_zz_nii__s_raw",
            "sector",
            None,
            Some("banki komercyjne"),
            "net_interest_income",
        );

        let resolved =
            resolve_definition_by_metric_key(&connection, &company_id, "net_interest_income")
                .expect("resolve");
        assert_eq!(resolved.as_deref(), Some("kpidef_zz_nii__s_raw"));
    }

    /// Dual-axis (e): the SQL predicate and the Rust twin
    /// (`sector_definition_matches`, pinned-commit eligibility) agree on the
    /// same truth table — mirror fidelity is asserted, not assumed.
    #[test]
    fn sector_eligibility_truth_table_agrees_between_sql_and_rust() {
        // (definition sector, raw company sector, statement_type, eligible)
        let cases: [(&str, Option<&str>, &str, bool); 5] = [
            ("Gry", Some("Gry"), "industrial", true),
            ("industrial", None, "industrial", false),
            ("banking", None, "banking", true),
            ("banking", Some("banki komercyjne"), "banking", true),
            ("banking", Some("Gry"), "industrial", false),
        ];
        for (definition_sector, raw_sector, statement_type, eligible) in cases {
            assert_eq!(
                sector_definition_matches(Some(definition_sector), raw_sector, statement_type),
                eligible,
                "rust: def={definition_sector} raw={raw_sector:?} statement={statement_type}"
            );

            let connection = open_in_memory_database().expect("db");
            let (company_id, _doc) = seed_company_and_document(&connection);
            if let Some(raw) = raw_sector {
                super::super::companies::set_company_sector(&connection, &company_id, Some(raw))
                    .expect("set sector");
            }
            connection
                .execute(
                    "UPDATE companies SET statement_type = ?1 WHERE id = ?2",
                    params![statement_type, company_id],
                )
                .expect("classify");
            insert_definition(
                &connection,
                "kpidef_truth_case",
                "sector",
                None,
                Some(definition_sector),
                "truth_case_metric",
            );
            let resolved =
                resolve_definition_by_metric_key(&connection, &company_id, "truth_case_metric")
                    .expect("resolve");
            assert_eq!(
                resolved.is_some(),
                eligible,
                "sql: def={definition_sector} raw={raw_sector:?} statement={statement_type}"
            );
        }
        assert!(!sector_definition_matches(None, Some("Gry"), "banking"));
    }

    /// Class guardrail (ADR 0099 dec. 6 / #383 sol R1): every key of every
    /// extraction-profile pack RESOLVES to a definition for a company of that
    /// statement type — a pack can never demand an unresolvable key again.
    #[test]
    fn every_profile_pack_key_resolves_for_its_statement_type() {
        use crate::storage::kpi_ingest_profiles::{expected_pack, PROFILE_VERSIONS};
        for statement_type in [
            "industrial",
            "banking",
            "insurance",
            "specialty_finance",
            "brokerage",
            "reit",
        ] {
            let connection = open_in_memory_database().expect("db");
            let (company_id, _doc) = seed_company_and_document(&connection);
            connection
                .execute(
                    "UPDATE companies SET statement_type = ?1 WHERE id = ?2",
                    params![statement_type, company_id],
                )
                .expect("classify");
            for profile in PROFILE_VERSIONS {
                for key in expected_pack(profile, statement_type) {
                    let resolved = resolve_definition_by_metric_key(&connection, &company_id, key)
                        .expect("resolve");
                    assert!(
                        resolved.is_some(),
                        "{profile} × {statement_type}: pack key {key} does not resolve"
                    );
                }
            }
        }
    }

    #[test]
    fn resolver_falls_back_to_remaining_global_definitions_lexicographically() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        // Neither row is canonical/sector/company -- both are "remaining
        // global", ordered lexicographically by id. Distinct `scope` values
        // (rather than two `user` rows) keep the unique index
        // `(metric_key, scope, company_id, sector)` happy -- the resolver's
        // catch-all bucket cares only that `scope NOT IN ('company', 'sector')`.
        insert_definition(
            &connection,
            "kpidef_custom_b",
            "user",
            None,
            None,
            "custom_metric",
        );
        insert_definition(
            &connection,
            "kpidef_custom_a",
            "legacy",
            None,
            None,
            "custom_metric",
        );

        let resolved = resolve_definition_by_metric_key(&connection, &company_id, "custom_metric")
            .expect("resolve");
        assert_eq!(resolved.as_deref(), Some("kpidef_custom_a"));
    }

    #[test]
    fn resolver_catch_all_excludes_company_bound_rows_for_a_different_company() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        connection
            .execute(
                "INSERT INTO companies (id, exchange, ticker, qualified_ticker, display_name)
                 VALUES ('c2', 'gpw', 'XYZ', 'GPW:XYZ', 'XYZ SA')",
                [],
            )
            .expect("company b");
        // A 'user' scoped definition bound to company A only -- not
        // 'company'/'sector' scope, so it would fall into the catch-all
        // bucket if that bucket did not also require company_id IS NULL.
        insert_definition(
            &connection,
            "kpidef_x__u_c1",
            "user",
            Some(&company_id),
            None,
            "custom_x",
        );

        let resolved =
            resolve_definition_by_metric_key(&connection, "c2", "custom_x").expect("resolve");
        assert_eq!(
            resolved, None,
            "company A's user-scoped definition must not leak to company B via the catch-all"
        );
    }

    #[test]
    fn resolver_is_deterministic_across_repeated_calls() {
        let connection = open_in_memory_database().expect("db");
        let (company_id, _doc) = seed_company_and_document(&connection);
        super::super::companies::set_company_sector(&connection, &company_id, Some("banking"))
            .expect("set sector");
        insert_definition(
            &connection,
            "kpidef_revenue__s_banking",
            "sector",
            None,
            Some("banking"),
            "revenue",
        );
        insert_definition(
            &connection,
            "kpidef_revenue__c_c1",
            "company",
            Some(&company_id),
            None,
            "revenue",
        );

        let first =
            resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
        let second =
            resolve_definition_by_metric_key(&connection, &company_id, "revenue").expect("resolve");
        assert_eq!(first, second);
    }
}
