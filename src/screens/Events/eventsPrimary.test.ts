import { describe, expect, it } from "vitest";
import { derivePrimary, type DerivePrimaryInput } from "./eventsPrimary";

const BASE: DerivePrimaryInput = {
  loading: false,
  error: false,
  composerOpen: false,
  selectedEventStatus: null,
  weekMode: true,
  weekIsEmpty: false,
  hasNextMatch: false,
  hasActiveFilters: false,
};

// One test per row of the F4b contract § Events, decision 5 primary-state table.
describe("derivePrimary (F4b contract § Events, decision 5)", () => {
  it("none: loading", () => {
    expect(derivePrimary({ ...BASE, loading: true })).toBe("none");
  });

  it("none: read error", () => {
    expect(derivePrimary({ ...BASE, error: true })).toBe("none");
  });

  it("addEvent: default state (nothing selected, composer closed, week has events)", () => {
    expect(derivePrimary(BASE)).toBe("addEvent");
  });

  it("addEvent: week empty, no active filters, no later match (refreshed-empty invitation)", () => {
    expect(
      derivePrimary({ ...BASE, weekIsEmpty: true, hasNextMatch: false, hasActiveFilters: false }),
    ).toBe("addEvent");
  });

  it("saveComposer: composer open", () => {
    expect(derivePrimary({ ...BASE, composerOpen: true })).toBe("saveComposer");
  });

  it("confirmProposed: selected event is proposed, in week mode", () => {
    expect(derivePrimary({ ...BASE, selectedEventStatus: "proposed" })).toBe("confirmProposed");
  });

  it("confirmProposed: selected event is proposed, in list mode", () => {
    expect(derivePrimary({ ...BASE, weekMode: false, selectedEventStatus: "proposed" })).toBe(
      "confirmProposed",
    );
  });

  it("jumpNextWeek: week mode, week empty, a later match exists", () => {
    expect(derivePrimary({ ...BASE, weekIsEmpty: true, hasNextMatch: true })).toBe("jumpNextWeek");
  });

  it("noLaterMatch: week mode, week empty, active filters, no later match", () => {
    expect(
      derivePrimary({ ...BASE, weekIsEmpty: true, hasNextMatch: false, hasActiveFilters: true }),
    ).toBe("noLaterMatch");
  });

  it("list mode never empties the week — a filter-empty list stays addEvent", () => {
    expect(derivePrimary({ ...BASE, weekMode: false, weekIsEmpty: true, hasActiveFilters: true })).toBe(
      "addEvent",
    );
  });

  it("loading/error override every other condition", () => {
    expect(derivePrimary({ ...BASE, loading: true, composerOpen: true })).toBe("none");
    expect(derivePrimary({ ...BASE, error: true, selectedEventStatus: "proposed" })).toBe("none");
  });
});
