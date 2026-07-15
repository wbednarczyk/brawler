use crate::{app_state, jobs::quote_backfill, jobs::source_refresh, storage};
use serde::Serialize;

#[tauri::command]
pub fn list_companies(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<storage::Company>, String> {
    state.list_companies().map_err(|error| error.to_string())
}

/// GPW is the only market a v1 quote provider is mapped to (ADR 0082, T1):
/// mirrors `commands::market_data`'s `MAPPED_EXCHANGE`.
const MAPPED_EXCHANGE: &str = "GPW";

#[tauri::command]
pub fn create_company(
    input: storage::NewCompany,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<storage::Company, String> {
    create_company_impl(input, &state)
}

/// The command's core logic, factored out so it is unit-testable without a
/// live `tauri::State` (the command wrapper's job is only to unwrap it).
fn create_company_impl(
    input: storage::NewCompany,
    state: &app_state::AppState,
) -> Result<storage::Company, String> {
    let company = state
        .create_company(input)
        .map_err(|error| error.to_string())?;

    // Kick off full-history quote backfill on add (v0.53 T5 wiring, ADR 0082
    // T2): idempotent and throttled, so a user can see price context without
    // a manual step. GPW-only in v1 — the same market gate `yahoo-eod` is
    // registered against.
    if company.exchange.eq_ignore_ascii_case(MAPPED_EXCHANGE) {
        quote_backfill::enqueue_backfill_for_company(state, &company.id);
    }

    Ok(company)
}

#[tauri::command]
pub fn lookup_company(
    input: storage::CompanyLookupInput,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<storage::CompanyLookupResult>, String> {
    let first_result = state
        .lookup_company(input.clone())
        .map_err(|error| error.to_string())?;
    if first_result.is_some()
        || !source_refresh::should_bootstrap_company_directories(&input, &state)?
    {
        return Ok(first_result);
    }

    source_refresh::refresh_company_directories_for_trigger(&state, "lookup")?;

    state
        .lookup_company(input)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_company(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<(), String> {
    state
        .delete_company(&company_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_company_ir_reports_url(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<String>, String> {
    state
        .get_company_ir_reports_url(&company_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_company_ir_reports_url(
    company_id: String,
    url: Option<String>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<String>, String> {
    state
        .set_company_ir_reports_url(&company_id, url.as_deref())
        .map_err(|error| error.to_string())
}

/// The company's current sector (ADR 0067 Decision 3), or None.
#[tauri::command]
pub fn get_company_sector(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<String>, String> {
    state
        .get_company_sector(&company_id)
        .map(|(sector, _source)| sector)
        .map_err(|error| error.to_string())
}

/// The distinct registry-sourced sector taxonomy — preset values for the
/// manual override picker (ADR 0067 Decision 3).
#[tauri::command]
pub fn list_company_sectors(
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Vec<String>, String> {
    state
        .list_company_sectors()
        .map_err(|error| error.to_string())
}

/// The Basic info panel's read model (owner request 2026-07-14): identity
/// facts (name, ticker, ISIN), the sector with its provenance, and the latest
/// recorded shares_outstanding fact with the period it was reported for.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CompanyBasicInfo {
    pub display_name: String,
    pub exchange: String,
    pub ticker: String,
    pub qualified_ticker: String,
    pub isin: Option<String>,
    pub sector: Option<String>,
    /// `"registry"` | `"manual"` | null — where the sector value came from.
    pub sector_source: Option<String>,
    /// Latest recorded shares_outstanding fact value (stored numeric string).
    pub shares_outstanding: Option<String>,
    /// Period label of that fact, e.g. `"2025 FY"`.
    pub shares_outstanding_period: Option<String>,
}

#[tauri::command]
pub fn get_company_basic_info(
    company_id: String,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<CompanyBasicInfo, String> {
    compute_company_basic_info(&state, &company_id)
}

/// The command's core logic, shared with the mock-fidelity harness so the
/// corpus can never diverge from real assembly.
pub fn compute_company_basic_info(
    state: &app_state::AppState,
    company_id: &str,
) -> Result<CompanyBasicInfo, String> {
    let company = state
        .list_companies()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|company| company.id == company_id)
        .ok_or_else(|| format!("no tracked company for id {company_id}"))?;
    let (sector, sector_source) = state
        .get_company_sector(company_id)
        .map_err(|error| error.to_string())?;
    let shares = state
        .latest_shares_outstanding(company_id)
        .map_err(|error| error.to_string())?;
    let (shares_outstanding, shares_outstanding_period) = match shares {
        Some((value, period)) => (Some(value), Some(period)),
        None => (None, None),
    };
    Ok(CompanyBasicInfo {
        display_name: company.display_name,
        exchange: company.exchange,
        ticker: company.ticker,
        qualified_ticker: company.qualified_ticker,
        isin: company.isin,
        sector,
        sector_source,
        shares_outstanding,
        shares_outstanding_period,
    })
}

/// Set a manual sector override that a registry refresh never clobbers; an empty
/// value clears the manual override (ADR 0067 Decision 3).
#[tauri::command]
pub fn set_company_sector(
    company_id: String,
    sector: Option<String>,
    state: tauri::State<'_, app_state::AppState>,
) -> Result<Option<String>, String> {
    state
        .set_company_sector(&company_id, sector.as_deref())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{open_in_memory_database, AppState};

    fn state() -> AppState {
        AppState::new(open_in_memory_database().expect("in-memory db"))
    }

    fn new_company(exchange: &str) -> storage::NewCompany {
        storage::NewCompany {
            exchange: exchange.to_owned(),
            ticker: "TST".to_owned(),
            display_name: "Test S.A.".to_owned(),
            isin: None,
            cik: None,
            lei: None,
        }
    }

    #[test]
    fn adding_a_gpw_company_enqueues_a_quote_backfill_job() {
        let state = state();

        let company = create_company_impl(new_company("GPW"), &state).expect("create");

        let job_id = format!("quote_backfill:{}", company.id);
        let job = state
            .jobs()
            .status(&job_id)
            .expect("job status query")
            .expect("backfill job enqueued on company add");
        assert_eq!(job.status, "pending");
    }

    #[test]
    fn basic_info_assembles_identity_sector_and_latest_shares_fact() {
        use crate::storage::{NewFinancialFact, NewFinancialPeriod};
        let state = state();
        let company = create_company_impl(new_company("GPW"), &state).expect("create");
        state
            .set_company_sector(&company.id, Some("Gry"))
            .expect("sector");
        // Two shares facts across years: the newest period wins.
        for (year, value) in [(2024, "90"), (2025, "100")] {
            let period = state
                .financials()
                .create_financial_period(NewFinancialPeriod {
                    company_id: company.id.clone(),
                    fiscal_year: year,
                    period_type: "FY".to_owned(),
                    period_end_date: None,
                    report_evidence_ref: None,
                })
                .expect("period");
            state
                .financials()
                .create_financial_fact(NewFinancialFact {
                    company_id: company.id.clone(),
                    period_id: period.id,
                    definition_id: "kpidef_shares_outstanding".to_owned(),
                    value_numeric: value.to_owned(),
                    currency: None,
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
                    source_document_ref: None,
                })
                .expect("fact");
        }

        let info = compute_company_basic_info(&state, &company.id).expect("basic info");
        assert_eq!(info.display_name, "Test S.A.");
        assert_eq!(info.qualified_ticker, "GPW:TST");
        assert_eq!(info.sector.as_deref(), Some("Gry"));
        assert_eq!(info.sector_source.as_deref(), Some("manual"));
        assert_eq!(info.shares_outstanding.as_deref(), Some("100"));
        assert_eq!(info.shares_outstanding_period.as_deref(), Some("2025 FY"));
    }

    #[test]
    fn basic_info_reports_empty_optionals_without_inventing_values() {
        let state = state();
        let company = create_company_impl(new_company("GPW"), &state).expect("create");

        let info = compute_company_basic_info(&state, &company.id).expect("basic info");
        assert_eq!(info.sector, None);
        assert_eq!(info.sector_source, None);
        assert_eq!(info.shares_outstanding, None);
        assert_eq!(info.shares_outstanding_period, None);

        assert!(compute_company_basic_info(&state, "missing").is_err());
    }

    #[test]
    fn adding_a_non_gpw_company_enqueues_no_backfill_job() {
        let state = state();

        create_company_impl(new_company("NASDAQ"), &state).expect("create");

        let counts = state.jobs().counts().expect("counts");
        assert_eq!(
            counts.pending, 0,
            "a v1-unmapped exchange must not enqueue a quote backfill"
        );
    }
}
