import { callCommand } from "./tauri";
import type { CompanyView } from "./generated/CompanyView";

// GENERATED DTO from src-tauri/src/commands/company_view.rs via ts-rs (F3a
// S1, ADR 0107 dec. 3, mirrors the `today.ts` composed-read pattern).
export type { CompanyView } from "./generated/CompanyView";
export type { CompanyViewCounters } from "./generated/CompanyViewCounters";
export type { CompanyViewSignalCounts } from "./generated/CompanyViewSignalCounts";
export type { CompanyViewCategoryCount } from "./generated/CompanyViewCategoryCount";
export type { CompanyViewClaimCounts } from "./generated/CompanyViewClaimCounts";
export type { CompanyViewShortCounts } from "./generated/CompanyViewShortCounts";
export type { CompanyViewEventCounts } from "./generated/CompanyViewEventCounts";
export type { CompanyViewKpi } from "./generated/CompanyViewKpi";
export type { CompanyViewKpiRow } from "./generated/CompanyViewKpiRow";
export type { CompanyViewKpiCell } from "./generated/CompanyViewKpiCell";
export type { CompanyViewFeedItem } from "./generated/CompanyViewFeedItem";
export type { CompanyViewPrice } from "./generated/CompanyViewPrice";
export type { CompanyViewCandle } from "./generated/CompanyViewCandle";
export type { CompanyViewSectionErrors } from "./generated/CompanyViewSectionErrors";

/// The Spółka screen's composed read (ADR 0107 dec. 3, F3a #429, S1): glance
/// bar counters + core sections in ONE call, degrading per-section via
/// `sectionErrors` rather than failing the whole read.
export function getCompanyView(companyId: string) {
  return callCommand<CompanyView>("get_company_view", { companyId });
}
