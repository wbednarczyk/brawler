//! Source reconciliation — the GPW ESPI/EBI second witness (ADR 0069 decision 2
//! as amended 2026-07-15; plan v0.55 T3).
//!
//! The GPW ESPI/EBI adapter runs as a **witness**, not a feed source: its official
//! ESPI/EBI listings are matched against the Bankier-sourced reports the primary
//! channel ingested, and the agreement is recorded per disclosure:
//!
//! - `matched` — a witness item a Bankier-sourced report also carries.
//! - `espi_only` — a witness item with no matching Bankier report (the primary
//!   channel missed an official report) → raises a SYSTEM attention event
//!   (`source_reconciliation`) for the tracked company, routed through the v0.54
//!   attention surfaces.
//! - `bankier_only` — a Bankier-sourced report for a tracked company inside the
//!   reconciliation window that no witness item matched.
//!
//! HARD RULE (plan tripwire "no dual ingestion"): this path NEVER inserts
//! `feed_items`. The witness closes the Bankier single-point-of-failure without
//! the ESPI items ever entering the feed/Inbox — deduplication is impossible by
//! construction.
//!
//! Matching is tolerant: exact ESPI report-number match first (e.g. Bankier
//! titles carry "RB 15/2026"), then a fallback on (company, disclosure date).
//! Untracked issuers are skipped (no reconciliation obligation). Re-running over
//! the same listings is idempotent — reconciliation rows carry a deterministic,
//! status-independent id and UPSERT in place, and the attention event dedups on
//! that id.

use rusqlite::{params, Connection};
use serde::Serialize;

use super::database::Database;
use super::*;
use crate::source_adapters::bankier_company::ADAPTER_ID as BANKIER_COMPANY_ADAPTER_ID;
use crate::source_adapters::gpw_espi_ebi::{GpwReportListing, ADAPTER_ID as WITNESS_ADAPTER_ID};

/// Default lookback when the witness listing is empty — the window over which a
/// Bankier report with no witness match is flagged `bankier_only`.
const DEFAULT_WINDOW_DAYS: i64 = 7;

pub const STATUS_MATCHED: &str = "matched";
pub const STATUS_ESPI_ONLY: &str = "espi_only";
pub const STATUS_BANKIER_ONLY: &str = "bankier_only";

/// One persisted reconciliation-pair result (read model for the diagnostics
/// ledger).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ReconciliationResult {
    pub id: String,
    pub witness_adapter_id: String,
    pub company_id: Option<String>,
    pub qualified_ticker: Option<String>,
    pub report_number: Option<String>,
    pub report_type: Option<String>,
    pub disclosure_date: String,
    pub witness_title: String,
    pub witness_url: Option<String>,
    #[cfg_attr(
        feature = "ts-export",
        ts(type = "\"matched\" | \"bankier_only\" | \"espi_only\"")
    )]
    pub status: String,
    pub primary_feed_item_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A Bankier-sourced official report loaded as a reconciliation candidate.
#[derive(Debug, Clone)]
struct BankierCandidate {
    feed_item_id: String,
    company_id: String,
    title: String,
    date: String,
    consumed: bool,
}

/// First 10 chars of an ISO datetime = its `YYYY-MM-DD` domain date.
fn date10(value: &str) -> String {
    value.chars().take(10).collect()
}

/// Extract an ESPI-style report number `N/YYYY` from arbitrary text (a witness
/// report number or a Bankier title like "Raport bieżący nr 15/2026"). Returns a
/// normalized `"15/2026"` (no leading zeros stripped — exact token) or `None`.
fn extract_report_number(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i < bytes.len() && bytes[i] == b'/' {
                let slash = i;
                i += 1;
                let year_start = i;
                while i < bytes.len() && bytes[i].is_ascii_digit() {
                    i += 1;
                }
                if i - year_start == 4 {
                    return Some(format!("{}/{}", &text[start..slash], &text[year_start..i]));
                }
            }
        } else {
            i += 1;
        }
    }
    None
}

/// The reconciliation window start: the earliest witness disclosure date, or
/// `today - DEFAULT_WINDOW_DAYS` when there are no witness items.
fn window_start(listings: &[GpwReportListing]) -> String {
    listings
        .iter()
        .map(|listing| date10(&listing.published_at))
        .filter(|value| value.len() == 10)
        .min()
        .unwrap_or_else(|| {
            let today = time::OffsetDateTime::now_utc().date();
            today
                .saturating_sub(time::Duration::days(DEFAULT_WINDOW_DAYS))
                .format(&time::macros::format_description!("[year]-[month]-[day]"))
                .unwrap_or_else(|_| "0000-01-01".to_owned())
        })
}

/// Deterministic, status-independent reconciliation id for a witness item.
fn witness_result_id(company_id: &str, date: &str, report_number: &str, title: &str) -> String {
    let discriminator = if report_number.is_empty() {
        slug_part(title)
    } else {
        slug_part(report_number)
    };
    format!(
        "recon_{}_{}_{}_{}",
        slug_part(WITNESS_ADAPTER_ID),
        slug_part(company_id),
        slug_part(date),
        discriminator
    )
}

/// Deterministic reconciliation id for a Bankier report with no witness match.
fn bankier_result_id(company_id: &str, date: &str, feed_item_id: &str) -> String {
    format!(
        "recon_{}_{}_{}_bankier_{}",
        slug_part(WITNESS_ADAPTER_ID),
        slug_part(company_id),
        slug_part(date),
        slug_part(feed_item_id)
    )
}

/// Reconcile the witness ESPI/EBI listings against Bankier-sourced reports.
/// Returns a [`SourceIngestionResult`] whose counters describe reconciliation
/// work (documented at the counter assignments below), NOT feed ingestion.
pub(super) fn reconcile_gpw_espi_witness(
    connection: &mut Connection,
    listings: &[GpwReportListing],
) -> StorageResult<SourceIngestionResult> {
    let transaction =
        connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

    let start = window_start(listings);

    // Load Bankier-sourced official reports for tracked companies in the window.
    let mut candidates = load_bankier_candidates(&transaction, &start)?;

    let mut items_matched = 0usize; // witness items matched to a Bankier report
    let mut items_espi_only = 0usize; // witness items the primary channel missed

    for listing in listings {
        let date = date10(&listing.published_at);
        let Some(company) = feed_matching::find_company_by_isin(&transaction, &listing.isin)?
        else {
            continue; // untracked issuer — no reconciliation obligation, skip
        };
        let witness_number = extract_report_number(&listing.report_number)
            .or_else(|| extract_report_number(&listing.title))
            .unwrap_or_default();

        let matched_index = candidates.iter().position(|candidate| {
            if candidate.consumed || candidate.company_id != company.id {
                return false;
            }
            // Exact: the ESPI report number appears in the Bankier title.
            if !witness_number.is_empty() {
                if let Some(bankier_number) = extract_report_number(&candidate.title) {
                    if bankier_number == witness_number {
                        return true;
                    }
                }
            }
            // Fallback: same company + same disclosure date.
            candidate.date == date
        });

        let (status, primary_feed_item_id) = match matched_index {
            Some(index) => {
                candidates[index].consumed = true;
                items_matched += 1;
                (STATUS_MATCHED, Some(candidates[index].feed_item_id.clone()))
            }
            None => {
                items_espi_only += 1;
                (STATUS_ESPI_ONLY, None)
            }
        };

        let id = witness_result_id(&company.id, &date, &witness_number, &listing.title);
        upsert_result(
            &transaction,
            &id,
            Some(&company.id),
            report_number_opt(&witness_number, &listing.report_number),
            Some(&listing.report_type),
            &date,
            &listing.title,
            Some(&listing.detail_url),
            status,
            primary_feed_item_id.as_deref(),
        )?;

        // espi_only on a tracked company: raise the SYSTEM attention event
        // (ADR 0069 D2 amendment). Deduped on this reconciliation id.
        if status == STATUS_ESPI_ONLY {
            attention::insert_system_attention_event(
                &transaction,
                attention::TRIGGER_SOURCE_RECONCILIATION,
                &company.id,
                attention::EVIDENCE_SOURCE_RECONCILIATION,
                &id,
                &date,
            )?;
        }
    }

    // Any Bankier report the witness did not match, inside the window → bankier_only.
    // The window's BOUNDARY date is excluded: the witness listing is the latest-N
    // page, so its earliest date is only partially covered — an unmatched Bankier
    // report on that date is a truncation artifact, not a missed disclosure
    // (retro v0.55: first live run produced 10 bankier_only rows, mostly this).
    let mut items_bankier_only = 0usize;
    for candidate in candidates
        .iter()
        .filter(|candidate| !candidate.consumed && candidate.date > start)
    {
        items_bankier_only += 1;
        let id = bankier_result_id(
            &candidate.company_id,
            &candidate.date,
            &candidate.feed_item_id,
        );
        let report_number = extract_report_number(&candidate.title);
        upsert_result(
            &transaction,
            &id,
            Some(&candidate.company_id),
            report_number.as_deref(),
            None,
            &candidate.date,
            &candidate.title,
            None,
            STATUS_BANKIER_ONLY,
            Some(&candidate.feed_item_id),
        )?;
    }

    // Record the run outcome on the adapter row (live-verify harvest 2026-07-15:
    // without this the Sources screen shows the witness as "never refreshed"
    // forever — `last_success_at` is only written here). Best-effort, like the
    // KNF path (`short_positions.rs`).
    let fetched_at = listings
        .first()
        .map(|listing| listing.fetched_at.clone())
        .unwrap_or_else(now_iso);
    let _ = super::ingestion::record_source_outcome(
        &transaction,
        WITNESS_ADAPTER_ID,
        &fetched_at,
        listings.len(),
        items_bankier_only,
        items_matched,
        items_espi_only,
    );

    transaction.commit()?;

    // Counter mapping (reconciliation work, not feed ingestion):
    //   items_fetched   = witness ESPI/EBI items fetched
    //   items_matched   = witness items matched to a Bankier report
    //   items_unmatched = espi_only (witness items the primary channel missed)
    //   items_created   = bankier_only results (reports with no witness match)
    Ok(SourceIngestionResult {
        adapter_id: WITNESS_ADAPTER_ID.to_owned(),
        items_fetched: listings.len(),
        items_created: items_bankier_only,
        items_matched,
        items_unmatched: items_espi_only,
        detail_items_attempted: 0,
        detail_items_stored: 0,
        detail_items_failed: 0,
        fetched_at: listings.first().map(|listing| listing.fetched_at.clone()),
    })
}

fn report_number_opt<'a>(normalized: &'a str, raw: &'a str) -> Option<&'a str> {
    if !normalized.is_empty() {
        Some(normalized)
    } else if raw.trim().is_empty() {
        None
    } else {
        Some(raw)
    }
}

fn load_bankier_candidates(
    connection: &Connection,
    window_start: &str,
) -> StorageResult<Vec<BankierCandidate>> {
    let mut statement = connection.prepare(
        "
        SELECT feed_items.id, feed_item_companies.company_id, feed_items.title,
               substr(COALESCE(feed_items.published_at, feed_items.fetched_at), 1, 10) AS date
        FROM feed_items
        JOIN feed_item_companies ON feed_item_companies.feed_item_id = feed_items.id
        WHERE feed_items.source_adapter_id = ?1
          AND substr(COALESCE(feed_items.published_at, feed_items.fetched_at), 1, 10) >= ?2
        ",
    )?;
    let rows = statement.query_map(params![BANKIER_COMPANY_ADAPTER_ID, window_start], |row| {
        Ok(BankierCandidate {
            feed_item_id: row.get(0)?,
            company_id: row.get(1)?,
            title: row.get(2)?,
            date: row.get(3)?,
            consumed: false,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

#[allow(clippy::too_many_arguments)]
fn upsert_result(
    connection: &Connection,
    id: &str,
    company_id: Option<&str>,
    report_number: Option<&str>,
    report_type: Option<&str>,
    disclosure_date: &str,
    witness_title: &str,
    witness_url: Option<&str>,
    status: &str,
    primary_feed_item_id: Option<&str>,
) -> StorageResult<()> {
    connection.execute(
        "
        INSERT INTO source_reconciliation_results (
            id, witness_adapter_id, company_id, report_number, report_type,
            disclosure_date, witness_title, witness_url, status, primary_feed_item_id
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
        ON CONFLICT(id) DO UPDATE SET
            status = excluded.status,
            report_number = excluded.report_number,
            report_type = excluded.report_type,
            witness_title = excluded.witness_title,
            witness_url = excluded.witness_url,
            primary_feed_item_id = excluded.primary_feed_item_id,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            id,
            WITNESS_ADAPTER_ID,
            company_id,
            report_number,
            report_type,
            disclosure_date,
            witness_title,
            witness_url,
            status,
            primary_feed_item_id,
        ],
    )?;
    Ok(())
}

pub(super) fn list_source_reconciliation(
    connection: &Connection,
    limit: i64,
) -> StorageResult<Vec<ReconciliationResult>> {
    let mut statement = connection.prepare(
        "
        SELECT r.id, r.witness_adapter_id, r.company_id, companies.qualified_ticker,
               r.report_number, r.report_type, r.disclosure_date, r.witness_title,
               r.witness_url, r.status, r.primary_feed_item_id, r.created_at, r.updated_at
        FROM source_reconciliation_results r
        LEFT JOIN companies ON companies.id = r.company_id
        ORDER BY r.disclosure_date DESC, r.id DESC
        LIMIT ?1
        ",
    )?;
    let rows = statement.query_map([limit.max(0)], |row| {
        Ok(ReconciliationResult {
            id: row.get(0)?,
            witness_adapter_id: row.get(1)?,
            company_id: row.get(2)?,
            qualified_ticker: row.get(3)?,
            report_number: row.get(4)?,
            report_type: row.get(5)?,
            disclosure_date: row.get(6)?,
            witness_title: row.get(7)?,
            witness_url: row.get(8)?,
            status: row.get(9)?,
            primary_feed_item_id: row.get(10)?,
            created_at: row.get(11)?,
            updated_at: row.get(12)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Source-reconciliation domain store (Architecture v2 / ADR 0050). Reach it via
/// `AppState::reconciliation()`.
#[derive(Clone)]
pub struct ReconciliationStore {
    db: Database,
}

impl ReconciliationStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    /// Reconcile witness ESPI/EBI listings against Bankier-sourced reports.
    pub fn reconcile_gpw_espi_witness(
        &self,
        listings: &[GpwReportListing],
    ) -> StorageResult<SourceIngestionResult> {
        let mut connection = self.db.checkout()?;
        reconcile_gpw_espi_witness(&mut connection, listings)
    }

    pub fn list_source_reconciliation(
        &self,
        limit: i64,
    ) -> StorageResult<Vec<ReconciliationResult>> {
        let connection = self.db.checkout()?;
        list_source_reconciliation(&connection, limit)
    }
}

fn now_iso() -> String {
    time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_adapters::gpw_espi_ebi::GpwReportListing;
    use crate::storage::{open_in_memory_database, AppState, AttentionEventListInput, NewCompany};

    fn listing(isin: &str, report_number: &str, date: &str, title: &str) -> GpwReportListing {
        GpwReportListing {
            report_type: "Bieżący".to_owned(),
            system: "ESPI".to_owned(),
            report_number: report_number.to_owned(),
            company_ticker: String::new(),
            company_name: "Test SA".to_owned(),
            isin: isin.to_owned(),
            title: title.to_owned(),
            detail_url: format!("https://www.gpw.pl/komunikaty?id={report_number}"),
            published_at: format!("{date}T09:00:00+02:00"),
            fetched_at: "2026-07-15T10:00:00Z".to_owned(),
            dedupe_key: format!("gpw-espi-ebi:espi:{isin}:{report_number}:{date}"),
            body_text: None,
            attachments: Vec::new(),
        }
    }

    /// Seed a tracked company with an ISIN and one Bankier-sourced official-report
    /// feed item linked to it.
    fn seed_company(state: &AppState, ticker: &str, isin: &str) -> String {
        let company = state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: ticker.to_owned(),
                display_name: format!("{ticker} SA"),
                isin: Some(isin.to_owned()),
                cik: None,
                lei: None,
            })
            .expect("company");
        company.id
    }

    fn seed_bankier_report(
        state: &AppState,
        company_id: &str,
        feed_id: &str,
        title: &str,
        published_date: &str,
    ) {
        let connection = state.checkout_for_tests().expect("conn");
        connection
            .execute(
                "INSERT INTO feed_items (id, type, source_adapter_id, source_name, source_url, title, published_at, fetched_at, dedupe_key)
                 VALUES (?1, 'Official report', ?2, 'Bankier', 'https://bankier/x', ?3, ?4, ?4, ?1)",
                params![feed_id, BANKIER_COMPANY_ADAPTER_ID, title, format!("{published_date}T09:00:00Z")],
            )
            .expect("insert feed item");
        connection
            .execute(
                "INSERT INTO feed_item_companies (feed_item_id, company_id, match_type) VALUES (?1, ?2, 'bankier_tag_id')",
                params![feed_id, company_id],
            )
            .expect("link company");
    }

    fn feed_item_count(state: &AppState, adapter_id: &str) -> i64 {
        let connection = state.checkout_for_tests().expect("conn");
        connection
            .query_row(
                "SELECT COUNT(*) FROM feed_items WHERE source_adapter_id = ?1",
                [adapter_id],
                |row| row.get(0),
            )
            .expect("count")
    }

    #[test]
    fn matches_witness_to_bankier_by_report_number() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        seed_bankier_report(
            &state,
            &company_id,
            "feed_rb",
            "CD PROJEKT: Raport bieżący nr 15/2026 o czymś",
            "2026-07-14",
        );

        let result = state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[listing(
                "PLOPTTC00011",
                "15/2026",
                "2026-07-14",
                "Zawarcie umowy",
            )])
            .expect("reconcile");

        assert_eq!(result.items_fetched, 1);
        assert_eq!(result.items_matched, 1, "matched by report number");
        assert_eq!(result.items_unmatched, 0);

        let ledger = state
            .reconciliation()
            .list_source_reconciliation(50)
            .expect("list");
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].status, STATUS_MATCHED);
        assert_eq!(ledger[0].primary_feed_item_id.as_deref(), Some("feed_rb"));
    }

    #[test]
    fn matches_witness_to_bankier_by_date_fallback() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        // Bankier title has no ESPI number; only the date lines up.
        seed_bankier_report(
            &state,
            &company_id,
            "feed_periodic",
            "CD PROJEKT: raport okresowy",
            "2026-07-14",
        );

        let result = state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[listing(
                "PLOPTTC00011",
                "",
                "2026-07-14",
                "Raport kwartalny",
            )])
            .expect("reconcile");

        assert_eq!(
            result.items_matched, 1,
            "matched by (company, date) fallback"
        );
        let ledger = state
            .reconciliation()
            .list_source_reconciliation(50)
            .expect("list");
        assert_eq!(ledger[0].status, STATUS_MATCHED);
    }

    #[test]
    fn unmatched_witness_is_espi_only_and_raises_attention_event() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        // No Bankier report at all → the primary channel missed it.

        let result = state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[listing(
                "PLOPTTC00011",
                "15/2026",
                "2026-07-14",
                "Zawarcie istotnej umowy",
            )])
            .expect("reconcile");

        assert_eq!(result.items_unmatched, 1, "espi_only");
        let ledger = state
            .reconciliation()
            .list_source_reconciliation(50)
            .expect("list");
        assert_eq!(ledger[0].status, STATUS_ESPI_ONLY);

        let events = state
            .attention()
            .list_attention_events(AttentionEventListInput::default())
            .expect("events");
        assert_eq!(
            events.len(),
            1,
            "espi_only raises one system attention event"
        );
        assert_eq!(events[0].trigger_type, "source_reconciliation");
        assert_eq!(events[0].company_id, company_id);
        assert!(
            events[0].rule_id.is_none(),
            "system event has no owning rule"
        );
        assert_eq!(events[0].evidence_ref, ledger[0].id);
    }

    #[test]
    fn same_date_witness_consumes_orphan_via_date_fallback() {
        // A same-day Bankier report with a different ESPI number still matches the
        // witness by the (company, date) fallback — so it is `matched`, not
        // `bankier_only` (documents the tolerant fallback).
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        seed_bankier_report(
            &state,
            &company_id,
            "feed_orphan",
            "CD PROJEKT: Raport bieżący nr 9/2026",
            "2026-07-14",
        );

        state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[listing(
                "PLOPTTC00011",
                "10/2026",
                "2026-07-14",
                "Inny raport",
            )])
            .expect("reconcile");

        let ledger = state
            .reconciliation()
            .list_source_reconciliation(50)
            .expect("list");
        assert!(ledger.iter().all(|r| r.status != STATUS_BANKIER_ONLY));
        assert!(ledger.iter().any(|r| r.status == STATUS_MATCHED));
    }

    #[test]
    fn bankier_only_when_no_witness_covers_the_report() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        // A Bankier report on 07-12 sits INSIDE the witness window (07-10..07-14)
        // but no witness item matches it (distinct number and date) → bankier_only.
        seed_bankier_report(
            &state,
            &company_id,
            "feed_mid",
            "CD PROJEKT: Raport bieżący nr 99/2026",
            "2026-07-12",
        );

        let result = state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[
                listing("PLOPTTC00011", "5/2026", "2026-07-10", "Wczesny"),
                listing("PLOPTTC00011", "7/2026", "2026-07-14", "Późny"),
            ])
            .expect("reconcile");

        assert_eq!(
            result.items_created, 1,
            "one bankier_only inside the window"
        );
        assert_eq!(
            result.items_unmatched, 2,
            "both witness items are espi_only"
        );
        let ledger = state
            .reconciliation()
            .list_source_reconciliation(50)
            .expect("list");
        assert!(ledger.iter().any(|r| r.status == STATUS_BANKIER_ONLY));
        assert_eq!(
            ledger
                .iter()
                .filter(|r| r.status == STATUS_ESPI_ONLY)
                .count(),
            2
        );
    }

    #[test]
    fn re_running_is_idempotent_for_results_and_events() {
        let state = AppState::new(open_in_memory_database().expect("db"));
        seed_company(&state, "CDR", "PLOPTTC00011");
        let items = [listing("PLOPTTC00011", "15/2026", "2026-07-14", "Umowa")];

        state
            .reconciliation()
            .reconcile_gpw_espi_witness(&items)
            .expect("first");
        state
            .reconciliation()
            .reconcile_gpw_espi_witness(&items)
            .expect("second");

        let ledger = state
            .reconciliation()
            .list_source_reconciliation(50)
            .expect("list");
        assert_eq!(
            ledger.len(),
            1,
            "no duplicate reconciliation rows on re-run"
        );
        let events = state
            .attention()
            .list_attention_events(AttentionEventListInput::default())
            .expect("events");
        assert_eq!(events.len(), 1, "no duplicate attention events on re-run");
    }

    #[test]
    fn witness_never_inserts_feed_items() {
        // Plan tripwire "no dual ingestion": reconciliation must not add feed_items
        // for the witness adapter, and must not change total feed_items.
        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        seed_bankier_report(
            &state,
            &company_id,
            "feed_rb",
            "CD PROJEKT: Raport bieżący nr 15/2026",
            "2026-07-14",
        );
        let before_total = feed_item_count(&state, BANKIER_COMPANY_ADAPTER_ID);

        state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[
                listing("PLOPTTC00011", "15/2026", "2026-07-14", "Umowa"),
                listing("PLOPTTC00011", "99/2026", "2026-07-14", "Missed report"),
            ])
            .expect("reconcile");

        assert_eq!(
            feed_item_count(&state, WITNESS_ADAPTER_ID),
            0,
            "no feed_items for the witness adapter"
        );
        assert_eq!(
            feed_item_count(&state, BANKIER_COMPANY_ADAPTER_ID),
            before_total,
            "total Bankier feed_items unchanged"
        );
    }

    #[test]
    fn extract_report_number_pulls_espi_token() {
        assert_eq!(
            extract_report_number("Raport bieżący nr 15/2026"),
            Some("15/2026".to_owned())
        );
        assert_eq!(extract_report_number("15/2026"), Some("15/2026".to_owned()));
        assert_eq!(extract_report_number("no number here"), None);
        assert_eq!(
            extract_report_number("/2026"),
            None,
            "periodic without a number"
        );
    }

    #[test]
    fn empty_witness_listing_is_rejected_not_reconciled() {
        // Same class as the KNF empty-register guard: an empty official listing is
        // a transient fault; reconciling against it would flag every Bankier
        // report in the synthetic fallback window as bankier_only.
        use crate::source_adapters::gpw_espi_ebi::{
            refresh_witness_with, GpwFetchError, GpwPageFetcher,
        };

        struct EmptyListingFetcher;

        impl GpwPageFetcher for EmptyListingFetcher {
            fn fetch_report_page(&self) -> Result<String, GpwFetchError> {
                Ok("<html><body><ul></ul></body></html>".to_owned())
            }
        }

        let state = AppState::new(open_in_memory_database().expect("db"));
        let company_id = seed_company(&state, "CDR", "PLOPTTC00011");
        seed_bankier_report(
            &state,
            &company_id,
            "feed_guarded",
            "CD PROJEKT: Raport bieżący nr 1/2026",
            "2026-07-14",
        );

        let ctx = crate::jobs::source_refresh::RefreshContext {
            trigger: "test",
            date: None,
        };
        let outcome = refresh_witness_with(&EmptyListingFetcher, &state, &ctx);

        let error = match outcome {
            Err(error) => error,
            Ok(_) => panic!("empty witness listing must be rejected"),
        };
        assert!(error.contains("zero items"), "unexpected error: {error}");
        let connection = state.checkout_for_tests().expect("conn");
        let results: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM source_reconciliation_results",
                [],
                |row| row.get(0),
            )
            .expect("count");
        assert_eq!(results, 0, "no bankier_only spray from an empty listing");
    }

    #[test]
    fn boundary_date_report_is_not_flagged_bankier_only() {
        // The witness listing's earliest date is only partially covered (latest-N
        // truncation), so an unmatched Bankier report on that date must NOT be
        // flagged as missed (retro v0.55 harvest).
        let state = AppState::new(open_in_memory_database().expect("db"));
        let _witnessed = seed_company(&state, "CDR", "PLOPTTC00011");
        // A DIFFERENT tracked company (no witness item at all) with a report on
        // the boundary date — the company+date fallback cannot consume it.
        let other_id = seed_company(&state, "PKN", "PLPKN0000018");
        seed_bankier_report(
            &state,
            &other_id,
            "feed_boundary",
            "PKN: Raport bieżący nr 99/2026",
            "2026-07-10",
        );

        let result = state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[
                listing("PLOPTTC00011", "5/2026", "2026-07-10", "Wczesny"),
                listing("PLOPTTC00011", "7/2026", "2026-07-14", "Późny"),
            ])
            .expect("reconcile");

        assert_eq!(
            result.items_created, 0,
            "a boundary-date report is a truncation artifact, not bankier_only"
        );
    }

    #[test]
    fn successful_reconcile_records_run_outcome_on_adapter_row() {
        // Live-verify harvest 2026-07-15: the Sources screen reads
        // `last_success_at` + last-result counters off the adapter row; a refresh
        // path that forgets `record_source_outcome` shows as "never refreshed"
        // forever (this reddened on the owner's real app).
        let state = AppState::new(open_in_memory_database().expect("db"));
        let _company_id = seed_company(&state, "CDR", "PLOPTTC00011");

        state
            .reconciliation()
            .reconcile_gpw_espi_witness(&[listing(
                "PLOPTTC00011",
                "15/2026",
                "2026-07-14",
                "Raport bieżący nr 15/2026",
            )])
            .expect("reconcile");

        let connection = state.checkout_for_tests().expect("conn");
        let last_success: Option<String> = connection
            .query_row(
                "SELECT last_success_at FROM source_adapters WHERE id = ?1",
                [WITNESS_ADAPTER_ID],
                |row| row.get(0),
            )
            .expect("adapter row");
        assert!(
            last_success.is_some_and(|value| !value.trim().is_empty()),
            "witness run outcome must land on the adapter row"
        );
    }
}
