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
        let existed = feed_item_exists(&transaction, &feed_item_id)?;

        transaction.execute(
            "
            INSERT INTO feed_items (
                id,
                type,
                source_adapter_id,
                source_name,
                source_url,
                title,
                summary,
                body_text,
                language,
                published_at,
                fetched_at,
                dedupe_key,
                attribution,
                display_company
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, 'pl', ?8, ?9, ?10, 'GPW', ?11)
            ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
                type = excluded.type,
                source_name = excluded.source_name,
                source_url = excluded.source_url,
                title = excluded.title,
                body_text = COALESCE(excluded.body_text, feed_items.body_text),
                language = excluded.language,
                published_at = excluded.published_at,
                fetched_at = excluded.fetched_at,
                attribution = excluded.attribution,
                display_company = excluded.display_company,
                updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
            ",
            params![
                feed_item_id,
                "Official report",
                ADAPTER_ID,
                DISPLAY_NAME,
                listing.detail_url,
                listing.title,
                listing.body_text.as_deref(),
                listing.published_at,
                listing.fetched_at,
                listing.dedupe_key,
                display_company,
            ],
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

    transaction.execute(
        "
        UPDATE source_adapters
        SET last_success_at = ?1,
            last_error_at = NULL,
            last_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![&fetched_at, ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_fetched",
        &listings.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_created",
        &items_created.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        ADAPTER_ID,
        "last_items_unmatched",
        &items_unmatched.to_string(),
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
            )?;
        } else if existing_duplicate_feed_item_id.is_none() {
            insert_bankier_feed_item(
                &transaction,
                &feed_item_id,
                item,
                &display_company,
                duplicate_signature.as_deref(),
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

    transaction.execute(
        "
        UPDATE source_adapters
        SET last_success_at = ?1,
            last_error_at = NULL,
            last_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![&fetched_at, BANKIER_RSS_ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_fetched",
        &items.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_created",
        &items_created.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_RSS_ADAPTER_ID,
        "last_items_unmatched",
        &items_unmatched.to_string(),
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
            }
        }
    }

    transaction.execute(
        "
        UPDATE source_adapters
        SET last_success_at = ?1,
            last_error_at = NULL,
            last_error = NULL,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?2
        ",
        params![&fetched_at, BANKIER_COMPANY_ADAPTER_ID],
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_fetched",
        &items.len().to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_created",
        &items_created.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_matched",
        &items_matched.to_string(),
    )?;
    set_source_adapter_state(
        &transaction,
        BANKIER_COMPANY_ADAPTER_ID,
        "last_items_unmatched",
        &items_unmatched.to_string(),
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
    connection.execute(
        "
        INSERT INTO feed_items (
            id,
            type,
            source_adapter_id,
            source_name,
            source_url,
            title,
            summary,
            body_text,
            language,
            published_at,
            fetched_at,
            dedupe_key,
            attribution,
            display_company,
            duplicate_signature
        ) VALUES (?1, 'Official report', ?2, ?3, ?4, ?5, ?6, ?7, 'pl', ?8, ?9, ?10, ?11, ?12, ?13)
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
            feed_item_id,
            BANKIER_COMPANY_ADAPTER_ID,
            BANKIER_COMPANY_DISPLAY_NAME,
            &item.link,
            &item.title,
            empty_string_to_none(Some(format_bankier_company_summary(item))),
            item.body_text.as_deref(),
            item.published_at.as_deref(),
            &item.fetched_at,
            &item.dedupe_key,
            BANKIER_COMPANY_ATTRIBUTION,
            &item.qualified_ticker,
            &item.duplicate_signature,
        ],
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
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO feed_items (
            id,
            type,
            source_adapter_id,
            source_name,
            source_url,
            title,
            summary,
            body_text,
            language,
            published_at,
            fetched_at,
            dedupe_key,
            attribution,
            display_company,
            duplicate_signature
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, 'pl', ?8, ?9, ?10, ?11, ?12, ?13)
        ON CONFLICT(source_adapter_id, dedupe_key) DO UPDATE SET
            type = excluded.type,
            source_name = excluded.source_name,
            source_url = excluded.source_url,
            title = excluded.title,
            summary = excluded.summary,
            language = excluded.language,
            published_at = excluded.published_at,
            fetched_at = excluded.fetched_at,
            attribution = excluded.attribution,
            display_company = excluded.display_company,
            duplicate_signature = excluded.duplicate_signature,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
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
        ],
    )?;

    Ok(())
}

pub(super) fn update_bankier_feed_item(
    connection: &Connection,
    feed_item_id: &str,
    item: &BankierRssItem,
    display_company: &str,
    duplicate_signature: Option<&str>,
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

#[derive(Debug, Clone)]
pub(super) struct MediaMatchCompany {
    pub(super) id: String,
    pub(super) ticker: String,
    pub(super) qualified_ticker: String,
    pub(super) display_name: String,
}

pub(super) fn list_media_match_companies(
    connection: &Connection,
) -> StorageResult<Vec<MediaMatchCompany>> {
    let mut statement = connection.prepare(
        "
        SELECT id, ticker, qualified_ticker, display_name
        FROM companies
        ORDER BY qualified_ticker
        ",
    )?;

    let rows = statement.query_map([], |row| {
        Ok(MediaMatchCompany {
            id: row.get(0)?,
            ticker: row.get(1)?,
            qualified_ticker: row.get(2)?,
            display_name: row.get(3)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn find_companies_for_media_item(
    companies: &[MediaMatchCompany],
    item: &BankierRssItem,
) -> Vec<MediaMatchCompany> {
    let haystack = normalize_media_match_text(&format!("{} {}", item.title, item.summary));
    let tokens = haystack.split_whitespace().collect::<Vec<_>>();

    companies
        .iter()
        .filter(|company| {
            let company_name = normalized_company_name_signal(&company.display_name);
            let ticker = company.ticker.to_uppercase();

            (!company_name.is_empty() && haystack.contains(&company_name))
                || (ticker.chars().count() >= 3 && tokens.iter().any(|token| *token == ticker))
        })
        .cloned()
        .collect()
}

pub(super) fn media_duplicate_signature(
    item: &BankierRssItem,
    matched_companies: &[MediaMatchCompany],
) -> Option<String> {
    if matched_companies.is_empty() {
        return None;
    }

    let normalized_title = normalize_media_match_text(&item.title);
    if normalized_title.chars().count() < 12 {
        return None;
    }

    let mut companies = matched_companies
        .iter()
        .map(|company| company.qualified_ticker.as_str())
        .collect::<Vec<_>>();
    companies.sort_unstable();
    companies.dedup();

    Some(format!(
        "media:{}:{}",
        companies.join("+"),
        slug_part(&normalized_title)
    ))
}

pub(super) fn normalized_company_name_signal(value: &str) -> String {
    let mut normalized = normalize_media_match_text(value);
    for suffix in [" SPOLKA AKCYJNA", " S A", " SA"] {
        if let Some(stripped) = normalized.strip_suffix(suffix) {
            normalized = stripped.trim().to_owned();
        }
    }

    if normalized.chars().count() < 4 {
        String::new()
    } else {
        normalized
    }
}

pub(super) fn normalize_media_match_text(value: &str) -> String {
    value
        .chars()
        .map(normalize_media_character)
        .map(|character| {
            if character.is_alphanumeric() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn normalize_media_character(character: char) -> char {
    match character {
        'ą' | 'Ą' => 'A',
        'ć' | 'Ć' => 'C',
        'ę' | 'Ę' => 'E',
        'ł' | 'Ł' => 'L',
        'ń' | 'Ń' => 'N',
        'ó' | 'Ó' => 'O',
        'ś' | 'Ś' => 'S',
        'ż' | 'Ż' | 'ź' | 'Ź' => 'Z',
        other => other.to_uppercase().next().unwrap_or(other),
    }
}

pub(super) struct MatchedCompany {
    pub(super) id: String,
    pub(super) qualified_ticker: String,
    pub(super) match_type: &'static str,
}

pub(super) fn find_company_for_gpw_listing(
    connection: &Connection,
    ticker: &str,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    find_company_for_exchange_listing(connection, "GPW", ticker, isin)
}

pub(super) fn find_company_for_exchange_listing(
    connection: &Connection,
    exchange: &str,
    ticker: &str,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    if let Some(company) = find_company_by_ticker(connection, exchange, ticker)? {
        return Ok(Some(company));
    }

    if let Some(mapped_ticker) = registry_ticker_for_exchange_isin(connection, exchange, isin)? {
        if let Some(company) = find_company_by_ticker(connection, exchange, &mapped_ticker)? {
            return Ok(Some(company));
        }
    }

    if let Some(company) = find_company_by_isin(connection, isin)? {
        return Ok(Some(company));
    }

    Ok(None)
}

pub(super) fn find_company_by_ticker(
    connection: &Connection,
    exchange: &str,
    ticker: &str,
) -> StorageResult<Option<MatchedCompany>> {
    let ticker = ticker.trim();
    if ticker.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT id, qualified_ticker
            FROM companies
            WHERE exchange = ?1 AND ticker = ?2
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            params![exchange.trim().to_uppercase(), ticker.to_uppercase()],
            |row| {
                Ok(MatchedCompany {
                    id: row.get(0)?,
                    qualified_ticker: row.get(1)?,
                    match_type: "ticker",
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn find_company_by_isin(
    connection: &Connection,
    isin: &str,
) -> StorageResult<Option<MatchedCompany>> {
    if isin.trim().is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT id, qualified_ticker
            FROM companies
            WHERE isin = ?1
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            [isin.trim()],
            |row| {
                Ok(MatchedCompany {
                    id: row.get(0)?,
                    qualified_ticker: row.get(1)?,
                    match_type: "isin",
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

pub(super) fn registry_ticker_for_exchange_isin(
    connection: &Connection,
    exchange: &str,
    isin: &str,
) -> StorageResult<Option<String>> {
    let isin = isin.trim();
    if isin.is_empty() {
        return Ok(None);
    }

    connection
        .query_row(
            "
            SELECT ticker
            FROM company_registry_entries
            WHERE exchange = ?1
                AND isin = ?2
                AND active = 1
            ORDER BY qualified_ticker
            LIMIT 1
            ",
            params![exchange.trim().to_uppercase(), isin.to_uppercase()],
            |row| row.get(0),
        )
        .optional()
        .map_err(StorageError::from)
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
