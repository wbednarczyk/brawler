import { callCommand } from "./tauri";

// Types GENERATED from src-tauri/src/storage/report_expectations.rs via ts-rs
// (ADR 0071/0048): change the struct and run `make types`.
import type { ReportExpectation } from "./generated/ReportExpectation";
import type { NewReportExpectation } from "./generated/NewReportExpectation";
import type { UpdateReportExpectation } from "./generated/UpdateReportExpectation";
import type { ListReportExpectationsInput } from "./generated/ListReportExpectationsInput";
import type { ExpectationReview } from "./generated/ExpectationReview";
import type { RecordExpectationResolutionInput } from "./generated/RecordExpectationResolutionInput";

export type { ReportExpectation } from "./generated/ReportExpectation";
export type { ExpectationMetric } from "./generated/ExpectationMetric";
export type { NewReportExpectation } from "./generated/NewReportExpectation";
export type { NewExpectationMetric } from "./generated/NewExpectationMetric";
export type { ExpectationReview } from "./generated/ExpectationReview";
export type { MetricExpectationReview } from "./generated/MetricExpectationReview";
export type { MetricExpectationOutcome } from "./generated/MetricExpectationOutcome";

/** Record a new pre-report expectation for an occurrence (ADR 0071). */
export function createReportExpectation(input: NewReportExpectation) {
  return callCommand<ReportExpectation>("create_report_expectation", { input });
}

/**
 * Edit an unfrozen expectation. Once the period's facts land the backend rejects
 * with the `conflict` {@link import("./tauri").CommandErrorCode} — the caller
 * flips to the read-only frozen state.
 */
export function updateReportExpectation(input: UpdateReportExpectation) {
  return callCommand<ReportExpectation>("update_report_expectation", { input });
}

export function listReportExpectations(input: ListReportExpectationsInput) {
  return callCommand<ReportExpectation[]>("list_report_expectations", { input });
}

/** Expectation-vs-actual read model, composed on read (no stored projection). */
export function expectationReview(input: { companyId: string; eventKey: string }) {
  return callCommand<ExpectationReview>("expectation_review", { input });
}

/** The user's own verdict at review time — recordable after the freeze. */
export function recordExpectationResolution(input: RecordExpectationResolutionInput) {
  return callCommand<ReportExpectation>("record_expectation_resolution", { input });
}
