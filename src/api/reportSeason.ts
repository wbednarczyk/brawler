import { callCommand } from "./tauri";
import type { ClaimsToVerify } from "./managementClaims";
import type { ResearchEvidenceItem, ResearchQuestion } from "./researchTypes";

export type ReportPreparationStatus = "upcoming" | "prepared" | "processed";

export type ReportSeasonEntry = {
  companyId: string;
  qualifiedTicker: string;
  displayName: string;
  eventKey: string;
  eventDate: string;
  eventTime: string | null;
  title: string;
  preparationStatus: ReportPreparationStatus;
};

export type CalendarFreshness = {
  lastFetchedAt: string | null;
  stale: boolean;
};

export type ReportSeasonResult = {
  upcoming: ReportSeasonEntry[];
  past: ReportSeasonEntry[];
  calendarFreshness: CalendarFreshness;
};

export type PreReportKpi = {
  periodId: string;
  metricKey: string;
  label: string;
  unit: string | null;
  valueNumeric: string;
};

export type PreReportCard = {
  companyId: string;
  eventKey: string;
  eventDate: string | null;
  preparationStatus: ReportPreparationStatus;
  linkedReportDocumentId: string | null;
  openQuestions: ResearchQuestion[];
  unresolvedClaims: ClaimsToVerify;
  lastPeriodKpis: PreReportKpi[];
  recentEvidence: ResearchEvidenceItem[];
};

export type ReportPreparation = {
  companyId: string;
  eventKey: string;
  status: ReportPreparationStatus;
  preparedAt: string | null;
  processedAt: string | null;
  linkedReportDocumentId: string | null;
};

export type ListReportSeasonInput = {
  watchlistId: string | null;
};

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
