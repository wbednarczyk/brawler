use super::feed_matching::{
    find_companies_for_media_item, find_company_for_gpw_listing, list_media_match_companies,
    media_duplicate_signature, normalize_media_match_text,
};
use super::*;

pub(super) fn ingest_gpw_report_listings(
    connection: &mut Connection,
    listings: &[GpwReportListing],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;
    let fetched_at = listings
        .first()
        .map(|listing| listing.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| current_timestamp(&transaction))?;

    for listing in listings {
        let feed_item_id = feed_item_id(&listing.dedupe_key);
        let matched_company =
            find_company_for_gpw_listing(&transaction, &listing.company_ticker, &listing.isin)?;
        let display_company = matched_company
            .as_ref()
            .map(|company| company.qualified_ticker.clone())
            .unwrap_or_else(|| listing.company_name.clone());
        let matched_tickers = matched_company
            .as_ref()
            .map(|company| vec![company.qualified_ticker.clone()])
            .unwrap_or_default();
        let story_key = super::ingestion::derive_story_key(
            &listing.title,
            &matched_tickers,
            Some(&listing.published_at),
        );
        let existed = feed_item_exists(&transaction, &feed_item_id)?;

        super::ingestion::upsert_feed_item(
            &transaction,
            &super::ingestion::NormalizedFeedItem {
                id: &feed_item_id,
                item_type: "Official report",
                source_adapter_id: ADAPTER_ID,
                source_name: DISPLAY_NAME,
                source_url: &listing.detail_url,
                title: &listing.title,
                summary: None,
                body_text: listing.body_text.as_deref(),
                language: "pl",
                published_at: Some(&listing.published_at),
                fetched_at: &listing.fetched_at,
                dedupe_key: &listing.dedupe_key,
                attribution: "GPW",
                display_company: &display_company,
                duplicate_signature: None,
                story_key: story_key.as_deref(),
            },
        )?;

        if !existed {
            items_created += 1;
        }

        transaction.execute(
            "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
            [&feed_item_id],
        )?;

        if let Some(company) = matched_company {
            transaction.execute(
                "
                INSERT INTO feed_item_companies (feed_item_id, company_id, match_type)
                VALUES (?1, ?2, ?3)
                ",
                params![feed_item_id, company.id, company.match_type],
            )?;
            items_matched += 1;
        } else {
            items_unmatched += 1;
        }

        if listing.body_text.is_some() {
            replace_feed_item_attachments(&transaction, &feed_item_id, &listing.attachments)?;
        }
    }

    super::ingestion::record_source_outcome(
        &transaction,
        ADAPTER_ID,
        &fetched_at,
        listings.len(),
        items_created,
        items_matched,
        items_unmatched,
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: ADAPTER_ID.to_owned(),
        items_fetched: listings.len(),
        items_created,
        items_matched,
        items_unmatched,
        detail_items_attempted: 0,
        detail_items_stored: listings
            .iter()
            .filter(|listing| listing.body_text.is_some())
            .count(),
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

pub(super) fn ingest_bankier_rss_items(
    connection: &mut Connection,
    items: &[BankierRssItem],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let tracked_companies = list_media_match_companies(&transaction)?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;
    let fetched_at = items
        .first()
        .map(|item| item.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| current_timestamp(&transaction))?;
    for item in items {
        let matched_companies = find_companies_for_media_item(&tracked_companies, item);
        let duplicate_signature = media_duplicate_signature(item, &matched_companies);
        let matched_tickers = matched_companies
            .iter()
            .map(|company| company.qualified_ticker.clone())
            .collect::<Vec<_>>();
        let story_key = super::ingestion::derive_story_key(
            &item.title,
            &matched_tickers,
            item.published_at.as_deref(),
        );
        let existing_feed_item_id = find_bankier_feed_item_by_source_url(&transaction, &item.link)?;
        let existing_duplicate_feed_item_id = if existing_feed_item_id.is_none() {
            find_media_feed_item_by_duplicate_signature(
                &transaction,
                duplicate_signature.as_deref(),
                BANKIER_RSS_ADAPTER_ID,
            )?
        } else {
            None
        };
        let feed_item_id = existing_feed_item_id
            .clone()
            .or(existing_duplicate_feed_item_id.clone())
            .unwrap_or_else(|| feed_item_id(&item.dedupe_key));
        let display_company = matched_companies
            .first()
            .map(|company| company.qualified_ticker.clone())
            .unwrap_or_else(|| BANKIER_RSS_ATTRIBUTION.to_owned());
        let existed = existing_feed_item_id.is_some()
            || existing_duplicate_feed_item_id.is_some()
            || feed_item_exists(&transaction, &feed_item_id)?;

        if existing_feed_item_id.is_some() {
            update_bankier_feed_item(
                &transaction,
                &feed_item_id,
                item,
                &display_company,
                duplicate_signature.as_deref(),
                story_key.as_deref(),
            )?;
        } else if existing_duplicate_feed_item_id.is_none() {
            insert_bankier_feed_item(
                &transaction,
                &feed_item_id,
                item,
                &display_company,
                duplicate_signature.as_deref(),
                story_key.as_deref(),
            )?;
        } else {
            record_media_duplicate_seen(&transaction, &feed_item_id, item)?;
        }

        if !existed {
            items_created += 1;
        }

        if existing_duplicate_feed_item_id.is_none() {
            transaction.execute(
                "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
                [&feed_item_id],
            )?;
        }

        if matched_companies.is_empty() {
            items_unmatched += 1;
        } else {
            items_matched += 1;
            for company in matched_companies {
                transaction.execute(
                    "
                    INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![
                        feed_item_id,
                        company.id,
                        if existing_duplicate_feed_item_id.is_some() {
                            "media_duplicate"
                        } else {
                            "media_signal"
                        },
                    ],
                )?;
            }
        }
    }

    super::ingestion::record_source_outcome(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        &fetched_at,
        items.len(),
        items_created,
        items_matched,
        items_unmatched,
    )?;

    transaction.commit()?;

    Ok(SourceIngestionResult {
        adapter_id: BANKIER_RSS_ADAPTER_ID.to_owned(),
        items_fetched: items.len(),
        items_created,
        items_matched,
        items_unmatched,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: Some(fetched_at),
    })
}

pub(super) fn list_bankier_company_targets(
    connection: &Connection,
) -> StorageResult<Vec<BankierCompanyTarget>> {
    let mut statement = connection.prepare(
        "
        SELECT
            companies.id,
            companies.ticker,
            companies.qualified_ticker,
            (
                SELECT source_value
                FROM company_source_ids
                WHERE company_id = companies.id
                    AND source_adapter_id = ?1
                    AND source_key = 'instrument_slug'
                LIMIT 1
            ) AS bankier_slug,
            (
                SELECT source_value
                FROM company_source_ids
                WHERE company_id = companies.id
                    AND source_adapter_id = ?1
                    AND source_key = 'tag_id'
                LIMIT 1
            ) AS bankier_tag_id
        FROM companies
        WHERE companies.exchange = 'GPW'
        ORDER BY companies.qualified_ticker
        ",
    )?;

    let rows = statement.query_map([BANKIER_COMPANY_ADAPTER_ID], |row| {
        Ok(BankierCompanyTarget {
            company_id: row.get(0)?,
            ticker: row.get(1)?,
            qualified_ticker: row.get(2)?,
            bankier_slug: row.get(3)?,
            bankier_tag_id: row.get(4)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn upsert_bankier_company_identifiers(
    connection: &Connection,
    company_id: &str,
    identifiers: &BankierCompanyIdentifiers,
) -> StorageResult<()> {
    upsert_company_source_id(
        connection,
        company_id,
        BANKIER_COMPANY_ADAPTER_ID,
        "instrument_slug",
        &identifiers.slug,
    )?;
    upsert_company_source_id(
        connection,
        company_id,
        BANKIER_COMPANY_ADAPTER_ID,
        "tag_id",
        &identifiers.tag_id,
    )?;

    Ok(())
}

pub(super) fn upsert_company_source_id(
    connection: &Connection,
    company_id: &str,
    source_adapter_id: &str,
    source_key: &str,
    source_value: &str,
) -> StorageResult<()> {
    let id = format!(
        "company_source_{}_{}_{}",
        slug_part(company_id),
        slug_part(source_adapter_id),
        slug_part(source_key)
    );

    let updated = connection.execute(
        "
        UPDATE company_source_ids
        SET source_value = ?1
        WHERE company_id = ?2
            AND source_adapter_id = ?3
            AND source_key = ?4
        ",
        params![source_value, company_id, source_adapter_id, source_key],
    )?;

    if updated > 0 {
        return Ok(());
    }

    connection.execute(
        "
        INSERT INTO company_source_ids (
            id,
            company_id,
            source_adapter_id,
            source_key,
            source_value
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        ON CONFLICT(source_adapter_id, source_key, source_value) DO UPDATE SET
            company_id = excluded.company_id
        ",
        params![id, company_id, source_adapter_id, source_key, source_value],
    )?;

    Ok(())
}

pub(super) fn list_bankier_company_detail_cached_urls(
    connection: &Connection,
) -> StorageResult<Vec<String>> {
    let mut statement = connection.prepare(
        "
        SELECT source_url
        FROM feed_items
        WHERE source_adapter_id = ?1
            AND NULLIF(TRIM(COALESCE(body_text, '')), '') IS NOT NULL
        ",
    )?;
    let rows = statement.query_map([BANKIER_COMPANY_ADAPTER_ID], |row| row.get(0))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Register a filing's ESPI/EBI attachment links as `report_documents`, linked to the
/// originating feed item. Status is decided per attachment (ADR 0036, ADR 0061 decision 1b):
/// a digital-signature file (`.xades`) always registers `metadata_only` (no data to fetch); a
/// structured ESEF/iXBRL statement (`.xhtml`) always registers `pending`, since the
/// periodic-report text classifier can miss an xhtml-only filing; everything else registers
/// `pending` only when the filing itself classifies as a periodic report, else `metadata_only`
/// (URL + attribution only, no bytes). Idempotent via the report-document `(company_id, url)`
/// UNIQUE key.
fn register_bankier_company_attachments(
    connection: &Connection,
    item: &BankierCompanyItem,
    feed_item_id: &str,
) -> StorageResult<()> {
    if item.company_id.trim().is_empty() || item.attachments.is_empty() {
        return Ok(());
    }

    let is_periodic = crate::source_adapters::bankier_company::is_periodic_report_item(item);

    for attachment in &item.attachments {
        let title = if attachment.label.trim().is_empty() {
            item.title.clone()
        } else {
            attachment.label.clone()
        };
        let input = CaptureReportDocumentInput {
            company_id: item.company_id.clone(),
            source_type: "espi_attachment".to_owned(),
            url: attachment.url.clone(),
            period_id: None,
            origin_ref: Some(feed_item_id.to_owned()),
            title: Some(title),
            attribution: Some(crate::source_adapters::bankier_company::ATTRIBUTION.to_owned()),
        };
        let status = attachment_fetch_status(is_periodic, &attachment.url);
        super::report_documents::create_or_find_with_status(connection, input, status)?;
    }

    Ok(())
}

/// Per-attachment fetch-status gate (ADR 0061 decision 1b). See
/// [`register_bankier_company_attachments`] for the full rationale.
fn attachment_fetch_status(is_periodic_report_item: bool, url: &str) -> &'static str {
    use crate::source_adapters::bankier_company::{
        is_signature_attachment_url, is_structured_attachment_url,
    };

    if is_signature_attachment_url(url) {
        "metadata_only"
    } else if is_periodic_report_item || is_structured_attachment_url(url) {
        "pending"
    } else {
        "metadata_only"
    }
}

pub(super) fn ingest_bankier_company_items(
    connection: &mut Connection,
    items: &[BankierCompanyItem],
) -> StorageResult<SourceIngestionResult> {
    let transaction = connection.transaction()?;
    let mut items_created = 0;
    let mut items_matched = 0;
    let mut items_unmatched = 0;
    let fetched_at = items
        .first()
        .map(|item| item.fetched_at.clone())
        .map(Ok)
        .unwrap_or_else(|| current_timestamp(&transaction))?;
    let detail_items_attempted = items
        .iter()
        .filter(|item| item.detail_fetch_attempted)
        .count();
    let detail_items_stored = items.iter().filter(|item| item.body_text.is_some()).count();
    let detail_items_failed = detail_items_attempted.saturating_sub(detail_items_stored);

    for item in items {
        let existing_feed_item_id =
            find_bankier_company_feed_item_by_source_url(&transaction, &item.link)?;
        let existing_gpw_item_id = if existing_feed_item_id.is_none() {
            find_existing_gpw_report_for_bankier_company_item(&transaction, item)?
        } else {
            None
        };

        if let Some(feed_item_id) = existing_gpw_item_id {
            record_bankier_company_duplicate_seen(&transaction, &feed_item_id, item)?;
            items_matched += 1;
            transaction.execute(
                "
                INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
                VALUES (?1, ?2, ?3)
                ",
                params![
                    feed_item_id,
                    item.company_id,
                    "secondary_official_duplicate",
                ],
            )?;
            register_bankier_company_attachments(&transaction, item, &feed_item_id)?;
        } else {
            let feed_item_id = existing_feed_item_id
                .clone()
                .unwrap_or_else(|| feed_item_id(&item.dedupe_key));
            let existed =
                existing_feed_item_id.is_some() || feed_item_exists(&transaction, &feed_item_id)?;

            upsert_bankier_company_feed_item(&transaction, &feed_item_id, item)?;
            transaction.execute(
                "DELETE FROM feed_item_companies WHERE feed_item_id = ?1",
                [&feed_item_id],
            )?;

            if !existed {
                items_created += 1;
            }

            if item.company_id.trim().is_empty() {
                items_unmatched += 1;
            } else {
                items_matched += 1;
                transaction.execute(
                    "
                    INSERT OR IGNORE INTO feed_item_companies (feed_item_id, company_id, match_type)
                    VALUES (?1, ?2, ?3)
                    ",
                    params![feed_item_id, item.company_id, "bankier_tag_id"],
                )?;
                register_bankier_company_attachments(&transaction, item, &feed_item_id)?;
            }
        }
    }

    super::ingestion::record_source_outcome(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        &fetched_at,
        items.len(),
        items_created,
        items_matched,
        items_unmatched,
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_detail_items_attempted",
        &detail_items_attempted.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_detail_items_stored",
        &detail_items_stored.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_detail_items_failed",
        &detail_items_failed.to_string(),
    )?;

    transaction.commit()?;

    // Typed ESPI/EBI classification (ADR 0034) runs after the feed-item commit
    // as a best-effort enhancement: it classifies the official filings into rule
    // signals, but a classification failure must never roll back ingestion or
    // fail the source refresh. Errors are logged and the refresh still succeeds.
    if let Err(error) = signals::classify_pending_feed_items(connection, BANKIER_COMPANY_ADAPTER_ID)
    {
        log::warn!(
            "module=signals stage=classify_pending adapter={} error={}",
            BANKIER_COMPANY_ADAPTER_ID,
            error
        );
    }

    Ok(SourceIngestionResult {
        adapter_id: BANKIER_COMPANY_ADAPTER_ID.to_owned(),
        items_fetched: items.len(),
        items_created,
        items_matched,
        items_unmatched,
        detail_items_attempted,
        detail_items_stored,
        detail_items_failed,
        fetched_at: Some(fetched_at),
    })
}

pub(super) fn find_bankier_company_feed_item_by_source_url(
    connection: &Connection,
    source_url: &str,
) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "
            SELECT id
            FROM feed_items
            WHERE source_adapter_id = ?1
                AND source_url = ?2
            ORDER BY updated_at DESC, id
            LIMIT 1
            ",
            params![BANKIER_COMPANY_ADAPTER_ID, source_url],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn find_existing_gpw_report_for_bankier_company_item(
    connection: &Connection,
    item: &BankierCompanyItem,
) -> StorageResult<Option<String>> {
    let mut statement = connection.prepare(
        "
        SELECT feed_items.id, feed_items.title
        FROM feed_items
        INNER JOIN feed_item_companies
            ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_item_companies.company_id = ?1
            AND feed_items.source_adapter_id = ?2
            AND feed_items.type = 'Official report'
        ORDER BY COALESCE(feed_items.published_at, feed_items.fetched_at) DESC,
            feed_items.updated_at DESC
        LIMIT 100
        ",
    )?;
    let rows = statement.query_map(params![&item.company_id, ADAPTER_ID], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let comparable_title = comparable_official_title(&item.title);

    for row in rows {
        let (feed_item_id, title) = row?;
        if comparable_official_title(&title) == comparable_title {
            return Ok(Some(feed_item_id));
        }
    }

    Ok(None)
}

pub(super) fn upsert_bankier_company_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierCompanyItem,
) -> StorageResult<()> {
    let summary = empty_string_to_none(Some(format_bankier_company_summary(item)));
    let story_key = super::ingestion::derive_story_key(
        &item.title,
        std::slice::from_ref(&item.qualified_ticker),
        item.published_at.as_deref(),
    );
    super::ingestion::upsert_feed_item(
        connection,
        &super::ingestion::NormalizedFeedItem {
            id: feed_item_id,
            item_type: "Official report",
            source_adapter_id: BANKIER_COMPANY_ADAPTER_ID,
            source_name: BANKIER_COMPANY_DISPLAY_NAME,
            source_url: &item.link,
            title: &item.title,
            summary: summary.as_deref(),
            body_text: item.body_text.as_deref(),
            language: "pl",
            published_at: item.published_at.as_deref(),
            fetched_at: &item.fetched_at,
            dedupe_key: &item.dedupe_key,
            attribution: BANKIER_COMPANY_ATTRIBUTION,
            display_company: &item.qualified_ticker,
            duplicate_signature: Some(&item.duplicate_signature),
            story_key: story_key.as_deref(),
        },
    )?;
    if item.body_text.is_some() {
        replace_bankier_company_feed_item_attachments(connection, feed_item_id, &item.attachments)?;
    }

    Ok(())
}

pub(super) fn replace_bankier_company_feed_item_attachments(
    connection: &Connection,
    feed_item_id: &str,
    attachments: &[BankierCompanyAttachment],
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM feed_item_attachments WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;

    for (position, attachment) in attachments.iter().enumerate() {
        connection.execute(
            "
            INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ",
            params![
                format!(
                    "feed_attachment_{}_{}",
                    slug_part(feed_item_id),
                    slug_part(&attachment.url)
                ),
                feed_item_id,
                attachment.label,
                attachment.url,
                position as i64,
            ],
        )?;
    }

    Ok(())
}

pub(super) fn record_bankier_company_duplicate_seen(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierCompanyItem,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE feed_items
        SET fetched_at = CASE
                WHEN fetched_at < ?2 THEN ?2
                ELSE fetched_at
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![feed_item_id, &item.fetched_at],
    )?;

    Ok(())
}

pub(super) fn format_bankier_company_summary(item: &BankierCompanyItem) -> String {
    let source_type = match item.pub_id {
        3 => "ESPI",
        379 => "EBI",
        _ => "Bankier komunikat",
    };

    if item
        .body_text
        .as_deref()
        .unwrap_or_default()
        .trim()
        .is_empty()
    {
        item.summary.trim().to_owned()
    } else {
        source_type.to_owned()
    }
}

pub(super) fn comparable_official_title(value: &str) -> String {
    let title = value
        .split_once(':')
        .map(|(_, title)| title)
        .unwrap_or(value)
        .trim();

    normalize_media_match_text(title)
}

pub(super) fn find_bankier_feed_item_by_source_url(
    connection: &Connection,
    source_url: &str,
) -> StorageResult<Option<String>> {
    connection
        .query_row(
            "
            SELECT id
            FROM feed_items
            WHERE source_adapter_id = ?1
                AND source_url = ?2
            ORDER BY updated_at DESC, id
            LIMIT 1
            ",
            params![BANKIER_RSS_ADAPTER_ID, source_url],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn find_media_feed_item_by_duplicate_signature(
    connection: &Connection,
    duplicate_signature: Option<&str>,
    excluded_source_adapter_id: &str,
) -> StorageResult<Option<String>> {
    let Some(duplicate_signature) = duplicate_signature else {
        return Ok(None);
    };

    connection
        .query_row(
            "
            SELECT id
            FROM feed_items
            WHERE duplicate_signature = ?1
                AND source_adapter_id <> ?2
                AND type IN ('Public media', 'Analysis')
            ORDER BY COALESCE(published_at, fetched_at) DESC, updated_at DESC, id
            LIMIT 1
            ",
            params![duplicate_signature, excluded_source_adapter_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn insert_bankier_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
    display_company: &str,
    duplicate_signature: Option<&str>,
    story_key: Option<&str>,
) -> StorageResult<()> {
    let summary = empty_string_to_none(Some(item.summary.clone()));
    super::ingestion::upsert_feed_item(
        connection,
        &super::ingestion::NormalizedFeedItem {
            id: feed_item_id,
            item_type: "Public media",
            source_adapter_id: BANKIER_RSS_ADAPTER_ID,
            source_name: BANKIER_RSS_DISPLAY_NAME,
            source_url: &item.link,
            title: &item.title,
            summary: summary.as_deref(),
            body_text: None,
            language: "pl",
            published_at: item.published_at.as_deref(),
            fetched_at: &item.fetched_at,
            dedupe_key: &item.dedupe_key,
            attribution: BANKIER_RSS_ATTRIBUTION,
            display_company,
            duplicate_signature,
            story_key,
        },
    )
}

pub(super) fn update_bankier_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
    display_company: &str,
    duplicate_signature: Option<&str>,
    story_key: Option<&str>,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE feed_items
        SET type = ?2,
            source_adapter_id = ?3,
            source_name = ?4,
            source_url = ?5,
            title = ?6,
            summary = ?7,
            body_text = NULL,
            language = 'pl',
            published_at = ?8,
            fetched_at = ?9,
            dedupe_key = ?10,
            attribution = ?11,
            display_company = ?12,
            duplicate_signature = ?13,
            story_key = ?14,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            feed_item_id,
            "Public media",
            BANKIER_RSS_ADAPTER_ID,
            BANKIER_RSS_DISPLAY_NAME,
            &item.link,
            &item.title,
            empty_string_to_none(Some(item.summary.clone())),
            item.published_at.as_deref(),
            &item.fetched_at,
            &item.dedupe_key,
            BANKIER_RSS_ATTRIBUTION,
            display_company,
            duplicate_signature,
            story_key,
        ],
    )?;

    Ok(())
}

pub(super) fn record_media_duplicate_seen(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
) -> StorageResult<()> {
    connection.execute(
        "
        UPDATE feed_items
        SET fetched_at = CASE
                WHEN fetched_at < ?2 THEN ?2
                ELSE fetched_at
            END,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![feed_item_id, &item.fetched_at],
    )?;

    Ok(())
}

pub(super) fn replace_feed_item_attachments(
    connection: &Connection,
    feed_item_id: &str,
    attachments: &[GpwReportAttachment],
) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM feed_item_attachments WHERE feed_item_id = ?1",
        [feed_item_id],
    )?;

    for (position, attachment) in attachments.iter().enumerate() {
        connection.execute(
            "
            INSERT INTO feed_item_attachments (id, feed_item_id, label, url, position)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(feed_item_id, url) DO UPDATE SET
                label = excluded.label,
                position = excluded.position
            ",
            params![
                feed_item_attachment_id(feed_item_id, &attachment.url),
                feed_item_id,
                attachment.label,
                attachment.url,
                position as i64,
            ],
        )?;
    }

    Ok(())
}

pub(super) fn feed_item_exists(connection: &Connection, feed_item_id: &str) -> StorageResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM feed_items WHERE id = ?1)",
            [feed_item_id],
            |row| row.get(0),
        )
        .map_err(StorageError::from)
}

pub(super) fn set_source_adapter_state(
    connection: &Connection,
    adapter_id: &str,
    key: &str,
    value: &str,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO source_adapter_state (source_adapter_id, state_key, state_value)
        VALUES (?1, ?2, ?3)
        ON CONFLICT(source_adapter_id, state_key) DO UPDATE SET
            state_value = excluded.state_value,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![adapter_id, key, value],
    )?;

    Ok(())
}

pub(super) fn current_timestamp(connection: &Connection) -> StorageResult<String> {
    connection
        .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
            row.get(0)
        })
        .map_err(StorageError::from)
}

use super::database::Database;
/// sources domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::sources()`.
#[derive(Clone)]
pub struct SourcesStore {
    db: Database,
}

impl SourcesStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn ingest_gpw_report_listings(
        &self,
        listings: &[GpwReportListing],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;

        ingest_gpw_report_listings(&mut connection, listings)
    }

    pub fn ingest_bankier_rss_items(
        &self,
        items: &[BankierRssItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;

        ingest_bankier_rss_items(&mut connection, items)
    }

    pub fn list_bankier_company_targets(&self) -> StorageResult<Vec<BankierCompanyTarget>> {
        let connection = self.db.checkout()?;

        list_bankier_company_targets(&connection)
    }

    pub fn upsert_bankier_company_identifiers(
        &self,
        company_id: &str,
        identifiers: &BankierCompanyIdentifiers,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        upsert_bankier_company_identifiers(&connection, company_id, identifiers)
    }

    pub fn list_bankier_company_detail_cached_urls(&self) -> StorageResult<Vec<String>> {
        let connection = self.db.checkout()?;

        list_bankier_company_detail_cached_urls(&connection)
    }

    pub fn ingest_bankier_company_items(
        &self,
        items: &[BankierCompanyItem],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;

        ingest_bankier_company_items(&mut connection, items)
    }

    pub fn record_source_adapter_state(
        &self,
        adapter_id: &str,
        key: &str,
        value: &str,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        set_source_adapter_state(&connection, adapter_id, key, value)
    }
}
