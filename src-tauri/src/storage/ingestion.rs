//! The shared ingestion-pipeline spine (Architecture v2 / ADR 0050).
//!
//! Brawler ingests many sources into one unified set. Historically each adapter's
//! `ingest_*` function re-implemented the same downstream spine — derive the
//! canonical **story key** for cross-source clustering, then record the
//! per-adapter run outcome (last-success + item counters). This module owns those
//! shared stages so every adapter feeds the *same* pipeline rather than copying
//! it, and so the entity-resolution layer (`crate::entity_resolution`) is the one
//! place canonical identity/story keys are decided.
//!
//! The adapter-specific parse/normalize/match stages still live in each adapter's
//! ingest path; they migrate behind this spine one at a time (strangler). The
//! `resolve → story-key → record-outcome` stages here are the unified part.

use super::*;
use crate::entity_resolution::story_key;

/// **Story-key stage.** Derive the canonical cross-source story key for a feed
/// item from the companies it matched, the publication time, and the title. This
/// is the clustering key persisted on `feed_items.story_key`; items from
/// different sources about the same event for the same companies on the same day
/// share it. Returns `None` when the item is unmatched or its title is too short
/// to anchor a cluster (see [`story_key`]).
pub(super) fn derive_story_key(
    title: &str,
    qualified_tickers: &[String],
    published_at: Option<&str>,
) -> Option<String> {
    story_key(title, qualified_tickers, published_at.unwrap_or(""))
}

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
    pub story_key: Option<&'a str>,
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
            display_company, duplicate_signature, story_key
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
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
            story_key = excluded.story_key,
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
            item.story_key,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_key_stage_clusters_matched_items_by_company_and_day() {
        let key = derive_story_key(
            "CD Projekt rośnie po komentarzu zarządu",
            &["GPW:CDR".to_owned()],
            Some("2026-05-31T09:15:00+02:00"),
        )
        .expect("a matched, long-enough title yields a story key");
        assert!(key.starts_with("story:GPW:CDR:2026-05-31:"), "{key}");
    }

    #[test]
    fn story_key_stage_is_none_without_a_matched_company() {
        assert!(
            derive_story_key("A long enough market headline", &[], Some("2026-05-31")).is_none()
        );
    }

    #[test]
    fn story_key_stage_tolerates_a_missing_publication_time() {
        // A missing published_at must not panic; it yields an empty day bucket.
        let key = derive_story_key(
            "CD Projekt rośnie po komentarzu",
            &["GPW:CDR".to_owned()],
            None,
        )
        .expect("still keyed without a date");
        assert!(key.starts_with("story:GPW:CDR::"), "{key}");
    }
}
