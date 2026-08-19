//! Row-mapping, id-generation and validation helpers for
//! [`super::KpiIngestStagingStore`] — internal to the
//! `kpi_ingest_staging` module tree (`pub(super)` only).

use std::str::FromStr;

use rust_decimal::Decimal;

use super::*;

pub(super) fn map_observation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<StagedObservation> {
    Ok(StagedObservation {
        id: row.get(0)?,
        run_id: row.get(1)?,
        revision: row.get(2)?,
        ordinal: row.get(3)?,
        raw_label: row.get(4)?,
        raw_value: row.get(5)?,
        raw_currency: row.get(6)?,
        raw_unit_scale: row.get(7)?,
        normalized_value: row.get(8)?,
        currency: row.get(9)?,
        unit_scale: row.get(10)?,
        measure_window: row.get(11)?,
        attribution: row.get(12)?,
        scope: row.get(13)?,
        metric_key_candidate: row.get(14)?,
        mapping_status: row.get(15)?,
        citation_page: row.get(16)?,
        citation_table: row.get(17)?,
        citation_row: row.get(18)?,
        citation_quote: row.get(19)?,
        validation_state: row.get(20)?,
        validation_codes_json: row.get(21)?,
        created_at: row.get(22)?,
        updated_at: row.get(23)?,
        exclusion_reason: row.get(24)?,
    })
}

pub(super) fn map_receipt_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CommitReceipt> {
    Ok(CommitReceipt {
        id: row.get(0)?,
        run_id: row.get(1)?,
        manifest_hash: row.get(2)?,
        manifest_revision: row.get(3)?,
        terminal_status: row.get(4)?,
        period_id: row.get(5)?,
        accepted_count: row.get(6)?,
        outcomes_schema_version: row.get(7)?,
        outcomes_json: row.get(8)?,
        committed_at: row.get(9)?,
    })
}

/// Mirrors `storage::financials::normalize_currency`, which is private to
/// that module (checked — not re-exported): trimmed, empty → absent,
/// otherwise exactly three ASCII letters upper-cased into the ISO-4217 shape.
/// Duplicated rather than made `pub(super)` in `financials.rs` because that
/// module treats it as an internal write-boundary helper for
/// `financial_facts`, a different table with its own error variant; keep
/// both in sync if the ISO-4217 shape rule ever changes.
pub(super) fn normalize_currency(currency: Option<String>) -> StorageResult<Option<String>> {
    let Some(currency) = empty_string_to_none(currency.map(|s| s.trim().to_owned())) else {
        return Ok(None);
    };
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(StorageError::InvalidKpiIngestRunValue {
            key: "currency",
            value: currency,
        });
    }
    Ok(Some(currency.to_ascii_uppercase()))
}

pub(super) fn validate_vocab(
    key: &'static str,
    value: &Option<String>,
    allowed: &[&str],
) -> StorageResult<()> {
    if let Some(value) = value.as_deref() {
        if !allowed.contains(&value) {
            return Err(StorageError::InvalidKpiIngestRunValue {
                key,
                value: value.to_owned(),
            });
        }
    }
    Ok(())
}

/// Collision-safe, non-deterministic id (the `generate_run_id` idiom,
/// `kpi_ingest_runs.rs`): `kpiobs_` + 32 hex chars of sha256 over the
/// identity plus a nanosecond time component.
pub(super) fn generate_observation_id(run_id: &str, revision: i64, ordinal: i64) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpiobs:{run_id}\u{1f}{revision}\u{1f}{ordinal}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpiobs_{hex}")
}

/// Collision-safe id for one `kpi_ingest_validation_attempts` row (the
/// `generate_observation_id` idiom): `kpiatt_` + 32 hex chars of sha256 over
/// the identity plus a nanosecond time component. `attempt` is part of the
/// hashed key (not just `run_id`/`revision`) purely for extra collision
/// distance across the rare case of two attempts landing at the same
/// nanosecond — the `UNIQUE(run_id, revision, attempt)` constraint is the
/// real invariant guard, not this id's uniqueness.
pub(super) fn generate_attempt_id(run_id: &str, revision: i64, attempt: i64) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpiatt:{run_id}\u{1f}{revision}\u{1f}{attempt}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpiatt_{hex}")
}

/// `normalized_value` equality for the content-tamper guard (finding 2):
/// the manifest side is Decimal-canonical (`fundamentals::kpi_manifest`'s
/// `canonical_normalized_value`) while the live row keeps the raw staged
/// string, so a byte comparison would false-positive on a coherent value
/// carrying trailing zeros (e.g. row `"1234.500"` vs projection `"1234.5"`).
/// Parse both sides and compare numerically; fall back to verbatim string
/// equality when either side does not parse (mirrors `value.unparseable`'s
/// own carry-verbatim rule).
pub(super) fn normalized_values_match(
    row_value: &Option<String>,
    projection_value: &Option<String>,
) -> bool {
    match (row_value, projection_value) {
        (None, None) => true,
        (Some(a), Some(b)) => match (Decimal::from_str(a.trim()), Decimal::from_str(b.trim())) {
            (Ok(da), Ok(db)) => da == db,
            _ => a == b,
        },
        _ => false,
    }
}

/// The sol r5-2 content-tamper guard: whether a [`SealedManifest`]'s
/// canonical staged-content projection for one observation matches the LIVE
/// `kpi_staged_observations` row, dimension for dimension (effective scale/
/// attribution defaulted the SAME way `unit.scale_incoherent`'s `scale_eff`
/// and the `attribution_eff` slot key are, everything else compared raw).
pub(super) fn content_matches_projection(
    row: &StagedObservation,
    projection: &crate::fundamentals::kpi_manifest::ObservationContentProjection,
) -> bool {
    row.ordinal == projection.ordinal
        && row.raw_label == projection.raw_label
        && (row.mapping_status == "excluded") == projection.excluded
        && row.exclusion_reason == projection.exclusion_reason
        && row.metric_key_candidate == projection.metric_key_candidate
        && normalized_values_match(&row.normalized_value, &projection.normalized_value)
        && row.currency == projection.currency
        && row.unit_scale.as_deref().unwrap_or("ones") == projection.unit_scale_eff
        && row.attribution.as_deref().unwrap_or("total") == projection.attribution_eff
        && row.measure_window == projection.measure_window_eff
        && row.scope == projection.scope
        && row.citation_page == projection.citation_page
        && row.citation_table == projection.citation_table
        && row.citation_row == projection.citation_row
        && row.citation_quote == projection.citation_quote
}

pub(super) fn generate_receipt_id(run_id: &str) -> String {
    use sha2::{Digest, Sha256};
    let now_nanos = time::OffsetDateTime::now_utc().unix_timestamp_nanos();
    let key = format!("kpircpt:{run_id}\u{1f}{now_nanos}");
    let digest = Sha256::digest(key.as_bytes());
    let mut hex = String::with_capacity(32);
    for byte in &digest[..16] {
        hex.push_str(&format!("{byte:02x}"));
    }
    format!("kpircpt_{hex}")
}
