import { callCommand } from "./tauri";

// Types GENERATED from src-tauri/src/storage/report_season.rs via ts-rs
// (ADR 0048): change the struct and run `make types`. ReportPreparationStatus
// comes from the marker enum in src-tauri/src/api_ts_unions.rs.
import type { ReportSeasonResult } from "./generated/ReportSeasonResult";
import type { PreReportCard } from "./generated/PreReportCard";
import type { ReportPreparation } from "./generated/ReportPreparation";
import type { ListReportSeasonInput } from "./generated/ListReportSeasonInput";

export type { ReportPreparationStatus } from "./generated/ReportPreparationStatus";
export type { ReportSeasonEntry } from "./generated/ReportSeasonEntry";
export type { CalendarFreshness } from "./generated/CalendarFreshness";
export type { ReportSeasonResult } from "./generated/ReportSeasonResult";
export type { PreReportKpi } from "./generated/PreReportKpi";
export type { PreReportCard } from "./generated/PreReportCard";
export type { ReportPreparation } from "./generated/ReportPreparation";
export type { ListReportSeasonInput } from "./generated/ListReportSeasonInput";

export function listReportSeason(input: ListReportSeasonInput) {
  return callCommand<ReportSeasonResult>("list_report_season", { input });
}

export function getPreReportCard(input: { companyId: string; eventKey: string }) {
  return callCommand<PreReportCard>("get_pre_report_card", { input });
}

export function markReportPrepared(input: { companyId: string; eventKey: string }) {
  return callCommand<ReportPreparation>("mark_report_prepared", { input });
}

export function markReportProcessed(input: {
  companyId: string;
  eventKey: string;
  linkedReportDocumentId: string | null;
}) {
  return callCommand<ReportPreparation>("mark_report_processed", { input });
}
