//! The expected-keys resolution and full catalog assembly for [`super`]:
//! feeds both `default_context` and the `catalog` section page.

use std::collections::{BTreeSet, HashMap};

use serde_json::Value;

use super::dto::CatalogEntryDto;
use super::paging::{internal, truncate_bytes};
use super::{ResolvedExpected, LABEL_MAX, STATEMENT_GROUP_MAX, UNIT_MAX};
use crate::commands::error::CommandError;
use crate::fundamentals::metrics::measure_window_for;
use crate::storage::{AppState, KpiDefinition, KpiIngestRun};

/// The run's expected metric keys: the creation-time stamp verbatim; a legacy
/// NULL row computes the validator's live fallback (`kpi_relevance` only) —
/// WITHOUT stamping, this is a pure read.
pub(super) fn expected_keys(
    state: &AppState,
    run: &KpiIngestRun,
) -> Result<BTreeSet<String>, CommandError> {
    if let Some(stored) = run.expected_kpis_json.as_deref() {
        let parsed: Value = serde_json::from_str(stored)
            .map_err(|_| internal("stored expected_kpis_json is not valid JSON"))?;
        let Some(keys) = parsed.get("keys").and_then(Value::as_array) else {
            return Err(internal("stored expected_kpis_json has no keys array"));
        };
        return keys
            .iter()
            .map(|key| {
                key.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| internal("stored expected_kpis_json carries a non-string key"))
            })
            .collect();
    }
    state
        .financials()
        .expected_primary_metric_keys(&run.company_id)
        .map_err(CommandError::from)
        .map(Option::unwrap_or_default)
}

/// Expected keys resolved to their full catalog definitions. An expected key
/// that does not resolve — or resolves to a definition the visible catalog
/// does not carry — is a data error (`internal`), never a silent omission:
/// the missing entry is exactly the repair context the agent needs.
pub(super) fn resolved_expected_definitions(
    state: &AppState,
    run: &KpiIngestRun,
    expected: &BTreeSet<String>,
    definitions: &[KpiDefinition],
) -> Result<ResolvedExpected, CommandError> {
    let by_id: HashMap<&str, &KpiDefinition> = definitions
        .iter()
        .map(|definition| (definition.id.as_str(), definition))
        .collect();
    let mut resolved = Vec::with_capacity(expected.len());
    for key in expected {
        let hit = state
            .kpi_extraction()
            .resolve_kpi_definition(&run.company_id, key)
            .map_err(CommandError::from)?
            .ok_or_else(|| {
                internal(format!(
                    "expected metric key {key} does not resolve to any KPI definition"
                ))
            })?;
        let definition = by_id.get(hit.definition_id.as_str()).ok_or_else(|| {
            internal(format!(
                "expected metric key {key} resolved to definition {} outside the visible catalog",
                hit.definition_id
            ))
        })?;
        resolved.push((key.clone(), (*definition).clone()));
    }
    Ok(resolved)
}

fn catalog_entry_full(definition: &KpiDefinition) -> CatalogEntryDto {
    CatalogEntryDto::Full {
        definition_id: definition.id.clone(),
        metric_key: definition.metric_key.clone(),
        label: truncate_bytes(&definition.label, LABEL_MAX),
        unit: definition
            .unit
            .as_deref()
            .map(|unit| truncate_bytes(unit, UNIT_MAX)),
        statement_group: truncate_bytes(&definition.statement_group, STATEMENT_GROUP_MAX),
        value_kind: definition.value_kind.clone(),
        origin: definition.origin.clone(),
    }
}

fn catalog_entry_compact(definition: &KpiDefinition) -> CatalogEntryDto {
    CatalogEntryDto::Compact {
        definition_id: definition.id.clone(),
        metric_key: definition.metric_key.clone(),
        label: truncate_bytes(&definition.label, LABEL_MAX),
    }
}

/// The full sorted catalog (ADR 0101 dec. 7): FULL metadata for the expected
/// keys plus the company's minted extras (`origin == "agent"` — ADR 0093
/// dec. 4 vocabulary), COMPACT `{metricKey, label}` for every other canonical
/// (shared, `company_id IS NULL`) definition — the full canon, not just what
/// this run happened to ask for. Deduped by definition id, sorted by
/// `(tier, metricKey, definitionId)`: tier keeps every Full entry ahead of
/// every Compact entry so the default call's truncated first page is never
/// dominated by canon the agent didn't ask about.
pub(super) fn build_catalog(
    resolved_expected: &[(String, KpiDefinition)],
    definitions: &[KpiDefinition],
    company_id: &str,
    statement_type: &str,
) -> Vec<CatalogEntryDto> {
    let mut by_id: HashMap<String, CatalogEntryDto> = HashMap::new();
    for (_, definition) in resolved_expected {
        by_id
            .entry(definition.id.clone())
            .or_insert_with(|| catalog_entry_full(definition));
    }
    for definition in definitions {
        if definition.origin == "agent" && definition.company_id.as_deref() == Some(company_id) {
            by_id
                .entry(definition.id.clone())
                .or_insert_with(|| catalog_entry_full(definition));
        }
    }
    for definition in definitions {
        // §G harvest (epic #399 S8): a sector-scoped row is offered ONLY to
        // companies of that statement type — anything else would never
        // resolve at staging and just baits a `mapping.unresolved` flag.
        let sector_visible = match definition.sector.as_deref() {
            None => true,
            Some(sector) => sector == statement_type,
        };
        if definition.company_id.is_none() && sector_visible {
            by_id
                .entry(definition.id.clone())
                .or_insert_with(|| catalog_entry_compact(definition));
        }
    }
    let mut entries: Vec<CatalogEntryDto> = by_id.into_values().collect();
    entries.sort_by(|a, b| {
        (a.tier(), a.metric_key(), a.definition_id()).cmp(&(
            b.tier(),
            b.metric_key(),
            b.definition_id(),
        ))
    });
    entries
}

/// The candidate default slot's measure window (ADR 0100 decision 6): the
/// definition's `period_nature` decides the axis -- `instant` is always
/// `point_in_time`, regardless of `value_kind` -- and the PROFILE picks the
/// duration window: interim publications are cumulative (ADR 0098 dec. 3),
/// everything else stages plain `flow`. TTM eligibility (ratio/percentage)
/// plays no role here, a distinct axis (`is_ttm_eligible`): a ratio is still
/// duration-reported and gets a flow/cumulative candidate slot.
pub(super) fn candidate_window(profile_version: &str, period_nature: &str) -> &'static str {
    measure_window_for(Some(period_nature), Some(profile_version))
}
