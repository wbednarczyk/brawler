import { callCommand } from "./tauri";
import type { SearchInput } from "./generated/SearchInput";
import type { SearchResults } from "./generated/SearchResults";

// GENERATED from src-tauri/src/commands/search.rs + storage/search.rs via ts-rs
// (ADR 0048); SearchContentType comes from the marker enum in api_ts_unions.rs.
export type { SearchContentType } from "./generated/SearchContentType";
export type { SearchMatch } from "./generated/SearchMatch";
export type { SearchGroup } from "./generated/SearchGroup";
export type { SearchResults } from "./generated/SearchResults";
export type { SearchInput } from "./generated/SearchInput";

export function search(input: SearchInput) {
  return callCommand<SearchResults>("search", { input });
}
