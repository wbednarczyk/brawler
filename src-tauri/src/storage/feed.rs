use super::database::Database;
use super::sources;
use super::*;

/// Feed domain store (Architecture v2 / ADR 0050). Owns its own [`Database`]
/// connection source and exposes only feed operations, so a command can depend
/// on this store instead of the whole `AppState` facade. Reach it via
/// `AppState::feed()`.
#[derive(Clone)]
pub struct FeedStore {
    db: Database,
}

impl FeedStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_feed_items(&self) -> StorageResult<Vec<FeedItem>> {
        let connection = self.db.checkout()?;
        list_feed_items(&connection)
    }

    pub fn list_unmatched_source_items(
        &self,
        adapter_id: &str,
    ) -> StorageResult<Vec<UnmatchedSourceItem>> {
        let connection = self.db.checkout()?;
        list_unmatched_source_items(&connection, adapter_id)
    }

    pub fn update_feed_item_state(&self, input: FeedItemStateInput) -> StorageResult<FeedItem> {
        let connection = self.db.checkout()?;
        update_feed_item_state(&connection, input)
    }

    pub fn get_feed_item(&self, feed_item_id: &str) -> StorageResult<FeedItem> {
        let connection = self.db.checkout()?;
        get_feed_item(&connection, feed_item_id)
    }

    pub fn prune_old_feed_items(&self, retention_days: i64) -> StorageResult<FeedPruneResult> {
        let mut connection = self.db.checkout()?;
        prune_old_feed_items(&mut connection, retention_days)
    }

    pub fn delete_unsaved_feed_items(&self) -> StorageResult<FeedDeleteResult> {
        let mut connection = self.db.checkout()?;
        delete_unsaved_feed_items(&mut connection)
    }
}

pub(super) fn list_feed_items(connection: &Connection) -> StorageResult<Vec<FeedItem>> {
    let mut statement = connection.prepare(
        "
        SELECT
            id,
            COALESCE(display_company, 'Unmatched') AS company,
            type,
            source_name,
            COALESCE(published_at, fetched_at) AS item_time,
            title,
            read,
            saved,
            source_url,
            COALESCE(language, 'unknown') AS language,
            COALESCE(published_at, '') AS published_at,
            fetched_at,
            COALESCE(attribution, source_name) AS attribution,
            COALESCE(summary, '') AS summary,
            COALESCE(body_text, '') AS body_text,
            source_adapter_id
        FROM feed_items
        WHERE display_company IN (
            SELECT qualified_ticker FROM companies
        )
        ORDER BY COALESCE(published_at, fetched_at) DESC, fetched_at DESC, id
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok((
            feed_item_from_row(connection, row)?,
            row.get::<_, String>(15)?,
        ))
    })?;
    let listed_feed_items = rows.collect::<Result<Vec<_>, _>>()?;
    let feed_items = suppress_duplicate_bankier_company_items(listed_feed_items);

    Ok(feed_items)
}

pub(super) fn suppress_duplicate_bankier_company_items(
    listed_feed_items: Vec<(FeedItem, String)>,
) -> Vec<FeedItem> {
    let gpw_titles = listed_feed_items
        .iter()
        .filter(|(_, adapter_id)| adapter_id == ADAPTER_ID)
        .map(|(item, _)| {
            (
                item.company.clone(),
                sources::comparable_official_title(&item.title),
            )
        })
        .collect::<Vec<_>>();

    listed_feed_items
        .into_iter()
        .filter_map(|(item, adapter_id)| {
            let is_duplicate_bankier_company_item = adapter_id == BANKIER_COMPANY_ADAPTER_ID
                && gpw_titles.iter().any(|(company, title)| {
                    company == &item.company
                        && title == &sources::comparable_official_title(&item.title)
                });

            if is_duplicate_bankier_company_item {
                None
            } else {
                Some(item)
            }
        })
        .collect()
}

pub(super) fn list_unmatched_source_items(
    connection: &Connection,
    adapter_id: &str,
) -> StorageResult<Vec<UnmatchedSourceItem>> {
    let mut statement = connection.prepare(
        "
        SELECT
            feed_items.id,
            feed_items.source_adapter_id,
            COALESCE(feed_items.display_company, 'Unmatched') AS company_name,
            feed_items.title,
            feed_items.source_url,
            COALESCE(feed_items.published_at, '') AS published_at,
            feed_items.fetched_at
        FROM feed_items
        LEFT JOIN feed_item_companies
            ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_items.source_adapter_id = ?1
            AND feed_item_companies.feed_item_id IS NULL
            AND COALESCE(feed_items.display_company, '') NOT IN (
                SELECT qualified_ticker FROM companies
            )
        ORDER BY COALESCE(feed_items.published_at, feed_items.fetched_at) DESC,
            feed_items.fetched_at DESC,
            feed_items.id
        LIMIT 20
        ",
    )?;

    let rows = statement.query_map([adapter_id], |row| {
        Ok(UnmatchedSourceItem {
            id: row.get(0)?,
            adapter_id: row.get(1)?,
            company_name: row.get(2)?,
            title: row.get(3)?,
            source_url: row.get(4)?,
            published_at: row.get(5)?,
            fetched_at: row.get(6)?,
        })
    })?;

    let unmatched_items = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(unmatched_items)
}

pub(super) fn update_feed_item_state(
    connection: &Connection,
    input: FeedItemStateInput,
) -> StorageResult<FeedItem> {
    if let Some(read) = input.read {
        connection.execute(
            "
            UPDATE feed_items
            SET read = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![input.id, read],
        )?;
    }

    if let Some(saved) = input.saved {
        connection.execute(
            "
            UPDATE feed_items
            SET saved = ?2,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            WHERE id = ?1
            ",
            params![input.id, saved],
        )?;
    }

    get_feed_item(connection, &input.id)
}

pub(super) fn prune_old_feed_items(
    connection: &mut Connection,
    retention_days: i64,
) -> StorageResult<FeedPruneResult> {
    let retention_days = retention_days.max(1);
    let retention_modifier = format!("-{retention_days} days");
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let pruned_at = sources::current_timestamp(&transaction)?;

    let candidate_ids = old_unsaved_feed_item_ids(&transaction, &retention_modifier)?;

    delete_feed_items_by_id(&transaction, &candidate_ids)?;

    transaction.commit()?;

    Ok(FeedPruneResult {
        retention_days,
        items_deleted: candidate_ids.len(),
        pruned_at,
    })
}

pub(super) fn delete_unsaved_feed_items(
    connection: &mut Connection,
) -> StorageResult<FeedDeleteResult> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let deleted_at = sources::current_timestamp(&transaction)?;
    let candidate_ids = unsaved_feed_item_ids(&transaction)?;

    delete_feed_items_by_id(&transaction, &candidate_ids)?;

    transaction.commit()?;

    Ok(FeedDeleteResult {
        items_deleted: candidate_ids.len(),
        deleted_at,
    })
}

pub(super) fn delete_feed_items_by_id(
    connection: &Connection,
    feed_item_ids: &[String],
) -> StorageResult<()> {
    for feed_item_id in feed_item_ids {
        transaction_delete_feed_item(connection, feed_item_id)?;
    }

    Ok(())
}

pub(super) fn transaction_delete_feed_item(
    connection: &Connection,
    feed_item_id: &str,
) -> StorageResult<()> {
    // The `ai_analysis_*` cascade that used to run here is gone with those tables
    // (ADR 0084 decision 5, migration 0102).
    connection.execute(
        "DELETE FROM feed_item_attachments WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;
    connection.execute(
        "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;
    connection.execute("DELETE FROM feed_items WHERE id = ?1", [feed_item_id])?;

    Ok(())
}

pub(super) fn old_unsaved_feed_item_ids(
    connection: &Connection,
    retention_modifier: &str,
) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT id
        FROM feed_items
        WHERE saved = 0
            AND datetime(COALESCE(published_at, fetched_at)) < datetime('now', ?1)
        ",
    )?;
    let rows = statement.query_map([retention_modifier], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn unsaved_feed_item_ids(connection: &Connection) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT id
        FROM feed_items
        WHERE saved = 0
        ",
    )?;
    let rows = statement.query_map([], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(crate) fn get_feed_item(
    connection: &Connection,
    feed_item_id: &str,
) -> StorageResult<FeedItem> {
    connection
        .query_row(
            "
            SELECT
                id,
                COALESCE(display_company, 'Unmatched') AS company,
                type,
                source_name,
                COALESCE(published_at, fetched_at) AS item_time,
                title,
                read,
                saved,
                source_url,
                COALESCE(language, 'unknown') AS language,
                COALESCE(published_at, '') AS published_at,
                fetched_at,
                COALESCE(attribution, source_name) AS attribution,
                COALESCE(summary, '') AS summary,
                COALESCE(body_text, '') AS body_text
            FROM feed_items
            WHERE id = ?1
            ",
            [feed_item_id],
            |row| feed_item_from_row(connection, row),
        )
        .map_err(StorageError::from)
}

pub(super) fn feed_item_from_row(
    connection: &Connection,
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<FeedItem> {
    let read: bool = row.get(6)?;
    let id: String = row.get(0)?;

    Ok(FeedItem {
        attachments: feed_item_attachments(connection, &id)?,
        id,
        company: row.get(1)?,
        item_type: row.get(2)?,
        source: row.get(3)?,
        time: row.get(4)?,
        title: row.get(5)?,
        unread: !read,
        saved: row.get(7)?,
        source_url: row.get(8)?,
        language: row.get(9)?,
        published_at: row.get(10)?,
        fetched_at: row.get(11)?,
        attribution: row.get(12)?,
        summary: row.get(13)?,
        body_text: row.get(14)?,
    })
}

pub(super) fn feed_item_attachments(
    connection: &Connection,
    feed_item_id: &str,
) -> rusqlite::Result<Vec<FeedItemAttachment>> {
    let mut statement = connection.prepare(
        "
        SELECT id, label, url
        FROM feed_item_attachments
        WHERE feed_item_id = ?1
        ORDER BY position, id
        ",
    )?;
    let rows = statement.query_map([feed_item_id], |row| {
        Ok(FeedItemAttachment {
            id: row.get(0)?,
            label: row.get(1)?,
            url: row.get(2)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
}
