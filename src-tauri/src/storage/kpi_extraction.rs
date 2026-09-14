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

/// The `statement_basis` vocabulary every explicit-basis writer accepts (ADR
/// 0100 decision 4, #508) — shared so [`record_pinned_fact`]'s manifest check
/// and [`prepare_fact_write`]'s ESEF caller-supplied basis can never drift.
fn is_known_statement_basis(basis: &str) -> bool {
    matches!(basis, "standalone" | "consolidated")
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
    /// Slot dimension `flow` | `point_in_time` | `trailing` | `cumulative` |
    /// `duration`. Every writer passes `None` — `slot_dims` derives the
    /// default from the resolved definition's `period_nature` (`instant` ->
    /// `point_in_time`, else `flow`; ADR 0100 decision 6, epic #398). An
    /// explicit value contradicting that axis is a typed
    /// `MeasureWindowPeriodNatureMismatch` refusal.
    pub measure_window: Option<&'a str>,
    /// `final` (default, `None` normalizes to it) | `preliminary` | `estimated`
    /// (ADR 0093 decision 2), normalized at the storage write boundary
    /// (`normalize_data_quality`). Every writer before the MCP agent tool (T7)
    /// passes `None` — no behavior change for the existing pipeline.
    pub data_quality: Option<&'a str>,
    /// `standalone` | `consolidated` (ADR 0100 decision 4, #508); `None`
    /// keeps the default for every writer but the ESEF commit loop, which
    /// passes the document's ONE selected basis (`select_primary_basis`) —
    /// the true label, never the `slot_dims` default.
    pub statement_basis: Option<&'a str>,
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

    // Every writer but the ESEF commit loop keeps the default basis (`None`
    // → `slot_dims`'s `or_default` resolves to 'consolidated') — ADR 0095.
    // #508: an explicit basis is validated against the same vocabulary
    // `record_pinned_fact`'s manifest check applies, never trusted verbatim.
    let statement_basis = match input.statement_basis {
        None => None,
        Some(basis) if is_known_statement_basis(basis) => Some(basis.to_owned()),
        Some(basis) => {
            return Err(StorageError::InvalidFinancialsValue {
                key: "statement_basis",
                value: basis.to_owned(),
            });
        }
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
    if !is_known_statement_basis(input.statement_basis) {
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
/// `unit.currency_*`, and `period_nature` for `period.window_kind_mismatch`,
/// ADR 0100 decision 6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedKpiDefinition {
    pub definition_id: String,
    pub metric_key: String,
    pub value_kind: String,
    pub period_nature: String,
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
    // Curated alias (ADR 0100 decision 12): a dead catalog key means the
    // live key it was fragmented from. The redirect is guarded by the
    // one-sidedness rule AT RUNTIME, not just in the curation table (sol
    // review finding 9): it applies only while the source key holds ZERO
    // facts FOR THIS COMPANY. Per-company, not database-global (sol round
    // 2): one company's legacy `inventory` series must never flip write
    // routing for every other company — each company's series stays
    // internally consistent, which is the whole point. On a company whose
    // source-key series already exists, the redirect never fires —
    // redirecting there would split one series across two keys, the exact
    // repaint ADR 0077 dec. 8 forbids. Never chained, never in reverse.
    let metric_key = match crate::fundamentals::kpi_aliases::resolve(metric_key.trim()) {
        Some(target) => {
            let source_has_facts: bool = connection.query_row(
                "SELECT EXISTS(
                     SELECT 1 FROM financial_facts f
                     JOIN kpi_definitions d ON d.id = f.definition_id
                     WHERE d.metric_key = ?1 AND f.company_id = ?2)",
                params![metric_key.trim(), company_id],
                |row| row.get(0),
            )?;
            if source_has_facts {
                metric_key.trim()
            } else {
                target
            }
        }
        None => metric_key.trim(),
    };
    let (sector, _source) = super::companies::get_company_sector(connection, company_id)?;
    let statement_type = super::companies::get_statement_type(connection, company_id)?;
    let existing: Option<(String, String, String, String)> = connection
        .query_row(
            "
            SELECT id, metric_key, value_kind, period_nature FROM kpi_definitions
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
            params![metric_key, company_id, sector, statement_type],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    Ok(
        existing.map(|(definition_id, metric_key, value_kind, period_nature)| {
            ResolvedKpiDefinition {
                definition_id,
                metric_key,
                value_kind,
                period_nature,
            }
        }),
    )
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
mod tests;
