import type { Company, Watchlist, WatchlistMembership } from "../api/types";
import type { CompanyWorkspaceTab } from "../screens/Companies/companyTypes";
import type { ReportSeasonScreenProps } from "../screens/ReportSeason/ReportSeasonScreen";
import type { WatchlistsScreenProps } from "../screens/Watchlists/WatchlistsScreen";

type WatchlistsScreenWiringInput = {
  companies: Company[];
  watchlists: Watchlist[];
  watchlistMemberships: WatchlistMembership[];
  watchlistsError: string | null;
  selectedWatchlistId: string | null;
  setSelectedWatchlistId: (watchlistId: string | null) => void;
  createWatchlist: (name: string) => void;
  renameWatchlist: (watchlist: Watchlist, name: string) => void;
  deleteWatchlist: (watchlist: Watchlist) => void;
  addCompanyToWatchlist: (watchlist: Watchlist, company: Company) => void;
  removeCompanyFromWatchlist: (watchlist: Watchlist, company: Company) => void;
  openCompanyWorkspaceById: (companyId: string, tab?: CompanyWorkspaceTab) => void;
};

/**
 * Composes the Watchlists screen's view model plus the Report Season tab-jump
 * wrap — extracted from AppStateRoot (file-size ratchet, ADR 0103; same
 * composer pattern as `buildTodayScreenProps`). Grouped together because both
 * are small and both close over `openCompanyWorkspaceById`, called here as a
 * plain parameter (no lazy/TDZ wrap needed — unlike the inline object
 * literals this replaces, `openCompanyWorkspaceById` is already a real value
 * by the time AppStateRoot calls this).
 */
export function buildWatchlistsScreenProps(input: WatchlistsScreenWiringInput): {
  watchlistsViewModel: WatchlistsScreenProps;
  reportSeasonViewModel: ReportSeasonScreenProps;
} {
  return {
    watchlistsViewModel: {
      companies: input.companies,
      watchlists: input.watchlists,
      watchlistMemberships: input.watchlistMemberships,
      watchlistsError: input.watchlistsError,
      selectedWatchlistId: input.selectedWatchlistId,
      setSelectedWatchlistId: input.setSelectedWatchlistId,
      createWatchlist: input.createWatchlist,
      renameWatchlist: input.renameWatchlist,
      deleteWatchlist: input.deleteWatchlist,
      addCompanyToWatchlist: input.addCompanyToWatchlist,
      removeCompanyFromWatchlist: input.removeCompanyFromWatchlist,
      openCompanyWorkspaceById: (companyId) => input.openCompanyWorkspaceById(companyId),
    },
    reportSeasonViewModel: {
      watchlists: input.watchlists,
      openCompanyWorkspace: (companyId, tab) => input.openCompanyWorkspaceById(companyId, tab),
    },
  };
}
