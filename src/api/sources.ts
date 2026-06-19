import { callCommand } from "./tauri";
import type {
  BackfillProgress,
  CompanyRegistryEntry,
  CompanyRegistryRefreshResult,
  SourceAdapter,
  SourceIngestionResult,
  SourceRefreshTrigger,
  UnmatchedSourceItem,
} from "./types";

// Input types GENERATED from src-tauri/src/commands/sources.rs via ts-rs (ADR 0048).
import type { RefreshSourceInput } from "./generated/RefreshSourceInput";
import type { SetSourceEnabledInput } from "./generated/SetSourceEnabledInput";
import type { ListSourceAdaptersInput } from "./generated/ListSourceAdaptersInput";
import type { RefreshCompanyRegistryIfStaleInput } from "./generated/RefreshCompanyRegistryIfStaleInput";

export type { RefreshSourceInput } from "./generated/RefreshSourceInput";
export type { RefreshCompanyRegistryIfStaleInput } from "./generated/RefreshCompanyRegistryIfStaleInput";
export type { ListSourceAdaptersInput } from "./generated/ListSourceAdaptersInput";
export type { SetSourceEnabledInput } from "./generated/SetSourceEnabledInput";

export function listSourceAdapters(input?: ListSourceAdaptersInput) {
  return callCommand<SourceAdapter[]>("list_source_adapters", input ? { input } : undefined);
}

export function setSourceAdapterEnabled(input: SetSourceEnabledInput) {
  return callCommand<SourceAdapter>("set_source_adapter_enabled", { input });
}

export function listUnmatchedSourceItems(adapterId: string) {
  return callCommand<UnmatchedSourceItem[]>("list_unmatched_source_items", { adapterId });
}

export function listCompanyRegistryEntries() {
  return callCommand<CompanyRegistryEntry[]>("list_company_registry_entries");
}

export function refreshSources(trigger: SourceRefreshTrigger) {
  return callCommand<SourceIngestionResult>("refresh_sources", { input: { trigger } });
}

export function refreshSource(input: RefreshSourceInput) {
  return callCommand<SourceIngestionResult>("refresh_source", { input });
}

export function refreshGpwCompanyRegistry(trigger: SourceRefreshTrigger) {
  return callCommand<CompanyRegistryRefreshResult>("refresh_gpw_company_registry", { input: { trigger } });
}

// Runs an explicit ~3-year history backfill for one tracked company (ADR 0036).
// Long-running and throttled; poll getBackfillProgress while it runs.
export function backfillCompanyHistory(companyId: string) {
  return callCommand<BackfillProgress>("backfill_company_history", { input: { companyId } });
}

export function getBackfillProgress(companyId: string) {
  return callCommand<BackfillProgress | null>("get_backfill_progress", { input: { companyId } });
}

export function refreshGpwCompanyRegistryIfStale(input: RefreshCompanyRegistryIfStaleInput) {
  return callCommand<CompanyRegistryRefreshResult | null>("refresh_gpw_company_registry_if_stale", { input });
}
