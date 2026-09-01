// The Events primary-action enum (F4b contract § Events, decision 5): derived
// from existing screen state, no new atoms. Read top-to-bottom — each branch
// overrides the ones below it, matching the escalation order the contract
// spells out (`addEvent` → `saveComposer` → `confirmProposed` →
// `jumpNextWeek`/`noLaterMatch`), with `loading`/`error` as the outermost
// override (nothing is filled while the screen has nothing to act on yet).
export type EventsPrimaryState =
  | "none"
  | "addEvent"
  | "saveComposer"
  | "confirmProposed"
  | "jumpNextWeek"
  | "noLaterMatch";

export type DerivePrimaryInput = {
  loading: boolean;
  error: boolean;
  composerOpen: boolean;
  /** The selected event's `status`, or `null` when nothing is selected. */
  selectedEventStatus: string | null;
  /** `true` when the screen is in week mode (list mode never empties this way). */
  weekMode: boolean;
  /** `true` when the displayed week has zero events (week mode only). */
  weekIsEmpty: boolean;
  /** The empty-week "later match" lookup hasn't settled yet (F4b sol R1) —
   * no primary while it's in flight, never the stale-looking "addEvent". */
  nextWeekLookupPending: boolean;
  /** The lookup itself failed — no primary; the screen shows an error, not
   * an invitation claiming there is nothing later. */
  nextWeekLookupError: boolean;
  /** `true` when a later week with a matching event exists (the jump target). */
  hasNextMatch: boolean;
  /** Any of the four Events filters (watchlist/company/type/status) ≠ "all". */
  hasActiveFilters: boolean;
};

export function derivePrimary(input: DerivePrimaryInput): EventsPrimaryState {
  if (input.loading || input.error) return "none";
  if (input.composerOpen) return "saveComposer";
  if (input.selectedEventStatus === "proposed") return "confirmProposed";
  if (input.weekMode && input.weekIsEmpty) {
    if (input.nextWeekLookupPending || input.nextWeekLookupError) return "none";
    if (input.hasNextMatch) return "jumpNextWeek";
    if (input.hasActiveFilters) return "noLaterMatch";
  }
  return "addEvent";
}
