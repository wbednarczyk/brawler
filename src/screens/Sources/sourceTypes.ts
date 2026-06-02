import type { KeyboardEvent } from "react";
import type {
  CompanyRegistryEntry,
  CompanyRegistryRefreshResult,
  SourceAdapter,
  SourceIngestionResult,
  SourceRefreshTrigger,
  UnmatchedSourceItem,
} from "../../api/types";

export type SourcesScreenProps = {
  sourceAdapters: SourceAdapter[];
  sourceAdaptersError: string | null;
  selectedSourceAdapterId: string | null;
  sourceRefreshState: string;
  sourceRefreshResult: SourceIngestionResult | null;
  sourceRefreshError: string | null;
  sourceAdapterRefreshInFlight: string | null;
  registryRefreshState: string;
  registryRefreshResult: CompanyRegistryRefreshResult | null;
  registryRefreshError: string | null;
  companyRegistryEntries: CompanyRegistryEntry[];
  filteredCompanyRegistryEntries: CompanyRegistryEntry[];
  companyRegistryEntriesError: string | null;
  isCompanyRegistryListExpanded: boolean;
  companyRegistrySearch: string;
  addingRegistryTicker: string | null;
  unmatchedSourceItems: Record<string, UnmatchedSourceItem[]>;
  unmatchedSourceItemsError: string | null;
  expandedUnmatchedAdapters: Record<string, boolean>;
  gpwRegistryAdapterId: string;
  refreshSources: (trigger: SourceRefreshTrigger) => void;
  refreshSingleSource: (adapter: SourceAdapter, trigger: SourceRefreshTrigger) => void;
  refreshCompanyRegistry: (trigger: SourceRefreshTrigger) => void;
  toggleSourceAdapter: (adapterId: string) => void;
  toggleSourceAdapterFromKeyboard: (
    event: KeyboardEvent<HTMLElement>,
    adapterId: string,
  ) => void;
  toggleCompanyRegistryList: () => void;
  toggleUnmatchedSourceItems: (adapterId: string) => void;
  setCompanyRegistrySearch: (value: string) => void;
  addCompanyFromRegistry: (entry: CompanyRegistryEntry) => void;
  openExternalUrl: (url: string) => void;
  formatSourceScheduler: (adapter: SourceAdapter) => string;
  formatNextRefresh: (adapter: SourceAdapter) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
};
