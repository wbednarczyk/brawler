import { callCommand } from "./tauri";

export type SearchContentType =
  | "company"
  | "watchlist"
  | "feed_item"
  | "notebook_entry"
  | "transcript_segment"
  | "event"
  | "research_brief"
  | "digest";

export type SearchMatch = {
  contentType: SearchContentType;
  sourceId: string;
  companyId: string | null;
  // Navigational container when the item's own id is not the target (e.g. the
  // transcript job for a transcript segment).
  parentId: string | null;
  title: string;
  snippet: string;
  score: number;
};

export type SearchGroup = {
  contentType: SearchContentType;
  matches: SearchMatch[];
};

export type SearchResults = {
  groups: SearchGroup[];
};

export type SearchInput = {
  query: string;
  contentTypes?: SearchContentType[];
  companyId?: string | null;
  limit?: number;
};

export function search(input: SearchInput) {
  return callCommand<SearchResults>("search", { input });
}
