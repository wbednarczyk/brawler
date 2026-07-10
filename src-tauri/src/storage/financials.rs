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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
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
            AND (?3 IS NULL OR company_id = ?3)
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

    let id = kpi_definition_id(&metric_key);

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
fn normalize_period_type_label(period_type: &str) -> String {
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

    get_kpi_relevance(connection, &id)
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
pub(super) fn stored_fact_set(
    connection: &Connection,
    company_id: &str,
    fiscal_year: i64,
    period_type: &str,
) -> StorageResult<Option<FactSet>> {
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
            period_id: Some(period.id),
            definition_id: None,
        },
    )?;
    if facts.is_empty() {
        return Ok(None);
    }

    // `kpi_definitions.id` is derived solely from `metric_key`
    // (`kpi_definition_id`), so listing the full catalog (no scope filter)
    // gives a stable id → metric_key map regardless of which scope produced
    // the definition a fact references.
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

    let mut set = FactSet::new();
    for fact in facts {
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
    let currency = empty_string_to_none(input.currency.map(|s| s.trim().to_owned()));
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
            source_document_ref
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
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
            source_document_ref
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

    let currency = input
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
        .or(current.currency);

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

    connection.execute(
        "
        UPDATE financial_facts
        SET value_numeric = ?2,
            currency = ?3,
            data_quality = ?4,
            confirmation_state = ?5,
            supersedes_id = ?6,
            source_document_ref = ?7,
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
            source_document_ref
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
        created_at: row.get(19)?,
        updated_at: row.get(20)?,
    })
}

fn kpi_definition_id(metric_key: &str) -> String {
    format!("kpidef_{}", slug_part(metric_key))
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

    /// The previously-stored fact set for one `(company, fiscal_year,
    /// period_type)`, bridged to `metric_key`s (ADR 0061 dec. 4b). See
    /// [`stored_fact_set`] for the read semantics.
    pub fn stored_fact_set(
        &self,
        company_id: &str,
        fiscal_year: i64,
        period_type: &str,
    ) -> StorageResult<Option<FactSet>> {
        let connection = self.db.checkout()?;

        stored_fact_set(&connection, company_id, fiscal_year, period_type)
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

fn facts_coverage_by_period(
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
