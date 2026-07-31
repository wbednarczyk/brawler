use super::*;
use crate::fundamentals::validation::FactSet;
use rust_decimal::Decimal;
use std::str::FromStr;

// ============================================================================
// Public Structs (DTO/serializable types)
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FinancialPeriod {
    pub id: String,
    pub company_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end_date: Option<String>,
    pub report_evidence_ref: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiDefinition {
    pub id: String,
    pub scope: String,
    pub company_id: Option<String>,
    pub sector: Option<String>,
    pub metric_key: String,
    pub label: String,
    pub value_kind: String,
    pub unit: Option<String>,
    pub computation: String,
    pub formula: Option<String>,
    pub display_format: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct KpiRelevance {
    pub id: String,
    pub company_id: String,
    pub definition_id: String,
    pub status: String,
    pub source: String,
    pub rank: Option<String>,
    pub first_seen_period: Option<String>,
    pub last_seen_period: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FinancialFact {
    pub id: String,
    pub company_id: String,
    pub period_id: String,
    pub definition_id: String,
    pub value_numeric: String,
    pub currency: Option<String>,
    pub statement_basis: String,
    pub attribution: String,
    pub variant: String,
    pub measure_window: String,
    pub data_quality: String,
    pub as_reported_value: Option<String>,
    pub as_reported_scale: Option<String>,
    pub reporting_standard: Option<String>,
    pub extraction_method: String,
    pub confidence: Option<String>,
    pub confirmation_state: String,
    pub supersedes_id: Option<String>,
    pub source_document_ref: Option<String>,
    /// User-authored one-off note (#156): the value contains a one-off event
    /// (e.g. discontinued operations inside net_profit). Renders as a visible
    /// '*' marker; the value itself stays exactly as reported. Never written by
    /// any extraction path.
    pub annotation: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewFinancialPeriod {
    pub company_id: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end_date: Option<String>,
    pub report_evidence_ref: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFinancialPeriod {
    pub id: String,
    pub period_end_date: Option<String>,
    pub report_evidence_ref: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewKpiDefinition {
    pub scope: String,
    pub company_id: Option<String>,
    pub sector: Option<String>,
    pub metric_key: String,
    pub label: String,
    pub value_kind: String,
    pub unit: Option<String>,
    pub computation: String,
    pub formula: Option<String>,
    pub display_format: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewKpiRelevance {
    pub company_id: String,
    pub definition_id: String,
    pub source: String,
    pub rank: Option<String>,
    pub first_seen_period: Option<String>,
    pub last_seen_period: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateKpiRelevance {
    pub id: String,
    pub status: Option<String>,
    pub rank: Option<String>,
    pub first_seen_period: Option<String>,
    pub last_seen_period: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct NewFinancialFact {
    pub company_id: String,
    pub period_id: String,
    pub definition_id: String,
    pub value_numeric: String,
    pub currency: Option<String>,
    pub statement_basis: Option<String>,
    pub attribution: Option<String>,
    pub variant: Option<String>,
    pub measure_window: Option<String>,
    pub data_quality: Option<String>,
    pub as_reported_value: Option<String>,
    pub as_reported_scale: Option<String>,
    pub reporting_standard: Option<String>,
    pub extraction_method: Option<String>,
    pub confidence: Option<String>,
    pub confirmation_state: Option<String>,
    pub supersedes_id: Option<String>,
    pub source_document_ref: Option<String>,
    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFinancialFact {
    pub id: String,
    pub value_numeric: Option<String>,
    pub currency: Option<String>,
    pub data_quality: Option<String>,
    pub confirmation_state: Option<String>,
    pub supersedes_id: Option<String>,
    pub source_document_ref: Option<String>,
    /// `None` keeps the stored annotation; `Some("")` clears it; any other
    /// value replaces it (#156).
    pub annotation: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ListKpiDefinitionsInput {
    pub scope: Option<String>,
    pub sector: Option<String>,
    pub company_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ListFinancialPeriodsInput {
    pub company_id: String,
    pub fiscal_year: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/", optional_fields)
)]
#[serde(rename_all = "camelCase")]
pub struct ListFinancialFactsInput {
    pub company_id: Option<String>,
    pub period_id: Option<String>,
    pub definition_id: Option<String>,
}

// ============================================================================
// Public Storage Functions
// ============================================================================

pub(super) fn list_kpi_definitions(
    connection: &Connection,
    input: ListKpiDefinitionsInput,
) -> StorageResult<Vec<KpiDefinition>> {
    let scope = empty_string_to_none(input.scope);
    let sector = empty_string_to_none(input.sector);
    let company_id = empty_string_to_none(input.company_id);

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            scope,
            company_id,
            sector,
            metric_key,
            label,
            value_kind,
            unit,
            computation,
            formula,
            display_format,
            created_at,
            updated_at
        FROM kpi_definitions
        WHERE (?1 IS NULL OR scope = ?1)
            AND (?2 IS NULL OR sector = ?2)
            -- company_id with NO scope = the catalog this company can see:
            -- shared rows (company_id NULL: canonical/sector/user) PLUS its own
            -- company-scoped rows. Never another company s customs, and never a
            -- filter that hides the shared catalog (owner-dogfooding catch
            -- 2026-07-22: the fact matrix synthesized placeholder definitions).
            -- With an explicit scope the exact filter applies unchanged.
            AND (?3 IS NULL OR company_id = ?3 OR (?1 IS NULL AND company_id IS NULL))
        ORDER BY metric_key COLLATE NOCASE, label COLLATE NOCASE
        ",
    )?;

    let rows = statement.query_map(params![scope, sector, company_id], kpi_definition_from_row)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn create_kpi_definition(
    connection: &Connection,
    input: NewKpiDefinition,
) -> StorageResult<KpiDefinition> {
    let scope = input.scope.trim().to_owned();
    let company_id = empty_string_to_none(input.company_id.map(|s| s.trim().to_owned()));
    let sector = empty_string_to_none(input.sector.map(|s| s.trim().to_owned()));
    let metric_key = input.metric_key.trim().to_owned();
    let label = input.label.trim().to_owned();
    let value_kind = input.value_kind.trim().to_owned();
    let unit = empty_string_to_none(input.unit.map(|s| s.trim().to_owned()));
    let computation = input.computation.trim().to_owned();
    let formula = empty_string_to_none(input.formula.map(|s| s.trim().to_owned()));
    let display_format = empty_string_to_none(input.display_format.map(|s| s.trim().to_owned()));

    if metric_key.is_empty() {
        return Err(StorageError::InvalidFinancialsValue {
            key: "metric_key",
            value: metric_key,
        });
    }

    let id = kpi_definition_id(
        &scope,
        company_id.as_deref(),
        sector.as_deref(),
        &metric_key,
    );

    connection.execute(
        "
        INSERT INTO kpi_definitions (
            id,
            scope,
            company_id,
            sector,
            metric_key,
            label,
            value_kind,
            unit,
            computation,
            formula,
            display_format
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ",
        params![
            id,
            scope,
            company_id,
            sector,
            metric_key,
            label,
            value_kind,
            unit,
            computation,
            formula,
            display_format
        ],
    )?;

    get_kpi_definition(connection, &id)
}

pub(super) fn list_financial_periods(
    connection: &Connection,
    input: ListFinancialPeriodsInput,
) -> StorageResult<Vec<FinancialPeriod>> {
    let company_id = input.company_id.trim();

    validate_reference_exists(connection, "companies", company_id)?;

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            company_id,
            fiscal_year,
            period_type,
            period_end_date,
            report_evidence_ref,
            created_at,
            updated_at
        FROM financial_periods
        WHERE company_id = ?1
            AND (?2 IS NULL OR fiscal_year = ?2)
        ORDER BY fiscal_year DESC, period_type
        ",
    )?;

    let rows = statement.query_map(
        params![company_id, input.fiscal_year],
        financial_period_from_row,
    )?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Fold the one known out-of-spec fiscal label into the canonical spec set
/// (card f64cea2): 'annual' (any case) is the FY alias. Everything else is left
/// untouched — this is a targeted guardrail, not a whitelist that could reject
/// legitimate labels produced by the extraction pipeline.
pub(super) fn normalize_period_type_label(period_type: &str) -> String {
    if period_type.eq_ignore_ascii_case("annual") {
        "FY".to_owned()
    } else {
        period_type.to_owned()
    }
}

pub(super) fn create_financial_period(
    connection: &Connection,
    input: NewFinancialPeriod,
) -> StorageResult<FinancialPeriod> {
    let company_id = input.company_id.trim().to_owned();
    // Guardrail (card f64cea2): normalize the one known out-of-spec fiscal label
    // at the write boundary so a legacy 'annual' can never be reintroduced after
    // migration 0066 folds it into FY. period_type is a fiscal label (FY, H1, H2,
    // Q1..Q4, 9M, M01..M12); 'annual' is the FY alias.
    let period_type = normalize_period_type_label(input.period_type.trim());
    let period_end_date = empty_string_to_none(input.period_end_date.map(|s| s.trim().to_owned()));
    let report_evidence_ref =
        empty_string_to_none(input.report_evidence_ref.map(|s| s.trim().to_owned()));

    validate_reference_exists(connection, "companies", &company_id)?;

    if period_type.is_empty() {
        return Err(StorageError::InvalidFinancialsValue {
            key: "period_type",
            value: period_type,
        });
    }

    let id = financial_period_id(&company_id, input.fiscal_year, &period_type);

    connection.execute(
        "
        INSERT INTO financial_periods (
            id,
            company_id,
            fiscal_year,
            period_type,
            period_end_date,
            report_evidence_ref
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        ",
        params![
            id,
            company_id,
            input.fiscal_year,
            period_type,
            period_end_date,
            report_evidence_ref
        ],
    )?;

    get_financial_period(connection, &id)
}

pub(super) fn update_financial_period(
    connection: &Connection,
    input: UpdateFinancialPeriod,
) -> StorageResult<FinancialPeriod> {
    let id = input.id.trim().to_owned();
    let current = get_financial_period(connection, &id)?;

    let period_end_date = input
        .period_end_date
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.period_end_date);

    let report_evidence_ref = input
        .report_evidence_ref
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.report_evidence_ref);

    connection.execute(
        "
        UPDATE financial_periods
        SET period_end_date = ?2,
            report_evidence_ref = ?3,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id, period_end_date, report_evidence_ref],
    )?;

    get_financial_period(connection, &id)
}

pub(super) fn delete_financial_period(connection: &Connection, id: &str) -> StorageResult<()> {
    let id = id.trim();
    get_financial_period(connection, id)?;

    connection.execute("DELETE FROM financial_periods WHERE id = ?1", [id])?;

    Ok(())
}

pub(super) fn list_kpi_relevance(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<KpiRelevance>> {
    let company_id = company_id.trim();
    validate_reference_exists(connection, "companies", company_id)?;

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            company_id,
            definition_id,
            status,
            source,
            rank,
            first_seen_period,
            last_seen_period,
            created_at,
            updated_at
        FROM kpi_relevance
        WHERE company_id = ?1
        ORDER BY status DESC, rank
        ",
    )?;

    let rows = statement.query_map([company_id], kpi_relevance_from_row)?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// The IFRS core KPI set migration 0106 seeded — the starting denominator for
/// the completeness check (`expected_primary_metric_keys`). Defensible for any
/// IFRS reporter; the durable per-sector/per-company selection is studied
/// separately (issue #81). Keep in lockstep with
/// `migrations/0106_seed_core_kpi_relevance.sql`.
pub(super) const CORE_KPI_METRIC_KEYS: [&str; 5] = [
    "revenue",
    "operating_profit",
    "net_profit",
    "total_assets",
    "total_equity",
];

/// Seed the 0106 core `kpi_relevance` set for ONE company — the create-time
/// half of the fix for issue #203's residual hole: migration 0106 seeded the
/// companies that existed when it applied, so every company created afterwards
/// had an empty denominator and the completeness check silently never fired
/// for it.
///
/// Deliberately identical in semantics to 0106 (the migration is the
/// backfill, this is the forward path — one shape, one behaviour):
///
/// * deterministic `kpirel_core_<company>_<metric_key>` ids, so re-seeding
///   converges instead of accumulating;
/// * `INSERT OR IGNORE` against `UNIQUE(company_id, definition_id)` plus the
///   `NOT EXISTS` guard, so a curated (`user`/`agent`/`sector`) row for the
///   same metric is never overwritten, re-ranked or duplicated;
/// * a missing canonical definition seeds nothing for that metric rather than
///   failing.
pub(super) fn seed_core_kpi_relevance(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<usize> {
    let placeholders = CORE_KPI_METRIC_KEYS
        .iter()
        .enumerate()
        .map(|(index, _)| format!("?{}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        INSERT OR IGNORE INTO kpi_relevance
            (id, company_id, definition_id, status, source, rank)
        SELECT
            'kpirel_core_' || c.id || '_' || d.metric_key,
            c.id,
            d.id,
            'active',
            'core',
            'primary'
        FROM companies c
        JOIN kpi_definitions d
          ON d.scope = 'canonical'
         AND d.metric_key IN ({placeholders})
        WHERE c.id = ?1
          AND NOT EXISTS (
              SELECT 1
              FROM kpi_relevance existing
              WHERE existing.company_id = c.id
                AND existing.definition_id = d.id
          )
        "
    );

    let mut parameters: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(6);
    parameters.push(&company_id);
    for key in CORE_KPI_METRIC_KEYS.iter() {
        parameters.push(key);
    }

    connection
        .execute(&sql, parameters.as_slice())
        .map_err(StorageError::from)
}

/// Layer 2 of [ADR 0092](../../../docs/adr/0092-kpi-relevance-lifecycle.md):
/// the statement-pack additions, keyed off `companies.statement_type` over the
/// `scope='sector'` packs migration 0034 seeded and nothing ever read.
///
/// **Conservative subset by construction** — the ADR asks for keys that are
/// genuinely universal within the statement type, not everything a pack lists,
/// because a key nobody reports inflates the recall denominator without making
/// the completeness gate any smarter. Each pick, and each deliberate omission:
///
/// * **banking** (pack has 12): `net_interest_income`, `net_fee_commission_income`,
///   `total_loans`, `total_deposits` — the four primary-statement lines every
///   bank's periodic report carries. *Omitted:* `operating_income` /
///   `operating_expenses` (aggregates whose composition varies by presentation),
///   and `nim` / `cost_income_ratio` / `npl_ratio` / `cost_of_risk` / `cet1` /
///   `tcr` (ratios and capital measures that live in the management commentary
///   or the notes, not the statements).
/// * **insurance** (pack has 7): `gross_insurance_revenue` only — the IFRS 17
///   top line, mandatory for every EU insurer since 2023. *Omitted:*
///   `gross_written_premium` and `net_earned_premium` (pre-IFRS-17 concepts,
///   now supplementary), `claims_ratio` / `combined_ratio` (non-life only),
///   `technical_result` / `investment_result` (presentation varies).
/// * **reit** (pack has 7): `ffo` only — the NAREIT-standard headline.
///   *Omitted:* the rest are property-type specific (`occupancy`,
///   `same_store_noi`, `walt`) or derived (`affo_payout_ratio`).
/// * **specialty_finance**: **nothing**. The pack (`recoveries`, `erc`,
///   `cash_ebitda`, `portfolio_purchases`) is debt-collector vocabulary, but
///   migration 0095 also maps exchanges and brokerage houses onto this same
///   `statement_type` (GPW and XTB on the owner's database, next to KRU). No
///   key is universal across that mix, and the ADR's rule is to leave out when
///   unsure. Splitting the type is a separate decision.
///
/// Keys are globally unique across the packs, so this flat allow-list plus the
/// `d.sector = c.statement_type` join selects exactly the company's own pack.
pub(super) const STATEMENT_PACK_METRIC_KEYS: [&str; 6] = [
    // banking
    "net_interest_income",
    "net_fee_commission_income",
    "total_loans",
    "total_deposits",
    // insurance
    "gross_insurance_revenue",
    // reit
    "ffo",
];

/// Seed the statement-pack `kpi_relevance` additions for ONE company.
///
/// Same semantics as [`seed_core_kpi_relevance`], one layer up: deterministic
/// `kpirel_sector_<company>_<metric_key>` ids, `INSERT OR IGNORE` against
/// `UNIQUE(company_id, definition_id)` plus the `NOT EXISTS` guard, so a core or
/// curated row for the same metric is never overwritten or duplicated.
///
/// **Additive on reclassification** (ADR 0092 layer 2): a `statement_type`
/// change re-seeds — it never deletes the previous type's rows. Automation
/// widens a company's expectations and leaves narrowing to the user.
pub(super) fn seed_statement_pack_kpi_relevance(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<usize> {
    let placeholders = STATEMENT_PACK_METRIC_KEYS
        .iter()
        .enumerate()
        .map(|(index, _)| format!("?{}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "
        INSERT OR IGNORE INTO kpi_relevance
            (id, company_id, definition_id, status, source, rank)
        SELECT
            'kpirel_sector_' || c.id || '_' || d.metric_key,
            c.id,
            d.id,
            'active',
            'sector',
            'primary'
        FROM companies c
        JOIN kpi_definitions d
          ON d.scope = 'sector'
         AND d.sector = c.statement_type
         AND d.metric_key IN ({placeholders})
        WHERE c.id = ?1
          AND NOT EXISTS (
              SELECT 1
              FROM kpi_relevance existing
              WHERE existing.company_id = c.id
                AND existing.definition_id = d.id
          )
        "
    );

    let mut parameters: Vec<&dyn rusqlite::ToSql> = Vec::with_capacity(7);
    parameters.push(&company_id);
    for key in STATEMENT_PACK_METRIC_KEYS.iter() {
        parameters.push(key);
    }

    connection
        .execute(&sql, parameters.as_slice())
        .map_err(StorageError::from)
}

/// `kpi_relevance.source` for a layer-3 observation. Named once because two
/// places must agree forever: the pass that writes it and the completeness
/// gate that structurally refuses it.
pub(super) const DERIVED_RELEVANCE_SOURCE: &str = "derived";

/// How many of a company's most recent periods the derived pass looks at, and
/// how many of them must carry a key for it to count as consistently reported
/// (ADR 0092 layer 3: "issuer-tier facts in ≥3 of the last 4 periods").
const DERIVED_OBSERVATION_WINDOW: i64 = 4;
const DERIVED_OBSERVATION_MIN_PERIODS: i64 = 3;

/// Layer 3 of [ADR 0092](../../../docs/adr/0092-kpi-relevance-lifecycle.md):
/// mark the keys a company **consistently reports** as `source='derived'`,
/// `rank='secondary'` — enrichment for the company-characteristic KPI surface
/// and the coverage display.
///
/// **These rows never gate.** [`expected_primary_metric_keys`] excludes
/// `source='derived'` structurally, not by relying on the rank. That exclusion
/// is the whole reason this layer is allowed to exist: the completeness gate
/// compares extraction output against expectations, so an expectation derived
/// FROM extraction output would let a systematic extraction hole (a parser that
/// never yields equity) silently erase the very expectation that would have
/// caught it.
///
/// Additive and conservative, like every other automatic layer:
/// * only **issuer-tier** facts count — a fact with no provenance row (manual)
///   or one stamped `html_aggregator` is third-party or hand-entered, never
///   evidence that the ISSUER reports the key;
/// * `INSERT OR IGNORE` + `NOT EXISTS`, so a `core`/`sector`/`user` row for the
///   same metric is left exactly as it is;
/// * a key that STOPS being reported is **not** deleted — staleness is a
///   display concern, and deleting would fight the user's curation.
pub(super) fn refresh_derived_kpi_relevance(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<usize> {
    connection
        .execute(
            "
        INSERT OR IGNORE INTO kpi_relevance
            (id, company_id, definition_id, status, source, rank)
        SELECT
            'kpirel_derived_' || ?1 || '_' || d.metric_key,
            ?1,
            f.definition_id,
            'active',
            'derived',
            'secondary'
        FROM financial_facts f
        JOIN kpi_definitions d
          ON d.id = f.definition_id
        JOIN financial_fact_provenance p
          ON p.fact_id = f.id
        WHERE f.company_id = ?1
          -- Issuer tiers only. `is_issuer()` in fundamentals::extraction is the
          -- Rust twin of this predicate: everything but the aggregator.
          AND p.source_tier <> 'html_aggregator'
          AND f.period_id IN (
              SELECT id
              FROM financial_periods
              WHERE company_id = ?1
              ORDER BY IFNULL(period_end_date, fiscal_year || '-12-31') DESC,
                       fiscal_year DESC
              LIMIT ?2
          )
          AND NOT EXISTS (
              SELECT 1
              FROM kpi_relevance existing
              WHERE existing.company_id = ?1
                AND existing.definition_id = f.definition_id
          )
        GROUP BY f.definition_id, d.metric_key
        HAVING COUNT(DISTINCT f.period_id) >= ?3
        ",
            params![
                company_id,
                DERIVED_OBSERVATION_WINDOW,
                DERIVED_OBSERVATION_MIN_PERIODS
            ],
        )
        .map_err(StorageError::from)
}

pub(super) fn create_kpi_relevance(
    connection: &Connection,
    input: NewKpiRelevance,
) -> StorageResult<KpiRelevance> {
    let company_id = input.company_id.trim().to_owned();
    let definition_id = input.definition_id.trim().to_owned();
    let source = input.source.trim().to_owned();
    let rank = empty_string_to_none(input.rank.map(|s| s.trim().to_owned()));
    let first_seen_period =
        empty_string_to_none(input.first_seen_period.map(|s| s.trim().to_owned()));
    let last_seen_period =
        empty_string_to_none(input.last_seen_period.map(|s| s.trim().to_owned()));

    validate_reference_exists(connection, "companies", &company_id)?;
    validate_reference_exists(connection, "kpi_definitions", &definition_id)?;

    let id = kpi_relevance_id(&company_id, &definition_id);

    // Curating a KPI that is ALREADY relevant re-states the profile, it does not
    // fail: since creation seeds the core set (issue #203), the five core
    // metrics are pre-occupied on every company, and without this upsert the
    // `create_kpi_relevance` command/act would hard-error on exactly the metrics
    // an investor curates most. The seed never overwrites curation; curation
    // always overwrites the seed.
    connection.execute(
        "
        INSERT INTO kpi_relevance (
            id,
            company_id,
            definition_id,
            status,
            source,
            rank,
            first_seen_period,
            last_seen_period
        ) VALUES (?1, ?2, ?3, 'active', ?4, ?5, ?6, ?7)
        ON CONFLICT(company_id, definition_id) DO UPDATE SET
            status = 'active',
            source = excluded.source,
            rank = COALESCE(excluded.rank, kpi_relevance.rank),
            first_seen_period =
                COALESCE(excluded.first_seen_period, kpi_relevance.first_seen_period),
            last_seen_period =
                COALESCE(excluded.last_seen_period, kpi_relevance.last_seen_period),
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        ",
        params![
            id,
            company_id,
            definition_id,
            source,
            rank,
            first_seen_period,
            last_seen_period
        ],
    )?;

    // Read back by the natural key, not `id`: on conflict the surviving row is
    // the pre-existing one, whose id may be the seed's (`kpirel_core_*`).
    let stored_id: String = connection.query_row(
        "SELECT id FROM kpi_relevance WHERE company_id = ?1 AND definition_id = ?2",
        params![company_id, definition_id],
        |row| row.get(0),
    )?;

    get_kpi_relevance(connection, &stored_id)
}

pub(super) fn update_kpi_relevance(
    connection: &Connection,
    input: UpdateKpiRelevance,
) -> StorageResult<KpiRelevance> {
    let id = input.id.trim().to_owned();
    let current = get_kpi_relevance(connection, &id)?;

    let status = input
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&current.status)
        .to_owned();

    let rank = input
        .rank
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.rank);

    let first_seen_period = input
        .first_seen_period
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.first_seen_period);

    let last_seen_period = input
        .last_seen_period
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.last_seen_period);

    connection.execute(
        "
        UPDATE kpi_relevance
        SET status = ?2,
            rank = ?3,
            first_seen_period = ?4,
            last_seen_period = ?5,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![id, status, rank, first_seen_period, last_seen_period],
    )?;

    get_kpi_relevance(connection, &id)
}

pub(super) fn delete_kpi_relevance(connection: &Connection, id: &str) -> StorageResult<()> {
    let id = id.trim();
    get_kpi_relevance(connection, id)?;

    connection.execute("DELETE FROM kpi_relevance WHERE id = ?1", [id])?;

    Ok(())
}

pub(super) fn list_financial_facts(
    connection: &Connection,
    input: ListFinancialFactsInput,
) -> StorageResult<Vec<FinancialFact>> {
    let company_id = empty_string_to_none(input.company_id.map(|s| s.trim().to_owned()));
    let period_id = empty_string_to_none(input.period_id.map(|s| s.trim().to_owned()));
    let definition_id = empty_string_to_none(input.definition_id.map(|s| s.trim().to_owned()));

    let mut statement = connection.prepare(
        "
        SELECT
            id,
            company_id,
            period_id,
            definition_id,
            value_numeric,
            currency,
            statement_basis,
            attribution,
            variant,
            measure_window,
            data_quality,
            as_reported_value,
            as_reported_scale,
            reporting_standard,
            extraction_method,
            confidence,
            confirmation_state,
            supersedes_id,
            source_document_ref,
            annotation,
            created_at,
            updated_at
        FROM financial_facts
        WHERE (?1 IS NULL OR company_id = ?1)
            AND (?2 IS NULL OR period_id = ?2)
            AND (?3 IS NULL OR definition_id = ?3)
        ORDER BY datetime(created_at) DESC, id
        ",
    )?;

    let rows = statement.query_map(
        params![company_id, period_id, definition_id],
        financial_fact_from_row,
    )?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Reads the previously-stored fact set for one `(company, fiscal_year,
/// period_type)`, bridging each fact's `definition_id` to its `metric_key` via
/// the KPI definition catalog (`kpi_definitions.id` is derived solely from
/// `metric_key`, so it is a stable 1:1 map regardless of scope). This is the
/// structured-extraction pipeline's comparative cross-check input (ADR 0061
/// dec. 4b) — the "already known" prior period the freshly-extracted
/// comparative column is checked against.
///
/// Returns `Ok(None)` when no period matches, or the period has no facts (a
/// fresh company/period — not an error). Facts whose `value_numeric` doesn't
/// parse as a decimal are skipped rather than failing the whole read.
/// The company's expected primary-KPI `metric_key`s (ADR 0061 dec. 4d): every
/// `kpi_relevance` row that is `active` and ranked `primary` (case-insensitively),
/// bridged to its `metric_key` via the definition catalog. `None` when there are
/// none (nothing to check completeness against) — never blocks emission by itself.
///
/// Lives here, at the connection level, so BOTH the document pipeline
/// (`jobs::structured_extraction`) and the ingest-time ESPI cover-note tier feed
/// the completeness gate from one query.
pub(super) fn expected_primary_metric_keys(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Option<std::collections::BTreeSet<String>>> {
    use std::collections::BTreeSet;

    let relevance = list_kpi_relevance(connection, company_id)?;
    let primary_definition_ids: BTreeSet<String> = relevance
        .into_iter()
        .filter(|r| {
            r.status == "active"
                && r.rank
                    .as_deref()
                    .is_some_and(|rank| rank.eq_ignore_ascii_case("primary"))
                // ADR 0092's no-self-referential-gate rule, enforced
                // STRUCTURALLY and not left to the rank the layer-3 pass
                // happens to write: this gate compares extraction output
                // against expectations, so an expectation derived FROM
                // extraction output would let a systematic extraction hole
                // silently erase the very expectation that would catch it.
                // Derived rows enrich the KPI surface; they never gate — even
                // if something (a user, a future job) ranks one `primary`.
                && !r.source.eq_ignore_ascii_case(DERIVED_RELEVANCE_SOURCE)
        })
        .map(|r| r.definition_id)
        .collect();
    if primary_definition_ids.is_empty() {
        return Ok(None);
    }

    let definitions = list_kpi_definitions(
        connection,
        ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        },
    )?;
    let keys: BTreeSet<String> = definitions
        .into_iter()
        .filter(|d| primary_definition_ids.contains(&d.id))
        .map(|d| d.metric_key)
        .collect();
    Ok(if keys.is_empty() { None } else { Some(keys) })
}

/// The stored fact set for one `(company, fiscal_year, period_type)`, bridged to
/// `metric_key`s — the ONE general body [`stored_fact_set`] and
/// [`stored_fact_set_for_cross_check`] share. `veto_filter` is the only knob:
/// - `None`: every stored fact (the plain comparative prior — [`stored_fact_set`]).
/// - `Some(incoming_tier)`: only facts an `incoming_tier` extraction may be VETOED
///   by (ADR 0086 dec. 3/4) — facts with no provenance row (manual, top of the
///   ladder) and facts whose provenance tier the incoming tier does NOT outrank. A
///   strictly-LOWER-tier prior (e.g. the daily BiznesRadar pull) is excluded so a
///   third-party number never fails an issuer filing's comparative cross-check and
///   discards the whole set.
///
/// The tier lookup is ONE provenance query for the whole period (not a per-fact
/// SELECT — the N+1 this consolidation removed, hot in the rebuild's pass-2 over
/// ~250 docs).
fn stored_fact_set_filtered(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    veto_filter: Option<crate::fundamentals::extraction::SourceTier>,
) -> StorageResult<Option<FactSet>> {
    use crate::fundamentals::extraction::SourceTier;

    let periods = list_financial_periods(
        connection,
        ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: Some(fiscal_year),
        },
    )?;
    let Some(period) = periods
        .into_iter()
        .find(|p| p.period_type.eq_ignore_ascii_case(period_type))
    else {
        return Ok(None);
    };

    let facts = list_financial_facts(
        connection,
        ListFinancialFactsInput {
            company_id: None,
            period_id: Some(period.id.clone()),
            definition_id: None,
        },
    )?;
    if facts.is_empty() {
        return Ok(None);
    }

    // The map is read out of the CATALOG (not derived from ids), so it stays
    // correct now that non-canonical ids carry a scope discriminator
    // (`kpi_definition_id`): listing with no scope filter returns every row,
    // whichever scope produced the definition a fact references.
    let definitions = list_kpi_definitions(
        connection,
        ListKpiDefinitionsInput {
            scope: None,
            sector: None,
            company_id: None,
        },
    )?;
    let metric_key_by_definition: HashMap<String, String> = definitions
        .into_iter()
        .map(|d| (d.id, d.metric_key))
        .collect();

    // One provenance query for the whole period, only when a veto filter is
    // active — a fact absent from this map has no provenance row (a manual entry).
    let tier_by_fact: HashMap<String, String> = if veto_filter.is_some() {
        fact_tiers_for_period(connection, &period.id)?
    } else {
        HashMap::new()
    };

    let mut set = FactSet::new();
    for fact in facts {
        if let Some(incoming_tier) = veto_filter {
            // A fact with no provenance row is a manual entry — always
            // veto-capable. A provenanced fact is veto-capable only when the
            // incoming tier does not outrank it; an unparsable stored tier is
            // treated as veto-capable (never silently discounted).
            let veto_capable = match tier_by_fact.get(&fact.id) {
                None => true,
                Some(stored) => match SourceTier::parse(stored) {
                    Some(stored_tier) => !incoming_tier.outranks(stored_tier),
                    None => true,
                },
            };
            if !veto_capable {
                continue;
            }
        }
        let Some(metric_key) = metric_key_by_definition.get(&fact.definition_id) else {
            continue;
        };
        let Ok(value) = Decimal::from_str(fact.value_numeric.trim()) else {
            continue;
        };
        set.insert(metric_key.clone(), value);
    }

    if set.is_empty() {
        Ok(None)
    } else {
        Ok(Some(set))
    }
}

/// `fact_id → source_tier` for every provenanced fact in one period, in a single
/// JOIN — the batched replacement for the per-fact `fact_source_tier` SELECT.
fn fact_tiers_for_period(
    connection: &Connection,
    period_id: &str,
) -> StorageResult<HashMap<String, String>> {
    let mut statement = connection.prepare(
        "SELECT p.fact_id, p.source_tier
         FROM financial_fact_provenance p
         JOIN financial_facts f ON f.id = p.fact_id
         WHERE f.period_id = ?1",
    )?;
    let rows = statement.query_map([period_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<rusqlite::Result<HashMap<_, _>>>()
        .map_err(StorageError::from)
}

/// The plain comparative prior — every stored fact for the period, bridged to
/// `metric_key`s (ADR 0061 dec. 4b). This is the cross-check-UNAWARE variant: a
/// future author wanting the reversed-witnessing veto semantics wants
/// [`stored_fact_set_for_cross_check`] instead. Only test harnesses read this
/// unfiltered form today (production callers go through the cross-check variant).
pub(super) fn stored_fact_set(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
) -> StorageResult<Option<FactSet>> {
    stored_fact_set_filtered(connection, company_id, fiscal_year, period_type, None)
}

/// [`stored_fact_set`] restricted to facts an `incoming_tier` extraction may be
/// VETOED by (ADR 0086 decisions 3/4). See [`stored_fact_set_filtered`] for the
/// veto semantics.
pub(super) fn stored_fact_set_for_cross_check(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
    incoming_tier: crate::fundamentals::extraction::SourceTier,
) -> StorageResult<Option<FactSet>> {
    stored_fact_set_filtered(
        connection,
        company_id,
        fiscal_year,
        period_type,
        Some(incoming_tier),
    )
}

/// All stored values of `metric_key` for `company_id` across every period OTHER
/// than `(exclude_fiscal_year, exclude_period_type)` — the history the runtime
/// plausibility gate ([`crate::fundamentals::validation::implausible_against_history`])
/// measures an about-to-persist fact against. The current period is excluded so a
/// re-extraction of a landed slot never checks a value against a stale copy of
/// itself. CONFIRMED values are INCLUDED: a user- or witness-trusted figure is
/// the trust anchor of the history, never a contaminant. Read-only. Bridges
/// `metric_key` to its definition id the same 1:1 way [`stored_fact_set`] does;
/// values that don't parse as decimals are skipped rather than failing the read.
pub(super) fn metric_history(
    connection: &Connection,
    company_id: &str,
    metric_key: &str,
    exclude_fiscal_year: i64,
    exclude_period_type: &str,
) -> StorageResult<Vec<Decimal>> {
    use std::collections::HashSet;

    let periods = list_financial_periods(
        connection,
        ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        },
    )?;
    let excluded: HashSet<String> = periods
        .iter()
        .filter(|p| {
            p.fiscal_year == exclude_fiscal_year
                && p.period_type.eq_ignore_ascii_case(exclude_period_type)
        })
        .map(|p| p.id.clone())
        .collect();

    let facts = list_financial_facts(
        connection,
        ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            // Deliberately the CANONICAL id: a company-scoped measure that
            // merely shares the key is a different measure (ADR 0077 d.8
            // no-repaint rule) and must not enter the canonical history.
            definition_id: Some(canonical_kpi_definition_id(metric_key)),
        },
    )?;

    let mut values = Vec::new();
    for fact in facts {
        if excluded.contains(&fact.period_id) {
            continue;
        }
        if let Ok(value) = Decimal::from_str(fact.value_numeric.trim()) {
            values.push(value);
        }
    }
    Ok(values)
}

/// The stored history of MANY metrics in a single read — the batched form of
/// [`metric_history`], for the per-fact history-plausibility gate that would
/// otherwise call `metric_history` once per fact (≈20 facts/document → 20
/// connection checkouts and 20 identical, loop-invariant period scans). The
/// period list and the whole-company fact scan are each done ONCE here and
/// grouped in memory.
///
/// Equivalence contract (the refactor's oracle): `result[k]` is byte-identical
/// to what `metric_history(company_id, k, exclude_fiscal_year,
/// exclude_period_type)` would return — same values, same order (the company
/// fact scan preserves the `created_at DESC, id` order each single read sees, and
/// filtering to one definition preserves that relative order). Every requested
/// key is present; a key with no history maps to an empty vector, exactly as the
/// single read returns `[]`.
pub(super) fn metric_histories(
    connection: &Connection,
    company_id: &str,
    metric_keys: &std::collections::BTreeSet<String>,
    exclude_fiscal_year: i64,
    exclude_period_type: &str,
) -> StorageResult<std::collections::HashMap<String, Vec<Decimal>>> {
    use std::collections::{HashMap, HashSet};

    // Pre-seed every requested key so a metric with no stored facts still maps to
    // an empty vector (parity with the single read's `Ok(vec![])`).
    let mut result: HashMap<String, Vec<Decimal>> = metric_keys
        .iter()
        .map(|k| (k.clone(), Vec::new()))
        .collect();
    if metric_keys.is_empty() {
        return Ok(result);
    }

    // Loop-invariant across all metrics: the company's periods, read ONCE, and the
    // excluded-period set (the very period being extracted) computed ONCE — the
    // waste `metric_history` repeated per fact.
    let periods = list_financial_periods(
        connection,
        ListFinancialPeriodsInput {
            company_id: company_id.to_owned(),
            fiscal_year: None,
        },
    )?;
    let excluded: HashSet<String> = periods
        .iter()
        .filter(|p| {
            p.fiscal_year == exclude_fiscal_year
                && p.period_type.eq_ignore_ascii_case(exclude_period_type)
        })
        .map(|p| p.id.clone())
        .collect();

    // `canonical_kpi_definition_id` is a 1:1 function of `metric_key`, so this
    // reverse map is unambiguous — the group a scanned fact belongs to. Only
    // canonical ids are mapped, keeping this the exact batched equivalent of
    // [`metric_history`] (which reads the canonical definition too).
    let def_to_metric: HashMap<String, String> = metric_keys
        .iter()
        .map(|k| (canonical_kpi_definition_id(k), k.clone()))
        .collect();

    // One scan for the whole company (definition_id = None), grouped by the
    // requested definitions — replaces N per-definition scans.
    let facts = list_financial_facts(
        connection,
        ListFinancialFactsInput {
            company_id: Some(company_id.to_owned()),
            period_id: None,
            definition_id: None,
        },
    )?;
    for fact in facts {
        let Some(metric_key) = def_to_metric.get(&fact.definition_id) else {
            continue;
        };
        if excluded.contains(&fact.period_id) {
            continue;
        }
        if let Ok(value) = Decimal::from_str(fact.value_numeric.trim()) {
            result
                .get_mut(metric_key)
                .expect("every requested key is pre-seeded")
                .push(value);
        }
    }
    Ok(result)
}

/// A cached per-document reporting-period derivation (migration 0109). `None`
/// period columns with `has_period = false` is the explicit none-marker — a
/// document whose content yields no derivable period, recorded so it is not
/// re-parsed on the next read.
pub struct CachedDerivedPeriod {
    pub has_period: bool,
    pub fiscal_year: Option<i64>,
    pub period_type: Option<String>,
    pub period_end: Option<String>,
    pub derivation_version: i64,
}

/// The five slot-dimension columns of a fact with per-column defaults applied —
/// the single source of truth for the uniqueness slot `(period_id, definition_id,
/// statement_basis, attribution, variant, measure_window, data_quality)`, shared
/// by the INSERT and the re-observation lookup so a re-extraction can never miss
/// (or spuriously match) an existing row through a defaulting mismatch.
pub(super) struct SlotDims {
    pub statement_basis: String,
    pub attribution: String,
    pub variant: String,
    pub measure_window: String,
    pub data_quality: String,
}

pub(super) fn slot_dims(input: &NewFinancialFact) -> SlotDims {
    fn or_default(value: &Option<String>, fallback: &str) -> String {
        value
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }
    SlotDims {
        statement_basis: or_default(&input.statement_basis, "consolidated"),
        attribution: or_default(&input.attribution, "total"),
        variant: or_default(&input.variant, "reported"),
        measure_window: or_default(&input.measure_window, "flow"),
        data_quality: or_default(&input.data_quality, "final"),
    }
}

/// The already-committed fact occupying a fully-qualified slot, if any — the
/// lookup a re-observation is decided against.
fn find_fact_by_slot(
    connection: &Connection,
    period_id: &str,
    definition_id: &str,
    dims: &SlotDims,
) -> StorageResult<Option<FinancialFact>> {
    let id: Option<String> = connection
        .query_row(
            "
            SELECT id FROM financial_facts
            WHERE period_id = ?1 AND definition_id = ?2 AND statement_basis = ?3
              AND attribution = ?4 AND variant = ?5 AND measure_window = ?6
              AND data_quality = ?7
            ",
            params![
                period_id,
                definition_id,
                dims.statement_basis,
                dims.attribution,
                dims.variant,
                dims.measure_window,
                dims.data_quality,
            ],
            |row| row.get(0),
        )
        .optional()?;
    match id {
        Some(id) => Ok(Some(get_financial_fact(connection, &id)?)),
        None => Ok(None),
    }
}

/// The outcome of a slot-aware fact write (re-observation semantics for the
/// structured pipeline): a brand-new fact was created, or an existing row already
/// occupies the slot — the incoming value either matches it (`Reobserved`) or
/// disagrees (`Divergent`). The uniqueness slot admits exactly one row, so a
/// re-extraction of an already-landed period is idempotent, never a UNIQUE
/// violation, and a confirmed fact is never silently overwritten.
pub(super) enum FactWriteOutcome {
    Created(FinancialFact),
    Reobserved(FinancialFact),
    Divergent {
        existing: FinancialFact,
        incoming: String,
    },
}

/// Create a fact, or — when its slot is already occupied — classify the
/// re-observation instead of raising the slot's UNIQUE violation. Values are
/// compared decimal-exact (so `25000` and `25000.0` are the same observation),
/// falling back to a trimmed string compare only when either side is unparseable.
/// Normalizes a fact currency at the write boundary: trimmed, empty → absent,
/// otherwise exactly three ASCII letters upper-cased into the ISO-4217 shape the
/// read side (comparison, valuation, FX) assumes.
///
/// #93: the ESEF divide-unit bug resolved every EPS unit to its `xbrli:shares`
/// denominator and 76 facts were stored with `currency = 'shares'`. The parser
/// is fixed, but the class only becomes impossible once the store refuses to
/// persist a non-currency unit — from ANY writer (extraction jobs, MCP, UI).
fn normalize_currency(currency: Option<String>) -> StorageResult<Option<String>> {
    let Some(currency) = empty_string_to_none(currency.map(|s| s.trim().to_owned())) else {
        return Ok(None);
    };
    if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
        return Err(StorageError::InvalidFinancialsValue {
            key: "currency",
            value: currency,
        });
    }
    Ok(Some(currency.to_ascii_uppercase()))
}

pub(super) fn create_or_reobserve_financial_fact(
    connection: &Connection,
    input: NewFinancialFact,
) -> StorageResult<FactWriteOutcome> {
    let period_id = input.period_id.trim().to_owned();
    let definition_id = input.definition_id.trim().to_owned();
    let dims = slot_dims(&input);
    if let Some(existing) = find_fact_by_slot(connection, &period_id, &definition_id, &dims)? {
        let incoming = input.value_numeric.trim().to_owned();
        let same = match (
            Decimal::from_str(existing.value_numeric.trim()),
            Decimal::from_str(incoming.trim()),
        ) {
            (Ok(a), Ok(b)) => a == b,
            _ => existing.value_numeric.trim() == incoming.trim(),
        };
        return Ok(if same {
            FactWriteOutcome::Reobserved(existing)
        } else {
            FactWriteOutcome::Divergent { existing, incoming }
        });
    }
    let created = create_financial_fact(connection, input)?;
    Ok(FactWriteOutcome::Created(created))
}

pub(super) fn create_financial_fact(
    connection: &Connection,
    input: NewFinancialFact,
) -> StorageResult<FinancialFact> {
    let company_id = input.company_id.trim().to_owned();
    let period_id = input.period_id.trim().to_owned();
    let definition_id = input.definition_id.trim().to_owned();
    let value_numeric = input.value_numeric.trim().to_owned();
    let SlotDims {
        statement_basis,
        attribution,
        variant,
        measure_window,
        data_quality,
    } = slot_dims(&input);
    let currency = normalize_currency(input.currency)?;
    let as_reported_value =
        empty_string_to_none(input.as_reported_value.map(|s| s.trim().to_owned()));
    let as_reported_scale =
        empty_string_to_none(input.as_reported_scale.map(|s| s.trim().to_owned()));
    let reporting_standard =
        empty_string_to_none(input.reporting_standard.map(|s| s.trim().to_owned()));
    let extraction_method = input
        .extraction_method
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("manual")
        .to_owned();
    let confidence = empty_string_to_none(input.confidence.map(|s| s.trim().to_owned()));
    let confirmation_state = input
        .confirmation_state
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("confirmed")
        .to_owned();
    let supersedes_id = empty_string_to_none(input.supersedes_id.map(|s| s.trim().to_owned()));
    let source_document_ref =
        empty_string_to_none(input.source_document_ref.map(|s| s.trim().to_owned()));
    let annotation = empty_string_to_none(input.annotation.map(|s| s.trim().to_owned()));

    validate_reference_exists(connection, "companies", &company_id)?;
    validate_reference_exists(connection, "financial_periods", &period_id)?;
    validate_reference_exists(connection, "kpi_definitions", &definition_id)?;

    if value_numeric.is_empty() {
        return Err(StorageError::InvalidFinancialsValue {
            key: "value_numeric",
            value: value_numeric,
        });
    }

    let id = financial_fact_id(
        &period_id,
        &definition_id,
        &statement_basis,
        &attribution,
        &variant,
        &measure_window,
        &data_quality,
    );

    connection.execute(
        "
        INSERT INTO financial_facts (
            id,
            company_id,
            period_id,
            definition_id,
            value_numeric,
            currency,
            statement_basis,
            attribution,
            variant,
            measure_window,
            data_quality,
            as_reported_value,
            as_reported_scale,
            reporting_standard,
            extraction_method,
            confidence,
            confirmation_state,
            supersedes_id,
            source_document_ref,
            annotation
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
        ",
        params![
            id,
            company_id,
            period_id,
            definition_id,
            value_numeric,
            currency,
            statement_basis,
            attribution,
            variant,
            measure_window,
            data_quality,
            as_reported_value,
            as_reported_scale,
            reporting_standard,
            extraction_method,
            confidence,
            confirmation_state,
            supersedes_id,
            source_document_ref,
            annotation
        ],
    )?;

    get_financial_fact(connection, &id)
}

pub(super) fn update_financial_fact(
    connection: &Connection,
    input: UpdateFinancialFact,
) -> StorageResult<FinancialFact> {
    let id = input.id.trim().to_owned();
    let current = get_financial_fact(connection, &id)?;

    let value_numeric = input
        .value_numeric
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&current.value_numeric)
        .to_owned();

    // An absent/empty input keeps the stored currency; whatever ends up written
    // goes through the same ISO-4217 guard as a create (#93).
    let currency = normalize_currency(
        input
            .currency
            .as_deref()
            .map(str::trim)
            .and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    Some(s.to_owned())
                }
            })
            .or(current.currency),
    )?;

    let data_quality = input
        .data_quality
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&current.data_quality)
        .to_owned();

    let confirmation_state = input
        .confirmation_state
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&current.confirmation_state)
        .to_owned();

    let supersedes_id = input
        .supersedes_id
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.supersedes_id);

    let source_document_ref = input
        .source_document_ref
        .as_deref()
        .map(str::trim)
        .and_then(|s| {
            if s.is_empty() {
                None
            } else {
                Some(s.to_owned())
            }
        })
        .or(current.source_document_ref);

    // Annotation (#156): absent keeps the stored note, an empty string clears
    // it (the user removed the marker), anything else replaces it.
    let annotation = match input.annotation.as_deref().map(str::trim) {
        None => current.annotation,
        Some("") => None,
        Some(text) => Some(text.to_owned()),
    };

    connection.execute(
        "
        UPDATE financial_facts
        SET value_numeric = ?2,
            currency = ?3,
            data_quality = ?4,
            confirmation_state = ?5,
            supersedes_id = ?6,
            source_document_ref = ?7,
            annotation = ?8,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
        WHERE id = ?1
        ",
        params![
            id,
            value_numeric,
            currency,
            data_quality,
            confirmation_state,
            supersedes_id,
            source_document_ref,
            annotation
        ],
    )?;

    get_financial_fact(connection, &id)
}

pub(super) fn delete_financial_fact(connection: &Connection, id: &str) -> StorageResult<()> {
    let id = id.trim();
    get_financial_fact(connection, id)?;

    connection.execute("DELETE FROM financial_facts WHERE id = ?1", [id])?;

    Ok(())
}

// ============================================================================
// Private Helper Functions
// ============================================================================

fn get_kpi_definition(connection: &Connection, id: &str) -> StorageResult<KpiDefinition> {
    connection
        .query_row(
            "
            SELECT
                id,
                scope,
                company_id,
                sector,
                metric_key,
                label,
                value_kind,
                unit,
                computation,
                formula,
                display_format,
                created_at,
                updated_at
            FROM kpi_definitions
            WHERE id = ?1
            ",
            [id],
            kpi_definition_from_row,
        )
        .map_err(StorageError::from)
}

fn get_financial_period(connection: &Connection, id: &str) -> StorageResult<FinancialPeriod> {
    connection
        .query_row(
            "
            SELECT
                id,
                company_id,
                fiscal_year,
                period_type,
                period_end_date,
                report_evidence_ref,
                created_at,
                updated_at
            FROM financial_periods
            WHERE id = ?1
            ",
            [id],
            financial_period_from_row,
        )
        .map_err(StorageError::from)
}

fn get_kpi_relevance(connection: &Connection, id: &str) -> StorageResult<KpiRelevance> {
    connection
        .query_row(
            "
            SELECT
                id,
                company_id,
                definition_id,
                status,
                source,
                rank,
                first_seen_period,
                last_seen_period,
                created_at,
                updated_at
            FROM kpi_relevance
            WHERE id = ?1
            ",
            [id],
            kpi_relevance_from_row,
        )
        .map_err(StorageError::from)
}

fn get_financial_fact(connection: &Connection, id: &str) -> StorageResult<FinancialFact> {
    connection
        .query_row(
            "
            SELECT
                id,
                company_id,
                period_id,
                definition_id,
                value_numeric,
                currency,
                statement_basis,
                attribution,
                variant,
                measure_window,
                data_quality,
                as_reported_value,
                as_reported_scale,
                reporting_standard,
                extraction_method,
                confidence,
                confirmation_state,
                supersedes_id,
                source_document_ref,
                annotation,
                created_at,
                updated_at
            FROM financial_facts
            WHERE id = ?1
            ",
            [id],
            financial_fact_from_row,
        )
        .map_err(StorageError::from)
}

fn kpi_definition_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KpiDefinition> {
    Ok(KpiDefinition {
        id: row.get(0)?,
        scope: row.get(1)?,
        company_id: row.get(2)?,
        sector: row.get(3)?,
        metric_key: row.get(4)?,
        label: row.get(5)?,
        value_kind: row.get(6)?,
        unit: row.get(7)?,
        computation: row.get(8)?,
        formula: row.get(9)?,
        display_format: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn financial_period_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FinancialPeriod> {
    Ok(FinancialPeriod {
        id: row.get(0)?,
        company_id: row.get(1)?,
        fiscal_year: row.get(2)?,
        period_type: row.get(3)?,
        period_end_date: row.get(4)?,
        report_evidence_ref: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn kpi_relevance_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<KpiRelevance> {
    Ok(KpiRelevance {
        id: row.get(0)?,
        company_id: row.get(1)?,
        definition_id: row.get(2)?,
        status: row.get(3)?,
        source: row.get(4)?,
        rank: row.get(5)?,
        first_seen_period: row.get(6)?,
        last_seen_period: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

fn financial_fact_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FinancialFact> {
    Ok(FinancialFact {
        id: row.get(0)?,
        company_id: row.get(1)?,
        period_id: row.get(2)?,
        definition_id: row.get(3)?,
        value_numeric: row.get(4)?,
        currency: row.get(5)?,
        statement_basis: row.get(6)?,
        attribution: row.get(7)?,
        variant: row.get(8)?,
        measure_window: row.get(9)?,
        data_quality: row.get(10)?,
        as_reported_value: row.get(11)?,
        as_reported_scale: row.get(12)?,
        reporting_standard: row.get(13)?,
        extraction_method: row.get(14)?,
        confidence: row.get(15)?,
        confirmation_state: row.get(16)?,
        supersedes_id: row.get(17)?,
        source_document_ref: row.get(18)?,
        annotation: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
    })
}

/// The CANONICAL catalog id for a metric key. Bare `kpidef_<key>`, and it stays
/// that way forever: facts, relevance rows, quality-framework criteria and the
/// metric-history reads all key on it, so re-shaping it would orphan them.
fn canonical_kpi_definition_id(metric_key: &str) -> String {
    format!("kpidef_{}", slug_part(metric_key))
}

/// The catalog id for a definition in ANY scope (issue #149).
///
/// `kpi_definitions` is unique on `(metric_key, scope, company_id, sector)` —
/// a metric key legitimately exists once per scope bucket. The PRIMARY KEY has
/// to carry the same discriminator, otherwise a company-scoped definition whose
/// reported measure shares a generic catalog key cannot be created at all (PK
/// conflict with the canonical row), and a company-scoped key with no canonical
/// twin squats on the id a later canonical seed needs — an `INSERT OR IGNORE`
/// seed then silently does nothing.
///
/// Canonical keeps the bare id; non-canonical scopes get a suffix built from
/// exactly the columns the unique index uses, so the id is unique wherever the
/// index is. Curated sector packs seeded with their own hand-written ids
/// (`kpidef_bank_nim`, migrations 0034/0048/…) are untouched — they never went
/// through this function.
fn kpi_definition_id(
    scope: &str,
    company_id: Option<&str>,
    sector: Option<&str>,
    metric_key: &str,
) -> String {
    let base = canonical_kpi_definition_id(metric_key);
    let scope = scope.trim().to_ascii_lowercase();

    if scope == "canonical" {
        return base;
    }

    let (marker, discriminator) = match scope.as_str() {
        "company" => ("c".to_owned(), slug_part(company_id.unwrap_or_default())),
        "sector" => ("s".to_owned(), slug_part(sector.unwrap_or_default())),
        other => (
            slug_part(other),
            slug_part(company_id.or(sector).unwrap_or_default()),
        ),
    };

    format!("{base}__{marker}_{discriminator}")
}

fn financial_period_id(company_id: &str, fiscal_year: i64, period_type: &str) -> String {
    format!(
        "period_{}_{}_{}_{}",
        slug_part(company_id),
        fiscal_year,
        slug_part(period_type),
        ulid_suffix()
    )
}

fn kpi_relevance_id(company_id: &str, definition_id: &str) -> String {
    format!(
        "relevance_{}_{}",
        slug_part(company_id),
        slug_part(definition_id)
    )
}

fn financial_fact_id(
    period_id: &str,
    definition_id: &str,
    statement_basis: &str,
    attribution: &str,
    variant: &str,
    measure_window: &str,
    data_quality: &str,
) -> String {
    format!(
        "fact_{}_{}_{}_{}_{}_{}_{}_{}",
        slug_part(period_id),
        slug_part(definition_id),
        slug_part(statement_basis),
        slug_part(attribution),
        slug_part(variant),
        slug_part(measure_window),
        slug_part(data_quality),
        ulid_suffix()
    )
}

fn ulid_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    format!("{:x}", timestamp % 0xFFFFFFFF)
}

fn validate_reference_exists(
    connection: &Connection,
    table_name: &str,
    id: &str,
) -> StorageResult<()> {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table_name} WHERE id = ?1)");
    let exists: bool = connection.query_row(&sql, [id], |row| row.get(0))?;

    if exists {
        Ok(())
    } else {
        Err(StorageError::MissingFinancialsReference {
            table: table_name.to_owned(),
            id: id.to_owned(),
        })
    }
}

/// One canonical fact selected for the cross-company comparison read model
/// (ADR 0089 dec. 1) — the single reporting variant per `(company, metric,
/// period)` slot, carrying just what the read model needs plus the provenance
/// `validation_status` evidence link. Slot preference mirrors
/// `quality_frameworks::load_period_facts` (final › reported › consolidated ›
/// total), so the two read paths never disagree about "the" value.
#[derive(Clone, Debug)]
pub struct CanonicalComparisonFact {
    pub company_id: String,
    pub metric_key: String,
    pub fiscal_year: i64,
    pub period_type: String,
    pub period_end_date: Option<String>,
    pub fact_id: String,
    pub value_numeric: String,
    pub currency: Option<String>,
    pub measure_window: String,
    pub validation_status: Option<String>,
}

/// Select the canonical confirmed fact per `(company, metric, period)` for the
/// requested companies × metric keys × period types (the granularity filter).
/// One row per slot: the DB returns every candidate ordered by the canonical
/// preference and we keep the first per slot (same collapse as
/// `load_period_facts`), LEFT-joining provenance for the evidence link. Empty
/// inputs short-circuit to no rows.
pub(super) fn comparison_facts(
    connection: &Connection,
    company_ids: &[String],
    metric_keys: &[String],
    period_types: &[&str],
) -> StorageResult<Vec<CanonicalComparisonFact>> {
    if company_ids.is_empty() || metric_keys.is_empty() || period_types.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders =
        |n: usize| -> String { std::iter::repeat_n("?", n).collect::<Vec<_>>().join(", ") };
    let sql = format!(
        "SELECT f.company_id, d.metric_key, p.fiscal_year, p.period_type,
                p.period_end_date, f.id, f.value_numeric, f.currency,
                f.measure_window, prov.validation_status
         FROM financial_facts f
         JOIN financial_periods p ON p.id = f.period_id
         JOIN kpi_definitions d ON d.id = f.definition_id
         LEFT JOIN financial_fact_provenance prov ON prov.fact_id = f.id
         WHERE f.confirmation_state = 'confirmed'
           AND f.company_id IN ({})
           AND d.metric_key IN ({})
           AND p.period_type IN ({})
         ORDER BY f.company_id, d.metric_key, p.fiscal_year, p.period_type,
                  CASE f.data_quality WHEN 'final' THEN 0 ELSE 1 END,
                  CASE f.variant WHEN 'reported' THEN 0 ELSE 1 END,
                  CASE f.statement_basis WHEN 'consolidated' THEN 0 ELSE 1 END,
                  CASE f.attribution WHEN 'total' THEN 0 WHEN 'owners_of_parent' THEN 1 ELSE 2 END,
                  f.id",
        placeholders(company_ids.len()),
        placeholders(metric_keys.len()),
        placeholders(period_types.len()),
    );

    let mut statement = connection.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = company_ids
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .chain(metric_keys.iter().map(|s| s as &dyn rusqlite::ToSql))
        .chain(period_types.iter().map(|s| s as &dyn rusqlite::ToSql))
        .collect();

    let rows = statement.query_map(params.as_slice(), |row| {
        Ok(CanonicalComparisonFact {
            company_id: row.get(0)?,
            metric_key: row.get(1)?,
            fiscal_year: row.get(2)?,
            period_type: row.get(3)?,
            period_end_date: row.get(4)?,
            fact_id: row.get(5)?,
            value_numeric: row.get(6)?,
            currency: row.get(7)?,
            measure_window: row.get(8)?,
            validation_status: row.get(9)?,
        })
    })?;

    // Keep the first (canonical) row per slot — the ORDER BY already ranks them.
    let mut seen: std::collections::HashSet<(String, String, i64, String)> =
        std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        let row = row?;
        let slot = (
            row.company_id.clone(),
            row.metric_key.clone(),
            row.fiscal_year,
            row.period_type.clone(),
        );
        if seen.insert(slot) {
            out.push(row);
        }
    }
    Ok(out)
}

/// The `value_kind` per requested `metric_key` (drives % vs p.p. deltas in the
/// comparison read model). Prefers the app-owned scope when several definitions
/// share a key (`canonical` › `sector` › `user` › `company`); a key with no
/// definition is simply absent from the map.
pub(super) fn metric_value_kinds(
    connection: &Connection,
    metric_keys: &[String],
) -> StorageResult<std::collections::HashMap<String, String>> {
    let mut out = std::collections::HashMap::new();
    if metric_keys.is_empty() {
        return Ok(out);
    }
    let placeholders = std::iter::repeat_n("?", metric_keys.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT metric_key, value_kind FROM kpi_definitions
         WHERE metric_key IN ({placeholders})
         ORDER BY CASE scope WHEN 'canonical' THEN 0 WHEN 'sector' THEN 1
                             WHEN 'user' THEN 2 ELSE 3 END"
    );
    let mut statement = connection.prepare(&sql)?;
    let params: Vec<&dyn rusqlite::ToSql> = metric_keys
        .iter()
        .map(|s| s as &dyn rusqlite::ToSql)
        .collect();
    let rows = statement.query_map(params.as_slice(), |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (metric_key, value_kind) = row?;
        out.entry(metric_key).or_insert(value_kind);
    }
    Ok(out)
}

use super::database::Database;
/// financials domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::financials()`.
#[derive(Clone)]
pub struct FinancialsStore {
    db: Database,
}

impl FinancialsStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_kpi_definitions(
        &self,
        input: ListKpiDefinitionsInput,
    ) -> StorageResult<Vec<KpiDefinition>> {
        let connection = self.db.checkout()?;

        list_kpi_definitions(&connection, input)
    }

    pub fn create_kpi_definition(&self, input: NewKpiDefinition) -> StorageResult<KpiDefinition> {
        let connection = self.db.checkout()?;

        create_kpi_definition(&connection, input)
    }

    pub fn list_financial_periods(
        &self,
        input: ListFinancialPeriodsInput,
    ) -> StorageResult<Vec<FinancialPeriod>> {
        let connection = self.db.checkout()?;

        list_financial_periods(&connection, input)
    }

    pub fn create_financial_period(
        &self,
        input: NewFinancialPeriod,
    ) -> StorageResult<FinancialPeriod> {
        let connection = self.db.checkout()?;

        create_financial_period(&connection, input)
    }

    pub fn update_financial_period(
        &self,
        input: UpdateFinancialPeriod,
    ) -> StorageResult<FinancialPeriod> {
        let connection = self.db.checkout()?;

        update_financial_period(&connection, input)
    }

    pub fn delete_financial_period(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        delete_financial_period(&connection, id)
    }

    pub fn list_kpi_relevance(&self, company_id: &str) -> StorageResult<Vec<KpiRelevance>> {
        let connection = self.db.checkout()?;

        list_kpi_relevance(&connection, company_id)
    }

    pub fn create_kpi_relevance(&self, input: NewKpiRelevance) -> StorageResult<KpiRelevance> {
        let connection = self.db.checkout()?;

        create_kpi_relevance(&connection, input)
    }

    pub fn update_kpi_relevance(&self, input: UpdateKpiRelevance) -> StorageResult<KpiRelevance> {
        let connection = self.db.checkout()?;

        update_kpi_relevance(&connection, input)
    }

    pub fn delete_kpi_relevance(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        delete_kpi_relevance(&connection, id)
    }

    pub fn list_financial_facts(
        &self,
        input: ListFinancialFactsInput,
    ) -> StorageResult<Vec<FinancialFact>> {
        let connection = self.db.checkout()?;

        list_financial_facts(&connection, input)
    }

    /// Canonical confirmed facts for the cross-company comparison read model
    /// (ADR 0089 dec. 1) — one per `(company, metric, period)` slot across the
    /// requested companies × metric keys × period types. See
    /// [`comparison_facts`].
    pub fn comparison_facts(
        &self,
        company_ids: &[String],
        metric_keys: &[String],
        period_types: &[&str],
    ) -> StorageResult<Vec<CanonicalComparisonFact>> {
        let connection = self.db.checkout()?;

        comparison_facts(&connection, company_ids, metric_keys, period_types)
    }

    /// The `value_kind` per requested `metric_key` — see [`metric_value_kinds`].
    pub fn metric_value_kinds(
        &self,
        metric_keys: &[String],
    ) -> StorageResult<std::collections::HashMap<String, String>> {
        let connection = self.db.checkout()?;

        metric_value_kinds(&connection, metric_keys)
    }

    /// The company's expected primary-KPI `metric_key`s (ADR 0061 dec. 4d). See
    /// [`expected_primary_metric_keys`] for the semantics.
    pub fn expected_primary_metric_keys(
        &self,
        company_id: &str,
    ) -> StorageResult<Option<std::collections::BTreeSet<String>>> {
        let connection = self.db.checkout()?;

        expected_primary_metric_keys(&connection, company_id)
    }

    /// Bring one company's automatic `kpi_relevance` layers up to date (ADR
    /// 0092 layers 2 + 3), in one checkout and one transaction.
    ///
    /// Idempotent, additive, and safe to call on any cadence: the statement
    /// pack converges after a `statement_type` change, the derived pass picks
    /// up newly-consistent keys. Neither ever overwrites a `core` or curated
    /// row, and neither ever deletes. Returns the rows added.
    pub fn refresh_kpi_relevance_layers(&self, company_id: &str) -> StorageResult<usize> {
        let connection = self.db.checkout()?;
        let transaction = connection.unchecked_transaction()?;
        let seeded = seed_statement_pack_kpi_relevance(&transaction, company_id)?
            + refresh_derived_kpi_relevance(&transaction, company_id)?;
        transaction.commit()?;
        Ok(seeded)
    }

    /// The previously-stored fact set for one `(company, fiscal_year,
    /// period_type)`, bridged to `metric_key`s (ADR 0061 dec. 4b) — the
    /// cross-check-UNAWARE variant (every stored fact, no reversed-witnessing
    /// veto). A comparative cross-check wants [`Self::stored_fact_set_for_cross_check`]
    /// instead; only test harnesses read this unfiltered form today.
    pub fn stored_fact_set(
        &self,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
    ) -> StorageResult<Option<FactSet>> {
        let connection = self.db.checkout()?;

        stored_fact_set(&connection, company_id, fiscal_year, period_type)
    }

    /// [`Self::stored_fact_set`] restricted to veto-capable facts for an
    /// `incoming_tier` cross-check (ADR 0086 dec. 3/4) — see
    /// [`stored_fact_set_for_cross_check`].
    pub fn stored_fact_set_for_cross_check(
        &self,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
        incoming_tier: crate::fundamentals::extraction::SourceTier,
    ) -> StorageResult<Option<FactSet>> {
        let connection = self.db.checkout()?;

        stored_fact_set_for_cross_check(
            &connection,
            company_id,
            fiscal_year,
            period_type,
            incoming_tier,
        )
    }

    /// A metric's stored history across the company's other periods, for the
    /// runtime plausibility gate. See [`metric_history`] for the read semantics.
    pub fn metric_history(
        &self,
        company_id: &str,
        metric_key: &str,
        exclude_fiscal_year: i64,
        exclude_period_type: &str,
    ) -> StorageResult<Vec<Decimal>> {
        let connection = self.db.checkout()?;

        metric_history(
            &connection,
            company_id,
            metric_key,
            exclude_fiscal_year,
            exclude_period_type,
        )
    }

    /// The stored history of many metrics in one read (one checkout, one period
    /// scan, one company fact scan). Batched form of [`Self::metric_history`] —
    /// see [`metric_histories`] for the equivalence contract the per-fact
    /// history-plausibility gate relies on.
    pub fn metric_histories(
        &self,
        company_id: &str,
        metric_keys: &std::collections::BTreeSet<String>,
        exclude_fiscal_year: i64,
        exclude_period_type: &str,
    ) -> StorageResult<std::collections::HashMap<String, Vec<Decimal>>> {
        let connection = self.db.checkout()?;

        metric_histories(
            &connection,
            company_id,
            metric_keys,
            exclude_fiscal_year,
            exclude_period_type,
        )
    }

    /// The cached period derivation for one document, if any (migration 0109).
    /// A hit lets `derive_report_period` skip the file read + text extraction the
    /// last-resort cover-page tier would otherwise run on every call.
    pub fn cached_derived_period(
        &self,
        report_document_id: &str,
    ) -> StorageResult<Option<CachedDerivedPeriod>> {
        let connection = self.db.checkout()?;

        connection
            .query_row(
                "SELECT has_period, fiscal_year, period_type, period_end, derivation_version
                 FROM document_derived_periods
                 WHERE report_document_id = ?1",
                [report_document_id],
                |row| {
                    Ok(CachedDerivedPeriod {
                        has_period: row.get::<_, i64>(0)? != 0,
                        fiscal_year: row.get(1)?,
                        period_type: row.get(2)?,
                        period_end: row.get(3)?,
                        derivation_version: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(StorageError::from)
    }

    /// Persist a document's derived period (migration 0109). `period = None`
    /// writes the explicit none-marker so an abstention is not re-parsed either.
    /// Upserts on the document key so a version bump re-derives and overwrites in
    /// place (self-healing). `derivation_version` is the caller's current
    /// derivation-grammar version.
    pub fn store_derived_period(
        &self,
        report_document_id: &str,
        period: Option<(i64, &str, &str)>,
        derivation_version: i64,
    ) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        let (has_period, fiscal_year, period_type, period_end) = match period {
            Some((fy, pt, pe)) => (1i64, Some(fy), Some(pt.to_owned()), Some(pe.to_owned())),
            None => (0i64, None, None, None),
        };
        connection.execute(
            "INSERT INTO document_derived_periods (
                report_document_id, has_period, fiscal_year, period_type, period_end,
                derivation_version, derived_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            ON CONFLICT(report_document_id) DO UPDATE SET
                has_period = excluded.has_period,
                fiscal_year = excluded.fiscal_year,
                period_type = excluded.period_type,
                period_end = excluded.period_end,
                derivation_version = excluded.derivation_version,
                derived_at = excluded.derived_at",
            params![
                report_document_id,
                has_period,
                fiscal_year,
                period_type,
                period_end,
                derivation_version
            ],
        )?;
        Ok(())
    }

    pub fn create_financial_fact(&self, input: NewFinancialFact) -> StorageResult<FinancialFact> {
        let connection = self.db.checkout()?;

        create_financial_fact(&connection, input)
    }

    pub fn update_financial_fact(
        &self,
        input: UpdateFinancialFact,
    ) -> StorageResult<FinancialFact> {
        let connection = self.db.checkout()?;

        update_financial_fact(&connection, input)
    }

    pub fn delete_financial_fact(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;

        delete_financial_fact(&connection, id)
    }

    /// Fact counts per `(fiscal_year, period_type)` for a company, split by
    /// provenance validation state — the facts axis of the coverage read model
    /// (ADR 0077 §2). `validated` = provenance `passed`/`witness_confirmed`,
    /// `flagged` = provenance `flagged`, `unvalidated` = everything else (no
    /// provenance row, `unreviewed`, `none`, …). Company-scoped through the
    /// period join; periods with no facts are simply absent.
    pub fn facts_coverage_by_period(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<PeriodFactCoverage>> {
        let connection = self.db.checkout()?;

        facts_coverage_by_period(&connection, company_id)
    }
}

/// One `(fiscal_year, period_type)` bucket of fact counts for the coverage read
/// model. Not an IPC DTO — the coverage command maps it into `CoverageFactsCell`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeriodFactCoverage {
    pub fiscal_year: i64,
    pub period_type: String,
    pub total: i64,
    pub validated: i64,
    pub unvalidated: i64,
    pub flagged: i64,
}

pub(super) fn facts_coverage_by_period(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<PeriodFactCoverage>> {
    let mut statement = connection.prepare(
        "SELECT p.fiscal_year,
                p.period_type,
                COUNT(*) AS total,
                SUM(CASE WHEN pv.validation_status IN ('passed', 'witness_confirmed')
                         THEN 1 ELSE 0 END) AS validated,
                SUM(CASE WHEN pv.validation_status = 'flagged' THEN 1 ELSE 0 END) AS flagged
         FROM financial_facts f
         JOIN financial_periods p ON p.id = f.period_id
         LEFT JOIN financial_fact_provenance pv ON pv.fact_id = f.id
         WHERE p.company_id = ?1
         GROUP BY p.fiscal_year, p.period_type",
    )?;
    let rows = statement.query_map([company_id], |row| {
        let fiscal_year: i64 = row.get(0)?;
        let period_type: String = row.get(1)?;
        let total: i64 = row.get(2)?;
        let validated: i64 = row.get(3)?;
        let flagged: i64 = row.get(4)?;
        Ok(PeriodFactCoverage {
            fiscal_year,
            period_type,
            total,
            validated,
            unvalidated: total - validated - flagged,
            flagged,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}
