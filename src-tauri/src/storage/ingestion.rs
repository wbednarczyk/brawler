//! The shared ingestion-pipeline spine (Architecture v2 / ADR 0050).
//!
//! Brawler ingests many sources into one unified set. Historically each adapter's
//! `ingest_*` function re-implemented the same downstream spine — the unified
//! feed-item upsert and the per-adapter run outcome (last-success + item
//! counters). This module owns those shared stages so every adapter feeds the
//! *same* pipeline rather than copying it. (The story-key stage that once lived
//! here was write-only — its consumer, story clustering, was dropped — and was
//! removed together with `feed_items.story_key` by ADR 0080.)
//!
//! The adapter-specific parse/normalize/match stages still live in each adapter's
//! ingest path; they migrate behind this spine one at a time (strangler). The
//! `upsert → record-outcome` stages here are the unified part.

use super::*;

/// The normalized shape every source produces before the **upsert stage** —
/// the unified feed-item record the pipeline writes, regardless of which adapter
/// parsed it. Borrowed fields so callers pass slices of their parsed item.
pub(super) struct NormalizedFeedItem<'a> {
    pub id: &'a str,
    pub item_type: &'a str,
    pub source_adapter_id: &'a str,
    pub source_name: &'a str,
    pub source_url: &'a str,
    pub title: &'a str,
    pub summary: Option<&'a str>,
    pub body_text: Option<&'a str>,
    pub language: &'a str,
    pub published_at: Option<&'a str>,
    pub fetched_at: &'a str,
    pub dedupe_key: &'a str,
    pub attribution: &'a str,
    pub display_company: &'a str,
    pub duplicate_signature: Option<&'a str>,
}

/// **Upsert stage.** Write one normalized feed item, deduping on
/// `(source_adapter_id, dedupe_key)`. The single INSERT every feed-item adapter
/// (media RSS, GPW listings, Bankier company) now shares instead of its own
/// near-duplicate SQL. `body_text` is preserved on conflict
/// (`COALESCE(excluded, existing)`) so a later detail-page fetch never clobbers a
/// stored body with a null.
pub(super) fn upsert_feed_item(
    connection: &Connection,
    item: &NormalizedFeedItem,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO feed_items (
            id, type, source_adapter_id, source_name, source_url, title, summary,
            body_text, language, published_at, fetched_at, dedupe_key, attribution,
            display_company, duplicate_signature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
            type = excluded.type,
            source_name = excluded.source_name,
            source_url = excluded.source_url,
            title = excluded.title,
            summary = excluded.summary,
            body_text = COALESCE(excluded.body_text, feed_items.body_text),
            language = excluded.language,
            published_at = excluded.published_at,
            fetched_at = excluded.fetched_at,
            attribution = excluded.attribution,
            display_company = excluded.display_company,
            duplicate_signature = excluded.duplicate_signature,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            item.id,
            item.item_type,
            item.source_adapter_id,
            item.source_name,
            item.source_url,
            item.title,
            item.summary,
            item.body_text,
            item.language,
            item.published_at,
            item.fetched_at,
            item.dedupe_key,
            item.attribution,
            item.display_company,
            item.duplicate_signature,
        ],
    )?;
    Ok(())
}

/// **Outcome-recording stage.** Persist the result of one adapter run: mark the
/// adapter healthy (last-success, cleared error) and record the item counters
/// other code and the Sources screen read back. Shared by every adapter's ingest
/// path so the ~20-line state-update block is written once.
pub(super) fn record_source_outcome(
    connection: &Connection,
    adapter_id: &str,
    fetched_at: &str,
    items_fetched: usize,
    items_created: usize,
    items_matched: usize,
    items_unmatched: usize,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE source_adapters
        SET last_success_at = ?1,
            last_error_at = NULL,
            last_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![fetched_at, adapter_id],
    )?;
    sources::set_source_adapter_state(
        connection,
        adapter_id,
        "last_items_fetched",
        &items_fetched.to_string(),
    )?;
    sources::set_source_adapter_state(
        connection,
        adapter_id,
        "last_items_created",
        &items_created.to_string(),
    )?;
    sources::set_source_adapter_state(
        connection,
        adapter_id,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    sources::set_source_adapter_state(
        connection,
        adapter_id,
        "last_items_unmatched",
        &items_unmatched.to_string(),
    )?;
    Ok(())
}
