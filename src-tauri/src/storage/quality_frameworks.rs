//! Storage for quality frameworks — quantitative checks (v0.44.0, ADR 0046).
//!
//! A framework is a user-owned checklist of criteria expressed in a free-text
//! DSL over KPI metric keys. The rule engine evaluates them against confirmed
//! `financial_facts` (latest period) into an immutable, versioned scorecard.
//! `origin` is a provenance label, not an edit lock: every framework is editable
//! and deletable in place. App templates seed from a Rust constant and can be
//! reset to it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use rusqlite::{params, Connection, OptionalExtension, Row};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use super::{slug_part, StorageError, StorageResult};
use crate::fundamentals::expr::{self, referenced_metrics};
use crate::fundamentals::metrics::{Computation, MetricDef, MetricsContext, PeriodFacts};
use crate::fundamentals::scorecard::{evaluate_criterion, VerdictCounts, ENGINE_VERSION};
use crate::fundamentals::templates;

// ============================================================================
// Domain types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityFramework {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub origin: String,
    pub template_key: Option<String>,
    pub cloned_from: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub criteria: Vec<FrameworkCriterion>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkCriterion {
    pub id: String,
    pub framework_id: String,
    pub ordinal: i64,
    pub label: String,
    pub expression: String,
    pub weight: Option<String>,
    pub partial_band: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkEvaluation {
    pub id: String,
    pub framework_id: String,
    pub framework_version: i64,
    pub company_id: String,
    pub period_id: Option<String>,
    pub pass_count: i64,
    pub partial_count: i64,
    pub fail_count: i64,
    pub unavailable_count: i64,
    pub engine_version: String,
    pub created_at: String,
    pub results: Vec<CriterionResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CriterionResult {
    pub id: String,
    pub evaluation_id: String,
    pub criterion_id: Option<String>,
    pub ordinal: i64,
    pub label: String,
    pub expression: String,
    pub verdict: String,
    pub measured_value: Option<String>,
    pub measured_unit: Option<String>,
    pub threshold: Option<String>,
    pub inputs_json: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateCriterionResult {
    pub ok: bool,
    pub error: Option<String>,
    pub referenced_metric_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricKeyInfo {
    pub key: String,
    pub label: String,
    pub unit: Option<String>,
    pub value_kind: String,
    pub computation: String,
    pub scope: String,
}

// ---- inputs ----------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewQualityFramework {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateQualityFramework {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloneFrameworkInput {
    pub framework_id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewFrameworkCriterion {
    pub framework_id: String,
    pub label: String,
    pub expression: String,
    #[serde(default)]
    pub weight: Option<String>,
    #[serde(default)]
    pub partial_band: Option<String>,
    #[serde(default)]
    pub ordinal: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFrameworkCriterion {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub expression: Option<String>,
    #[serde(default)]
    pub weight: Option<String>,
    #[serde(default)]
    pub partial_band: Option<String>,
    #[serde(default)]
    pub ordinal: Option<i64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateFrameworkInput {
    pub framework_id: String,
    pub company_id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFrameworkEvaluationsInput {
    pub framework_id: String,
    pub company_id: String,
}

// ============================================================================
// Frameworks CRUD
// ============================================================================

pub(super) fn list_quality_frameworks(
    connection: &Connection,
) -> StorageResult<Vec<QualityFramework>> {
    let mut statement = connection.prepare(
        "SELECT id, name, description, origin, template_key, cloned_from, version, created_at, updated_at
         FROM quality_frameworks ORDER BY name COLLATE NOCASE",
    )?;
    let rows = statement.query_map([], framework_from_row)?;
    let mut frameworks = rows.collect::<Result<Vec<_>, _>>()?;
    for framework in &mut frameworks {
        framework.criteria = list_criteria(connection, &framework.id)?;
    }
    Ok(frameworks)
}

pub(super) fn get_quality_framework(
    connection: &Connection,
    id: &str,
) -> StorageResult<QualityFramework> {
    let mut framework = connection
        .query_row(
            "SELECT id, name, description, origin, template_key, cloned_from, version, created_at, updated_at
             FROM quality_frameworks WHERE id = ?1",
            [id],
            framework_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingFrameworkReference {
            table: "quality_frameworks".to_owned(),
            id: id.to_owned(),
        })?;
    framework.criteria = list_criteria(connection, &framework.id)?;
    Ok(framework)
}

pub(super) fn create_quality_framework(
    connection: &Connection,
    input: NewQualityFramework,
) -> StorageResult<QualityFramework> {
    let name = input.name.trim().to_owned();
    if name.is_empty() {
        return Err(StorageError::InvalidFrameworkValue {
            key: "name",
            value: name,
        });
    }
    let description = empty_to_none(input.description);
    let id = framework_id(&name);

    connection.execute(
        "INSERT INTO quality_frameworks (id, name, description, origin, version)
         VALUES (?1, ?2, ?3, 'user', 1)",
        params![id, name, description],
    )?;
    get_quality_framework(connection, &id)
}

pub(super) fn update_quality_framework(
    connection: &Connection,
    input: UpdateQualityFramework,
) -> StorageResult<QualityFramework> {
    let existing = get_quality_framework(connection, &input.id)?;
    let name = match input.name {
        Some(n) => {
            let n = n.trim().to_owned();
            if n.is_empty() {
                return Err(StorageError::InvalidFrameworkValue {
                    key: "name",
                    value: n,
                });
            }
            n
        }
        None => existing.name,
    };
    let description = match input.description {
        Some(d) => empty_to_none(Some(d)),
        None => existing.description,
    };

    connection.execute(
        "UPDATE quality_frameworks
         SET name = ?2, description = ?3, version = version + 1,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        params![input.id, name, description],
    )?;
    get_quality_framework(connection, &input.id)
}

pub(super) fn delete_quality_framework(connection: &Connection, id: &str) -> StorageResult<()> {
    let affected = connection.execute("DELETE FROM quality_frameworks WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(StorageError::MissingFrameworkReference {
            table: "quality_frameworks".to_owned(),
            id: id.to_owned(),
        });
    }
    Ok(())
}

pub(super) fn clone_framework(
    connection: &Connection,
    input: CloneFrameworkInput,
) -> StorageResult<QualityFramework> {
    let source = get_quality_framework(connection, &input.framework_id)?;
    let name = input
        .name
        .map(|n| n.trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("{} (copy)", source.name));
    let id = framework_id(&name);

    connection.execute(
        "INSERT INTO quality_frameworks (id, name, description, origin, cloned_from, version)
         VALUES (?1, ?2, ?3, 'user', ?4, 1)",
        params![id, name, source.description, source.id],
    )?;
    for criterion in &source.criteria {
        insert_criterion(
            connection,
            &id,
            criterion.ordinal,
            &criterion.label,
            &criterion.expression,
            criterion.weight.as_deref(),
            criterion.partial_band.as_deref(),
        )?;
    }
    get_quality_framework(connection, &id)
}

/// Reset an `app_template`-origin framework's criteria to the shipped template
/// constant (ADR 0046 Decision 6). Errors for `user`-origin frameworks.
pub(super) fn reset_framework_to_template(
    connection: &Connection,
    id: &str,
) -> StorageResult<QualityFramework> {
    let framework = get_quality_framework(connection, id)?;
    if framework.origin != "app_template" {
        return Err(StorageError::NotATemplate { id: id.to_owned() });
    }
    let template = framework
        .template_key
        .as_deref()
        .and_then(templates::template_by_key)
        .ok_or_else(|| StorageError::NotATemplate { id: id.to_owned() })?;

    connection.execute(
        "DELETE FROM framework_criteria WHERE framework_id = ?1",
        [id],
    )?;
    seed_template_criteria(connection, id, template)?;
    connection.execute(
        "UPDATE quality_frameworks
         SET version = version + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        [id],
    )?;
    get_quality_framework(connection, id)
}

// ============================================================================
// Criteria CRUD
// ============================================================================

fn list_criteria(
    connection: &Connection,
    framework_id: &str,
) -> StorageResult<Vec<FrameworkCriterion>> {
    let mut statement = connection.prepare(
        "SELECT id, framework_id, ordinal, label, expression, weight, partial_band, created_at, updated_at
         FROM framework_criteria WHERE framework_id = ?1 ORDER BY ordinal, created_at",
    )?;
    let rows = statement.query_map([framework_id], criterion_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

pub(super) fn create_framework_criterion(
    connection: &Connection,
    input: NewFrameworkCriterion,
) -> StorageResult<FrameworkCriterion> {
    // Framework must exist.
    get_quality_framework(connection, &input.framework_id)?;
    let label = input.label.trim().to_owned();
    if label.is_empty() {
        return Err(StorageError::InvalidFrameworkValue {
            key: "label",
            value: label,
        });
    }
    let expression = input.expression.trim().to_owned();
    validate_predicate(&expression)?;
    validate_band(input.partial_band.as_deref())?;

    let ordinal = match input.ordinal {
        Some(o) => o,
        None => next_ordinal(connection, &input.framework_id)?,
    };
    let id = insert_criterion(
        connection,
        &input.framework_id,
        ordinal,
        &label,
        &expression,
        input.weight.as_deref(),
        input.partial_band.as_deref(),
    )?;
    bump_framework_version(connection, &input.framework_id)?;
    get_criterion(connection, &id)
}

pub(super) fn update_framework_criterion(
    connection: &Connection,
    input: UpdateFrameworkCriterion,
) -> StorageResult<FrameworkCriterion> {
    let existing = get_criterion(connection, &input.id)?;
    let label = match input.label {
        Some(l) => {
            let l = l.trim().to_owned();
            if l.is_empty() {
                return Err(StorageError::InvalidFrameworkValue {
                    key: "label",
                    value: l,
                });
            }
            l
        }
        None => existing.label,
    };
    let expression = match input.expression {
        Some(e) => {
            let e = e.trim().to_owned();
            validate_predicate(&e)?;
            e
        }
        None => existing.expression,
    };
    let partial_band = match input.partial_band {
        Some(b) => empty_to_none(Some(b)),
        None => existing.partial_band,
    };
    validate_band(partial_band.as_deref())?;
    let weight = match input.weight {
        Some(w) => empty_to_none(Some(w)),
        None => existing.weight,
    };
    let ordinal = input.ordinal.unwrap_or(existing.ordinal);
    let ast = cached_ast(&expression);

    connection.execute(
        "UPDATE framework_criteria
         SET label = ?2, expression = ?3, expression_ast = ?4, weight = ?5, partial_band = ?6,
             ordinal = ?7, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        params![
            input.id,
            label,
            expression,
            ast,
            weight,
            partial_band,
            ordinal
        ],
    )?;
    bump_framework_version(connection, &existing.framework_id)?;
    get_criterion(connection, &input.id)
}

pub(super) fn delete_framework_criterion(connection: &Connection, id: &str) -> StorageResult<()> {
    let framework_id: Option<String> = connection
        .query_row(
            "SELECT framework_id FROM framework_criteria WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(framework_id) = framework_id else {
        return Err(StorageError::MissingFrameworkReference {
            table: "framework_criteria".to_owned(),
            id: id.to_owned(),
        });
    };
    connection.execute("DELETE FROM framework_criteria WHERE id = ?1", [id])?;
    bump_framework_version(connection, &framework_id)?;
    Ok(())
}

fn get_criterion(connection: &Connection, id: &str) -> StorageResult<FrameworkCriterion> {
    connection
        .query_row(
            "SELECT id, framework_id, ordinal, label, expression, weight, partial_band, created_at, updated_at
             FROM framework_criteria WHERE id = ?1",
            [id],
            criterion_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingFrameworkReference {
            table: "framework_criteria".to_owned(),
            id: id.to_owned(),
        })
}

// ============================================================================
// Validation
// ============================================================================

/// Parse-only validation for the criteria editor.
pub(super) fn validate_criterion_expression(expression: &str) -> ValidateCriterionResult {
    match expr::parse(expression.trim()) {
        Ok(parsed) => {
            if expr::is_predicate(&parsed) {
                ValidateCriterionResult {
                    ok: true,
                    error: None,
                    referenced_metric_keys: referenced_metrics(&parsed),
                }
            } else {
                ValidateCriterionResult {
                    ok: false,
                    error: Some(
                        "a criterion must be a comparison or boolean test (e.g. roic >= 15%)"
                            .to_owned(),
                    ),
                    referenced_metric_keys: referenced_metrics(&parsed),
                }
            }
        }
        Err(error) => ValidateCriterionResult {
            ok: false,
            error: Some(error.to_string()),
            referenced_metric_keys: Vec::new(),
        },
    }
}

fn validate_predicate(expression: &str) -> StorageResult<()> {
    let parsed =
        expr::parse(expression).map_err(|error| StorageError::InvalidCriterionExpression {
            message: error.to_string(),
        })?;
    if !expr::is_predicate(&parsed) {
        return Err(StorageError::InvalidCriterionExpression {
            message: "a criterion must be a comparison or boolean test (e.g. roic >= 15%)"
                .to_owned(),
        });
    }
    Ok(())
}

fn validate_band(band: Option<&str>) -> StorageResult<()> {
    if let Some(band) = band {
        if !band.trim().is_empty() {
            expr::parse(band).map_err(|error| StorageError::InvalidCriterionExpression {
                message: format!("partial band: {error}"),
            })?;
        }
    }
    Ok(())
}

// ============================================================================
// Evaluation (the rule engine entry point)
// ============================================================================

pub(super) fn evaluate_framework(
    connection: &Connection,
    input: EvaluateFrameworkInput,
) -> StorageResult<FrameworkEvaluation> {
    let framework = get_quality_framework(connection, &input.framework_id)?;
    ensure_company_exists(connection, &input.company_id)?;

    let context = load_metrics_context(connection, &input.company_id)?;
    let period_id = context.latest_period_id().map(|s| s.to_owned());

    let mut results: Vec<(
        i64,
        &FrameworkCriterion,
        crate::fundamentals::CriterionOutcome,
    )> = Vec::with_capacity(framework.criteria.len());
    for criterion in &framework.criteria {
        let outcome = evaluate_criterion(
            &criterion.expression,
            criterion.partial_band.as_deref(),
            &context,
        );
        results.push((criterion.ordinal, criterion, outcome));
    }

    let counts = VerdictCounts::tally(results.iter().map(|(_, _, o)| o.verdict));
    let evaluation_id = evaluation_id(&framework.id, &input.company_id);

    connection.execute(
        "INSERT INTO framework_evaluations
            (id, framework_id, framework_version, company_id, period_id,
             pass_count, partial_count, fail_count, unavailable_count, engine_version)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            evaluation_id,
            framework.id,
            framework.version,
            input.company_id,
            period_id,
            counts.pass,
            counts.partial,
            counts.fail,
            counts.unavailable,
            ENGINE_VERSION,
        ],
    )?;

    for (ordinal, criterion, outcome) in &results {
        let inputs_json = inputs_json(&criterion.expression);
        connection.execute(
            "INSERT INTO criterion_results
                (id, evaluation_id, criterion_id, ordinal, label, expression, verdict,
                 measured_value, measured_unit, threshold, inputs_json, note)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                criterion_result_id(&evaluation_id, &criterion.id),
                evaluation_id,
                criterion.id,
                ordinal,
                criterion.label,
                criterion.expression,
                outcome.verdict.as_str(),
                outcome.measured_value,
                outcome.measured_unit,
                outcome.threshold,
                inputs_json,
                None::<String>,
            ],
        )?;
    }

    get_framework_evaluation(connection, &evaluation_id)
}

pub(super) fn list_framework_evaluations(
    connection: &Connection,
    input: ListFrameworkEvaluationsInput,
) -> StorageResult<Vec<FrameworkEvaluation>> {
    let mut statement = connection.prepare(
        "SELECT id, framework_id, framework_version, company_id, period_id,
                pass_count, partial_count, fail_count, unavailable_count, engine_version, created_at
         FROM framework_evaluations
         WHERE framework_id = ?1 AND company_id = ?2
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = statement.query_map(
        params![input.framework_id, input.company_id],
        evaluation_from_row,
    )?;
    let mut evaluations = rows.collect::<Result<Vec<_>, _>>()?;
    for evaluation in &mut evaluations {
        evaluation.results = list_criterion_results(connection, &evaluation.id)?;
    }
    Ok(evaluations)
}

pub(super) fn get_framework_evaluation(
    connection: &Connection,
    id: &str,
) -> StorageResult<FrameworkEvaluation> {
    let mut evaluation = connection
        .query_row(
            "SELECT id, framework_id, framework_version, company_id, period_id,
                    pass_count, partial_count, fail_count, unavailable_count, engine_version, created_at
             FROM framework_evaluations WHERE id = ?1",
            [id],
            evaluation_from_row,
        )
        .optional()?
        .ok_or_else(|| StorageError::MissingFrameworkReference {
            table: "framework_evaluations".to_owned(),
            id: id.to_owned(),
        })?;
    evaluation.results = list_criterion_results(connection, &evaluation.id)?;
    Ok(evaluation)
}

/// Prune one evaluation run from the history. Cascades to its `criterion_results`
/// (FK `ON DELETE CASCADE`). Removing a whole run never mutates a retained run's
/// snapshotted values, so the immutability guarantee holds for what remains.
pub(super) fn delete_framework_evaluation(connection: &Connection, id: &str) -> StorageResult<()> {
    let affected = connection.execute("DELETE FROM framework_evaluations WHERE id = ?1", [id])?;
    if affected == 0 {
        return Err(StorageError::MissingFrameworkReference {
            table: "framework_evaluations".to_owned(),
            id: id.to_owned(),
        });
    }
    Ok(())
}

fn list_criterion_results(
    connection: &Connection,
    evaluation_id: &str,
) -> StorageResult<Vec<CriterionResult>> {
    let mut statement = connection.prepare(
        "SELECT id, evaluation_id, criterion_id, ordinal, label, expression, verdict,
                measured_value, measured_unit, threshold, inputs_json, note
         FROM criterion_results WHERE evaluation_id = ?1 ORDER BY ordinal, id",
    )?;
    let rows = statement.query_map([evaluation_id], criterion_result_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

// ============================================================================
// Metric discovery + context loading
// ============================================================================

pub(super) fn list_available_metric_keys(
    connection: &Connection,
    company_id: Option<&str>,
) -> StorageResult<Vec<MetricKeyInfo>> {
    let mut statement = connection.prepare(
        "SELECT metric_key, label, unit, value_kind, computation, scope, company_id
         FROM kpi_definitions
         WHERE scope IN ('canonical', 'user', 'sector')
            OR (scope = 'company' AND company_id = ?1)
         ORDER BY metric_key COLLATE NOCASE",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            MetricKeyInfo {
                key: row.get(0)?,
                label: row.get(1)?,
                unit: row.get(2)?,
                value_kind: row.get(3)?,
                computation: row.get(4)?,
                scope: row.get(5)?,
            },
        ))
    })?;

    // Dedup by metric_key, keeping the first (scope-preference handled by query order).
    let mut seen = HashMap::new();
    let mut keys = Vec::new();
    for row in rows {
        let (key, info) = row?;
        if seen.insert(key, ()).is_none() {
            keys.push(info);
        }
    }
    Ok(keys)
}

/// Build the metrics context for a company: all global + company-scoped
/// definitions and the company's confirmed-fact period series (newest first).
fn load_metrics_context(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<MetricsContext> {
    // Definitions (scope precedence: company first, then global buckets).
    let mut def_statement = connection.prepare(
        "SELECT metric_key, computation, formula, value_kind, unit, scope
         FROM kpi_definitions
         WHERE scope IN ('canonical', 'user', 'sector')
            OR (scope = 'company' AND company_id = ?1)
         ORDER BY CASE scope WHEN 'company' THEN 0 WHEN 'sector' THEN 1
                             WHEN 'user' THEN 2 ELSE 3 END",
    )?;
    let def_rows = def_statement.query_map([company_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    let mut definitions: HashMap<String, MetricDef> = HashMap::new();
    for row in def_rows {
        let (metric_key, computation, formula, value_kind, unit) = row?;
        definitions.entry(metric_key).or_insert_with(|| {
            let computation = if computation == "derived" {
                Computation::Derived
            } else {
                Computation::Reported
            };
            let formula = formula
                .filter(|f| !f.trim().is_empty())
                .and_then(|f| crate::fundamentals::metrics::parse_formula(&f));
            MetricDef {
                computation,
                formula,
                value_kind,
                unit,
            }
        });
    }

    // Periods newest-first.
    let mut period_statement = connection.prepare(
        "SELECT id, fiscal_year, period_type FROM financial_periods
         WHERE company_id = ?1 ORDER BY period_end_date DESC, fiscal_year DESC",
    )?;
    let period_meta = period_statement
        .query_map([company_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let mut periods = Vec::with_capacity(period_meta.len());
    for (period_id, fiscal_year, period_type) in period_meta {
        let reported = load_period_facts(connection, &period_id)?;
        periods.push(PeriodFacts {
            period_id,
            fiscal_year,
            is_annual: period_type == "FY",
            reported,
        });
    }

    Ok(MetricsContext::new(definitions, periods))
}

/// Load confirmed facts for one period as `metric_key -> value`, preferring the
/// canonical reporting variant when several exist.
fn load_period_facts(
    connection: &Connection,
    period_id: &str,
) -> StorageResult<HashMap<String, Decimal>> {
    let mut statement = connection.prepare(
        "SELECT d.metric_key, f.value_numeric
         FROM financial_facts f
         JOIN kpi_definitions d ON d.id = f.definition_id
         WHERE f.period_id = ?1 AND f.confirmation_state = 'confirmed'
         ORDER BY
            CASE f.data_quality WHEN 'final' THEN 0 ELSE 1 END,
            CASE f.variant WHEN 'reported' THEN 0 ELSE 1 END,
            CASE f.statement_basis WHEN 'consolidated' THEN 0 ELSE 1 END,
            CASE f.attribution WHEN 'total' THEN 0 WHEN 'owners_of_parent' THEN 1 ELSE 2 END",
    )?;
    let rows = statement.query_map([period_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut facts = HashMap::new();
    for row in rows {
        let (metric_key, value_text) = row?;
        if facts.contains_key(&metric_key) {
            continue;
        }
        if let Ok(value) = Decimal::from_str(value_text.trim()) {
            facts.insert(metric_key, value);
        }
    }
    Ok(facts)
}

// ============================================================================
// Helpers
// ============================================================================

/// Seed all app templates idempotently (called at startup, after migrations).
pub(super) fn seed_templates(connection: &Connection) -> StorageResult<()> {
    for template in templates::TEMPLATES {
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM quality_frameworks WHERE template_key = ?1)",
            [template.template_key],
            |row| row.get(0),
        )?;
        if exists {
            continue;
        }
        let id = format!("qframework_{}", slug_part(template.template_key));
        connection.execute(
            "INSERT OR IGNORE INTO quality_frameworks (id, name, description, origin, template_key, version)
             VALUES (?1, ?2, ?3, 'app_template', ?4, 1)",
            params![id, template.name, template.description, template.template_key],
        )?;
        seed_template_criteria(connection, &id, template)?;
    }
    Ok(())
}

fn seed_template_criteria(
    connection: &Connection,
    framework_id: &str,
    template: &templates::FrameworkTemplate,
) -> StorageResult<()> {
    for (index, criterion) in template.criteria.iter().enumerate() {
        insert_criterion(
            connection,
            framework_id,
            index as i64,
            criterion.label,
            criterion.expression,
            None,
            criterion.partial_band,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_criterion(
    connection: &Connection,
    framework_id: &str,
    ordinal: i64,
    label: &str,
    expression: &str,
    weight: Option<&str>,
    partial_band: Option<&str>,
) -> StorageResult<String> {
    let id = criterion_id(framework_id, label);
    let ast = cached_ast(expression);
    connection.execute(
        "INSERT INTO framework_criteria
            (id, framework_id, ordinal, label, expression, expression_ast, weight, partial_band)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            framework_id,
            ordinal,
            label,
            expression,
            ast,
            weight,
            partial_band
        ],
    )?;
    Ok(id)
}

fn next_ordinal(connection: &Connection, framework_id: &str) -> StorageResult<i64> {
    let max: Option<i64> = connection.query_row(
        "SELECT MAX(ordinal) FROM framework_criteria WHERE framework_id = ?1",
        [framework_id],
        |row| row.get(0),
    )?;
    Ok(max.map(|m| m + 1).unwrap_or(0))
}

fn bump_framework_version(connection: &Connection, framework_id: &str) -> StorageResult<()> {
    connection.execute(
        "UPDATE quality_frameworks
         SET version = version + 1, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        [framework_id],
    )?;
    Ok(())
}

fn ensure_company_exists(connection: &Connection, company_id: &str) -> StorageResult<()> {
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM companies WHERE id = ?1)",
        [company_id],
        |row| row.get(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::MissingFrameworkReference {
            table: "companies".to_owned(),
            id: company_id.to_owned(),
        })
    }
}

/// The cached AST JSON for a criterion expression (best-effort; `None` if unparseable).
fn cached_ast(expression: &str) -> Option<String> {
    expr::parse(expression)
        .ok()
        .and_then(|ast| serde_json::to_string(&ast).ok())
}

/// The metric keys a criterion references, as a JSON array for the audit trail.
fn inputs_json(expression: &str) -> Option<String> {
    let keys = expr::parse(expression)
        .ok()
        .map(|ast| referenced_metrics(&ast))
        .unwrap_or_default();
    serde_json::to_string(&keys).ok()
}

fn empty_to_none(value: Option<String>) -> Option<String> {
    value.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn framework_from_row(row: &Row<'_>) -> rusqlite::Result<QualityFramework> {
    Ok(QualityFramework {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        origin: row.get(3)?,
        template_key: row.get(4)?,
        cloned_from: row.get(5)?,
        version: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        criteria: Vec::new(),
    })
}

fn criterion_from_row(row: &Row<'_>) -> rusqlite::Result<FrameworkCriterion> {
    Ok(FrameworkCriterion {
        id: row.get(0)?,
        framework_id: row.get(1)?,
        ordinal: row.get(2)?,
        label: row.get(3)?,
        expression: row.get(4)?,
        weight: row.get(5)?,
        partial_band: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn evaluation_from_row(row: &Row<'_>) -> rusqlite::Result<FrameworkEvaluation> {
    Ok(FrameworkEvaluation {
        id: row.get(0)?,
        framework_id: row.get(1)?,
        framework_version: row.get(2)?,
        company_id: row.get(3)?,
        period_id: row.get(4)?,
        pass_count: row.get(5)?,
        partial_count: row.get(6)?,
        fail_count: row.get(7)?,
        unavailable_count: row.get(8)?,
        engine_version: row.get(9)?,
        created_at: row.get(10)?,
        results: Vec::new(),
    })
}

fn criterion_result_from_row(row: &Row<'_>) -> rusqlite::Result<CriterionResult> {
    Ok(CriterionResult {
        id: row.get(0)?,
        evaluation_id: row.get(1)?,
        criterion_id: row.get(2)?,
        ordinal: row.get(3)?,
        label: row.get(4)?,
        expression: row.get(5)?,
        verdict: row.get(6)?,
        measured_value: row.get(7)?,
        measured_unit: row.get(8)?,
        threshold: row.get(9)?,
        inputs_json: row.get(10)?,
        note: row.get(11)?,
    })
}

// ---- id generation ---------------------------------------------------------

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn unique_suffix() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}{:x}", millis & 0xFFFF_FFFF, counter & 0xFFFF)
}

fn framework_id(name: &str) -> String {
    format!("qframework_{}_{}", slug_part(name), unique_suffix())
}

fn criterion_id(framework_id: &str, label: &str) -> String {
    format!(
        "qcriterion_{}_{}_{}",
        slug_part(framework_id),
        slug_part(label),
        unique_suffix()
    )
}

fn evaluation_id(framework_id: &str, company_id: &str) -> String {
    format!(
        "qeval_{}_{}_{}",
        slug_part(framework_id),
        slug_part(company_id),
        unique_suffix()
    )
}

fn criterion_result_id(evaluation_id: &str, criterion_id: &str) -> String {
    format!(
        "qresult_{}_{}",
        slug_part(evaluation_id),
        slug_part(criterion_id)
    )
}
