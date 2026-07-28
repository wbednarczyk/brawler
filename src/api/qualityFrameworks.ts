import { callCommand } from "./tauri";
import type {
  CloneFrameworkInput,
  EvaluateFrameworkInput,
  FrameworkCriterion,
  FrameworkEvaluation,
  ListFrameworkEvaluationsInput,
  MetricKeyInfo,
  NewFrameworkCriterion,
  NewQualityFramework,
  QualityFramework,
  ValidateCriterionResult,
} from "./qualityFrameworksTypes";

// ============================================================================
// Frameworks
// ============================================================================

export function listQualityFrameworks() {
  return callCommand<QualityFramework[]>("list_quality_frameworks");
}

export function createQualityFramework(input: NewQualityFramework) {
  return callCommand<QualityFramework>("create_quality_framework", { input });
}

export function deleteQualityFramework(id: string) {
  return callCommand<void>("delete_quality_framework", { id });
}

export function cloneFramework(input: CloneFrameworkInput) {
  return callCommand<QualityFramework>("clone_framework", { input });
}

export function resetFrameworkToTemplate(id: string) {
  return callCommand<QualityFramework>("reset_framework_to_template", { id });
}

// ============================================================================
// Criteria
// ============================================================================

export function createFrameworkCriterion(input: NewFrameworkCriterion) {
  return callCommand<FrameworkCriterion>("create_framework_criterion", { input });
}

export function deleteFrameworkCriterion(id: string) {
  return callCommand<void>("delete_framework_criterion", { id });
}

export function validateCriterionExpression(expression: string) {
  return callCommand<ValidateCriterionResult>("validate_criterion_expression", { expression });
}

// ============================================================================
// Evaluation + discovery
// ============================================================================

export function evaluateFramework(input: EvaluateFrameworkInput) {
  return callCommand<FrameworkEvaluation>("evaluate_framework", { input });
}

export function listFrameworkEvaluations(input: ListFrameworkEvaluationsInput) {
  return callCommand<FrameworkEvaluation[]>("list_framework_evaluations", { input });
}

export function deleteFrameworkEvaluation(id: string) {
  return callCommand<void>("delete_framework_evaluation", { id });
}

export function listAvailableMetricKeys(companyId?: string) {
  return callCommand<MetricKeyInfo[]>("list_available_metric_keys", {
    companyId: companyId ?? null,
  });
}

// ============================================================================
// Qualitative criteria (ADR 0075, amended by ADR 0084)
// ============================================================================
//
// Qualitative criteria remain user-authored (label + assessment guidance) and
// are managed through the criterion CRUD above. The in-app AI assessor is
// retired and its stored verdicts were dropped with their table (ADR 0084
// decision 5), so there is no `get_qualitative_assessment` read any more —
// verdicts arrive as agent writes over MCP (`set_qualitative_verdicts`, shipped
// v0.60, ADR 0088).
