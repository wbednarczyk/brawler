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
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct QualityFramework {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[cfg_attr(feature = "ts-export", ts(type = "\"app_template\" | \"user\""))]
    pub origin: String,
    pub template_key: Option<String>,
    pub cloned_from: Option<String>,
    pub version: i64,
    pub created_at: String,
    pub updated_at: String,
    pub criteria: Vec<FrameworkCriterion>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct FrameworkCriterion {
    pub id: String,
    pub framework_id: String,
    pub ordinal: i64,
    pub label: String,
    pub expression: String,
    pub weight: Option<String>,
    pub partial_band: Option<String>,
    /// `quantitative` (DSL expression) | `qualitative` (agent-assessed, ADR 0075).
    #[cfg_attr(feature = "ts-export", ts(type = "\"quantitative\" | \"qualitative\""))]
    pub kind: String,
    /// Owner-authored prompt seed for a qualitative criterion; `None` for quantitative.
    pub assessment_guidance: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
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
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct CriterionResult {
    pub id: String,
    pub evaluation_id: String,
    pub criterion_id: Option<String>,
    pub ordinal: i64,
    pub label: String,
    pub expression: String,
    #[cfg_attr(
        feature = "ts-export",
        ts(as = "crate::api_ts_unions::CriterionVerdict")
    )]
    pub verdict: String,
    pub measured_value: Option<String>,
    pub measured_unit: Option<String>,
    pub threshold: Option<String>,
    pub inputs_json: Option<String>,
    pub note: Option<String>,
    // Agent-assessed fields (ADR 0075) — populated only for `source = agent` rows.
    /// Short agent rationale for a qualitative verdict.
    pub reasoning: Option<String>,
    /// JSON array of typed evidence refs (evidenceType/evidenceId/label/snippet).
    pub citations: Option<String>,
    /// Agent confidence: `low` | `medium` | `high`.
    pub confidence: Option<String>,
    /// Versioned prompt id that produced this assessment.
    pub prompt_version: Option<String>,
    /// `engine` (deterministic quantitative) | `agent` (qualitative assessment).
    #[cfg_attr(feature = "ts-export", ts(type = "\"engine\" | \"agent\""))]
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ValidateCriterionResult {
    pub ok: bool,
    pub error: Option<String>,
    pub referenced_metric_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
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
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewQualityFramework {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateQualityFramework {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct CloneFrameworkInput {
    pub framework_id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
#[serde(rename_all = "camelCase")]
pub struct NewFrameworkCriterion {
    pub framework_id: String,
    pub label: String,
    #[serde(default)]
    pub expression: String,
    #[serde(default)]
    pub weight: Option<String>,
    #[serde(default)]
    pub partial_band: Option<String>,
    #[serde(default)]
    pub ordinal: Option<i64>,
    /// `quantitative` (default) | `qualitative` (ADR 0075). Absent ⇒ quantitative.
    #[serde(default)]
    pub kind: Option<String>,
    /// Required (non-empty) for a qualitative criterion; ignored for quantitative.
    #[serde(default)]
    pub assessment_guidance: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(
        export,
        export_to = "../../src/api/generated/",
        optional_fields = nullable
    )
)]
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
    /// Switch a criterion's kind (ADR 0075); absent ⇒ keep existing.
    #[serde(default)]
    pub kind: Option<String>,
    /// Update the qualitative guidance seed; absent ⇒ keep existing.
    #[serde(default)]
    pub assessment_guidance: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct EvaluateFrameworkInput {
    pub framework_id: String,
    pub company_id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "ts-export", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "ts-export",
    ts(export, export_to = "../../src/api/generated/")
)]
#[serde(rename_all = "camelCase")]
pub struct ListFrameworkEvaluationsInput {
    pub framework_id: String,
    pub company_id: String,
}

/// A batch of agent-assessed qualitative results to persist as one immutable
/// snapshot (ADR 0075, T4). The snapshot may be qualitative-only (Decision 5
/// anticipates quant-only, qual-only, and combined snapshots).
#[derive(Debug, Clone)]
pub struct PersistQualitativeAssessmentInput {
    pub framework_id: String,
    pub company_id: String,
    pub results: Vec<QualitativeCriterionResult>,
}

/// One validated qualitative criterion result, ready to write with `source =
/// agent`. `citations_json` is the already-validated, serialized typed-evidence
/// array (the job resolves every citation to supplied evidence before this).
#[derive(Debug, Clone)]
pub struct QualitativeCriterionResult {
    pub criterion_id: String,
    pub ordinal: i64,
    pub label: String,
    /// `pass` | `partial` | `fail` | `insufficient_evidence`.
    pub verdict: String,
    pub reasoning: String,
    pub citations_json: String,
    /// `low` | `medium` | `high`.
    pub confidence: String,
    pub prompt_version: String,
}

/// A detected change in a qualitative criterion's agent verdict between its two
/// most-recent agent-assessed rows across snapshots (ADR 0075 Decision 5,
/// §T6 change detection). Consumed by the research digest and autopilot notify.
#[derive(Debug, Clone)]
pub struct QualitativeVerdictChange {
    pub criterion_id: String,
    pub label: String,
    /// The second-most-recent agent verdict for this criterion.
    pub previous_verdict: String,
    /// The most-recent agent verdict for this criterion.
    pub current_verdict: String,
    /// `created_at` of the snapshot carrying the current (most-recent) verdict.
    pub changed_at: String,
}

/// A framework that has at least one agent-assessed (qualitative) result for a
/// company — the enumeration seam for company-scoped verdict-change detection
/// (digest, §T6c) and autopilot re-enqueue (§T6d). No explicit framework↔company
/// assignment model exists; prior assessment is the bounded, meaningful source.
#[derive(Debug, Clone)]
pub struct AssessedFrameworkRef {
    pub framework_id: String,
    pub framework_name: String,
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
            &criterion.kind,
            criterion.assessment_guidance.as_deref(),
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

    let locale = super::settings::seed_locale(connection)?;
    connection.execute(
        "DELETE FROM framework_criteria WHERE framework_id = ?1",
        [id],
    )?;
    seed_template_criteria(connection, id, template, &locale)?;
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
        "SELECT id, framework_id, ordinal, label, expression, weight, partial_band, created_at, updated_at, kind, assessment_guidance
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
    let kind = normalize_kind(input.kind.as_deref())?;
    // A quantitative criterion validates its DSL predicate; a qualitative one
    // (ADR 0075) carries owner guidance and no expression.
    let (expression, guidance) = match kind {
        "qualitative" => (
            String::new(),
            Some(require_guidance(input.assessment_guidance.clone())?),
        ),
        _ => {
            let expression = input.expression.trim().to_owned();
            validate_predicate(&expression)?;
            (expression, None)
        }
    };
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
        kind,
        guidance.as_deref(),
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
        None => existing.label.clone(),
    };
    // Resolve the effective kind (input override, else the existing kind), then
    // apply kind-specific rules (ADR 0075).
    let kind = match input.kind.as_deref() {
        Some(_) => normalize_kind(input.kind.as_deref())?.to_owned(),
        None => existing.kind.clone(),
    };
    let (expression, guidance) = if kind == "qualitative" {
        // No DSL expression; guidance from the input, else the existing value —
        // but the effective value must be non-empty.
        let raw = match input.assessment_guidance.clone() {
            Some(g) => Some(g),
            None => existing.assessment_guidance.clone(),
        };
        (String::new(), Some(require_guidance(raw)?))
    } else {
        // Validate the *effective* expression, not just an explicitly-supplied one:
        // a qualitative→quantitative switch (or a blank kind that normalizes to
        // quantitative) must not persist the empty expression a qualitative row
        // carries. Re-validating an unchanged, already-valid expression is idempotent.
        let expression = match input.expression {
            Some(e) => e.trim().to_owned(),
            None => existing.expression.clone(),
        };
        validate_predicate(&expression)?;
        (expression, None)
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
             ordinal = ?7, kind = ?8, assessment_guidance = ?9,
             updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
         WHERE id = ?1",
        params![
            input.id,
            label,
            expression,
            ast,
            weight,
            partial_band,
            ordinal,
            kind,
            guidance
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
            "SELECT id, framework_id, ordinal, label, expression, weight, partial_band, created_at, updated_at, kind, assessment_guidance
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
    // Skip qualitative criteria (ADR 0075): they carry no DSL expression and are
    // agent-assessed on a separate snapshot. Scoring them here would write a
    // phantom `unavailable` row into the quantitative scorecard.
    for criterion in framework
        .criteria
        .iter()
        .filter(|c| c.kind != "qualitative")
    {
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

/// Persist a batch of agent-assessed qualitative results as one immutable
/// snapshot with `source = agent` (ADR 0075, T4). Company-level, not
/// period-scoped, so `period_id` stays NULL; `engine_version` pins the shared
/// snapshot schema version (the agent provenance lives per-criterion in
/// `prompt_version`). `insufficient_evidence` counts into the "could not
/// determine" bucket (`unavailable_count`), mirroring the quantitative
/// `unavailable` verdict. Never touches quantitative rows or facts.
pub(super) fn persist_qualitative_assessment(
    connection: &Connection,
    input: PersistQualitativeAssessmentInput,
) -> StorageResult<FrameworkEvaluation> {
    let framework = get_quality_framework(connection, &input.framework_id)?;
    ensure_company_exists(connection, &input.company_id)?;

    let mut pass = 0i64;
    let mut partial = 0i64;
    let mut fail = 0i64;
    let mut unavailable = 0i64;
    for result in &input.results {
        match result.verdict.as_str() {
            "pass" => pass += 1,
            "partial" => partial += 1,
            "fail" => fail += 1,
            // insufficient_evidence (and any non-quantitative verdict) → could-not-determine.
            _ => unavailable += 1,
        }
    }

    let evaluation_id = evaluation_id(&framework.id, &input.company_id);
    connection.execute(
        "INSERT INTO framework_evaluations
            (id, framework_id, framework_version, company_id, period_id,
             pass_count, partial_count, fail_count, unavailable_count, engine_version)
         VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?7, ?8, ?9)",
        params![
            evaluation_id,
            framework.id,
            framework.version,
            input.company_id,
            pass,
            partial,
            fail,
            unavailable,
            ENGINE_VERSION,
        ],
    )?;

    for result in &input.results {
        connection.execute(
            "INSERT INTO criterion_results
                (id, evaluation_id, criterion_id, ordinal, label, expression, verdict,
                 measured_value, measured_unit, threshold, inputs_json, note,
                 reasoning, citations, confidence, prompt_version, source)
             VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, NULL, NULL, NULL, NULL, NULL,
                     ?7, ?8, ?9, ?10, 'agent')",
            params![
                criterion_result_id(&evaluation_id, &result.criterion_id),
                evaluation_id,
                result.criterion_id,
                result.ordinal,
                result.label,
                result.verdict,
                result.reasoning,
                result.citations_json,
                result.confidence,
                result.prompt_version,
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

/// Current-state qualitative read (ADR 0075 Decision 5, "two read surfaces"):
/// per criterion, the most-recent agent-assessed (`source = agent`) row across
/// ALL snapshots for this framework × company. A later quant-only snapshot has
/// no agent rows and so never blanks an assessment; a later single-criterion
/// re-run only replaces that one criterion's row. A never-assessed criterion is
/// absent (empty state — distinct from an `insufficient_evidence` verdict).
pub(super) fn get_qualitative_assessment(
    connection: &Connection,
    framework_id: &str,
    company_id: &str,
) -> StorageResult<Vec<CriterionResult>> {
    // "Most recent per criterion" tie-breaks on `framework_evaluations.rowid`
    // (monotonic insertion order) when two snapshots share a `created_at`
    // millisecond — the evaluation id's hex suffix is not lexically ordered.
    let mut statement = connection.prepare(
        "SELECT cr.id, cr.evaluation_id, cr.criterion_id, cr.ordinal, cr.label, cr.expression,
                cr.verdict, cr.measured_value, cr.measured_unit, cr.threshold, cr.inputs_json,
                cr.note, cr.reasoning, cr.citations, cr.confidence, cr.prompt_version, cr.source
         FROM criterion_results cr
         JOIN framework_evaluations fe ON fe.id = cr.evaluation_id
         WHERE fe.framework_id = ?1 AND fe.company_id = ?2
           AND cr.source = 'agent' AND cr.criterion_id IS NOT NULL
           AND NOT EXISTS (
             SELECT 1 FROM criterion_results cr2
             JOIN framework_evaluations fe2 ON fe2.id = cr2.evaluation_id
             WHERE fe2.framework_id = ?1 AND fe2.company_id = ?2
               AND cr2.source = 'agent' AND cr2.criterion_id = cr.criterion_id
               AND (fe2.created_at > fe.created_at
                    OR (fe2.created_at = fe.created_at AND fe2.rowid > fe.rowid))
           )
         ORDER BY cr.ordinal, cr.id",
    )?;
    let rows = statement.query_map(params![framework_id, company_id], criterion_result_from_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Per qualitative criterion, report a change when its most-recent agent verdict
/// differs from its second-most-recent (ADR 0075 Decision 5, §T6). A criterion
/// with a single agent assessment (no previous) or an unchanged verdict yields
/// no entry. Ordering by criterion ordinal keeps digest output stable.
pub(super) fn qualitative_verdict_changes(
    connection: &Connection,
    framework_id: &str,
    company_id: &str,
) -> StorageResult<Vec<QualitativeVerdictChange>> {
    // Rank agent rows per criterion by recency (created_at, then rowid to break a
    // same-millisecond tie — the same tie-break the current-state read uses).
    let mut statement = connection.prepare(
        "WITH ranked AS (
             SELECT cr.criterion_id AS criterion_id,
                    cr.label AS label,
                    cr.verdict AS verdict,
                    cr.ordinal AS ordinal,
                    fe.created_at AS created_at,
                    ROW_NUMBER() OVER (
                        PARTITION BY cr.criterion_id
                        ORDER BY fe.created_at DESC, fe.rowid DESC
                    ) AS rn
             FROM criterion_results cr
             JOIN framework_evaluations fe ON fe.id = cr.evaluation_id
             WHERE fe.framework_id = ?1 AND fe.company_id = ?2
               AND cr.source = 'agent' AND cr.criterion_id IS NOT NULL
         )
         SELECT cur.criterion_id, cur.label, prev.verdict, cur.verdict, cur.created_at
         FROM ranked cur
         JOIN ranked prev ON prev.criterion_id = cur.criterion_id AND prev.rn = 2
         WHERE cur.rn = 1 AND cur.verdict <> prev.verdict
         ORDER BY cur.ordinal, cur.criterion_id",
    )?;
    let rows = statement.query_map(params![framework_id, company_id], |row| {
        Ok(QualitativeVerdictChange {
            criterion_id: row.get(0)?,
            label: row.get(1)?,
            previous_verdict: row.get(2)?,
            current_verdict: row.get(3)?,
            changed_at: row.get(4)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
}

/// Frameworks that carry at least one agent-assessed result for a company
/// (§T6c/§T6d enumeration seam). Ordered by name for stable digest output.
pub(super) fn frameworks_with_qualitative_assessments(
    connection: &Connection,
    company_id: &str,
) -> StorageResult<Vec<AssessedFrameworkRef>> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT qf.id, qf.name
         FROM framework_evaluations fe
         JOIN criterion_results cr ON cr.evaluation_id = fe.id
         JOIN quality_frameworks qf ON qf.id = fe.framework_id
         WHERE fe.company_id = ?1 AND cr.source = 'agent'
         ORDER BY qf.name COLLATE NOCASE, qf.id",
    )?;
    let rows = statement.query_map([company_id], |row| {
        Ok(AssessedFrameworkRef {
            framework_id: row.get(0)?,
            framework_name: row.get(1)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StorageError::from)
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
                measured_value, measured_unit, threshold, inputs_json, note,
                reasoning, citations, confidence, prompt_version, source
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
///
/// Bilingual seeds (ADR 0076 Decision 8): the language is resolved once from the
/// persisted app-locale setting. A template with no framework yet is inserted
/// with localized name/description + all criteria. A template whose framework
/// already exists is **topped up**, not re-inserted: an `app_template`-origin
/// framework still at `version == 1` (never user-edited — every edit bumps the
/// version) receives any template criteria it is missing, additively, and has its
/// human-facing strings **re-localized** to the current locale (baba638) — a
/// framework seeded before the bilingual pass (ADR 0076 Decision 8) otherwise
/// keeps its seed-time locale's text forever. Edited (`version > 1`) or
/// user-created frameworks are left untouched (ADR 0046 no-overwrite rule). This
/// closes the "new template criteria invisible without a destructive reset" gap
/// for installs created before the criteria existed.
pub(super) fn seed_templates(connection: &Connection) -> StorageResult<()> {
    let locale = super::settings::seed_locale(connection)?;
    for template in templates::TEMPLATES {
        let existing: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT id, origin, version FROM quality_frameworks WHERE template_key = ?1",
                [template.template_key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        match existing {
            None => {
                let id = format!("qframework_{}", slug_part(template.template_key));
                connection.execute(
                    "INSERT OR IGNORE INTO quality_frameworks (id, name, description, origin, template_key, version)
                     VALUES (?1, ?2, ?3, 'app_template', ?4, 1)",
                    params![
                        id,
                        template.name.get(&locale),
                        template.description.get(&locale),
                        template.template_key
                    ],
                )?;
                seed_template_criteria(connection, &id, template, &locale)?;
            }
            // An untouched app template (origin app_template, version == 1) tops
            // up any missing criteria; anything the user edited or created is left
            // alone. Top-up is not a user edit, so it must NOT bump the version —
            // idempotency comes from matching on the criterion's stable index.
            Some((id, origin, version)) if origin == "app_template" && version == 1 => {
                top_up_template_criteria(connection, &id, template, &locale)?;
                relocalize_template_framework(connection, &id, template, &locale)?;
            }
            Some(_) => {}
        }
    }
    Ok(())
}

fn seed_template_criteria(
    connection: &Connection,
    framework_id: &str,
    template: &templates::FrameworkTemplate,
    locale: &str,
) -> StorageResult<()> {
    for (index, criterion) in template.criteria.iter().enumerate() {
        // A template criterion carries its own kind + guidance (ADR 0075):
        // quantitative criteria have a DSL expression; qualitative ones have an
        // empty expression and an owner-authored guidance seed. Only the label
        // and guidance are localized (ADR 0076 Decision 8).
        insert_criterion(
            connection,
            framework_id,
            index as i64,
            criterion.label.get(locale),
            criterion.expression,
            None,
            criterion.partial_band,
            criterion.kind,
            criterion
                .assessment_guidance
                .as_ref()
                .map(|g| g.get(locale)),
        )?;
    }
    Ok(())
}

/// Additively add any template criteria missing from an untouched (version == 1)
/// `app_template` framework (ADR 0076 Decision 8). Matching is by the template
/// criterion's **stable index** in the constant — which the seed writes as the
/// `ordinal` — not by label, so a re-localized label never causes a duplicate.
/// Existing rows are never modified or deleted; running again adds nothing.
fn top_up_template_criteria(
    connection: &Connection,
    framework_id: &str,
    template: &templates::FrameworkTemplate,
    locale: &str,
) -> StorageResult<()> {
    let mut statement =
        connection.prepare("SELECT ordinal FROM framework_criteria WHERE framework_id = ?1")?;
    let present: std::collections::HashSet<i64> = statement
        .query_map([framework_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<_, _>>()?;

    for (index, criterion) in template.criteria.iter().enumerate() {
        let ordinal = index as i64;
        if present.contains(&ordinal) {
            continue;
        }
        insert_criterion(
            connection,
            framework_id,
            ordinal,
            criterion.label.get(locale),
            criterion.expression,
            None,
            criterion.partial_band,
            criterion.kind,
            criterion
                .assessment_guidance
                .as_ref()
                .map(|g| g.get(locale)),
        )?;
    }
    Ok(())
}

/// Re-localize an untouched (`app_template`, `version == 1`) template framework's
/// human-facing strings to the current locale (baba638). A framework seeded before
/// the bilingual pass (ADR 0076 Decision 8) keeps its seed-time locale's text;
/// top-up only *adds* missing criteria, never re-localizes existing rows, so a
/// locale switch never reaches the stale strings.
///
/// A field is rewritten only when its current value **exactly matches the shipped
/// template text in some locale** (`IN (pl, en)`) — proof it is untouched template
/// text, never a user-authored string. Any user edit also bumps the version, which
/// excludes the framework from this branch entirely; the field-level match is a
/// belt-and-suspenders guard on top of that. Criteria are matched by the template
/// criterion's stable index (written as `ordinal`), like top-up, so a re-localized
/// label never mismatches. Re-localization is not a user edit, so it must NOT bump
/// the version. Idempotent: once a field already reads the current locale it stays
/// in the match set and is rewritten to the same value.
fn relocalize_template_framework(
    connection: &Connection,
    framework_id: &str,
    template: &templates::FrameworkTemplate,
    locale: &str,
) -> StorageResult<()> {
    // Framework name + description.
    connection.execute(
        "UPDATE quality_frameworks SET name = ?1 WHERE id = ?2 AND name IN (?3, ?4)",
        params![
            template.name.get(locale),
            framework_id,
            template.name.pl,
            template.name.en
        ],
    )?;
    connection.execute(
        "UPDATE quality_frameworks SET description = ?1 WHERE id = ?2 AND description IN (?3, ?4)",
        params![
            template.description.get(locale),
            framework_id,
            template.description.pl,
            template.description.en
        ],
    )?;

    // Criterion label + guidance, matched by the template's stable index.
    for (index, criterion) in template.criteria.iter().enumerate() {
        let ordinal = index as i64;
        connection.execute(
            "UPDATE framework_criteria SET label = ?1
             WHERE framework_id = ?2 AND ordinal = ?3 AND label IN (?4, ?5)",
            params![
                criterion.label.get(locale),
                framework_id,
                ordinal,
                criterion.label.pl,
                criterion.label.en
            ],
        )?;
        if let Some(guidance) = &criterion.assessment_guidance {
            connection.execute(
                "UPDATE framework_criteria SET assessment_guidance = ?1
                 WHERE framework_id = ?2 AND ordinal = ?3 AND assessment_guidance IN (?4, ?5)",
                params![
                    guidance.get(locale),
                    framework_id,
                    ordinal,
                    guidance.pl,
                    guidance.en
                ],
            )?;
        }
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
    kind: &str,
    assessment_guidance: Option<&str>,
) -> StorageResult<String> {
    let id = criterion_id(framework_id, label);
    let ast = cached_ast(expression);
    connection.execute(
        "INSERT INTO framework_criteria
            (id, framework_id, ordinal, label, expression, expression_ast, weight, partial_band,
             kind, assessment_guidance)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            id,
            framework_id,
            ordinal,
            label,
            expression,
            ast,
            weight,
            partial_band,
            kind,
            assessment_guidance
        ],
    )?;
    Ok(id)
}

/// Normalize a criterion `kind` input: absent/blank ⇒ `quantitative`; an
/// unrecognized value is rejected (ADR 0075).
pub(crate) fn normalize_kind(kind: Option<&str>) -> StorageResult<&'static str> {
    match kind.map(str::trim).filter(|s| !s.is_empty()) {
        None | Some("quantitative") => Ok("quantitative"),
        Some("qualitative") => Ok("qualitative"),
        Some(other) => Err(StorageError::InvalidFrameworkValue {
            key: "kind",
            value: other.to_owned(),
        }),
    }
}

/// A qualitative criterion must carry a non-empty guidance seed (ADR 0075).
pub(crate) fn require_guidance(guidance: Option<String>) -> StorageResult<String> {
    empty_to_none(guidance).ok_or(StorageError::InvalidFrameworkValue {
        key: "assessment_guidance",
        value: String::new(),
    })
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
        // Appended columns (migration 0059). Safe-default read (ADR 0075): a
        // missing/NULL kind is quantitative.
        kind: row
            .get::<_, Option<String>>(9)?
            .unwrap_or_else(|| "quantitative".to_owned()),
        assessment_guidance: row.get(10)?,
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
        // Appended columns (migration 0059, ADR 0075).
        reasoning: row.get(12)?,
        citations: row.get(13)?,
        confidence: row.get(14)?,
        prompt_version: row.get(15)?,
        // Safe-default read: a missing/NULL source is the deterministic engine.
        source: row
            .get::<_, Option<String>>(16)?
            .unwrap_or_else(|| "engine".to_owned()),
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

use super::database::Database;
/// quality_frameworks domain store (Architecture v2 / ADR 0050). Owns a [`Database`] and
/// exposes only this domain's operations. Reach it via `AppState::quality_frameworks()`.
#[derive(Clone)]
pub struct QualityFrameworkStore {
    db: Database,
}

impl QualityFrameworkStore {
    pub(super) fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_quality_frameworks(&self) -> StorageResult<Vec<QualityFramework>> {
        let connection = self.db.checkout()?;
        list_quality_frameworks(&connection)
    }

    pub fn get_quality_framework(&self, id: &str) -> StorageResult<QualityFramework> {
        let connection = self.db.checkout()?;
        get_quality_framework(&connection, id)
    }

    pub fn create_quality_framework(
        &self,
        input: NewQualityFramework,
    ) -> StorageResult<QualityFramework> {
        let connection = self.db.checkout()?;
        create_quality_framework(&connection, input)
    }

    pub fn update_quality_framework(
        &self,
        input: UpdateQualityFramework,
    ) -> StorageResult<QualityFramework> {
        let connection = self.db.checkout()?;
        update_quality_framework(&connection, input)
    }

    pub fn delete_quality_framework(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_quality_framework(&connection, id)
    }

    pub fn clone_framework(&self, input: CloneFrameworkInput) -> StorageResult<QualityFramework> {
        let connection = self.db.checkout()?;
        clone_framework(&connection, input)
    }

    pub fn reset_framework_to_template(&self, id: &str) -> StorageResult<QualityFramework> {
        let connection = self.db.checkout()?;
        reset_framework_to_template(&connection, id)
    }

    pub fn create_framework_criterion(
        &self,
        input: NewFrameworkCriterion,
    ) -> StorageResult<FrameworkCriterion> {
        let connection = self.db.checkout()?;
        create_framework_criterion(&connection, input)
    }

    pub fn update_framework_criterion(
        &self,
        input: UpdateFrameworkCriterion,
    ) -> StorageResult<FrameworkCriterion> {
        let connection = self.db.checkout()?;
        update_framework_criterion(&connection, input)
    }

    pub fn delete_framework_criterion(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_framework_criterion(&connection, id)
    }

    pub fn evaluate_framework(
        &self,
        input: EvaluateFrameworkInput,
    ) -> StorageResult<FrameworkEvaluation> {
        let connection = self.db.checkout()?;
        evaluate_framework(&connection, input)
    }

    /// Build the derived-metrics context for a company (definitions +
    /// confirmed period facts), for callers outside the rule engine — e.g.
    /// the v0.53 price-context read model, which attaches a `QuoteFacts`
    /// snapshot on top via `MetricsContext::with_quotes` and resolves the
    /// level-0 market ratios seeded by migration 0072.
    pub fn metrics_context(&self, company_id: &str) -> StorageResult<MetricsContext> {
        let connection = self.db.checkout()?;
        load_metrics_context(&connection, company_id)
    }

    pub fn persist_qualitative_assessment(
        &self,
        input: PersistQualitativeAssessmentInput,
    ) -> StorageResult<FrameworkEvaluation> {
        let connection = self.db.checkout()?;
        persist_qualitative_assessment(&connection, input)
    }

    pub fn list_framework_evaluations(
        &self,
        input: ListFrameworkEvaluationsInput,
    ) -> StorageResult<Vec<FrameworkEvaluation>> {
        let connection = self.db.checkout()?;
        list_framework_evaluations(&connection, input)
    }

    pub fn get_framework_evaluation(&self, id: &str) -> StorageResult<FrameworkEvaluation> {
        let connection = self.db.checkout()?;
        get_framework_evaluation(&connection, id)
    }

    pub fn get_qualitative_assessment(
        &self,
        framework_id: &str,
        company_id: &str,
    ) -> StorageResult<Vec<CriterionResult>> {
        let connection = self.db.checkout()?;
        get_qualitative_assessment(&connection, framework_id, company_id)
    }

    pub fn qualitative_verdict_changes(
        &self,
        framework_id: &str,
        company_id: &str,
    ) -> StorageResult<Vec<QualitativeVerdictChange>> {
        let connection = self.db.checkout()?;
        qualitative_verdict_changes(&connection, framework_id, company_id)
    }

    pub fn frameworks_with_qualitative_assessments(
        &self,
        company_id: &str,
    ) -> StorageResult<Vec<AssessedFrameworkRef>> {
        let connection = self.db.checkout()?;
        frameworks_with_qualitative_assessments(&connection, company_id)
    }

    pub fn delete_framework_evaluation(&self, id: &str) -> StorageResult<()> {
        let connection = self.db.checkout()?;
        delete_framework_evaluation(&connection, id)
    }

    pub fn list_available_metric_keys(
        &self,
        company_id: Option<&str>,
    ) -> StorageResult<Vec<MetricKeyInfo>> {
        let connection = self.db.checkout()?;
        list_available_metric_keys(&connection, company_id)
    }
}
