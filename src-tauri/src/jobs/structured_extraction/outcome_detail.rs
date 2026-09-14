//! Outcome-detail composition for `run_structured_extraction`: the per-fact
//! hold-backs (history quarantine, #509 store refusals) and the JSON merge that
//! folds them onto the gate's failing checks. A child module so the pinned
//! parent stays under its file-size ratchet.

/// One fact held back by the runtime history-plausibility gate — its magnitude
/// was ≥100× off its own stored history (a dropped `w tys.` multiplier or a note
/// reference read as the value). Carries the figures the flagged-outcome detail
/// cites so a reviewer sees *why* it was quarantined.
pub(super) struct QuarantinedFact {
    pub(super) metric_key: String,
    pub(super) value: rust_decimal::Decimal,
    pub(super) history_median: rust_decimal::Decimal,
}

/// One fact refused by a fact-local store guard (#509 decision 2) — held back, never a set-level abort.
pub(super) struct RejectedFact {
    pub(super) metric_key: String,
    pub(super) field: &'static str,
    pub(super) value: String,
}

/// Whether `key` names a per-fact write-slot field (#509 decision 2) — a
/// refusal on one of these costs a single fact, never the whole set. Any
/// other key (e.g. `period_type`, a shared run-context field) is fatal.
pub(super) fn is_fact_local_refusal(key: &str) -> bool {
    matches!(
        key,
        "currency" | "value_numeric" | "attribution" | "data_quality"
    )
}

/// Object-merges a detail-payload array under `key` onto `base` (never
/// nested) — the shared core `quarantine_detail`/`rejected_detail` use.
fn merge_detail_array(
    key: &str,
    items: Vec<serde_json::Value>,
    base: Option<String>,
) -> Option<String> {
    let mut payload = base
        .as_deref()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    payload.insert(key.to_owned(), serde_json::Value::Array(items));
    serde_json::to_string(&serde_json::Value::Object(payload)).ok()
}

/// The `validation_failed` detail for facts held back by the history-plausibility gate.
pub(super) fn quarantine_detail(
    quarantined: &[QuarantinedFact],
    base: Option<String>,
) -> Option<String> {
    let facts = quarantined
        .iter()
        .map(|q| {
            serde_json::json!({ "metricKey": q.metric_key, "value": q.value.to_string(), "historyMedian": q.history_median.to_string() })
        })
        .collect();
    merge_detail_array("quarantinedFacts", facts, base)
}

/// The `validation_failed` detail for facts REJECTED by a fact-local store guard (#509 decision 2).
pub(super) fn rejected_detail(rejected: &[RejectedFact], base: Option<String>) -> Option<String> {
    let facts = rejected
        .iter()
        .map(|r| serde_json::json!({ "metricKey": r.metric_key, "field": r.field, "value": r.value }))
        .collect();
    merge_detail_array("rejectedFacts", facts, base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_fact_local_refusal_matches_exactly_the_four_per_fact_write_slot_fields() {
        for key in ["currency", "value_numeric", "attribution", "data_quality"] {
            assert!(is_fact_local_refusal(key), "{key} must be fact-local");
        }
        for key in [
            "period_type",
            "metric_key",
            "origin",
            "statement_group",
            "unknown_key",
        ] {
            assert!(!is_fact_local_refusal(key), "{key} must NOT be fact-local");
        }
    }
}
