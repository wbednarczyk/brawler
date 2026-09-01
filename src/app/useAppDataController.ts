import type { Dispatch, SetStateAction } from "react";
import * as companiesApi from "../api/companies";
import * as credentialsApi from "../api/credentials";
import * as feedApi from "../api/feed";
import * as settingsApi from "../api/settings";
import * as sourcesApi from "../api/sources";
import * as systemApi from "../api/system";
import * as watchlistsApi from "../api/watchlists";
import * as signalsApi from "../api/signals";
import type {
  Company,
  CompanyRegistryEntry,
  CompanySignal,
  CredentialStatus,
  DatabaseStatus,
  FeedItem,
  HealthResponse,
  SourceAdapter,
  UserSettings,
  Watchlist,
  WatchlistMembership,
} from "../api/types";
import type { DbRefreshState } from "./appTypes";

type AppDataControllerInput = {
  refreshCompanyEvents: () => Promise<void>;
  setCompanies: Dispatch<SetStateAction<Company[]>>;
  setCompaniesError: Dispatch<SetStateAction<string | null>>;
  setCompanyRegistryEntries: Dispatch<SetStateAction<CompanyRegistryEntry[]>>;
  setCompanyRegistryEntriesError: Dispatch<SetStateAction<string | null>>;
  setDatabaseError: Dispatch<SetStateAction<string | null>>;
  setDatabaseStatus: Dispatch<SetStateAction<DatabaseStatus | null>>;
  setDbRefreshState: Dispatch<SetStateAction<DbRefreshState>>;
  setFeedError: Dispatch<SetStateAction<string | null>>;
  setFeedState: Dispatch<SetStateAction<FeedItem[]>>;
  setGeminiCredentialError: Dispatch<SetStateAction<string | null>>;
  setGeminiCredentialStatus: Dispatch<SetStateAction<CredentialStatus | null>>;
  setHealth: Dispatch<SetStateAction<HealthResponse | null>>;
  setHealthError: Dispatch<SetStateAction<string | null>>;
  setSelectedFeedItemId: Dispatch<SetStateAction<string | null>>;
  setSignals: Dispatch<SetStateAction<CompanySignal[]>>;
  setSignalsError: Dispatch<SetStateAction<string | null>>;
  setSettings: Dispatch<SetStateAction<UserSettings | null>>;
  setSettingsError: Dispatch<SetStateAction<string | null>>;
  setAccentPalette: Dispatch<SetStateAction<UserSettings["accentPalette"]>>;
  setLocale: Dispatch<SetStateAction<UserSettings["locale"]>>;
  setSourceAdapters: Dispatch<SetStateAction<SourceAdapter[]>>;
  setSourceAdaptersError: Dispatch<SetStateAction<string | null>>;
  setTheme: Dispatch<SetStateAction<UserSettings["theme"]>>;
  setWatchlistMemberships: Dispatch<SetStateAction<WatchlistMembership[]>>;
  setWatchlists: Dispatch<SetStateAction<Watchlist[]>>;
  setWatchlistsError: Dispatch<SetStateAction<string | null>>;
};

export function useAppDataController({
  refreshCompanyEvents,
  setCompanies,
  setCompaniesError,
  setCompanyRegistryEntries,
  setCompanyRegistryEntriesError,
  setDatabaseError,
  setDatabaseStatus,
  setDbRefreshState,
  setFeedError,
  setFeedState,
  setGeminiCredentialError,
  setGeminiCredentialStatus,
  setHealth,
  setHealthError,
  setSelectedFeedItemId,
  setSignals,
  setSignalsError,
  setSettings,
  setSettingsError,
  setAccentPalette,
  setLocale,
  setSourceAdapters,
  setSourceAdaptersError,
  setTheme,
  setWatchlistMemberships,
  setWatchlists,
  setWatchlistsError,
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

  function refreshSignals() {
    return signalsApi
      .listCompanySignals({ companyId: null, watchlistId: null, category: null, status: null })
      .then((response) => {
        setSignals(response);
        setSignalsError(null);
      })
      .catch((error) => {
        setSignals([]);
        setSignalsError(String(error));
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
        setAccentPalette(response.accentPalette);
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
      refreshSignals(),
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

  return {
    refreshCompanies,
    refreshCompanyRegistryEntries,
    refreshDatabaseBackedViews,
    refreshDatabaseStatus,
    refreshFeedItems,
    refreshSignals,
    refreshGeminiCredentialStatus,
    refreshHealth,
    refreshSettings,
    refreshSourceAdapters,
    refreshWatchlistMemberships,
    refreshWatchlists,
  };
}
