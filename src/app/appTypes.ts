export type DbRefreshState = "idle" | "refreshing" | "done";

export type SourceRefreshState = "idle" | "refreshing" | "done";

export type WatchlistFeedback = {
  companyId: string;
  message: string;
};
