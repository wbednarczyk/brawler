//! DB-backed property tests for the unified ingestion spine (issue #194 S3,
//! ADR 0049 dec. 1): `ingestion::upsert_feed_item`'s `ON CONFLICT` merge
//! contract and `ingestion::record_source_outcome`'s last-write-wins update.
//!
//! `updated_at` changes on every conflicting upsert and a fresh insert gets a
//! wall-clock `created_at`/`updated_at`, so rows are never compared whole —
//! every assertion below reads [`FeedItemProjection`] (every `feed_items`
//! column except `created_at`/`updated_at`). Generators pre-assign unique
//! `(id, dedupe_key)` pairs and fixed `published_at`/`fetched_at` strings —
//! no wall clock inside a generated item.

use super::*;
use crate::storage::ingestion;
use crate::storage::ingestion::{record_source_outcome, upsert_feed_item, NormalizedFeedItem};
use crate::transform_invariants::assert_order_independent;
use proptest::prelude::*;

const ADAPTER_ID: &str = "test-ingestion-adapter";

/// The adapter's catalog row (seeded by the registry wiring in production;
/// inserted here so the feed-item FK and run-outcome UPDATE resolve).
fn seed_adapter(state: &AppState) {
    let connection = state.checkout().expect("connection");
    connection
        .execute(
            "
            INSERT INTO source_adapters (
                id, display_name, source_type, fetch_mode, enabled, default_poll_interval_seconds
            ) VALUES (?1, 'Test Ingestion Adapter', 'disclosure', 'public_json', 1, 86400)
            ON CONFLICT(id) DO NOTHING
            ",
            [ADAPTER_ID],
        )
        .expect("adapter row should seed");
}

fn new_state() -> AppState {
    let state = AppState::new(open_in_memory_database().expect("db"));
    seed_adapter(&state);
    state
}

/// One generated feed item before upsert. `id`/`dedupe_key` are assigned by
/// the generator's index (never randomized) so a batch's keys are always
/// distinct by construction; only content varies.
#[derive(Debug, Clone)]
struct GenItem {
    id: String,
    dedupe_key: String,
    title: String,
    summary: Option<String>,
    body_text: Option<String>,
    published_at: String,
}

fn normalized(item: &GenItem) -> NormalizedFeedItem<'_> {
    NormalizedFeedItem {
        id: &item.id,
        item_type: "Official report",
        source_adapter_id: ADAPTER_ID,
        source_name: "Test Ingestion Adapter",
        source_url: "https://example.test/report",
        title: &item.title,
        summary: item.summary.as_deref(),
        body_text: item.body_text.as_deref(),
        language: "pl",
        published_at: Some(&item.published_at),
        fetched_at: "2026-01-01T10:00:00Z",
        dedupe_key: &item.dedupe_key,
        attribution: "Test Source",
        display_company: "GPW:TEST",
        duplicate_signature: None,
    }
}

fn insert_batch(state: &AppState, items: &[GenItem]) {
    let connection = state.checkout().expect("connection");
    for item in items {
        upsert_feed_item(&connection, &normalized(item)).expect("upsert item");
    }
}

/// Every `feed_items` column except `created_at`/`updated_at` (both change on
/// every conflicting upsert, so comparing them would make every property
/// vacuously fail on re-upsert).
#[derive(Debug, Clone, PartialEq, Eq)]
struct FeedItemProjection {
    id: String,
    item_type: String,
    source_adapter_id: String,
    source_name: String,
    source_url: String,
    title: String,
    summary: Option<String>,
    body_text: Option<String>,
    language: Option<String>,
    published_at: Option<String>,
    fetched_at: String,
    dedupe_key: String,
    read: bool,
    saved: bool,
    attribution: Option<String>,
    display_company: Option<String>,
    duplicate_signature: Option<String>,
}

fn projection(state: &AppState) -> Vec<FeedItemProjection> {
    let connection = state.checkout().expect("connection");
    let mut statement = connection
        .prepare(
            "SELECT id, type, source_adapter_id, source_name, source_url, title, summary, \
             body_text, language, published_at, fetched_at, dedupe_key, read, saved, \
             attribution, display_company, duplicate_signature \
             FROM feed_items ORDER BY id ASC",
        )
        .expect("prepare projection");
    statement
        .query_map([], |row| {
            Ok(FeedItemProjection {
                id: row.get(0)?,
                item_type: row.get(1)?,
                source_adapter_id: row.get(2)?,
                source_name: row.get(3)?,
                source_url: row.get(4)?,
                title: row.get(5)?,
                summary: row.get(6)?,
                body_text: row.get(7)?,
                language: row.get(8)?,
                published_at: row.get(9)?,
                fetched_at: row.get(10)?,
                dedupe_key: row.get(11)?,
                read: row.get(12)?,
                saved: row.get(13)?,
                attribution: row.get(14)?,
                display_company: row.get(15)?,
                duplicate_signature: row.get(16)?,
            })
        })
        .expect("query projection")
        .collect::<Result<Vec<_>, _>>()
        .expect("collect projection")
}

fn item_content_strategy() -> impl Strategy<Value = (String, Option<String>, Option<String>, String)>
{
    (
        prop::sample::select(vec!["Title A", "Title B", "Title C"]),
        prop::option::of(prop::sample::select(vec!["Summary A", "Summary B"])),
        prop::option::of(prop::sample::select(vec!["Body A", "Body B"])),
        prop::sample::select(vec![
            "2026-01-01T09:00:00Z",
            "2026-01-02T09:00:00Z",
            "2026-01-03T09:00:00Z",
        ]),
    )
        .prop_map(|(title, summary, body_text, published_at)| {
            (
                title.to_owned(),
                summary.map(str::to_owned),
                body_text.map(str::to_owned),
                published_at.to_owned(),
            )
        })
}

/// A batch of items with distinct `(source_adapter_id, dedupe_key)` by
/// construction (index-assigned keys); only content is randomized.
fn items_strategy(max_len: usize) -> impl Strategy<Value = Vec<GenItem>> {
    prop::collection::vec(item_content_strategy(), 0..=max_len).prop_map(|contents| {
        contents
            .into_iter()
            .enumerate()
            .map(
                |(index, (title, summary, body_text, published_at))| GenItem {
                    id: format!("feed-item-{index}"),
                    dedupe_key: format!("dedupe-{index}"),
                    title,
                    summary,
                    body_text,
                    published_at,
                },
            )
            .collect()
    })
}

/// A batch that may repeat the same `(source_adapter_id, dedupe_key)` more
/// than once — always with an IDENTICAL payload each repeat (same
/// id/title/summary/body_text/published_at), since a repeat is a literal
/// clone of one of `max_distinct` distinct generated items. `items_strategy`
/// cannot exercise this: its keys are distinct by construction, so a broken
/// `ON CONFLICT ... DO UPDATE` merge path never gets exercised against
/// itself.
fn items_with_repeats_strategy(max_distinct: usize) -> impl Strategy<Value = Vec<GenItem>> {
    (
        prop::collection::vec(item_content_strategy(), 1..=max_distinct),
        prop::collection::vec(0usize..max_distinct, 0..=(max_distinct * 2)),
    )
        .prop_map(|(contents, repeat_indices)| {
            let distinct: Vec<GenItem> = contents
                .into_iter()
                .enumerate()
                .map(
                    |(index, (title, summary, body_text, published_at))| GenItem {
                        id: format!("feed-item-{index}"),
                        dedupe_key: format!("dedupe-{index}"),
                        title,
                        summary,
                        body_text,
                        published_at,
                    },
                )
                .collect();
            let mut batch = distinct.clone();
            for index in repeat_indices {
                batch.push(distinct[index % distinct.len()].clone());
            }
            batch
        })
}

fn state_value(state: &AppState, key: &str) -> String {
    let connection = state.checkout().expect("connection");
    connection
        .query_row(
            "SELECT state_value FROM source_adapter_state WHERE source_adapter_id = ?1 AND state_key = ?2",
            params![ADAPTER_ID, key],
            |row| row.get(0),
        )
        .expect("read adapter state")
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// (a) Re-upsert idempotence: upserting a batch of distinct-key items
    /// twice yields the same projection and row count as once.
    #[test]
    fn re_upsert_is_idempotent(items in items_strategy(8)) {
        let state = new_state();
        insert_batch(&state, &items);
        let once = projection(&state);
        prop_assert_eq!(once.len(), items.len());

        // The second pass calls the transform by its module path on purpose:
        // the manifest's `proptest_in` guard credits this property to
        // `storage::ingestion` only if the property body itself references it.
        let connection = state.checkout().expect("connection");
        for item in &items {
            ingestion::upsert_feed_item(&connection, &normalized(item)).expect("re-upsert item");
        }
        drop(connection);
        let twice = projection(&state);
        prop_assert_eq!(twice, once);
    }

    /// (b) Batch-order independence: inserting the same batch in any
    /// permutation yields the same projection as the original order.
    #[test]
    fn batch_order_is_independent(items in items_strategy(8)) {
        assert_order_independent(
            |batch: Vec<GenItem>| {
                let state = new_state();
                insert_batch(&state, &batch);
                projection(&state)
            },
            items,
        );
    }

    /// (b2) Batch-order independence WITH repeated keys: a batch that
    /// repeats the same `(source_adapter_id, dedupe_key)` — always with an
    /// identical payload — still yields the same projection for any
    /// permutation, and the row count equals the number of DISTINCT keys,
    /// never the batch length. `items_strategy`'s distinct-key generator
    /// can never exercise a real in-batch conflict; this arm does.
    #[test]
    fn batch_order_is_independent_with_repeated_keys(items in items_with_repeats_strategy(4)) {
        let expected_len = {
            let mut keys: Vec<&str> = items.iter().map(|item| item.dedupe_key.as_str()).collect();
            keys.sort_unstable();
            keys.dedup();
            keys.len()
        };
        assert_order_independent(
            |batch: Vec<GenItem>| {
                let state = new_state();
                insert_batch(&state, &batch);
                let rows = projection(&state);
                assert_eq!(
                    rows.len(),
                    expected_len,
                    "row count must equal distinct dedupe keys, not batch length"
                );
                rows
            },
            items,
        );
    }

    /// (e) `record_source_outcome` is last-write-wins: the second call's
    /// timestamp and all four counters stick, and a previously set error is
    /// cleared (by either call, so it stays cleared after the second too).
    #[test]
    fn record_source_outcome_is_last_write_wins(
        fetched_index in 0usize..3,
        first_counts in (0usize..1000, 0usize..1000, 0usize..1000, 0usize..1000),
        second_counts in (0usize..1000, 0usize..1000, 0usize..1000, 0usize..1000),
    ) {
        const FETCHED_AT_POOL: [&str; 3] = [
            "2026-01-01T09:00:00Z",
            "2026-02-01T09:00:00Z",
            "2026-03-01T09:00:00Z",
        ];
        let first_at = FETCHED_AT_POOL[fetched_index];
        let second_at = FETCHED_AT_POOL[(fetched_index + 1) % FETCHED_AT_POOL.len()];

        let state = new_state();
        {
            let connection = state.checkout().expect("connection");
            connection
                .execute(
                    "UPDATE source_adapters SET last_error_at = ?1, last_error = 'boom' WHERE id = ?2",
                    params!["2025-01-01T00:00:00Z", ADAPTER_ID],
                )
                .expect("seed a previous error");
        }

        {
            let connection = state.checkout().expect("connection");
            record_source_outcome(
                &connection,
                ADAPTER_ID,
                first_at,
                first_counts.0,
                first_counts.1,
                first_counts.2,
                first_counts.3,
            )
            .expect("first outcome");
        }
        {
            let connection = state.checkout().expect("connection");
            record_source_outcome(
                &connection,
                ADAPTER_ID,
                second_at,
                second_counts.0,
                second_counts.1,
                second_counts.2,
                second_counts.3,
            )
            .expect("second outcome");
        }

        let (last_success_at, last_error_at, last_error): (String, Option<String>, Option<String>) = {
            let connection = state.checkout().expect("connection");
            connection
                .query_row(
                    "SELECT last_success_at, last_error_at, last_error FROM source_adapters WHERE id = ?1",
                    [ADAPTER_ID],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("read adapter row")
        };
        prop_assert_eq!(last_success_at, second_at.to_owned());
        prop_assert_eq!(last_error_at, None);
        prop_assert_eq!(last_error, None);

        prop_assert_eq!(state_value(&state, "last_items_fetched"), second_counts.0.to_string());
        prop_assert_eq!(state_value(&state, "last_items_created"), second_counts.1.to_string());
        prop_assert_eq!(state_value(&state, "last_items_matched"), second_counts.2.to_string());
        prop_assert_eq!(state_value(&state, "last_items_unmatched"), second_counts.3.to_string());
    }
}

/// (c) `body_text` follows the `COALESCE(excluded.body_text, feed_items.body_text)`
/// contract: a `None` on conflict never clobbers a previously stored body, but
/// a `Some` does. Other mutable columns (title, summary here) always follow
/// `excluded` on conflict, `None` included.
#[test]
fn body_text_survives_null_updates_but_not_replacement() {
    let state = new_state();
    let dedupe_key = "dedupe-body-text";

    let first = GenItem {
        id: "feed-item-1".to_owned(),
        dedupe_key: dedupe_key.to_owned(),
        title: "First title".to_owned(),
        summary: Some("First summary".to_owned()),
        body_text: Some("Body one".to_owned()),
        published_at: "2026-01-01T09:00:00Z".to_owned(),
    };
    insert_batch(&state, std::slice::from_ref(&first));

    let second = GenItem {
        body_text: None,
        title: "Second title".to_owned(),
        summary: Some("Second summary".to_owned()),
        ..first.clone()
    };
    insert_batch(&state, std::slice::from_ref(&second));

    let rows = projection(&state);
    assert_eq!(
        rows.len(),
        1,
        "conflicting key must not create a second row"
    );
    assert_eq!(
        rows[0].body_text.as_deref(),
        Some("Body one"),
        "None must not clobber a stored body"
    );
    assert_eq!(
        rows[0].title, "Second title",
        "title always follows excluded"
    );
    assert_eq!(
        rows[0].summary.as_deref(),
        Some("Second summary"),
        "summary always follows excluded"
    );

    let third = GenItem {
        body_text: Some("Body two".to_owned()),
        title: "Third title".to_owned(),
        ..second
    };
    insert_batch(&state, std::slice::from_ref(&third));

    let rows = projection(&state);
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].body_text.as_deref(),
        Some("Body two"),
        "Some must replace the stored body"
    );
    assert_eq!(rows[0].title, "Third title");
}

/// (d) Collision keeps identity and user state — kills a delete-and-reinsert
/// implementation. A re-upsert on the same `(source_adapter_id, dedupe_key)`
/// with a DIFFERENT proposed id must update the EXISTING row in place: same
/// id, untouched `read`/`saved` flags, and its attachment row survives (a
/// delete+insert would cascade-delete it via the FK and reset the flags).
#[test]
fn collision_updates_the_existing_row_in_place() {
    let state = new_state();
    let dedupe_key = "dedupe-collision";
    let original = GenItem {
        id: "feed-item-original".to_owned(),
        dedupe_key: dedupe_key.to_owned(),
        title: "Original title".to_owned(),
        summary: Some("Original summary".to_owned()),
        body_text: Some("Original body".to_owned()),
        published_at: "2026-01-01T09:00:00Z".to_owned(),
    };
    insert_batch(&state, std::slice::from_ref(&original));

    {
        let connection = state.checkout().expect("connection");
        connection
            .execute(
                "UPDATE feed_items SET read = 1, saved = 1 WHERE id = ?1",
                [&original.id],
            )
            .expect("mark read/saved");
        connection
            .execute(
                "INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position) \
                 VALUES ('attachment-1', ?1, 'report.pdf', 'https://example.test/report.pdf', 0)",
                [&original.id],
            )
            .expect("seed attachment");
    }

    let colliding = GenItem {
        id: "feed-item-colliding".to_owned(),
        title: "Colliding title".to_owned(),
        ..original.clone()
    };
    insert_batch(&state, std::slice::from_ref(&colliding));

    let rows = projection(&state);
    assert_eq!(rows.len(), 1, "row count must not change on collision");
    assert_eq!(rows[0].id, original.id, "existing row keeps its id");
    assert_eq!(
        rows[0].title, "Colliding title",
        "title still follows excluded"
    );
    assert!(rows[0].read, "read flag survives the collision");
    assert!(rows[0].saved, "saved flag survives the collision");

    let connection = state.checkout().expect("connection");
    let colliding_id_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM feed_items WHERE id = ?1)",
            [&colliding.id],
            |row| row.get(0),
        )
        .expect("check colliding id");
    assert!(
        !colliding_id_exists,
        "the proposed new id must never be inserted"
    );

    let attachment_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM feed_item_attachments WHERE feed_item_id = ?1",
            [&original.id],
            |row| row.get(0),
        )
        .expect("count attachments");
    assert_eq!(attachment_count, 1, "attachment row survives the collision");
}
