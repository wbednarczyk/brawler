// Quality frameworks — quantitative checks (ADR 0046, v0.44.0).
// Mirrors the Rust DTOs in src-tauri/src/storage/quality_frameworks.rs.

export type CriterionVerdict = "pass" | "partial" | "fail" | "unavailable";

export type QualityFramework = {
  id: string;
  name: string;
  description: string | null;
  origin: "app_template" | "user";
  templateKey: string | null;
  clonedFrom: string | null;
  version: number;
  createdAt: string;
  updatedAt: string;
  criteria: FrameworkCriterion[];
};

export type FrameworkCriterion = {
  id: string;
  frameworkId: string;
  ordinal: number;
  label: string;
  expression: string;
  weight: string | null;
  partialBand: string | null;
  createdAt: string;
  updatedAt: string;
};

export type FrameworkEvaluation = {
  id: string;
  frameworkId: string;
  frameworkVersion: number;
  companyId: string;
  periodId: string | null;
  passCount: number;
  partialCount: number;
  failCount: number;
  unavailableCount: number;
  engineVersion: string;
  createdAt: string;
  results: CriterionResult[];
};

export type CriterionResult = {
  id: string;
  evaluationId: string;
  criterionId: string | null;
  ordinal: number;
  label: string;
  expression: string;
  verdict: CriterionVerdict;
  measuredValue: string | null;
  measuredUnit: string | null;
  threshold: string | null;
  inputsJson: string | null;
  note: string | null;
};

export type ValidateCriterionResult = {
  ok: boolean;
  error: string | null;
  referencedMetricKeys: string[];
};

export type MetricKeyInfo = {
  key: string;
  label: string;
  unit: string | null;
  valueKind: string;
  computation: string;
  scope: string;
};

export type NewQualityFramework = {
  name: string;
  description?: string | null;
};

export type UpdateQualityFramework = {
  id: string;
  name?: string | null;
  description?: string | null;
};

export type CloneFrameworkInput = {
  frameworkId: string;
  name?: string | null;
};

export type NewFrameworkCriterion = {
  frameworkId: string;
  label: string;
  expression: string;
  weight?: string | null;
  partialBand?: string | null;
  ordinal?: number | null;
};

export type UpdateFrameworkCriterion = {
  id: string;
  label?: string | null;
  expression?: string | null;
  weight?: string | null;
  partialBand?: string | null;
  ordinal?: number | null;
};

export type EvaluateFrameworkInput = {
  frameworkId: string;
  companyId: string;
};

export type ListFrameworkEvaluationsInput = {
  frameworkId: string;
  companyId: string;
};
