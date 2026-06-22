import { callCommand } from "./tauri";
import type { ExtractReportSectionsResult } from "./generated/ExtractReportSectionsResult";
import type { FetchReportDocumentResult } from "./generated/FetchReportDocumentResult";
import type { ReportDiffCandidates } from "./generated/ReportDiffCandidates";
import type { ReportDiffResult } from "./generated/ReportDiffResult";

// GENERATED types from src-tauri/src/commands/report_diff.rs + report_diff module
// via ts-rs (ADR 0048/0052). Report-over-report diff (v0.47.0).
export type { ExtractReportSectionsResult } from "./generated/ExtractReportSectionsResult";
export type { ReportDiffCandidate } from "./generated/ReportDiffCandidate";
export type { ReportDiffCandidates } from "./generated/ReportDiffCandidates";
export type { ReportDiffDocumentRef } from "./generated/ReportDiffDocumentRef";
export type { ReportDiffResult } from "./generated/ReportDiffResult";
export type { SectionDiff } from "./generated/SectionDiff";
export type { SectionDiffStatus } from "./generated/SectionDiffStatus";
export type { SectionsDiff } from "./generated/SectionsDiff";

/** Download a pending report document's file on demand (idempotent if already fetched). */
export function fetchReportDocument(reportDocumentId: string) {
  return callCommand<FetchReportDocumentResult>("fetch_report_document", {
    input: { reportDocumentId },
  });
}

/** Extract (or refresh) the persisted section index for one report document. */
export function extractReportSections(reportDocumentId: string) {
  return callCommand<ExtractReportSectionsResult>("extract_report_sections", {
    input: { reportDocumentId },
  });
}

/** Consecutive same-type financial-statement pairs available to diff for a company. */
export function listReportDiffCandidates(companyId: string) {
  return callCommand<ReportDiffCandidates>("list_report_diff_candidates", {
    input: { companyId },
  });
}

/** The on-demand section diff between two consecutive same-type statements. */
export function getReportDiff(
  olderReportDocumentId: string,
  newerReportDocumentId: string,
) {
  return callCommand<ReportDiffResult>("get_report_diff", {
    input: { olderReportDocumentId, newerReportDocumentId },
  });
}
