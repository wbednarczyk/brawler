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
  UpdateFrameworkCriterion,
  UpdateQualityFramework,
  ValidateCriterionResult,
} from "./qualityFrameworksTypes";

// ============================================================================
// Frameworks
// ============================================================================

export function listQualityFrameworks() {
  return callCommand<QualityFramework[]>("list_quality_frameworks");
}

export function getQualityFramework(id: string) {
  return callCommand<QualityFramework>("get_quality_framework", { id });
}

export function createQualityFramework(input: NewQualityFramework) {
  return callCommand<QualityFramework>("create_quality_framework", { input });
}

export function updateQualityFramework(input: UpdateQualityFramework) {
  return callCommand<QualityFramework>("update_quality_framework", { input });
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

export function updateFrameworkCriterion(input: UpdateFrameworkCriterion) {
  return callCommand<FrameworkCriterion>("update_framework_criterion", { input });
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

export function getFrameworkEvaluation(id: string) {
  return callCommand<FrameworkEvaluation>("get_framework_evaluation", { id });
}

export function deleteFrameworkEvaluation(id: string) {
  return callCommand<void>("delete_framework_evaluation", { id });
}

export function listAvailableMetricKeys(companyId?: string) {
  return callCommand<MetricKeyInfo[]>("list_available_metric_keys", {
    companyId: companyId ?? null,
  });
}
