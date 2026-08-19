//! Validator-equivalent plausibility evidence for [`super`]: the observed and
//! candidate slot builder feeding `default_context` and the `plausibility`
//! section page.

use std::collections::BTreeSet;

use rust_decimal::Decimal;

use super::catalog::candidate_window;
use super::dto::{PlausibilityEntryDto, RecentPointDto, SlotDto};
use super::paging::truncate_bytes;
use super::{ABSTENTION_REASON_MAX, RECENT_POINTS_MAX};
use crate::commands::error::CommandError;
use crate::fundamentals::kpi_manifest::{abstention_reason_str, decimal_str};
use crate::fundamentals::validation::{
    history_median, is_split_sensitive_metric, AbstentionReason,
};
use crate::storage::{AppState, HistoryPoint, HistorySlotKey, KpiDefinition, KpiIngestRun};

use super::super::kpi_ingest::period_dto;

fn abstention_for(metric_key: &str, median: Option<&Decimal>) -> Option<&'static str> {
    if is_split_sensitive_metric(metric_key) {
        return Some(abstention_reason_str(AbstentionReason::SplitSensitive));
    }
    if median.is_none() {
        return Some(abstention_reason_str(AbstentionReason::ThinHistory));
    }
    None
}

/// Total chronology for `recentPoints`: effective period end (stored
/// `period_end_date`, else the calendar-year derivation the comparison module
/// uses), then fiscal year, then an intra-year period rank, then the period
/// label itself — deterministic under ties.
fn point_sort_key(point: &HistoryPoint) -> (String, i64, u8, String) {
    let effective_end = point.period_end.clone().unwrap_or_else(|| {
        let suffix = match point.period_type.as_str() {
            "Q1" => "-03-31",
            "Q2" | "H1" => "-06-30",
            "Q3" | "9M" => "-09-30",
            _ => "-12-31",
        };
        format!("{}{}", point.fiscal_year, suffix)
    });
    let rank = match point.period_type.as_str() {
        "Q1" => 1,
        "Q2" => 2,
        "H1" => 3,
        "Q3" => 4,
        "9M" => 5,
        "Q4" => 6,
        "H2" => 7,
        "FY" => 8,
        _ => 0,
    };
    (
        effective_end,
        point.fiscal_year,
        rank,
        point.period_type.clone(),
    )
}

fn plausibility_entry(
    metric_key: &str,
    slot: &HistorySlotKey,
    slot_origin: &'static str,
    points: &[HistoryPoint],
) -> PlausibilityEntryDto {
    let values: Vec<Decimal> = points.iter().map(|point| point.value).collect();
    let median = history_median(&values);
    let non_zero_count = values.iter().filter(|value| !value.is_zero()).count();
    let mut recent: Vec<&HistoryPoint> = points.iter().collect();
    recent.sort_by_key(|point| point_sort_key(point));
    let recent_points = recent
        .iter()
        .rev()
        .take(RECENT_POINTS_MAX)
        .rev()
        .map(|point| RecentPointDto {
            fiscal_year: point.fiscal_year,
            period_type: point.period_type.clone(),
            value: decimal_str(point.value),
        })
        .collect();
    PlausibilityEntryDto {
        metric_key: metric_key.to_owned(),
        slot: SlotDto {
            definition_id: slot.definition_id.clone(),
            scope: slot.statement_basis.clone(),
            attribution: slot.attribution_eff.clone(),
            measure_window: slot
                .measure_window_eff
                .clone()
                .unwrap_or_else(|| "flow".to_owned()),
        },
        slot_origin,
        median: median.map(decimal_str),
        non_zero_count,
        abstention_reason: abstention_for(metric_key, median.as_ref())
            .map(|reason| truncate_bytes(reason, ABSTENTION_REASON_MAX)),
        recent_points,
    }
}

/// Validator-equivalent plausibility evidence: per expected definition, every
/// slot the fact store actually realizes (`observed`, filtered to the run's
/// scope — both bases while the scope is unattached, which is evidence FOR the
/// scope choice) plus the recommended default `candidate` slot a staged
/// observation would land in. A candidate that matches an observed slot is the
/// observed entry; a pure candidate always has an empty history.
pub(super) fn build_plausibility(
    state: &AppState,
    run: &KpiIngestRun,
    resolved_expected: &[(String, KpiDefinition)],
) -> Result<Vec<PlausibilityEntryDto>, CommandError> {
    let definition_ids: BTreeSet<String> = resolved_expected
        .iter()
        .map(|(_, definition)| definition.id.clone())
        .collect();
    if definition_ids.is_empty() {
        return Ok(Vec::new());
    }
    let (exclude_fy, exclude_pt) = match period_dto(state, run)? {
        Some(period) => (period.fiscal_year, period.period_type),
        // No period attached yet: exclude nothing (fy 0 matches no period).
        None => (0, String::new()),
    };
    let points_by_slot = state
        .financials()
        .slot_history_points(&run.company_id, &definition_ids, exclude_fy, &exclude_pt)
        .map_err(CommandError::from)?;
    let scopes: Vec<&str> = match run.scope.as_deref() {
        Some(scope) => vec![scope],
        None => vec!["standalone", "consolidated"],
    };

    let mut entries: Vec<PlausibilityEntryDto> = Vec::new();
    for (metric_key, definition) in resolved_expected {
        let mut seen_slots: BTreeSet<HistorySlotKey> = BTreeSet::new();
        for (slot, points) in &points_by_slot {
            if slot.definition_id != definition.id {
                continue;
            }
            if !scopes.contains(&slot.statement_basis.as_str()) {
                continue;
            }
            seen_slots.insert(slot.clone());
            entries.push(plausibility_entry(metric_key, slot, "observed", points));
        }
        for scope in &scopes {
            let candidate = HistorySlotKey {
                definition_id: definition.id.clone(),
                statement_basis: (*scope).to_owned(),
                attribution_eff: "total".to_owned(),
                measure_window_eff: Some(
                    candidate_window(&run.profile_version, &definition.period_nature).to_owned(),
                ),
            };
            if seen_slots.contains(&candidate) {
                continue;
            }
            entries.push(plausibility_entry(metric_key, &candidate, "candidate", &[]));
        }
    }
    entries.sort_by(|a, b| {
        (
            a.metric_key.as_str(),
            a.slot.definition_id.as_str(),
            a.slot.scope.as_str(),
            a.slot.attribution.as_str(),
            a.slot.measure_window.as_str(),
        )
            .cmp(&(
                b.metric_key.as_str(),
                b.slot.definition_id.as_str(),
                b.slot.scope.as_str(),
                b.slot.attribution.as_str(),
                b.slot.measure_window.as_str(),
            ))
    });
    Ok(entries)
}
