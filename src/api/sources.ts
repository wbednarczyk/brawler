import { callCommand } from "./tauri";
import type {
  CompanyRegistryEntry,
  CompanyRegistryRefreshResult,
  SourceAdapter,
  SourceIngestionResult,
  SourceRefreshTrigger,
  UnmatchedSourceItem,
} from "./types";

export type RefreshSourceInput = {
  adapterId: string;
  trigger: SourceRefreshTrigger;
  date?: string | null;
};

export type RefreshCompanyRegistryIfStaleInput = {
  trigger: "scheduler";
  staleAfterSeconds: number;
};

export type ListSourceAdaptersInput = {
  includeDeveloperOnly?: boolean;
};

export type SetSourceEnabledInput = {
  adapterId: string;
  enabled: boolean;
};

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

export function refreshGpwCompanyRegistryIfStale(input: RefreshCompanyRegistryIfStaleInput) {
  return callCommand<CompanyRegistryRefreshResult | null>("refresh_gpw_company_registry_if_stale", { input });
}
