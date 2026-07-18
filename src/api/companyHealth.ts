import { callCommand } from "./tauri";
import type { CompanyHealth } from "./generated/CompanyHealth";

// GENERATED DTOs from src-tauri/src/commands/company_health.rs +
// fundamentals/health.rs via ts-rs (ADR 0048/0083).
export type { CompanyHealth } from "./generated/CompanyHealth";
export type { HealthPeriodScores } from "./generated/HealthPeriodScores";
export type { PiotroskiScore } from "./generated/PiotroskiScore";
export type { PiotroskiSignal } from "./generated/PiotroskiSignal";
export type { AltmanScore } from "./generated/AltmanScore";
export type { AltmanComponent } from "./generated/AltmanComponent";
export type { AltmanBand } from "./generated/AltmanBand";
export type { MeasuredInput } from "./generated/MeasuredInput";
export type { MissingInput } from "./generated/MissingInput";

/// Deterministic Piotroski F + Altman Z″ scores for the Quality panel
/// (ADR 0083): latest-FY tiles + per-FY history, each score a headline value,
/// `insufficient_data`, or `not_applicable`.
export function getCompanyHealth(companyId: string) {
  return callCommand<CompanyHealth>("get_company_health", { companyId });
}
