import { callCommand } from "./tauri";

export type ImportExportSummary = {
  companies: number;
  watchlists: number;
  memberships: number;
  notebookEntries: number;
  researchQuestions: number;
  evidenceLinks: number;
  aiResearchBriefs: number;
  aiResearchBriefCitations: number;
  researchReminders: number;
  aiResearchDigests: number;
  aiResearchDigestCitations: number;
  qualityFrameworks: number;
  userMetrics: number;
  settings: number;
};

export type ImportApplySummary = {
  companiesCreated: number;
  companiesMerged: number;
  watchlistsCreated: number;
  watchlistsMerged: number;
  membershipsCreated: number;
  notebookEntriesCreated: number;
  notebookEntriesSkipped: number;
  researchQuestionsCreated: number;
  researchQuestionsMerged: number;
  evidenceLinksCreated: number;
  evidenceLinksSkipped: number;
  aiResearchBriefsCreated: number;
  aiResearchBriefsSkipped: number;
  aiResearchBriefCitationsCreated: number;
  aiResearchBriefCitationsSkipped: number;
  researchRemindersCreated: number;
  researchRemindersSkipped: number;
  aiResearchDigestsCreated: number;
  aiResearchDigestsSkipped: number;
  aiResearchDigestCitationsCreated: number;
  aiResearchDigestCitationsSkipped: number;
  qualityFrameworksCreated: number;
  qualityFrameworksSkipped: number;
  userMetricsCreated: number;
  userMetricsSkipped: number;
  settingsUpdated: number;
};

export type ExportPayload = {
  fileName: string;
  mediaType: string;
  contents: string;
  summary: ImportExportSummary;
};

export type ImportPreview = {
  valid: boolean;
  summary: ImportApplySummary;
  warnings: string[];
  errors: string[];
};

export type ImportApplyResult = {
  summary: ImportApplySummary;
  warnings: string[];
};

export function exportResearchData() {
  return callCommand<ExportPayload>("export_research_data");
}

export function previewResearchImport(contents: string) {
  return callCommand<ImportPreview>("preview_research_import", { input: { contents } });
}

export function applyResearchImport(contents: string) {
  return callCommand<ImportApplyResult>("apply_research_import", { input: { contents } });
}

export function exportSettingsData() {
  return callCommand<ExportPayload>("export_settings_data");
}

export function previewSettingsImport(contents: string) {
  return callCommand<ImportPreview>("preview_settings_import", { input: { contents } });
}

export function applySettingsImport(contents: string) {
  return callCommand<ImportApplyResult>("apply_settings_import", { input: { contents } });
}
