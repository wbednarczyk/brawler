import { callCommand } from "./tauri";
import type { Watchlist, WatchlistMembership } from "./types";

export type CreateWatchlistInput = {
  name: string;
  description: string | null;
};

export type WatchlistMembershipInput = {
  watchlistId: string;
  companyId: string;
};

export function listWatchlists() {
  return callCommand<Watchlist[]>("list_watchlists");
}

export function listWatchlistMemberships() {
  return callCommand<WatchlistMembership[]>("list_watchlist_memberships");
}

export function createWatchlist(input: CreateWatchlistInput) {
  return callCommand<Watchlist>("create_watchlist", { input });
}

export function addCompanyToWatchlist(input: WatchlistMembershipInput) {
  return callCommand<void>("add_company_to_watchlist", { input });
}

export function removeCompanyFromWatchlist(input: WatchlistMembershipInput) {
  return callCommand<void>("remove_company_from_watchlist", { input });
}
