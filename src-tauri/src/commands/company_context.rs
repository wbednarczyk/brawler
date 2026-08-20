//! Company-context read model (ADR 0106 dec. 3): the Inbox detail pane's
//! company-context block composed in ONE command — latest-period facts,
//! upcoming events, notebook coverage, and claims-due counts — instead of the
//! frontend fanning out 5 IPC calls per selection. Offloaded off the UI
//! thread via `spawn_blocking`, mirroring [`crate::commands::company_health`].

use serde::Serialize;

use crate::app_state::AppState;
use crate::storage::{
    CompanyEventListInput, FinancialPeriod, ListFinancialFactsInput, ListFinancialPeriodsInput,
};

const MAX_LATEST_PERIOD_FACTS: usize = 6;
const MAX_UPCOMING_EVENTS: usize = 3;

/// One fact inside [`CompanyContextLatestPeriod`].
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyContextFact {
    pub metric_key: String,
    pub value_numeric: String,
    pub currency: Option<String>,
    pub source_document_ref: Option<String>,
    pub created_at: String,
}

/// The company's latest financial period (by domain period ordering, never
/// `created_at`) and up to [`MAX_LATEST_PERIOD_FACTS`] of its facts.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyContextLatestPeriod {
    pub period_label: String,
    pub facts: Vec<CompanyContextFact>,
}

/// One upcoming calendar event (`event_date >= today`).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyContextEvent {
    pub title: String,
    pub event_date: String,
    pub event_type: String,
}

/// Notebook coverage summary.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyContextNotebook {
    pub count: i64,
    pub latest_at: Option<String>,
}

/// Management-claims due-period counts — the `due`/`overdue` bucket sizes of
/// the existing `list_claims_to_verify` read model (ADR 0040), reused rather
/// than reimplemented.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyContextClaimsDue {
    pub due: i64,
    pub overdue: i64,
}

/// Everything the Inbox detail pane's company-context block renders for one
/// company, composed in a single read (ADR 0106 dec. 3).
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyContext {
    pub company_id: String,
    /// `None` when the company has no financial period yet.
    pub latest_period_facts: Option<CompanyContextLatestPeriod>,
    pub upcoming_events: Vec<CompanyContextEvent>,
    pub notebook: CompanyContextNotebook,
    pub claims_due: CompanyContextClaimsDue,
}

/// The domain-date sort key for one period: its `period_end_date`, falling
/// back to the fiscal year's calendar end when unset. Never `created_at`
/// (data-model.md § Model principles — "domain recency is the domain date").
/// Mirrors the SQL ordering `refresh_derived_kpi_relevance` already uses to
/// pick a company's most-recent periods.
fn period_domain_key(period: &FinancialPeriod) -> (String, i64) {
    let end = period
        .period_end_date
        .clone()
        .unwrap_or_else(|| format!("{:04}-12-31", period.fiscal_year));
    (end, period.fiscal_year)
}

/// A short human label for a period, `"{period_type} {fiscal_year}"` (e.g.
/// `"H1 2026"`) — the shape already used by the coverage read model's period
/// grouping (`t7_cbf_corpus.rs`), not a new format.
fn period_label(period: &FinancialPeriod) -> String {
    format!("{} {}", period.period_type, period.fiscal_year)
}

/// Compute the company-context read model (sync core, unit-testable).
pub fn compute_company_context(
    state: &AppState,
    company_id: &str,
) -> Result<CompanyContext, String> {
    let periods = state
        .financials()
        .list_financial_periods(ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        })
        .map_err(|error| error.to_string())?;

    let latest_period = periods
        .into_iter()
        .max_by(|a, b| period_domain_key(a).cmp(&period_domain_key(b)));

    let latest_period_facts = match latest_period {
        Some(period) => {
            let facts = state
                .financials()
                .list_financial_facts(ListFinancialFactsInput {
                    company_id: Some(company_id.to_owned()),
                    period_id: Some(period.id.clone()),
                    definition_id: None,
                })
                .map_err(|error| error.to_string())?
                .into_iter()
                .take(MAX_LATEST_PERIOD_FACTS)
                .map(|fact| CompanyContextFact {
                    metric_key: fact.metric_key,
                    value_numeric: fact.value_numeric,
                    currency: fact.currency,
                    source_document_ref: fact.source_document_ref,
                    created_at: fact.created_at,
                })
                .collect();
            Some(CompanyContextLatestPeriod {
                period_label: period_label(&period),
                facts,
            })
        }
        None => None,
    };

    // `list_company_events` defaults `mode` to "upcoming" (event_date >= today,
    // soonest first) — exactly the ordering/filter this block needs.
    let upcoming_events = state
        .list_company_events(CompanyEventListInput {
            mode: None,
            company_id: Some(company_id.to_owned()),
            watchlist_id: None,
            event_type: None,
            status: None,
            date_from: None,
            date_to: None,
        })
        .map_err(|error| error.to_string())?
        .into_iter()
        .take(MAX_UPCOMING_EVENTS)
        .map(|event| CompanyContextEvent {
            title: event.title,
            event_date: event.event_date,
            event_type: event.event_type,
        })
        .collect();

    // `list_notebook_entries` orders `updated_at DESC, created_at DESC, id`,
    // so the first entry is the latest.
    let notebook_entries = state
        .notebooks()
        .list_notebook_entries(company_id)
        .map_err(|error| error.to_string())?;
    let notebook = CompanyContextNotebook {
        count: notebook_entries.len() as i64,
        latest_at: notebook_entries
            .first()
            .map(|entry| entry.updated_at.clone()),
    };

    let claims = state
        .list_claims_to_verify(company_id)
        .map_err(|error| error.to_string())?;
    let claims_due = CompanyContextClaimsDue {
        due: claims.due.len() as i64,
        overdue: claims.overdue.len() as i64,
    };

    Ok(CompanyContext {
        company_id: company_id.to_owned(),
        latest_period_facts,
        upcoming_events,
        notebook,
        claims_due,
    })
}

/// Composed company-context read model for the Inbox detail pane (ADR 0106
/// dec. 3). Offloaded off the UI thread — composes financial facts/periods,
/// events, notebook entries, and claims-due across four domain stores.
#[tauri::command]
pub async fn get_company_context(
    company_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<CompanyContext, String> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || compute_company_context(&state, &company_id))
        .await
        .map_err(|error| format!("company context task failed: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{
        open_in_memory_database, AppState, NewCompany, NewCompanyEvent, NewFinancialFact,
        NewFinancialPeriod, NewKpiDefinition, NewManagementClaim, NewNotebookEntry,
        SetClaimVerdictInput,
    };

    const NET_PROFIT: &str = "kpidef_net_profit";
    const REVENUE: &str = "kpidef_revenue";

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn company(state: &AppState) -> String {
        state
            .create_company(NewCompany {
                exchange: "GPW".to_owned(),
                ticker: "CTX".to_owned(),
                display_name: "Context Test S.A.".to_owned(),
                isin: None,
                cik: None,
                lei: None,
            })
            .expect("company")
            .id
    }

    fn period(
        state: &AppState,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
        period_end_date: &str,
    ) -> String {
        state
            .financials()
            .create_financial_period(NewFinancialPeriod {
                company_id: company_id.to_owned(),
                fiscal_year,
                period_type: period_type.to_owned(),
                period_end_date: Some(period_end_date.to_owned()),
                report_evidence_ref: None,
            })
            .expect("period")
            .id
    }

    fn fact(state: &AppState, company_id: &str, period_id: &str, definition_id: &str) {
        state
            .financials()
            .create_financial_fact(NewFinancialFact {
                company_id: company_id.to_owned(),
                period_id: period_id.to_owned(),
                definition_id: definition_id.to_owned(),
                value_numeric: "1000".to_owned(),
                currency: Some("PLN".to_owned()),
                statement_basis: None,
                attribution: None,
                variant: None,
                measure_window: None,
                data_quality: None,
                as_reported_value: None,
                as_reported_scale: None,
                reporting_standard: None,
                extraction_method: None,
                confidence: None,
                confirmation_state: None,
                supersedes_id: None,
                source_document_ref: Some("doc_evidence_1".to_owned()),
                annotation: None,
            })
            .expect("fact");
    }

    /// A company-scoped KPI definition with a fresh `metric_key` — used where
    /// a test needs several DISTINCT facts on one period (the fact id is
    /// derived from `(period_id, definition_id, ...dims)`, so reusing one
    /// definition twice on the same period/dims collides).
    fn definition(state: &AppState, company_id: &str, metric_key: &str) -> String {
        state
            .financials()
            .create_kpi_definition(NewKpiDefinition {
                scope: "company".to_owned(),
                company_id: Some(company_id.to_owned()),
                sector: None,
                metric_key: metric_key.to_owned(),
                label: metric_key.to_owned(),
                value_kind: "monetary".to_owned(),
                unit: None,
                computation: "reported".to_owned(),
                formula: None,
                display_format: None,
                origin: None,
                statement_group: None,
                period_nature: None,
            })
            .expect("kpi definition")
            .id
    }

    /// The hard ordering-rule test (docs/data-model.md § Model principles):
    /// two periods exist, the OLDER one (by fiscal year / period end) is
    /// created SECOND — i.e. `created_at` order is the reverse of domain-date
    /// order. The domain-date winner (FY2025, not the more-recently-created
    /// FY2024) must be picked as `latestPeriodFacts`.
    #[test]
    fn latest_period_is_chosen_by_domain_date_not_created_at() {
        let state = state();
        let company_id = company(&state);

        // Created FIRST (earlier created_at) but is the domain-LATEST period.
        let newer_period = period(&state, &company_id, 2025, "FY", "2025-12-31");
        fact(&state, &company_id, &newer_period, NET_PROFIT);

        // Created SECOND (later created_at) but is the domain-OLDER period —
        // created_at order and domain-date order disagree.
        let older_period = period(&state, &company_id, 2024, "FY", "2024-12-31");
        fact(&state, &company_id, &older_period, NET_PROFIT);

        let context = compute_company_context(&state, &company_id).expect("context");
        let latest = context.latest_period_facts.expect("latest period");
        assert_eq!(
            latest.period_label, "FY 2025",
            "the domain-latest period (FY2025) must win even though FY2024 was created later"
        );
    }

    /// Full-DTO assertion: a company with 2 periods, past + upcoming events,
    /// notebook entries, and due/overdue/verified claims.
    #[test]
    fn full_dto_composes_every_section() {
        let state = state();
        let company_id = company(&state);

        // Two periods; the FY2025 one is domain-latest and carries 2 facts.
        let fy2024 = period(&state, &company_id, 2024, "FY", "2024-12-31");
        let fy2025 = period(&state, &company_id, 2025, "FY", "2025-12-31");
        fact(&state, &company_id, &fy2024, NET_PROFIT);
        fact(&state, &company_id, &fy2025, NET_PROFIT);
        fact(&state, &company_id, &fy2025, REVENUE);

        // Events: one past (excluded), two future (kept, soonest first).
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "custom".to_owned(),
                title: "Past event".to_owned(),
                event_date: "2000-01-01".to_owned(),
                event_time: None,
                status: None,
                source_type: None,
                source_adapter_id: None,
                source_event_key: None,
                source_url: None,
                attribution: None,
                fetched_at: None,
            })
            .expect("past event");
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "custom".to_owned(),
                title: "Later future event".to_owned(),
                event_date: "2999-06-01".to_owned(),
                event_time: None,
                status: None,
                source_type: None,
                source_adapter_id: None,
                source_event_key: None,
                source_url: None,
                attribution: None,
                fetched_at: None,
            })
            .expect("later future event");
        state
            .create_company_event(NewCompanyEvent {
                company_id: company_id.clone(),
                event_type: "custom".to_owned(),
                title: "Sooner future event".to_owned(),
                event_date: "2999-01-01".to_owned(),
                event_time: None,
                status: None,
                source_type: None,
                source_adapter_id: None,
                source_event_key: None,
                source_url: None,
                attribution: None,
                fetched_at: None,
            })
            .expect("sooner future event");

        // Notebook: two entries.
        state
            .create_notebook_entry(NewNotebookEntry {
                company_id: company_id.clone(),
                title: "First note".to_owned(),
                body: "Body one.".to_owned(),
                body_format: None,
                tags: vec![],
                kind: "manual".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
                origins: vec![],
            })
            .expect("note 1");
        state
            .create_notebook_entry(NewNotebookEntry {
                company_id: company_id.clone(),
                title: "Second note".to_owned(),
                body: "Body two.".to_owned(),
                body_format: None,
                tags: vec![],
                kind: "manual".to_owned(),
                claim_status: None,
                event_date: None,
                follow_up_after: None,
                follow_up_date: None,
                origins: vec![],
            })
            .expect("note 2");

        // Claims: due (period already arrived), overdue (a later period
        // arrived while still pending), and verified (not pending — must not
        // count in either bucket).
        state
            .create_management_claim(NewManagementClaim {
                company_id: company_id.clone(),
                statement: "Due claim".to_owned(),
                body: None,
                made_at: None,
                source_period_id: None,
                due_fiscal_year: Some(2025),
                due_period_type: Some("FY".to_owned()),
                status: None,
                source_evidence_type: None,
                source_evidence_id: None,
                target_metric_key: None,
                target_comparator: None,
                target_value_numeric: None,
                target_unit: None,
            })
            .expect("due claim");
        state
            .create_management_claim(NewManagementClaim {
                company_id: company_id.clone(),
                statement: "Overdue claim".to_owned(),
                body: None,
                made_at: None,
                source_period_id: None,
                due_fiscal_year: Some(2023),
                due_period_type: Some("FY".to_owned()),
                status: None,
                source_evidence_type: None,
                source_evidence_id: None,
                target_metric_key: None,
                target_comparator: None,
                target_value_numeric: None,
                target_unit: None,
            })
            .expect("overdue claim");
        let verified_claim = state
            .create_management_claim(NewManagementClaim {
                company_id: company_id.clone(),
                statement: "Verified claim".to_owned(),
                body: None,
                made_at: None,
                source_period_id: None,
                due_fiscal_year: Some(2025),
                due_period_type: Some("FY".to_owned()),
                status: None,
                source_evidence_type: None,
                source_evidence_id: None,
                target_metric_key: None,
                target_comparator: None,
                target_value_numeric: None,
                target_unit: None,
            })
            .expect("verified claim");
        state
            .set_claim_verdict(SetClaimVerdictInput {
                claim_id: verified_claim.id,
                status: "delivered".to_owned(),
                verifying_fact_id: None,
                verifying_relation: None,
                revises_claim_id: None,
            })
            .expect("verify claim");

        let context = compute_company_context(&state, &company_id).expect("context");

        assert_eq!(context.company_id, company_id);

        let latest = context.latest_period_facts.expect("latest period");
        assert_eq!(latest.period_label, "FY 2025");
        assert_eq!(latest.facts.len(), 2);

        assert_eq!(context.upcoming_events.len(), 2);
        assert_eq!(context.upcoming_events[0].title, "Sooner future event");
        assert_eq!(context.upcoming_events[1].title, "Later future event");

        assert_eq!(context.notebook.count, 2);
        assert!(context.notebook.latest_at.is_some());

        assert_eq!(context.claims_due.due, 1);
        assert_eq!(context.claims_due.overdue, 1);
    }

    /// Empty-company case: every section reads back empty/null, no error.
    #[test]
    fn empty_company_composes_without_error() {
        let state = state();
        let company_id = company(&state);

        let context = compute_company_context(&state, &company_id).expect("context");

        assert_eq!(context.company_id, company_id);
        assert!(context.latest_period_facts.is_none());
        assert!(context.upcoming_events.is_empty());
        assert_eq!(context.notebook.count, 0);
        assert!(context.notebook.latest_at.is_none());
        assert_eq!(context.claims_due.due, 0);
        assert_eq!(context.claims_due.overdue, 0);
    }

    /// Cap check: more than 6 facts on the latest period, more than 3
    /// upcoming events — both truncate.
    #[test]
    fn caps_facts_at_six_and_upcoming_events_at_three() {
        let state = state();
        let company_id = company(&state);

        let period_id = period(&state, &company_id, 2025, "FY", "2025-12-31");
        // 7 distinct definitions so the fact id (derived from period +
        // definition + dims) never collides across rows.
        for i in 0..7 {
            let definition_id = definition(&state, &company_id, &format!("metric_{i}"));
            fact(&state, &company_id, &period_id, &definition_id);
        }

        for i in 0..4 {
            state
                .create_company_event(NewCompanyEvent {
                    company_id: company_id.clone(),
                    event_type: "custom".to_owned(),
                    title: format!("Event {i}"),
                    event_date: format!("2999-01-0{}", i + 1),
                    event_time: None,
                    status: None,
                    source_type: None,
                    source_adapter_id: None,
                    source_event_key: None,
                    source_url: None,
                    attribution: None,
                    fetched_at: None,
                })
                .expect("event");
        }

        let context = compute_company_context(&state, &company_id).expect("context");
        assert_eq!(context.latest_period_facts.expect("latest").facts.len(), 6);
        assert_eq!(context.upcoming_events.len(), 3);
    }
}
