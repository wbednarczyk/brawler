//! Borrowed-connection reads for [`crate::commands::company_view`]'s
//! `get_company_view` (perf fix 2026-08-27, owner P1: ~10s real-world load —
//! root cause was pool-checkout fan-out, not query cost: one view composed
//! six independent domain stores, each store method checking out its own
//! `r2d2` connection, totalling ~50 acquisitions for a company with a deep
//! history (CDR: 53). Each is cheap uncontended, but each is an independent
//! queueing point against the production pool's small `max_connections`
//! (default 4) whenever a background job holds a connection.
//!
//! One function per composed-view section, each taking an already
//! checked-out `&Connection` so `compute_company_view` can check out ONE
//! connection and thread it through every section instead. Every function
//! here is a thin wrapper over the SAME borrowed-connection core its
//! `Store::x(&self, ...)` sibling already delegates to (`pub(super)` in its
//! own module — reachable here as a sibling within `storage`) — except
//! `coverage_rows`, which composes several such reads plus the periodic
//! report-document period derivation (below), because that assembly is
//! itself the file-size-ratcheted `fundamentals_coverage::compute_
//! fundamentals_coverage`'s job and duplicating its ~90-line body there
//! would have re-raised that pin (ADR 0103).

use rusqlite::Connection;

use crate::commands::fundamentals_coverage::{
    canonical_period_label, period_sort_index, period_type_for_index, CoverageFactsCell,
    CoveragePeriodRow, CoverageReportCell, CoverageReviewCell,
};
use crate::fundamentals::extraction::classify::{
    canonical_reports_per_period, CanonicalReportCandidate, DocKind,
};
use crate::jobs::autopilot::{is_structured_document, report_disclosure_key};
use crate::jobs::structured_extraction::{
    derive_report_period_uncached, intern_period_type, DERIVATION_VERSION,
};

use super::*;

/// Unacknowledged + acknowledged red flags (counters § signals).
pub(crate) fn red_flags_active(
    connection: &Connection,
    company_id: &str,
    today: &str,
) -> StorageResult<RedFlagsView> {
    super::red_flags::load_red_flags_view(connection, company_id, today)
}

/// Management claims for a company (counters § claims).
pub(crate) fn open_claims(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<ManagementClaim>> {
    super::management_claims::list_management_claims(connection, company_id)
}

/// The per-company short-selling cockpit view (counters § shorts).
pub(crate) fn short_positions(
    connection: &Connection,
    company_id: &str,
    today: &str,
) -> StorageResult<ShortPositionsView> {
    super::short_positions::load_short_positions_view(connection, company_id, today)
}

/// Calendar events matching the input filter (counters § events).
pub(crate) fn upcoming_events(
    connection: &Connection,
    input: CompanyEventListInput,
) -> StorageResult<Vec<CompanyEvent>> {
    super::events::list_company_events(connection, input)
}

/// Financial periods for the KPI trend section.
pub(crate) fn financial_periods(
    connection: &Connection,
    input: ListFinancialPeriodsInput,
) -> StorageResult<Vec<FinancialPeriod>> {
    super::financials::list_financial_periods(connection, input)
}

/// Financial facts for the KPI trend section.
pub(crate) fn financial_facts(
    connection: &Connection,
    input: ListFinancialFactsInput,
) -> StorageResult<Vec<FinancialFact>> {
    super::financials::list_financial_facts(connection, input)
}

/// Newest company-matched feed items (the glance feed strip).
pub(crate) fn feed_newest(
    connection: &Connection,
    company_id: &str,
    limit: i64,
) -> StorageResult<Vec<TodayFeedRow>> {
    super::feed::list_company_feed_newest(connection, company_id, limit)
}

/// Tracked companies (identity lookup + the price section's exchange check).
pub(crate) fn companies(connection: &Connection) -> StorageResult<Vec<Company>> {
    super::companies::list_companies(connection)
}

/// The most recent stored daily quote bar, if any (price section).
pub(crate) fn latest_quote(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Option<super::market_data::QuoteBar>> {
    super::market_data::latest_quote_for(connection, company_id)
}

/// Daily quote bars on or after `from_date` (price section chart window).
pub(crate) fn quote_bars_since(
    connection: &Connection,
    company_id: &str,
    from_date: &str,
) -> StorageResult<Vec<super::market_data::QuoteBar>> {
    super::market_data::quotes_since(connection, company_id, from_date)
}

/// Analyst recommendation history, newest first.
pub(crate) fn recommendations(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<AnalystRecommendationRow>> {
    super::analyst_recommendations::list_analyst_recommendations(connection, company_id)
}

// ============================================================================
// Coverage rows — composes documents + facts + budget-skip + period
// derivation, mirroring `fundamentals_coverage::compute_fundamentals_
// coverage`'s assembly exactly (ADR 0077 §2), over ONE shared connection.
// ============================================================================

/// The fundamentals-coverage map for one company, read over the shared
/// connection. Same assembly as `fundamentals_coverage::compute_
/// fundamentals_coverage` (the standalone `get_fundamentals_coverage`
/// command keeps its own checkout for that entry point) — kept here instead
/// of a `_over`/`_with` sibling on that function to avoid re-raising its
/// file-size pin (ADR 0103).
pub(crate) fn coverage_rows(
    connection: &Connection,
    state: &AppState,
    company_id: &str,
) -> Result<Vec<CoveragePeriodRow>, String> {
    let documents = super::report_documents::list_by_company(connection, company_id)
        .map_err(|e| e.to_string())?;

    let mut doc_by_id: std::collections::BTreeMap<String, ReportDocument> =
        std::collections::BTreeMap::new();
    let mut candidates: Vec<CanonicalReportCandidate> = Vec::new();
    for document in documents {
        let kind = match document.doc_kind.as_deref() {
            Some("periodic_ssf") => DocKind::PeriodicSsf,
            Some("periodic_jsf") => DocKind::PeriodicJsf,
            _ => continue,
        };
        let Some((fiscal_year, _period_type, index)) =
            document_period_over(connection, state, &document)
        else {
            continue;
        };
        candidates.push(CanonicalReportCandidate {
            document_id: document.id.clone(),
            doc_kind: kind,
            period: (fiscal_year as i32, index),
            disclosure_key: report_disclosure_key(&document),
            structured: is_structured_document(&document),
        });
        doc_by_id.insert(document.id.clone(), document);
    }

    let mut report_cells: std::collections::BTreeMap<(i64, String), CoverageReportCell> =
        std::collections::BTreeMap::new();
    for ((fiscal_year, index), candidate) in canonical_reports_per_period(&candidates) {
        let Some(period_type) = period_type_for_index(index) else {
            continue;
        };
        let Some(document) = doc_by_id.get(&candidate.document_id) else {
            continue;
        };
        report_cells.insert(
            (i64::from(fiscal_year), period_type.to_owned()),
            CoverageReportCell {
                document_id: document.id.clone(),
                doc_kind: candidate.doc_kind.as_str().to_owned(),
                title: document.title.clone(),
                structured: candidate.structured,
                fetched: document.fetch_status == "fetched",
            },
        );
    }

    let mut fact_cells: std::collections::BTreeMap<(i64, String), CoverageFactsCell> =
        std::collections::BTreeMap::new();
    for row in super::financials::facts_coverage_by_period(connection, company_id)
        .map_err(|e| e.to_string())?
    {
        let cell = fact_cells
            .entry((row.fiscal_year, canonical_period_label(&row.period_type)))
            .or_insert(CoverageFactsCell {
                total: 0,
                validated: 0,
                unvalidated: 0,
                flagged: 0,
            });
        cell.total += row.total;
        cell.validated += row.validated;
        cell.unvalidated += row.unvalidated;
        cell.flagged += row.flagged;
    }

    let budget_skipped_docs = super::autopilot::documents_skipped_by_budget(connection, company_id)
        .map_err(|e| e.to_string())?;

    let mut keys: std::collections::BTreeMap<(i64, String), ()> = std::collections::BTreeMap::new();
    for key in report_cells.keys() {
        keys.insert(key.clone(), ());
    }
    for key in fact_cells.keys() {
        keys.insert(key.clone(), ());
    }

    let mut periods: Vec<CoveragePeriodRow> = keys
        .into_keys()
        .map(|(fiscal_year, period_type)| {
            let facts = fact_cells
                .get(&(fiscal_year, period_type.clone()))
                .cloned()
                .unwrap_or(CoverageFactsCell {
                    total: 0,
                    validated: 0,
                    unvalidated: 0,
                    flagged: 0,
                });
            let report = report_cells
                .get(&(fiscal_year, period_type.clone()))
                .cloned();
            let skipped_budget = report
                .as_ref()
                .is_some_and(|cell| budget_skipped_docs.contains(&cell.document_id));
            CoveragePeriodRow {
                report,
                review: CoverageReviewCell {
                    flagged_facts: facts.flagged,
                },
                facts,
                skipped_budget,
                fiscal_year,
                period_type,
            }
        })
        .collect();

    periods.sort_by(|a, b| {
        b.fiscal_year
            .cmp(&a.fiscal_year)
            .then_with(|| period_sort_index(&b.period_type).cmp(&period_sort_index(&a.period_type)))
            .then_with(|| b.period_type.cmp(&a.period_type))
    });

    Ok(periods)
}

/// Borrowed-connection sibling of `fundamentals_coverage::document_period` —
/// same derivation + title/URL fallback, routed over `connection` instead of
/// `derive_report_period`'s own per-call checkout (the N+1 checkout this
/// whole module exists to remove: up to 34 periodic documents for CDR).
fn document_period_over(
    connection: &Connection,
    state: &AppState,
    document: &ReportDocument,
) -> Option<(i64, String, u8)> {
    if let Some((fiscal_year, period_type, _end)) =
        derive_report_period_over(connection, state, document)
    {
        let index = period_sort_index(period_type);
        return Some((fiscal_year, period_type.to_owned(), index));
    }
    let title = document.title.as_deref().unwrap_or("");
    let (year, index) = crate::report_diff::classify::period_sort_key(title, &document.url)?;
    let period_type = period_type_for_index(index)?;
    Some((i64::from(year), period_type.to_owned(), index))
}

/// Borrowed-connection sibling of
/// `structured_extraction::derive_report_period` — identical cache-first
/// logic (migration 0109/0140), reading/writing `document_derived_periods`
/// over `connection` instead of a fresh checkout per document.
fn derive_report_period_over(
    connection: &Connection,
    state: &AppState,
    document: &ReportDocument,
) -> Option<(i64, &'static str, String)> {
    if let Ok(Some(cached)) = cached_derived_period(connection, &document.id) {
        let provenance_matches = matches!(
            (cached.content_hash.as_deref(), document.content_hash.as_deref()),
            (Some(cached_hash), Some(document_hash)) if cached_hash == document_hash
        );
        if cached.derivation_version >= DERIVATION_VERSION && provenance_matches {
            if !cached.has_period {
                return None;
            }
            if let (Some(fiscal_year), Some(period_type), Some(period_end)) = (
                cached.fiscal_year,
                cached.period_type.as_deref().and_then(intern_period_type),
                cached.period_end.clone(),
            ) {
                return Some((fiscal_year, period_type, period_end));
            }
        }
    }

    let derived = derive_report_period_uncached(state, document);

    if document.fetch_status == "fetched" && document.local_path.is_some() {
        let _ = store_derived_period(
            connection,
            &document.id,
            derived.as_ref().map(|(fy, pt, pe)| (*fy, *pt, pe.as_str())),
            DERIVATION_VERSION,
            document.content_hash.as_deref(),
        );
    }

    derived
}

/// Same read as `FinancialsStore::cached_derived_period`, over a borrowed
/// connection — duplicated here (rather than split out of `financials.rs`,
/// which is already at its file-size pin, ADR 0103) since this table read is
/// only ever needed by this composed-view path.
fn cached_derived_period(
    connection: &Connection,
    report_document_id: &str,
) -> StorageResult<Option<super::financials::CachedDerivedPeriod>> {
    connection
        .query_row(
            "SELECT has_period, fiscal_year, period_type, period_end, derivation_version,
                    content_hash
             FROM document_derived_periods
             WHERE report_document_id = ?1",
            [report_document_id],
            |row| {
                Ok(super::financials::CachedDerivedPeriod {
                    has_period: row.get::<_, i64>(0)? != 0,
                    fiscal_year: row.get(1)?,
                    period_type: row.get(2)?,
                    period_end: row.get(3)?,
                    derivation_version: row.get(4)?,
                    content_hash: row.get(5)?,
                })
            },
        )
        .optional()
        .map_err(StorageError::from)
}

/// Same write as `FinancialsStore::store_derived_period`, over a borrowed
/// connection (see `cached_derived_period` doc for why it's duplicated here).
fn store_derived_period(
    connection: &Connection,
    report_document_id: &str,
    period: Option<(i64, &str, &str)>,
    derivation_version: i64,
    content_hash: Option<&str>,
) -> StorageResult<()> {
    let (has_period, fiscal_year, period_type, period_end) = match period {
        Some((fy, pt, pe)) => (1i64, Some(fy), Some(pt.to_owned()), Some(pe.to_owned())),
        None => (0i64, None, None, None),
    };
    connection.execute(
        "INSERT INTO document_derived_periods (
            report_document_id, has_period, fiscal_year, period_type, period_end,
            derivation_version, content_hash, derived_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        ON CONFLICT(report_document_id) DO UPDATE SET
            has_period = excluded.has_period,
            fiscal_year = excluded.fiscal_year,
            period_type = excluded.period_type,
            period_end = excluded.period_end,
            derivation_version = excluded.derivation_version,
            content_hash = excluded.content_hash,
            derived_at = excluded.derived_at",
        params![
            report_document_id,
            has_period,
            fiscal_year,
            period_type,
            period_end,
            derivation_version,
            content_hash
        ],
    )?;
    Ok(())
}
