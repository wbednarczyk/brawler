import type { Dispatch, SetStateAction } from "react";
import * as companiesApi from "../api/companies";
import * as credentialsApi from "../api/credentials";
import * as feedApi from "../api/feed";
import * as settingsApi from "../api/settings";
import * as sourcesApi from "../api/sources";
import * as systemApi from "../api/system";
import * as watchlistsApi from "../api/watchlists";
import type {
  Company,
  CompanyRegistryEntry,
  CredentialStatus,
  DatabaseStatus,
  FeedItem,
  FeedPruneResult,
  HealthResponse,
  SourceAdapter,
  UnmatchedSourceItem,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";
import type { DbRefreshState } from "./appTypes";

type AppDataControllerInput = {
  companies: Company[];
  feedPruneRetentionDays: number;
  refreshCompanyEvents: () => Promise<void>;
  setCompanies: Dispatch<SetStateAction<Company[]>>;
  setCompaniesError: Dispatch<SetStateAction<string | null>>;
  setCompanyRegistryEntries: Dispatch<SetStateAction<CompanyRegistryEntry[]>>;
  setCompanyRegistryEntriesError: Dispatch<SetStateAction<string | null>>;
  setDatabaseError: Dispatch<SetStateAction<string | null>>;
  setDatabaseStatus: Dispatch<SetStateAction<DatabaseStatus | null>>;
  setDbRefreshState: Dispatch<SetStateAction<DbRefreshState>>;
  setDeleteUnsavedFeedError: Dispatch<SetStateAction<string | null>>;
  setDeleteUnsavedFeedState: Dispatch<SetStateAction<DbRefreshState>>;
  setFeedError: Dispatch<SetStateAction<string | null>>;
  setFeedPruneResult: Dispatch<SetStateAction<FeedPruneResult | null>>;
  setFeedState: Dispatch<SetStateAction<FeedItem[]>>;
  setGeminiCredentialError: Dispatch<SetStateAction<string | null>>;
  setGeminiCredentialStatus: Dispatch<SetStateAction<CredentialStatus | null>>;
  setHealth: Dispatch<SetStateAction<HealthResponse | null>>;
  setHealthError: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  setSettings: Dispatch<SetStateAction<UserSettings | null>>;
  setSettingsError: Dispatch<SetStateAction<string | null>>;
  setLocale: Dispatch<SetStateAction<UserSettings["locale"]>>;
  setSourceAdapters: Dispatch<SetStateAction<SourceAdapter[]>>;
  setSourceAdaptersError: Dispatch<SetStateAction<string | null>>;
  setTheme: Dispatch<SetStateAction<UserSettings["theme"]>>;
  setUnmatchedSourceItems: Dispatch<SetStateAction<Record<string, UnmatchedSourceItem[]>>>;
  setUnmatchedSourceItemsError: Dispatch<SetStateAction<string | null>>;
  setWatchlistAssignments: Dispatch<SetStateAction<Record<string, string>>>;
  setWatchlistMemberships: Dispatch<SetStateAction<WatchlistMembership[]>>;
  setWatchlists: Dispatch<SetStateAction<Watchlist[]>>;
  setWatchlistsError: Dispatch<SetStateAction<string | null>>;
  text: (value: string) => string;
};

export function useAppDataController({
  companies,
  feedPruneRetentionDays,
  refreshCompanyEvents,
  setCompanies,
  setCompaniesError,
  setCompanyRegistryEntries,
  setCompanyRegistryEntriesError,
  setDatabaseError,
  setDatabaseStatus,
  setDbRefreshState,
  setDeleteUnsavedFeedError,
  setDeleteUnsavedFeedState,
  setFeedError,
  setFeedPruneResult,
  setFeedState,
  setGeminiCredentialError,
  setGeminiCredentialStatus,
  setHealth,
  setHealthError,
  setSelectedFeedItemId,
  setSettings,
  setSettingsError,
  setLocale,
  setSourceAdapters,
  setSourceAdaptersError,
  setTheme,
  setUnmatchedSourceItems,
  setUnmatchedSourceItemsError,
  setWatchlistAssignments,
  setWatchlistMemberships,
  setWatchlists,
  setWatchlistsError,
  text,
}: AppDataControllerInput) {
  function refreshHealth() {
    return systemApi.getHealth()
      .then((response) => {
        setHealth(response);
        setHealthError(null);
      })
      .catch((error) => {
        setHealth(null);
        setHealthError(String(error));
      });
  }

  function refreshDatabaseStatus() {
    return systemApi.getDatabaseStatus()
      .then((response) => {
        setDatabaseStatus(response);
        setDatabaseError(null);
      })
      .catch((error) => {
        setDatabaseStatus(null);
        setDatabaseError(String(error));
      });
  }

  function refreshCompanies() {
    return companiesApi.listCompanies()
      .then((response) => {
        setCompanies(response);
        setCompaniesError(null);
      })
      .catch((error) => {
        setCompanies([]);
        setCompaniesError(String(error));
      });
  }

  function refreshWatchlists() {
    return watchlistsApi.listWatchlists()
      .then((response) => {
        setWatchlists(response);
        setWatchlistsError(null);
        setWatchlistAssignments((current) => {
          const fallback = response[0]?.id ?? "";
          const next = { ...current };

          for (const company of companies) {
            if (!next[company.id]) {
              next[company.id] = fallback;
            }
          }

          return next;
        });
      })
      .catch((error) => {
        setWatchlists([]);
        setWatchlistsError(String(error));
      });
  }

  function refreshWatchlistMemberships() {
    return watchlistsApi.listWatchlistMemberships()
      .then((response) => {
        setWatchlistMemberships(response);
        setWatchlistsError(null);
      })
      .catch((error) => {
        setWatchlistMemberships([]);
        setWatchlistsError(String(error));
      });
  }

  function refreshFeedItems() {
    return feedApi.listFeedItems()
      .then((response) => {
        setFeedState(response);
        setFeedError(null);
        setSelectedFeedItemId((current) => {
          if (current && response.some((item) => item.id === current)) {
            return current;
          }

          return response[0]?.id ?? null;
        });
      })
      .catch((error) => {
        setFeedState([]);
        setFeedError(String(error));
      });
  }

  function refreshSourceAdapters() {
    return sourcesApi.listSourceAdapters()
      .then((response) => {
        setSourceAdapters(response);
        setSourceAdaptersError(null);
      })
      .catch((error) => {
        setSourceAdapters([]);
        setSourceAdaptersError(String(error));
      });
  }

  function refreshUnmatchedSourceItems(adapterId: string) {
    return sourcesApi.listUnmatchedSourceItems(adapterId)
      .then((response) => {
        setUnmatchedSourceItems((current) => ({
          ...current,
          [adapterId]: response,
        }));
        setUnmatchedSourceItemsError(null);
      })
      .catch((error) => {
        setUnmatchedSourceItems((current) => ({
          ...current,
          [adapterId]: [],
        }));
        setUnmatchedSourceItemsError(String(error));
      });
  }

  function refreshCompanyRegistryEntries() {
    return sourcesApi.listCompanyRegistryEntries()
      .then((response) => {
        setCompanyRegistryEntries(response);
        setCompanyRegistryEntriesError(null);
      })
      .catch((error) => {
        setCompanyRegistryEntries([]);
        setCompanyRegistryEntriesError(String(error));
      });
  }

  function refreshSettings() {
    return settingsApi.getSettings()
      .then((response) => {
        setSettings(response);
        setLocale(response.locale);
        setTheme(response.theme);
        setSettingsError(null);
      })
      .catch((error) => {
        setSettings(null);
        setSettingsError(String(error));
      });
  }

  function refreshGeminiCredentialStatus() {
    return credentialsApi.getGeminiTranscriptionCredentialStatus()
      .then((response) => {
        setGeminiCredentialStatus(response);
        setGeminiCredentialError(null);
      })
      .catch((error) => {
        setGeminiCredentialStatus(null);
        setGeminiCredentialError(String(error));
      });
  }

  function refreshDatabaseBackedViews() {
    setDbRefreshState("refreshing");

    Promise.all([
      refreshDatabaseStatus(),
      refreshCompanies(),
      refreshWatchlists(),
      refreshWatchlistMemberships(),
      refreshFeedItems(),
      refreshCompanyEvents(),
      refreshSourceAdapters(),
      refreshSettings(),
      refreshGeminiCredentialStatus(),
    ]).then(() => {
      setDbRefreshState("done");
      window.setTimeout(() => {
        setDbRefreshState("idle");
      }, 900);
    });
  }

  function deleteUnsavedFeedItems() {
    const confirmed = window.confirm(text("Delete all unsaved feed items? Saved items will stay."));

    if (!confirmed) {
      return;
    }

    setDeleteUnsavedFeedState("refreshing");
    setDeleteUnsavedFeedError(null);

    feedApi.deleteUnsavedFeedItems()
      .then(() => Promise.all([refreshFeedItems(), refreshDatabaseStatus()]))
      .then(() => {
        setDeleteUnsavedFeedState("done");
        window.setTimeout(() => {
          setDeleteUnsavedFeedState("idle");
        }, 900);
      })
      .catch((error) => {
        setDeleteUnsavedFeedError(String(error));
        setDeleteUnsavedFeedState("idle");
      });
  }

  function pruneOldFeedItems() {
    return feedApi.pruneOldFeedItems({ retentionDays: feedPruneRetentionDays })
      .then((response) => {
        setFeedPruneResult(response);
        return refreshFeedItems();
      })
      .catch(() => undefined);
  }

  return {
    deleteUnsavedFeedItems,
    pruneOldFeedItems,
    refreshCompanies,
    refreshCompanyRegistryEntries,
    refreshDatabaseBackedViews,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshGeminiCredentialStatus,
    refreshHealth,
    refreshSettings,
    refreshSourceAdapters,
    refreshUnmatchedSourceItems,
    refreshWatchlistMemberships,
    refreshWatchlists,
  };
}
