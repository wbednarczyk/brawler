//! Acquisition-workflow context read model (#385, ADR 0099): the two pure
//! reads of the ten-tool surface (ADR 0101, epic #399 S4) —
//! `get_kpi_ingest_context` (everything one
//! report's extraction needs, within hard response budgets: run state,
//! document metadata, the hash-guarded derived-period hint, the expected+
//! minted definition catalog, validator-equivalent plausibility evidence,
//! profile doctrine and the paginated repair manifest) and
//! `get_kpi_ingest_document` (chunked bytes from the run's content-addressed
//! blob, verified against the frozen `source_content_hash` — the only portable
//! byte channel).
//!
//! Budgets are runtime mechanisms (ADR 0099 dec. 7): sections are capped and
//! keyset-paginated, output strings are byte-bounded with `…` truncation, the
//! default call dynamically shrinks its pageable sections to stay ≤256 KiB
//! (overflow always leaves a cursor, never a dead end), and unsatisfiable
//! requests refuse with `response_budget_exceeded`.

use std::collections::{BTreeSet, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde_json::Value;

use super::kpi_ingest::{
    get_existing_run, is_content_hash, reject_control_chars, run_status_dto, SNAPSHOT_DIR,
};
use super::tools::{run, ToolCallError, ToolOutcome};
use crate::commands::error::{CommandError, CommandErrorCode};
use crate::storage::{
    AppState, KpiDefinition, KpiIngestRun, KpiIngestRunState, ListKpiDefinitionsInput,
};

mod catalog;
mod document;
mod dto;
mod paging;
mod plausibility;
#[cfg(test)]
mod tests;

use catalog::{build_catalog, expected_keys, resolved_expected_definitions};
use document::{derived_period_hint, document_meta, profile_rules};
pub use dto::{
    CatalogEntryDto, ContextDto, ContextSection, DerivedPeriodDto, DocumentChunkDto,
    DocumentMetaDto, GetKpiIngestContextInput, GetKpiIngestDocumentInput, PlausibilityEntryDto,
    RecentPointDto, SectionDto, SlotDto, TruncatedDto,
};
use paging::{
    budget_refusal, catalog_cursor_for, catalog_start_index, decode_section_cursor, encode_cursor,
    fit_section_page, internal, invalid_cursor, plausibility_cursor_for, plausibility_start_index,
    serialized_len, start_sentinel_cursor, validate_section_limit, CatalogCursor, ManifestCursor,
    PlausibilityCursor, ReceiptCursor, SectionCursor,
};
use plausibility::build_plausibility;

// ============================================================================
// Budgets (contracts.md § Budgets — frozen numbers)
// ============================================================================

const CATALOG_PAGE_MAX: usize = 64;
const PLAUSIBILITY_PAGE_MAX: usize = 64;
const MANIFEST_PAGE_MAX: usize = 50;
/// The paged commit receipt (ADR 0102 dec. 12, epic #399 S6) — mirrors
/// `MANIFEST_PAGE_MAX`; `commit_kpi_ingest`'s own response stays a bounded
/// summary, the full outcomes ledger (up to `AGGREGATE_OBSERVATIONS_MAX`
/// 1000 rows) is read here, paginated.
const RECEIPT_PAGE_MAX: usize = 50;
const RECENT_POINTS_MAX: usize = 8;
/// Every context response is ≤256 KiB; a document chunk is ≤256 KiB of RAW
/// bytes (its base64 envelope may exceed this — the chunk cap is the budget).
const RESPONSE_BUDGET_BYTES: usize = 262_144;
const DOCUMENT_CHUNK_MAX: u64 = 262_144;

// Output-string byte caps (per-field `…` truncation, contracts.md § Budgets).
const LABEL_MAX: usize = 256;
const UNIT_MAX: usize = 64;
const STATEMENT_GROUP_MAX: usize = 64;
const PROFILE_RULE_MAX: usize = 512;
const ABSTENTION_REASON_MAX: usize = 256;
const URL_MAX: usize = 512;
const TITLE_MAX: usize = 256;
const CONTENT_TYPE_MAX: usize = 128;
const LOCAL_PATH_MAX: usize = 512;

fn get_kpi_ingest_context(
    state: &AppState,
    input: GetKpiIngestContextInput,
) -> Result<Value, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    let run = get_existing_run(state, &input.run_id)?;

    match input.section {
        None => {
            if input.cursor.is_some() || input.limit.is_some() {
                return Err(CommandError::new(
                    CommandErrorCode::InvalidInput,
                    "cursor and limit apply to section calls only",
                ));
            }
            default_context(state, &run).and_then(|dto| {
                serde_json::to_value(dto)
                    .map_err(|error| internal(format!("serialization failed: {error}")))
            })
        }
        Some(section) => {
            section_context(state, &run, section, input.cursor, input.limit).and_then(|dto| {
                serde_json::to_value(dto)
                    .map_err(|error| internal(format!("serialization failed: {error}")))
            })
        }
    }
}

/// One expected metric key paired with its resolved catalog definition.
type ResolvedExpected = Vec<(String, KpiDefinition)>;

/// The expected keys resolved against the company's visible definitions —
/// the shared substrate of the catalog and plausibility sections.
fn resolved_catalog_inputs(
    state: &AppState,
    run: &KpiIngestRun,
) -> Result<(ResolvedExpected, Vec<KpiDefinition>), CommandError> {
    let expected = expected_keys(state, run)?;
    let definitions = state
        .financials()
        .list_kpi_definitions(ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: Some(run.company_id.clone()),
        })
        .map_err(CommandError::from)?;
    let resolved = resolved_expected_definitions(state, run, &expected, &definitions)?;
    Ok((resolved, definitions))
}

fn default_context(state: &AppState, run: &KpiIngestRun) -> Result<ContextDto, CommandError> {
    let run_dto = run_status_dto(state, run)?;
    let document = document_meta(state, run)?;
    let derived_period = derived_period_hint(state, run)?;
    let (resolved_expected, definitions) = resolved_catalog_inputs(state, run)?;
    let statement_type = state.companies().get_statement_type(&run.company_id)?;
    let catalog_all = build_catalog(
        &resolved_expected,
        &definitions,
        &run.company_id,
        &statement_type,
    );
    let plausibility_all = build_plausibility(state, run, &resolved_expected)?;
    let expected_metric_keys: BTreeSet<&str> = resolved_expected
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    let rules = profile_rules(run)?;
    let manifest_available = state
        .kpi_ingest_runs()
        .latest_validation_attempt(&run.id)
        .map_err(CommandError::from)?
        .is_some();

    // Cap, then dynamically shrink to the byte budget: pop the last entry of
    // the larger section until the serialized response fits. Dropped entries
    // reappear on the section pages named by the emitted cursors; zero
    // retained entries emit the `{}` start-of-section sentinel (the section
    // call's baseline is small, so its pages fit normally).
    let mut catalog_kept = catalog_all.len().min(CATALOG_PAGE_MAX);
    let mut plausibility_kept = plausibility_all.len().min(PLAUSIBILITY_PAGE_MAX);
    loop {
        let truncated = TruncatedDto {
            catalog: section_cursor_or_sentinel(&catalog_all, catalog_kept, catalog_cursor_for),
            plausibility: section_cursor_or_sentinel(
                &plausibility_all,
                plausibility_kept,
                plausibility_cursor_for,
            ),
            manifest: manifest_available.then_some(true),
        };
        let catalog_page = &catalog_all[..catalog_kept];
        let not_requested: Vec<String> = catalog_page
            .iter()
            .filter(|entry| !expected_metric_keys.contains(entry.metric_key()))
            .map(|entry| entry.metric_key().to_owned())
            .collect();
        let dto = ContextDto {
            run: run_dto.clone(),
            document: document.clone(),
            derived_period: derived_period.clone(),
            catalog: catalog_page.to_vec(),
            not_requested,
            plausibility: plausibility_all[..plausibility_kept].to_vec(),
            profile_rules: rules.clone(),
            manifest_available,
            truncated,
        };
        let size = serialized_len(&dto)?;
        if size <= RESPONSE_BUDGET_BYTES {
            return Ok(dto);
        }
        if catalog_kept == 0 && plausibility_kept == 0 {
            // Defensive gate: unreachable through current writers (every
            // baseline field is write-time bounded); refusing beats silently
            // truncating the frozen shape.
            return Err(budget_refusal(
                "the context baseline exceeds the 256 KiB response budget — pre-bound legacy \
                 data; repair the stored row",
            ));
        }
        if plausibility_kept >= catalog_kept {
            plausibility_kept -= 1;
        } else {
            catalog_kept -= 1;
        }
    }
}

/// The default call's `truncated` cursor for one pageable section: absent when
/// everything fit, the keyset of the last retained entry when trimmed, and the
/// `{}` start-of-section sentinel when nothing was retained.
fn section_cursor_or_sentinel<T>(
    all: &[T],
    kept: usize,
    cursor_for: impl Fn(&T) -> String,
) -> Option<String> {
    if kept >= all.len() {
        return None;
    }
    if kept == 0 {
        return Some(start_sentinel_cursor());
    }
    Some(cursor_for(&all[kept - 1]))
}

fn section_context(
    state: &AppState,
    run: &KpiIngestRun,
    section: ContextSection,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<SectionDto, CommandError> {
    match section {
        ContextSection::Catalog => {
            let limit = validate_section_limit(limit, CATALOG_PAGE_MAX, "catalog")?;
            let decoded: SectionCursor<CatalogCursor> = match cursor.as_deref() {
                Some(cursor) => decode_section_cursor(cursor)?,
                None => SectionCursor::Start,
            };
            let (resolved, definitions) = resolved_catalog_inputs(state, run)?;
            let statement_type = state.companies().get_statement_type(&run.company_id)?;
            let all = build_catalog(&resolved, &definitions, &run.company_id, &statement_type);
            let start = catalog_start_index(&all, &decoded);
            let end = (start + limit).min(all.len());
            let page: Vec<CatalogEntryDto> = all[start..end].to_vec();
            let has_more = end < all.len();
            let run_id = run.id.clone();
            let (page, next_cursor) = fit_section_page(
                page,
                has_more,
                catalog_cursor_for,
                |entries, next_cursor| {
                    serialized_len(&SectionDto {
                        run_id: run_id.clone(),
                        section: "catalog",
                        catalog: Some(entries.to_vec()),
                        plausibility: None,
                        manifest: None,
                        receipt: None,
                        next_cursor,
                    })
                },
                |entry| format!("catalog entry {}", entry.definition_id()),
            )?;
            Ok(SectionDto {
                run_id: run.id.clone(),
                section: "catalog",
                catalog: Some(page),
                plausibility: None,
                manifest: None,
                receipt: None,
                next_cursor,
            })
        }
        ContextSection::Plausibility => {
            let limit = validate_section_limit(limit, PLAUSIBILITY_PAGE_MAX, "plausibility")?;
            let decoded: SectionCursor<PlausibilityCursor> = match cursor.as_deref() {
                Some(cursor) => decode_section_cursor(cursor)?,
                None => SectionCursor::Start,
            };
            let (resolved, _definitions) = resolved_catalog_inputs(state, run)?;
            let all = build_plausibility(state, run, &resolved)?;
            let start = plausibility_start_index(&all, &decoded);
            let end = (start + limit).min(all.len());
            let page: Vec<PlausibilityEntryDto> = all[start..end].to_vec();
            let has_more = end < all.len();
            let run_id = run.id.clone();
            let (page, next_cursor) = fit_section_page(
                page,
                has_more,
                plausibility_cursor_for,
                |entries, next_cursor| {
                    serialized_len(&SectionDto {
                        run_id: run_id.clone(),
                        section: "plausibility",
                        catalog: None,
                        plausibility: Some(entries.to_vec()),
                        manifest: None,
                        receipt: None,
                        next_cursor,
                    })
                },
                |entry| {
                    format!(
                        "plausibility slot {}/{}",
                        entry.metric_key, entry.slot.definition_id
                    )
                },
            )?;
            Ok(SectionDto {
                run_id: run.id.clone(),
                section: "plausibility",
                catalog: None,
                plausibility: Some(page),
                manifest: None,
                receipt: None,
                next_cursor,
            })
        }
        ContextSection::Manifest => manifest_section(state, run, cursor, limit),
        ContextSection::Receipt => receipt_section(state, run, cursor, limit),
    }
}

/// The manifest section (repair context): page 1 pins the LATEST validation
/// attempt (including `failed` — the run row's `manifest_hash` is NULL after a
/// failed validation by design) and serves the full manifest header with the
/// first observation page; continuation cursors carry the pinned `attemptId`
/// plus an observation offset, so a newer attempt appearing between pages
/// never splices two manifests together.
fn manifest_section(
    state: &AppState,
    run: &KpiIngestRun,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<SectionDto, CommandError> {
    let limit = validate_section_limit(limit, MANIFEST_PAGE_MAX, "manifest")?;
    let (attempt, offset) = match cursor.as_deref() {
        None => {
            let attempt = state
                .kpi_ingest_runs()
                .latest_validation_attempt(&run.id)
                .map_err(CommandError::from)?
                .ok_or_else(|| {
                    CommandError::new(
                        CommandErrorCode::Conflict,
                        "no validation attempt exists for this run yet — check manifestAvailable",
                    )
                })?;
            (attempt, 0usize)
        }
        Some(cursor) => {
            let decoded: ManifestCursor = match decode_section_cursor(cursor)? {
                // `{}` is a catalog/plausibility-only sentinel: a manifest
                // continuation must pin its attempt.
                SectionCursor::Start => return Err(invalid_cursor()),
                SectionCursor::After(decoded) => decoded,
            };
            let attempt = state
                .kpi_ingest_runs()
                .validation_attempt_by_id(&run.id, &decoded.attempt_id)
                .map_err(CommandError::from)?
                .ok_or_else(invalid_cursor)?;
            (attempt, decoded.offset)
        }
    };

    let mut manifest: Value = serde_json::from_str(&attempt.manifest_json)
        .map_err(|_| internal("stored manifest bytes are not valid JSON"))?;
    let Some(manifest_object) = manifest.as_object_mut() else {
        return Err(internal("stored manifest is not a JSON object"));
    };
    let observations = match manifest_object.remove("observations") {
        Some(Value::Array(observations)) => observations,
        Some(_) => return Err(internal("stored manifest observations is not an array")),
        None => Vec::new(),
    };

    let start = offset.min(observations.len());
    let end = (start + limit).min(observations.len());
    let mut page: Vec<Value> = observations[start..end].to_vec();
    let mut has_more = end < observations.len();
    let attempt_id = attempt.id.clone();

    loop {
        let next_cursor = has_more.then(|| {
            encode_cursor(&ManifestCursor {
                attempt_id: attempt_id.clone(),
                offset: start + page.len(),
            })
        });
        let manifest_value = if start == 0 {
            let mut header = manifest_object.clone();
            header.insert("observations".to_owned(), Value::Array(page.clone()));
            Value::Object(header)
        } else {
            serde_json::json!({ "observations": page })
        };
        let dto = SectionDto {
            run_id: run.id.clone(),
            section: "manifest",
            catalog: None,
            plausibility: None,
            manifest: Some(manifest_value),
            receipt: None,
            next_cursor: next_cursor.clone(),
        };
        if serialized_len(&dto)? <= RESPONSE_BUDGET_BYTES {
            return Ok(dto);
        }
        if page.is_empty() {
            return Err(budget_refusal(
                "the manifest header alone exceeds the 256 KiB response budget — pre-bound \
                 legacy data; invalidate and re-validate the run",
            ));
        }
        page.pop();
        has_more = true;
    }
}

/// The paged commit receipt (ADR 0102 dec. 12): unlike the manifest section,
/// no attempt pinning is needed — a run has AT MOST one commit receipt EVER
/// (ADR 0098 dec. 5, immutable, `UNIQUE(run_id)`), so the cursor is a bare
/// offset into the stored `outcomes` array. No receipt yet → `conflict`
/// (mirrors `manifestAvailable`'s "no attempt yet" gate).
fn receipt_section(
    state: &AppState,
    run: &KpiIngestRun,
    cursor: Option<String>,
    limit: Option<i64>,
) -> Result<SectionDto, CommandError> {
    let limit = validate_section_limit(limit, RECEIPT_PAGE_MAX, "receipt")?;
    let offset = match cursor.as_deref() {
        None => 0usize,
        Some(cursor) => {
            let decoded: ReceiptCursor = match decode_section_cursor(cursor)? {
                SectionCursor::Start => return Err(invalid_cursor()),
                SectionCursor::After(decoded) => decoded,
            };
            decoded.offset
        }
    };

    let receipt = state
        .kpi_ingest_staging()
        .get_commit_receipt(&run.id)
        .map_err(CommandError::from)?
        .ok_or_else(|| {
            CommandError::new(
                CommandErrorCode::Conflict,
                "no commit receipt exists for this run yet",
            )
        })?;
    let outcomes: Vec<Value> = serde_json::from_str(&receipt.outcomes_json)
        .map_err(|_| internal("stored receipt outcomes are not valid JSON"))?;

    let start = offset.min(outcomes.len());
    let end = (start + limit).min(outcomes.len());
    let mut page: Vec<Value> = outcomes[start..end].to_vec();
    let mut has_more = end < outcomes.len();

    loop {
        let next_cursor = has_more.then(|| {
            encode_cursor(&ReceiptCursor {
                offset: start + page.len(),
            })
        });
        let receipt_value = if start == 0 {
            serde_json::json!({
                "runId": receipt.run_id,
                "terminalStatus": receipt.terminal_status,
                "periodId": receipt.period_id,
                "acceptedCount": receipt.accepted_count,
                "outcomesSchemaVersion": receipt.outcomes_schema_version,
                "manifestHash": receipt.manifest_hash,
                "manifestRevision": receipt.manifest_revision,
                "committedAt": receipt.committed_at,
                "outcomes": page,
            })
        } else {
            serde_json::json!({ "outcomes": page })
        };
        let dto = SectionDto {
            run_id: run.id.clone(),
            section: "receipt",
            catalog: None,
            plausibility: None,
            manifest: None,
            receipt: Some(receipt_value),
            next_cursor: next_cursor.clone(),
        };
        if serialized_len(&dto)? <= RESPONSE_BUDGET_BYTES {
            return Ok(dto);
        }
        if page.is_empty() {
            return Err(budget_refusal(
                "the receipt header alone exceeds the 256 KiB response budget",
            ));
        }
        page.pop();
        has_more = true;
    }
}

// ============================================================================
// get_kpi_ingest_document
// ============================================================================

/// Process-wide verified-blob cache: full-buffer hash verification happens
/// once per (canonical path, hash, size, mtime); later chunk reads seek. Keyed
/// by the canonical path so one data dir can never authorize another's
/// same-named blob. Metadata-preserving external replacement inside the
/// app-owned content-addressed store is outside this boundary (documented,
/// data-model § blobs).
/// Cache key: (canonical blob path, frozen hash) → verified (size, mtime).
type VerifiedBlobKey = (PathBuf, String);
type VerifiedBlobStamp = (u64, SystemTime);
static VERIFIED_BLOBS: OnceLock<Mutex<HashMap<VerifiedBlobKey, VerifiedBlobStamp>>> =
    OnceLock::new();

#[cfg(test)]
thread_local! {
    /// Per-thread so a parallel test run can measure its own delta: registry
    /// dispatch is synchronous on the calling test thread.
    pub(crate) static BLOB_HASH_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn get_kpi_ingest_document(
    state: &AppState,
    input: GetKpiIngestDocumentInput,
) -> Result<DocumentChunkDto, CommandError> {
    reject_control_chars("runId", &input.run_id)?;
    if input.length < 1 || input.length > DOCUMENT_CHUNK_MAX {
        return Err(budget_refusal(format!(
            "length {} is outside 1..={DOCUMENT_CHUNK_MAX}",
            input.length
        )));
    }
    let run = get_existing_run(state, &input.run_id)?;
    // Availability = SOURCE availability, not status: a run cancelled/failed
    // straight from `discovered` never captured bytes (conflict), while any
    // hash-bearing run — terminal included — stays readable.
    let Some(hash) = run.source_content_hash.clone() else {
        return Err(CommandError::new(
            CommandErrorCode::Conflict,
            "the run has not captured its source yet — finish start_kpi_ingest first",
        ));
    };
    if run.status == KpiIngestRunState::Discovered {
        return Err(internal(
            "invariant violated: a discovered run carries a source hash",
        ));
    }
    if !is_content_hash(&hash) {
        return Err(internal(
            "stored source_content_hash is not 64 lowercase hex bytes",
        ));
    }

    let path = state.data_dir().join(SNAPSHOT_DIR).join(&hash);
    let metadata = std::fs::metadata(&path)
        .map_err(|_| internal("the run's pinned source blob is missing on disk"))?;
    let total_bytes = metadata.len();
    let mtime = metadata
        .modified()
        .map_err(|_| internal("the blob's modification time is unreadable"))?;
    let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    let cache_key = (canonical, hash.clone());

    // Poison recovery: every critical section leaves the map consistent (one
    // get / one insert), so a panicked writer must not wedge the dispatcher.
    let cache = VERIFIED_BLOBS.get_or_init(|| Mutex::new(HashMap::new()));
    let verified = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&cache_key)
        .is_some_and(|&(size, cached_mtime)| size == total_bytes && cached_mtime == mtime);

    let chunk: Vec<u8> = if verified {
        // Verified once already — chunk-only IO.
        let mut file = std::fs::File::open(&path)
            .map_err(|_| internal("the run's pinned source blob is unreadable"))?;
        let start = input.offset.min(total_bytes);
        file.seek(SeekFrom::Start(start))
            .map_err(|error| internal(format!("blob seek failed: {error}")))?;
        let mut chunk = Vec::new();
        file.take(input.length)
            .read_to_end(&mut chunk)
            .map_err(|error| internal(format!("blob read failed: {error}")))?;
        chunk
    } else {
        // First (or invalidated) access: read the whole blob, verify against
        // the frozen hash, cache the verification, serve from the buffer.
        #[cfg(test)]
        BLOB_HASH_COUNT.with(|count| count.set(count.get() + 1));
        let bytes = std::fs::read(&path)
            .map_err(|_| internal("the run's pinned source blob is unreadable"))?;
        let actual = crate::report_documents_capture::content_hash_hex(&bytes);
        if actual != hash {
            return Err(internal(
                "the pinned source blob no longer matches the run's frozen content hash",
            ));
        }
        cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(cache_key, (total_bytes, mtime));
        let start = usize::try_from(input.offset.min(total_bytes)).expect("≤ file size");
        let end = usize::try_from(input.offset.saturating_add(input.length).min(total_bytes))
            .expect("≤ file size");
        bytes[start..end].to_vec()
    };

    let read_len = chunk.len() as u64;
    Ok(DocumentChunkDto {
        bytes_base64: STANDARD.encode(&chunk),
        offset: input.offset,
        length: read_len,
        total_bytes,
        sha256: hash,
        eof: input.offset.saturating_add(read_len) >= total_bytes,
    })
}

// ============================================================================
// Registered handlers
// ============================================================================

pub fn get_kpi_ingest_context_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_kpi_ingest_context(state, input))
}

pub fn get_kpi_ingest_document_handler(
    state: &AppState,
    arguments: &Value,
) -> Result<ToolOutcome, ToolCallError> {
    run(arguments, |input| get_kpi_ingest_document(state, input))
}
