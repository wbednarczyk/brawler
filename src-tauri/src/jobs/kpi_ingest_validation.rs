//! Deterministic KPI ingest validation orchestration (ADR 0098 dec. 4; epic
//! #352, card #361). Thin: maps `AppState` rows onto the storage-free DTOs
//! [`crate::fundamentals::kpi_manifest`] takes, runs the pure rule core,
//! seals the result, and applies it through
//! [`crate::storage::KpiIngestStagingStore::apply_validation_outcome`] — the
//! sole write path from `staged` to `{ready_to_commit, validation_failed}`.
//! Headless-only for #361 (no Tauri command twin, the `record_financial_facts`
//! idiom): the MCP surface is #353; the queue trigger is the
//! `kpi_ingest_validate` handler (#364, [`crate::jobs::kpi_ingest_queue`]).

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;

use crate::app_state::AppState;
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::fundamentals::kpi_manifest::{
    build_manifest, ExpectedKpis, ExpectedSnapshot, KpiIngestManifest, ManifestObservationInput,
    ManifestRunInput, MissingReasons, ResolvedDefinitionInput, SealedManifest, SlotHistoryInput,
};
use crate::storage::{HistorySlotKey, KpiIngestRunState};

/// The validator's result — `outcome` mirrors [`SealedManifest::outcome`]
/// (`"ready"` | `"failed"`); a `failed` `manifest` IS the typed repair
/// report (every rejected observation carries its own diagnostic codes).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KpiIngestValidationOutcome {
    pub manifest: KpiIngestManifest,
    pub manifest_hash: String,
    pub outcome: &'static str,
}

/// Parses `kpi_ingest_runs.missing_reasons_json` into [`MissingReasons`] —
/// the ONLY place this column's JSON is interpreted; `evaluate` itself only
/// branches on the already-classified enum (fundamentals stays storage/IO
/// free).
fn parse_missing_reasons(raw: Option<&str>) -> MissingReasons {
    let Some(raw) = raw else {
        return MissingReasons::None;
    };
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(raw) else {
        return MissingReasons::Invalid;
    };
    let mut reasons = std::collections::BTreeMap::new();
    for (key, value) in map {
        match value {
            serde_json::Value::String(s) if !s.trim().is_empty() => {
                reasons.insert(key, s);
            }
            _ => return MissingReasons::Invalid,
        }
    }
    MissingReasons::Valid(reasons)
}

/// Validates the CURRENT staging revision of `run_id` and applies the sealed
/// verdict atomically (`apply_validation_outcome`). Steps (data-model.md §
/// KPI ingest / #361):
/// 1. Load the run; refuse a non-`staged` run (`Conflict`) before doing any
///    other work.
/// 2. Load the current revision's staged observations.
/// 3. Consume the creation-time `expected_kpis_json` stamp (ADR 0099 dec. 6);
///    a legacy raw-seeded row with a NULL column is live-stamped here as the
///    fallback — set-once either way, frozen for every later attempt.
/// 4. Resolve each observation's definition via the sector-aware resolver
///    (`storage::kpi_extraction::KpiExtractionStore::resolve_kpi_definition`).
/// 5. Batch-read slot-aware history for every resolved definition
///    (`storage::financials::FinancialsStore::slot_metric_histories`).
/// 6. `evaluate` -> `build_manifest` -> `SealedManifest::seal`.
/// 7. `apply_validation_outcome` — the one atomic write.
pub fn validate_kpi_ingest_run(
    state: &AppState,
    run_id: &str,
) -> Result<KpiIngestValidationOutcome, CommandError> {
    let runs_store = state.kpi_ingest_runs();
    let run = runs_store.get_run(run_id)?.ok_or_else(|| {
        CommandError::new(
            CommandErrorCode::NotFound,
            format!("kpi ingest run not found: {run_id}"),
        )
    })?;
    if run.status != KpiIngestRunState::Staged {
        return Err(CommandError::new(
            CommandErrorCode::Conflict,
            format!(
                "kpi ingest run {run_id} is not staged (status={})",
                run.status.as_str()
            ),
        ));
    }
    let revision = run.manifest_revision;

    // The run-owned natural-key period descriptor (migration 0138) — never a
    // `financial_periods` lookup, which may legally not exist yet at
    // validation time (ADR 0098 dec. 5).
    let (Some(fiscal_year), Some(period_type)) = (run.period_fiscal_year, run.period_type.clone())
    else {
        return Err(CommandError::new(
            CommandErrorCode::Internal,
            format!("kpi ingest run {run_id} has no period descriptor to validate against"),
        ));
    };
    let Some(scope) = run.scope.clone() else {
        return Err(CommandError::new(
            CommandErrorCode::Internal,
            format!("kpi ingest run {run_id} has no scope"),
        ));
    };
    let Some(data_quality) = run.data_quality.clone() else {
        return Err(CommandError::new(
            CommandErrorCode::Internal,
            format!("kpi ingest run {run_id} has no data_quality"),
        ));
    };

    let staging_store = state.kpi_ingest_staging();
    let observations = staging_store.list_staged_observations(run_id, Some(revision))?;
    if observations.is_empty() {
        return Err(CommandError::new(
            CommandErrorCode::Conflict,
            format!("kpi ingest run {run_id} revision {revision} has no staged observations"),
        ));
    }

    // Consume the creation-time stamp verbatim (ADR 0099 dec. 6) — the bytes
    // are what staging/commit byte-compare against. A legacy raw-seeded row
    // (NULL column) is live-stamped as the fallback: set-once, status/revision
    // guarded, source "kpi_relevance", packVersion null.
    let effective_json = match run.expected_kpis_json.clone() {
        Some(stamped) => stamped,
        None => {
            let expected_keys = state
                .financials()
                .expected_primary_metric_keys(&run.company_id)?
                .unwrap_or_default();
            let stamp = ExpectedKpis {
                schema_version: 1,
                source: "kpi_relevance".to_owned(),
                pack_version: None,
                keys: expected_keys,
            };
            let stamp_json = serde_json::to_string(&stamp)
                .map_err(|e| CommandError::new(CommandErrorCode::Internal, e.to_string()))?;
            runs_store.stamp_expected_kpis(run_id, revision, &stamp_json)?
        }
    };
    let effective: ExpectedKpis = serde_json::from_str(&effective_json).map_err(|e| {
        CommandError::new(
            CommandErrorCode::Internal,
            format!("stored expected_kpis_json for run {run_id} is malformed: {e}"),
        )
    })?;
    let expected = ExpectedSnapshot {
        schema_version: effective.schema_version,
        source: effective.source,
        pack_version: effective.pack_version,
        keys: effective.keys,
    };

    // Resolve every observation's definition (sector-aware resolver) BEFORE
    // batching the slot-history read -- the history batch needs the resolved
    // `definition_id`s to build its slot keys.
    let extraction_store = state.kpi_extraction();
    let mut per_observation_definition = Vec::with_capacity(observations.len());
    for observation in &observations {
        let candidate = observation
            .metric_key_candidate
            .as_deref()
            .map(str::trim)
            .filter(|key| !key.is_empty());
        let resolved = match candidate {
            Some(key) => extraction_store.resolve_kpi_definition(&run.company_id, key)?,
            None => None,
        };
        per_observation_definition.push(resolved);
    }

    let mut slots: BTreeSet<HistorySlotKey> = BTreeSet::new();
    for (observation, definition) in observations.iter().zip(&per_observation_definition) {
        if let Some(definition) = definition {
            slots.insert(HistorySlotKey {
                definition_id: definition.definition_id.clone(),
                statement_basis: scope.clone(),
                attribution_eff: observation
                    .attribution
                    .clone()
                    .unwrap_or_else(|| "total".to_owned()),
                measure_window_eff: observation.measure_window.clone(),
            });
        }
    }
    let histories: HashMap<HistorySlotKey, Vec<rust_decimal::Decimal>> = state
        .financials()
        .slot_metric_histories(&run.company_id, &slots, fiscal_year, &period_type)?;

    let manifest_observations: Vec<ManifestObservationInput> = observations
        .iter()
        .zip(per_observation_definition)
        .map(|(observation, resolved)| {
            let definition = resolved.map(|resolved| {
                let key = HistorySlotKey {
                    definition_id: resolved.definition_id.clone(),
                    statement_basis: scope.clone(),
                    attribution_eff: observation
                        .attribution
                        .clone()
                        .unwrap_or_else(|| "total".to_owned()),
                    measure_window_eff: observation.measure_window.clone(),
                };
                let history = histories.get(&key).cloned().unwrap_or_default();
                ResolvedDefinitionInput {
                    definition_id: resolved.definition_id,
                    metric_key: resolved.metric_key,
                    value_kind: resolved.value_kind,
                    period_nature: resolved.period_nature,
                    history: SlotHistoryInput { values: history },
                }
            });
            ManifestObservationInput {
                observation_id: observation.id.clone(),
                ordinal: observation.ordinal,
                raw_label: observation.raw_label.clone(),
                raw_value: observation.raw_value.clone(),
                metric_key_candidate: observation.metric_key_candidate.clone(),
                mapping_status: observation.mapping_status.clone(),
                exclusion_reason: observation.exclusion_reason.clone(),
                normalized_value: observation.normalized_value.clone(),
                currency: observation.currency.clone(),
                unit_scale: observation.unit_scale.clone(),
                measure_window: observation.measure_window.clone(),
                attribution: observation.attribution.clone(),
                scope: observation.scope.clone(),
                citation_page: observation.citation_page,
                citation_table: observation.citation_table.clone(),
                citation_row: observation.citation_row.clone(),
                citation_quote: observation.citation_quote.clone(),
                definition,
            }
        })
        .collect();

    let run_input = ManifestRunInput {
        run_id: run.id.clone(),
        revision,
        company_id: run.company_id.clone(),
        report_document_id: run.report_document_id.clone(),
        source_content_hash: run.source_content_hash.clone(),
        scope,
        data_quality,
        period_id: run.period_id.clone(),
        fiscal_year,
        period_type,
        missing_reasons: parse_missing_reasons(run.missing_reasons_json.as_deref()),
        expected: Some(expected),
    };

    let manifest = build_manifest(&run_input, &manifest_observations);
    let sealed = SealedManifest::seal(manifest.clone()).map_err(|e| {
        CommandError::new(
            CommandErrorCode::Internal,
            format!("manifest builder produced an inconsistent manifest: {e}"),
        )
    })?;

    let outcome = sealed.outcome().as_str();
    let manifest_hash = sealed.manifest_hash().to_owned();
    staging_store.apply_validation_outcome(run_id, revision, sealed)?;

    Ok(KpiIngestValidationOutcome {
        manifest,
        manifest_hash,
        outcome,
    })
}

#[cfg(test)]
mod tests;
