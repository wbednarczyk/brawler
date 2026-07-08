// Quality frameworks — quantitative checks (ADR 0046, v0.44.0).
//
// GENERATED from the Rust DTOs in src-tauri/src/storage/quality_frameworks.rs
// via ts-rs (ADR 0048): change the struct and run `make types`. The
// `CriterionVerdict` union is generated from the marker enum in
// src-tauri/src/api_ts_unions.rs.

export type { CriterionVerdict } from "./generated/CriterionVerdict";

export type { QualityFramework } from "./generated/QualityFramework";
export type { FrameworkCriterion } from "./generated/FrameworkCriterion";
export type { FrameworkEvaluation } from "./generated/FrameworkEvaluation";
export type { CriterionResult } from "./generated/CriterionResult";
export type { ValidateCriterionResult } from "./generated/ValidateCriterionResult";
export type { MetricKeyInfo } from "./generated/MetricKeyInfo";

// Input DTOs
export type { NewQualityFramework } from "./generated/NewQualityFramework";
export type { UpdateQualityFramework } from "./generated/UpdateQualityFramework";
export type { CloneFrameworkInput } from "./generated/CloneFrameworkInput";
export type { NewFrameworkCriterion } from "./generated/NewFrameworkCriterion";
export type { UpdateFrameworkCriterion } from "./generated/UpdateFrameworkCriterion";
export type { EvaluateFrameworkInput } from "./generated/EvaluateFrameworkInput";
export type { ListFrameworkEvaluationsInput } from "./generated/ListFrameworkEvaluationsInput";

// Qualitative assessment (ADR 0075, v0.50.0)
export type { RunQualitativeAssessmentInput } from "./generated/RunQualitativeAssessmentInput";
export type { RerunQualitativeCriterionInput } from "./generated/RerunQualitativeCriterionInput";
export type { GetQualitativeAssessmentInput } from "./generated/GetQualitativeAssessmentInput";
export type { GetQualitativeAssessmentStatusInput } from "./generated/GetQualitativeAssessmentStatusInput";
export type { QualitativeAssessmentStatus } from "./generated/QualitativeAssessmentStatus";
