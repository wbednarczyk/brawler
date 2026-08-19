//! Cursor encoding and section-pagination mechanics for [`super`]: opaque
//! keyset cursors, the byte-budget shrink loop, and the shared error/
//! truncation helpers every section builder and the two entry points draw on.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::super::kpi_ingest::reject_control_chars;
use super::dto::{CatalogEntryDto, PlausibilityEntryDto};
use super::RESPONSE_BUDGET_BYTES;
use crate::commands::error::{CommandError, CommandErrorCode};

// ============================================================================
// Cursors — opaque base64url(JSON); `{}` = start-of-section sentinel
// (catalog/plausibility only)
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CatalogCursor {
    t: u8,
    m: String,
    d: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlausibilityCursor {
    m: String,
    d: String,
    s: String,
    a: String,
    w: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ManifestCursor {
    pub(super) attempt_id: String,
    pub(super) offset: usize,
}

/// No attempt-pinning needed (unlike [`ManifestCursor`]) — a run has at most
/// one commit receipt ever (ADR 0098 dec. 5, `UNIQUE(run_id)`).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct ReceiptCursor {
    pub(super) offset: usize,
}

/// A decoded keyset cursor: `Start` is the `{}` sentinel (emitted by the
/// default call's dynamic shrink when zero entries were retained; also
/// accepted from clients), `After` resumes strictly after the named keyset.
pub(super) enum SectionCursor<T> {
    Start,
    After(T),
}

pub(super) fn invalid_cursor() -> CommandError {
    CommandError::new(
        CommandErrorCode::InvalidInput,
        "cursor is not a cursor this tool issued",
    )
}

fn decode_cursor_value(cursor: &str) -> Result<Value, CommandError> {
    reject_control_chars("cursor", cursor)?;
    let bytes = URL_SAFE_NO_PAD
        .decode(cursor.as_bytes())
        .map_err(|_| invalid_cursor())?;
    serde_json::from_slice(&bytes).map_err(|_| invalid_cursor())
}

/// No input length cap of our own: the 1 MiB transport bound is the limit — a
/// smaller cap could reject a cursor this tool itself emitted from a legally
/// long stored metric key (a dead end ADR 0099 forbids).
pub(super) fn decode_section_cursor<T: serde::de::DeserializeOwned>(
    cursor: &str,
) -> Result<SectionCursor<T>, CommandError> {
    let value = decode_cursor_value(cursor)?;
    if value.as_object().is_some_and(|map| map.is_empty()) {
        return Ok(SectionCursor::Start);
    }
    serde_json::from_value(value)
        .map(SectionCursor::After)
        .map_err(|_| invalid_cursor())
}

pub(super) fn encode_cursor<T: Serialize>(cursor: &T) -> String {
    URL_SAFE_NO_PAD.encode(serde_json::to_vec(cursor).expect("cursor serialization is total"))
}

pub(super) fn start_sentinel_cursor() -> String {
    URL_SAFE_NO_PAD.encode(b"{}")
}

// ============================================================================
// Shared helpers
// ============================================================================

pub(super) fn internal(message: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::Internal, message.into())
}

pub(super) fn budget_refusal(message: impl Into<String>) -> CommandError {
    CommandError::new(CommandErrorCode::ResponseBudgetExceeded, message.into())
}

/// Byte-bound an output string on a char boundary, marking truncation with a
/// trailing `…` (total stays ≤ `max`).
pub(super) fn truncate_bytes(value: &str, max: usize) -> String {
    const MARKER: &str = "…";
    if value.len() <= max {
        return value.to_owned();
    }
    let mut end = max.saturating_sub(MARKER.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{MARKER}", &value[..end])
}

pub(super) fn serialized_len<T: Serialize>(value: &T) -> Result<usize, CommandError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| internal(format!("response serialization failed: {error}")))
}

// ============================================================================
// get_kpi_ingest_context
// ============================================================================

pub(super) fn catalog_cursor_for(entry: &CatalogEntryDto) -> String {
    encode_cursor(&CatalogCursor {
        t: entry.tier(),
        m: entry.metric_key().to_owned(),
        d: entry.definition_id().to_owned(),
    })
}

pub(super) fn plausibility_cursor_for(entry: &PlausibilityEntryDto) -> String {
    encode_cursor(&PlausibilityCursor {
        m: entry.metric_key.clone(),
        d: entry.slot.definition_id.clone(),
        s: entry.slot.scope.clone(),
        a: entry.slot.attribution.clone(),
        w: Some(entry.slot.measure_window.clone()),
    })
}

pub(super) fn catalog_start_index(
    entries: &[CatalogEntryDto],
    cursor: &SectionCursor<CatalogCursor>,
) -> usize {
    match cursor {
        SectionCursor::Start => 0,
        SectionCursor::After(after) => entries.partition_point(|entry| {
            (entry.tier(), entry.metric_key(), entry.definition_id())
                <= (after.t, after.m.as_str(), after.d.as_str())
        }),
    }
}

pub(super) fn plausibility_start_index(
    entries: &[PlausibilityEntryDto],
    cursor: &SectionCursor<PlausibilityCursor>,
) -> usize {
    match cursor {
        SectionCursor::Start => 0,
        SectionCursor::After(after) => {
            let after_window = after.w.clone().unwrap_or_default();
            entries.partition_point(|entry| {
                (
                    entry.metric_key.as_str(),
                    entry.slot.definition_id.as_str(),
                    entry.slot.scope.as_str(),
                    entry.slot.attribution.as_str(),
                    entry.slot.measure_window.as_str(),
                ) <= (
                    after.m.as_str(),
                    after.d.as_str(),
                    after.s.as_str(),
                    after.a.as_str(),
                    after_window.as_str(),
                )
            })
        }
    }
}

pub(super) fn validate_section_limit(
    limit: Option<i64>,
    cap: usize,
    section: &str,
) -> Result<usize, CommandError> {
    let limit = limit.unwrap_or(cap as i64);
    if limit < 1 || limit > cap as i64 {
        return Err(budget_refusal(format!(
            "limit {limit} is outside 1..={cap} for the {section} section"
        )));
    }
    Ok(limit as usize)
}

/// Shrink a section page until the response fits the budget: drop trailing
/// entries (they reappear on the next page via the cursor); a single entry
/// that cannot fit alone is a typed refusal — the documented floor, reachable
/// only with pre-bound legacy identities.
pub(super) fn fit_section_page<T: Clone>(
    mut page: Vec<T>,
    mut has_more: bool,
    cursor_for: impl Fn(&T) -> String,
    render: impl Fn(&[T], Option<String>) -> Result<usize, CommandError>,
    describe: impl Fn(&T) -> String,
) -> Result<(Vec<T>, Option<String>), CommandError> {
    loop {
        let cursor = match (has_more, page.last()) {
            (true, Some(last)) => Some(cursor_for(last)),
            _ => None,
        };
        if render(&page, cursor.clone())? <= RESPONSE_BUDGET_BYTES {
            return Ok((page, cursor));
        }
        if page.len() <= 1 {
            let offender = page
                .first()
                .map(&describe)
                .unwrap_or_else(|| "the section baseline".to_owned());
            return Err(budget_refusal(format!(
                "a single entry ({offender}) exceeds the 256 KiB response budget on its own — \
                 pre-bound legacy data; repair the stored row"
            )));
        }
        page.pop();
        has_more = true;
    }
}
