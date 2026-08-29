import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  renderApp,
  screen,
  userEvent,
  within,
} from "../../test/appWorkflowHarness";
import { collectActionInventory, collectEmptyStates, expectSinglePrimary } from "../../test/uxContracts";

// F4a S1 contract harness for the Watchlists redesign
// (docs/plans/frontend-v2-f4a.md § Watchlists). Watchlists is a REDESIGN wave
// (S3 implements it); this file exercises today's screen against the
// contract's action inventory / primary-action / empty-state rows, so the
// gaps are named and provable rather than discovered ad hoc. Assertions that
// already hold render as `it`; assertions the redesign has not landed yet are
// `it.fails` naming the exact contract row — S3 flips them to `it` as it
// implements each one (never delete a row, flip it).
//
// The default ("minimal") scenario seeds one watchlist ("Main GPW") with one
// member (CD PROJEKT, ticker CDR) — the harness auto-selects it on mount.

const REGION_NAME = { en: "Watchlists", pl: "Listy obserwowane" } as const;

async function openWatchlists(locale: "en" | "pl") {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Watchlists" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

describe("Watchlists action inventory (F4a contract § Watchlists, Action inventory)", () => {
  it.fails("every static contract-named action carries its dictionary kind (EN)", async () => {
    const region = await openWatchlists("en");
    // Wait for the default-selected watchlist's member table to render.
    await within(region).findByText("CD PROJEKT S.A.");

    const inventory = collectActionInventory(region, "en");
    const byName = new Map(inventory.map((entry) => [entry.name, entry.kind]));

    expect(byName.get("Create")).toBe("create");
    expect(byName.get("Search watchlists")).toBe("control");
    expect(byName.get("Rename")).toBe("rename");
    expect(byName.get("Remove")).toBe("remove");
    expect(byName.get("Add companies")).toBe("add");
    expect(byName.get("Open company")).toBe("destination");
    expect(byName.get("Remove from list")).toBe("remove");
  });

  it.fails("every static contract-named action carries its dictionary kind (PL)", async () => {
    const region = await openWatchlists("pl");
    await within(region).findByText("CD PROJEKT S.A.");

    const inventory = collectActionInventory(region, "pl");
    const byName = new Map(inventory.map((entry) => [entry.name, entry.kind]));

    expect(byName.get("Utwórz")).toBe("create");
    expect(byName.get("Szukaj list")).toBe("control");
    expect(byName.get("Zmień nazwę")).toBe("rename");
    expect(byName.get("Usuń")).toBe("remove");
    expect(byName.get("Dodaj spółki")).toBe("add");
    expect(byName.get("Otwórz spółkę")).toBe("destination");
    expect(byName.get("Usuń z listy")).toBe("remove");
  });

  it.fails("the add-companies picker's Add selected / Cancel carry their contract kinds", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    await user.click(await within(region).findByRole("button", { name: "Add companies" }));

    const inventory = collectActionInventory(region, "en");
    const byName = new Map(inventory.map((entry) => [entry.name, entry.kind]));
    expect(byName.get("Add selected")).toBe("add");
    // The picker's dismiss control is icon-only today (no accessible name) —
    // the contract requires a "Cancel" label.
    expect(byName.get("Cancel")).toBe("control");
  });

  it.fails("no button in the screen root is left unclassified", async () => {
    const region = await openWatchlists("en");
    await within(region).findByText("CD PROJEKT S.A.");

    const unclassified = collectActionInventory(region, "en").filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });
});

describe("Watchlists primary action per state (F4a contract § Watchlists, State matrix)", () => {
  it.fails("Success: the selected list's Add companies is the one filled action", async () => {
    const region = await openWatchlists("en");
    await within(region).findByText("CD PROJEKT S.A.");
    expectSinglePrimary(region, 1);
  });

  it.fails("Empty (no lists): the invitation's Create action is the one filled action", async () => {
    appTestState.watchlistsResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists("en");
    await within(region).findByText("No watchlists yet.");
    expectSinglePrimary(region, 1);
  });

  it.fails("Empty (search no match): the invitation's Create action is the one filled action", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    const search = await within(region).findByLabelText("Search watchlists");
    await user.type(search, "zzz-no-such-list");
    await within(region).findByText("No watchlists match this search.");
    expectSinglePrimary(region, 1);
  });
});

describe("Watchlists empty states (F4a contract § Watchlists, State matrix)", () => {
  it.fails("Empty (no lists) renders the invitation kind, not the legacy shape", async () => {
    appTestState.watchlistsResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists("en");
    await within(region).findByText("No watchlists yet.");
    expect(collectEmptyStates(region)).toContain("invitation");
  });

  it.fails("Empty (search no match) renders the invitation kind, not the legacy shape", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    const search = await within(region).findByLabelText("Search watchlists");
    await user.type(search, "zzz-no-such-list");
    await within(region).findByText("No watchlists match this search.");
    expect(collectEmptyStates(region)).toContain("invitation");
  });

  it.fails("Empty (selected list has 0 members) renders the invitation kind, not the legacy shape", async () => {
    // Remove the seeded member so the members table is empty while a list
    // stays selected.
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists("en");
    await within(region).findByText("No companies in this watchlist.");
    expect(collectEmptyStates(region)).toContain("invitation");
  });
});
