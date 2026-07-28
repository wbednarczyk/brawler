import { callCommand } from "./tauri";
import type { ExportPayload } from "./generated/ExportPayload";
import type { ImportPreview } from "./generated/ImportPreview";
import type { ImportApplyResult } from "./generated/ImportApplyResult";
import type { WriteExportFileInput } from "./generated/WriteExportFileInput";

// GENERATED from src-tauri/src/storage/import_export/types.rs via ts-rs (ADR 0048).
// The summaries are the complete Rust-side counts (incl. managementClaims).
export type { ImportExportSummary } from "./generated/ImportExportSummary";
export type { ImportApplySummary } from "./generated/ImportApplySummary";
export type { ExportPayload } from "./generated/ExportPayload";
export type { ImportPreview } from "./generated/ImportPreview";
export type { ImportApplyResult } from "./generated/ImportApplyResult";

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

export type { WriteExportFileInput } from "./generated/WriteExportFileInput";

// Issue #106: the export write is a typed backend command (extension whitelist
// enforced Rust-side; returns the final path) — the webview holds no
// filesystem permission at all.
export function writeExportFile(input: WriteExportFileInput) {
  return callCommand<string>("write_export_file", { input });
}
