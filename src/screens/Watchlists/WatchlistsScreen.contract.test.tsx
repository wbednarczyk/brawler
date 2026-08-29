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
import type { ActionInventoryEntry } from "../../test/uxContracts";

// F4a S1 contract harness for the Watchlists redesign
// (docs/plans/frontend-v2-f4a.md § Watchlists). Watchlists is a REDESIGN wave
// (S3 implements it); this file exercises today's screen against the
// contract's action inventory / primary-action / empty-state rows.
//
// Fix-C guardrail 3 (sol F4a R1 finding 3): every state below asserts the
// FULL sorted action inventory (`toEqual`), not a `Map` lookup of a handful
// of names — a stray/misclassified/duplicated button now reddens even when
// it isn't one of the specific names a narrower assertion would have
// checked. "No unclassified" is implicit in a full array match (an
// unclassified entry would make the arrays unequal), but is also asserted
// explicitly for readability at the point a failure would matter most.
//
// The default ("minimal") scenario seeds one watchlist ("Main GPW",
// `src/test/scenarios/legacyMinimal.ts`) with one member (CD PROJEKT, ticker
// CDR) — the harness auto-selects it on mount. Note: the watchlist ROW's
// accessible name is its raw textContent (name + Figure count concatenated,
// no separating space, e.g. "Main GPW1") — a pre-existing quirk of
// `collectActionInventory`'s simplified name algorithm (no aria-label on the
// row), not something this guardrail wave changes.

const REGION_NAME = { en: "Watchlists", pl: "Listy obserwowane" } as const;
const ROW_NAME = "Main GPW1";

async function openWatchlists(locale: "en" | "pl") {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Watchlists" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

const DEFAULT_EN: ActionInventoryEntry[] = sorted([
  { name: "Add companies", kind: "add" },
  { name: "Create", kind: "create" },
  { name: ROW_NAME, kind: "destination" },
  { name: "Open company", kind: "destination" },
  { name: "Remove", kind: "remove" },
  { name: "Remove from list", kind: "remove" },
  { name: "Rename", kind: "rename" },
]);

const DEFAULT_PL: ActionInventoryEntry[] = sorted([
  { name: "Dodaj spółki", kind: "add" },
  { name: ROW_NAME, kind: "destination" },
  { name: "Otwórz spółkę", kind: "destination" },
  { name: "Usuń", kind: "remove" },
  { name: "Usuń z listy", kind: "remove" },
  { name: "Utwórz", kind: "create" },
  { name: "Zmień nazwę", kind: "rename" },
]);

describe("Watchlists action inventory (F4a contract § Watchlists, Action inventory)", () => {
  it("the full sorted action inventory matches the contract table (EN)", async () => {
    const region = await openWatchlists("en");
    await within(region).findByText("CD PROJEKT S.A.");
    expect(collectActionInventory(region, "en")).toEqual(DEFAULT_EN);
  });

  it("the full sorted action inventory matches the contract table (PL)", async () => {
    const region = await openWatchlists("pl");
    await within(region).findByText("CD PROJEKT S.A.");
    expect(collectActionInventory(region, "pl")).toEqual(DEFAULT_PL);
  });

  it("the add-companies picker's Add selected / Cancel join the inventory, classified, everything else unchanged", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    await user.click(await within(region).findByRole("button", { name: "Add companies" }));

    // "Add companies" folds away while the picker is open (mutually exclusive
    // with "Add selected"/"Cancel") — everything else in the default
    // inventory stays.
    expect(collectActionInventory(region, "en")).toEqual(
      sorted([
        ...DEFAULT_EN.filter((entry) => entry.name !== "Add companies"),
        { name: "Add selected", kind: "add" },
        { name: "Cancel", kind: "control" },
      ]),
    );
  });

  it("the rename form's Save / Cancel join the inventory in place of Rename/Add companies/Remove", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    await within(region).findByText("CD PROJEKT S.A.");
    await user.click(within(region).getByRole("button", { name: "Rename" }));

    expect(collectActionInventory(region, "en")).toEqual(
      sorted([
        { name: "Cancel", kind: "control" },
        { name: "Create", kind: "create" },
        { name: ROW_NAME, kind: "destination" },
        { name: "Open company", kind: "destination" },
        { name: "Remove from list", kind: "remove" },
        { name: "Save", kind: "save" },
      ]),
    );
  });

  it("no button in the screen root is left unclassified (default state)", async () => {
    const region = await openWatchlists("en");
    await within(region).findByText("CD PROJEKT S.A.");

    const unclassified = collectActionInventory(region, "en").filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });
});

describe("Watchlists primary action per state (F4a contract § Watchlists, State matrix)", () => {
  it("Success: the selected list's Add companies is the one filled action", async () => {
    const region = await openWatchlists("en");
    await within(region).findByText("CD PROJEKT S.A.");
    expectSinglePrimary(region, 1);
  });

  it("Empty (no lists): the invitation's Create action is the one filled action", async () => {
    appTestState.watchlistsResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists("en");
    await within(region).findByText("No watchlists yet.");
    expectSinglePrimary(region, 1);
  });

  it("Empty (search no match): the invitation's Create action is the one filled action", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    const search = await within(region).findByLabelText("Search watchlists");
    await user.type(search, "zzz-no-such-list");
    await within(region).findByText("No watchlists match this search.");
    expectSinglePrimary(region, 1);
  });
});

describe("Watchlists empty states (F4a contract § Watchlists, State matrix)", () => {
  it("Empty (no lists): full inventory is just the header Create form + the invitation's Create, both classified", async () => {
    appTestState.watchlistsResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists("en");
    await within(region).findByText("No watchlists yet.");
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, "en")).toEqual(
      sorted([
        { name: "Create", kind: "create" },
        { name: "Create your first watchlist", kind: "create" },
      ]),
    );
  });

  it("Empty (search no match): full inventory is the header Create form + clear search + the invitation's Create, all classified", async () => {
    const user = userEvent.setup();
    const region = await openWatchlists("en");
    const search = await within(region).findByLabelText("Search watchlists");
    await user.type(search, "zzz-no-such-list");
    await within(region).findByText("No watchlists match this search.");
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, "en")).toEqual(
      sorted([
        { name: "Clear watchlist search", kind: "control" },
        { name: "Create", kind: "create" },
        { name: 'Create watchlist "zzz-no-such-list"', kind: "create" },
      ]),
    );
  });

  it("Empty (selected list has 0 members): full inventory drops the member row + Open company, keeps the rest classified", async () => {
    // Remove the seeded member so the members table is empty while a list
    // stays selected.
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists("en");
    await within(region).findByText("No companies in this watchlist.");
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, "en")).toEqual(
      sorted([
        { name: "Add companies", kind: "add" },
        { name: "Create", kind: "create" },
        { name: ROW_NAME, kind: "destination" },
        { name: "Remove", kind: "remove" },
        { name: "Rename", kind: "rename" },
      ]),
    );
  });
});
