//! Typed ESPI/EBI event classification storage (ADR 0034).
//!
//! Classification output is the canonical `company_signals` entity. The rule
//! classifier runs synchronously during official-report ingestion and writes
//! `confirmed` signals for confident matches; filings the rules cannot place
//! stay unclassified (no signal) and are picked up later by the opt-in AI
//! fallback (which writes `proposed` signals). The taxonomy lives in the
//! seeded, extensible `signal_categories` registry, consumed through the
//! interpretation-layer `RuleClassifier` capability (ADR 0035).

use serde::Deserialize;
use tauri::async_runtime::block_on;

use crate::interpretation::{
    build_classifier, CategoryRule, ClassificationRequest, STATIC_STRATEGY,
};

use super::research_reminders::{create_research_reminder, NewResearchReminder};
use super::*;

/// Categories important enough to generate a research reminder when classified
/// (ADR 0034 §6). High-signal disclosures the investor should not miss.
const HIGH_SIGNAL_CATEGORIES: &[&str] = &["insider_transaction", "profit_warning"];

/// The deterministic id of a signal: a filing has at most one signal per category.
pub(super) fn signal_id(feed_item_id: &str, category: &str) -> String {
    format!("signal_{}_{}", slug_part(feed_item_id), slug_part(category))
}

/// Raw `rule_definition_json` shape consumed by the rule classifier.
#[derive(Debug, Default, Deserialize)]
struct RuleDefinition {
    #[serde(default)]
    patterns: Vec<String>,
    #[serde(default)]
    confidence: f32,
}

/// One seeded signal category with its parsed classification rule.
struct CategoryRow {
    key: String,
    rule: RuleDefinition,
}

/// Load the seeded taxonomy and build the rule classifier's category rules.
/// Categories with no patterns (e.g. `other`) never produce a rule match.
fn load_category_rules(connection: &Connection) -> StorageResult<Vec<CategoryRule>> {
    let categories = load_categories(connection)?;
    Ok(categories
        .into_iter()
        .filter(|row| !row.rule.patterns.is_empty())
        .map(|row| CategoryRule {
            category: row.key,
            patterns: row.rule.patterns,
            confidence: row.rule.confidence,
        })
        .collect())
}

fn load_categories(connection: &Connection) -> StorageResult<Vec<CategoryRow>> {
    let mut statement =
        connection.prepare("SELECT key, rule_definition_json FROM signal_categories")?;
    let rows = statement.query_map([], |row| {
        let key: String = row.get(0)?;
        let rule_json: String = row.get(1)?;
        Ok((key, rule_json))
    })?;

    let mut categories = Vec::new();
    for row in rows {
        let (key, rule_json) = row?;
        let rule = serde_json::from_str::<RuleDefinition>(&rule_json).unwrap_or_default();
        categories.push(CategoryRow { key, rule });
    }
    Ok(categories)
}

/// Classify a single official ESPI/EBI filing and persist a `confirmed` rule
/// signal when a category matches confidently. Idempotent: a signal already
/// recorded for `(feed_item_id, category)` is left untouched, so re-ingestion
/// never duplicates or overwrites a signal. Returns `true` when a new signal
/// was created. Source-neutral: callers pass any official-report feed item.
pub(super) fn classify_and_store_signal(
    connection: &Connection,
    cached_rules: &[CategoryRule],
    feed_item_id: &str,
    company_id: &str,
    title: &str,
    signal_date: Option<&str>,
) -> StorageResult<bool> {
    if cached_rules.is_empty() || company_id.trim().is_empty() || title.trim().is_empty() {
        return Ok(false);
    }

    let classifier = build_classifier(STATIC_STRATEGY, cached_rules.to_vec())
        .map_err(|error| StorageError::Classification(error.to_string()))?;
    let classification = block_on(classifier.classify(ClassificationRequest {
        text: title.to_owned(),
        candidate_categories: Vec::new(),
    }))
    .map_err(|error| StorageError::Classification(error.to_string()))?;

    // Conservative posture (ADR 0034): only a confident category match becomes a
    // confirmed signal; the unknown outcome produces no signal.
    let Some(category) = classification.category else {
        return Ok(false);
    };

    let id = signal_id(feed_item_id, &category);
    let normalized_date = signal_date
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(10).collect::<String>());

    let affected = connection.execute(
        "
        INSERT INTO company_signals (
            id, company_id, feed_item_id, category, confidence,
            classified_by, status, signal_date
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'rule', 'confirmed', ?6)
        ON CONFLICT(feed_item_id, category) DO NOTHING
        ",
        params![
            id,
            company_id,
            feed_item_id,
            category,
            classification.confidence as f64,
            normalized_date,
        ],
    )?;

    Ok(affected > 0)
}

/// Ensure every confirmed high-signal disclosure (ADR 0034 §6) has a
/// `signal_review` research reminder. Idempotent: it creates a reminder only for
/// high-signal signals that do not already have one (matched by `source_id`), so
/// it both back-fills signals classified before the reminder hook existed and
/// never duplicates on re-runs. Best-effort per signal — a single failure is
/// logged and does not abort the sweep.
pub(super) fn ensure_high_signal_reminders(connection: &Connection) -> StorageResult<usize> {
    let placeholders = HIGH_SIGNAL_CATEGORIES
        .iter()
        .map(|category| format!("'{category}'"))
        .collect::<Vec<_>>()
        .join(", ");
    let query = format!(
        "
        SELECT
            company_signals.id,
            company_signals.company_id,
            company_signals.category,
            feed_items.title
        FROM company_signals
        JOIN feed_items ON feed_items.id = company_signals.feed_item_id
        WHERE company_signals.status = 'confirmed'
          AND company_signals.category IN ({placeholders})
          AND NOT EXISTS (
              SELECT 1 FROM research_reminders
              WHERE research_reminders.source_type = 'company_signal'
                AND research_reminders.source_id = company_signals.id
          )
        "
    );
    let mut statement = connection.prepare(&query)?;
    let pending = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut created = 0;
    for (signal_id, company_id, category, title) in pending {
        let readable = category.replace('_', " ");
        match create_research_reminder(
            connection,
            NewResearchReminder {
                scope_type: "company".to_owned(),
                scope_id: company_id.clone(),
                company_id: Some(company_id),
                reminder_kind: "signal_review".to_owned(),
                source_type: Some("company_signal".to_owned()),
                source_id: Some(signal_id.clone()),
                title: title.trim().chars().take(160).collect::<String>(),
                body: Some(format!("High-signal disclosure classified as {readable}.")),
                due_at: None,
            },
        ) {
            Ok(_) => created += 1,
            Err(error) => log::warn!(
                "module=signals stage=reminder_hook signalId={} error={}",
                signal_id,
                error
            ),
        }
    }
    Ok(created)
}

/// Classify every official-report feed item already matched to a company that
/// does not yet have a signal. Called after official ingestion so newly stored
/// filings are classified in the same refresh. Returns the number of new signals.
pub(super) fn classify_pending_feed_items(
    connection: &Connection,
    source_adapter_id: &str,
) -> StorageResult<usize> {
    let rules = load_category_rules(connection)?;
    if rules.is_empty() {
        return Ok(0);
    }

    let mut statement = connection.prepare(
        "
        SELECT
            feed_items.id,
            feed_item_companies.company_id,
            feed_items.title,
            feed_items.published_at
        FROM feed_items
        JOIN feed_item_companies ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_items.source_adapter_id = ?1
        ",
    )?;
    let rows = statement
        .query_map([source_adapter_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut created = 0;
    for (feed_item_id, company_id, title, published_at) in rows {
        if classify_and_store_signal(
            connection,
            &rules,
            &feed_item_id,
            &company_id,
            &title,
            published_at.as_deref(),
        )? {
            created += 1;
        }
    }

    // Back-fill / refresh reminders for any high-signal disclosures (including
    // those classified before the reminder hook existed). Best-effort.
    if let Err(error) = ensure_high_signal_reminders(connection) {
        log::warn!("module=signals stage=reminder_sweep error={error}");
    }

    Ok(created)
}

/// Persist an AI-proposed signal for a filing the rule classifier left unknown.
/// Always `proposed` (requires user confirmation, ADR 0034) with provider
/// provenance. Idempotent on `(feed_item_id, category)`: an existing signal
/// (rule-confirmed or already proposed) is left untouched. Returns `true` when a
/// new proposal was created.
pub(super) fn create_proposed_signal(
    connection: &Connection,
    input: &ProposedSignalInput,
) -> StorageResult<bool> {
    let id = signal_id(&input.feed_item_id, &input.category);
    let normalized_date = input
        .signal_date
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(10).collect::<String>());

    let affected = connection.execute(
        "
        INSERT INTO company_signals (
            id, company_id, feed_item_id, category, confidence,
            classified_by, status, signal_date, provider_id, model_id
        )
        VALUES (?1, ?2, ?3, ?4, ?5, 'ai', 'proposed', ?6, ?7, ?8)
        ON CONFLICT(feed_item_id, category) DO NOTHING
        ",
        params![
            id,
            input.company_id,
            input.feed_item_id,
            input.category,
            input.confidence,
            normalized_date,
            input.provider_id,
            input.model_id,
        ],
    )?;

    Ok(affected > 0)
}

/// Confirm a `proposed` (AI) signal into `confirmed`. Idempotent and terminal:
/// `confirmed` signals are left unchanged. Derived-event creation for dated
/// categories is deferred to v0.41 (ADR 0034 Scope boundary).
pub(super) fn confirm_company_signal(
    connection: &Connection,
    signal_id: &str,
) -> StorageResult<CompanySignal> {
    connection.execute(
        "
        UPDATE company_signals
        SET status = 'confirmed',
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1 AND status = 'proposed'
        ",
        [signal_id],
    )?;
    // A confirmed high-signal AI proposal also raises a review reminder.
    if let Err(error) = ensure_high_signal_reminders(connection) {
        log::warn!("module=signals stage=reminder_sweep error={error}");
    }
    get_company_signal(connection, signal_id)
}

/// Reject a `proposed` (AI) signal, discarding it without creating any signal or
/// event. `confirmed` signals are never deleted by reject.
pub(super) fn reject_company_signal(connection: &Connection, signal_id: &str) -> StorageResult<()> {
    connection.execute(
        "DELETE FROM company_signals WHERE id = ?1 AND status = 'proposed'",
        [signal_id],
    )?;
    Ok(())
}

fn get_company_signal(connection: &Connection, signal_id: &str) -> StorageResult<CompanySignal> {
    let signal = connection.query_row(
        "
        SELECT
            company_signals.id,
            company_signals.company_id,
            companies.qualified_ticker,
            companies.display_name,
            company_signals.feed_item_id,
            company_signals.category,
            signal_categories.display_name,
            company_signals.confidence,
            company_signals.classified_by,
            company_signals.status,
            company_signals.signal_date,
            company_signals.provider_id,
            company_signals.model_id,
            company_signals.derived_event_id,
            feed_items.title,
            feed_items.source_url,
            company_signals.created_at,
            company_signals.updated_at
        FROM company_signals
        JOIN companies ON companies.id = company_signals.company_id
        JOIN feed_items ON feed_items.id = company_signals.feed_item_id
        JOIN signal_categories ON signal_categories.key = company_signals.category
        WHERE company_signals.id = ?1
        ",
        [signal_id],
        company_signal_from_row,
    )?;
    Ok(signal)
}

/// List the seeded categories (key + display name) for the AI fallback prompt,
/// excluding `other` (a real disclosure type is always preferred; `other` is the
/// model's implicit fallback expressed as the unknown outcome).
pub(super) fn list_categories_for_ai(
    connection: &Connection,
) -> StorageResult<Vec<SignalCategorySummary>> {
    let mut statement = connection.prepare(
        "SELECT key, display_name FROM signal_categories WHERE key != 'other' ORDER BY key",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(SignalCategorySummary {
            key: row.get(0)?,
            display_name: row.get(1)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

/// Official-report filings matched to a company that have no signal yet — the
/// candidates for the opt-in AI fallback. Bounded by `limit`.
pub(super) fn list_unclassified_official_filings(
    connection: &Connection,
    source_adapter_id: &str,
    limit: i64,
) -> StorageResult<Vec<UnclassifiedFiling>> {
    let mut statement = connection.prepare(
        "
        SELECT
            feed_items.id,
            feed_item_companies.company_id,
            feed_items.title,
            COALESCE(feed_items.body_text, ''),
            feed_items.published_at
        FROM feed_items
        JOIN feed_item_companies ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_items.source_adapter_id = ?1
          AND NOT EXISTS (
              SELECT 1 FROM company_signals
              WHERE company_signals.feed_item_id = feed_items.id
          )
        ORDER BY feed_items.published_at DESC
        LIMIT ?2
        ",
    )?;
    let rows = statement.query_map(params![source_adapter_id, limit], |row| {
        Ok(UnclassifiedFiling {
            feed_item_id: row.get(0)?,
            company_id: row.get(1)?,
            title: row.get(2)?,
            body_text: row.get(3)?,
            signal_date: row.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub(super) fn list_company_signals(
    connection: &Connection,
    input: CompanySignalListInput,
) -> StorageResult<Vec<CompanySignal>> {
    let mut statement = connection.prepare(
        "
        SELECT
            company_signals.id,
            company_signals.company_id,
            companies.qualified_ticker,
            companies.display_name,
            company_signals.feed_item_id,
            company_signals.category,
            signal_categories.display_name,
            company_signals.confidence,
            company_signals.classified_by,
            company_signals.status,
            company_signals.signal_date,
            company_signals.provider_id,
            company_signals.model_id,
            company_signals.derived_event_id,
            feed_items.title,
            feed_items.source_url,
            company_signals.created_at,
            company_signals.updated_at
        FROM company_signals
        JOIN companies ON companies.id = company_signals.company_id
        JOIN feed_items ON feed_items.id = company_signals.feed_item_id
        JOIN signal_categories ON signal_categories.key = company_signals.category
        ORDER BY company_signals.signal_date DESC, company_signals.created_at DESC
        ",
    )?;
    let rows = statement.query_map([], company_signal_from_row)?;
    let signals = rows.collect::<Result<Vec<_>, _>>()?;

    Ok(signals
        .into_iter()
        .filter(|signal| {
            input
                .company_id
                .as_deref()
                .map(|company_id| signal.company_id == company_id)
                .unwrap_or(true)
        })
        .filter(|signal| {
            input
                .category
                .as_deref()
                .map(|category| signal.category == category)
                .unwrap_or(true)
        })
        .filter(|signal| {
            input
                .status
                .as_deref()
                .map(|status| signal.status == status)
                .unwrap_or(true)
        })
        .filter(|signal| {
            if let Some(watchlist_id) = input.watchlist_id.as_deref() {
                watchlists::company_is_in_watchlist(connection, watchlist_id, &signal.company_id)
                    .unwrap_or(false)
            } else {
                true
            }
        })
        .collect())
}

fn company_signal_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CompanySignal> {
    Ok(CompanySignal {
        id: row.get(0)?,
        company_id: row.get(1)?,
        company: row.get(2)?,
        company_name: row.get(3)?,
        feed_item_id: row.get(4)?,
        category: row.get(5)?,
        category_display_name: row.get(6)?,
        confidence: row.get(7)?,
        classified_by: row.get(8)?,
        status: row.get(9)?,
        signal_date: row.get(10)?,
        provider_id: row.get(11)?,
        model_id: row.get(12)?,
        derived_event_id: row.get(13)?,
        title: row.get(14)?,
        source_url: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}
