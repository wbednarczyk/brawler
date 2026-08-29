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
import { plText } from "../../shared/locale/resources/plText";
const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];
/** The rendered label for an EN copy key in `locale` (PL via plText, exactly what `text()` renders). */
function L(locale: Locale, en: string, vars: Record<string, string> = {}): string {
  const base = locale === "pl" ? (plText[en] ?? en) : en;
  return Object.entries(vars).reduce((s, [k, v]) => s.replace(`{${k}}`, v), base);
}


// F4a S1 contract harness for the Watchlists redesign
// (docs/plans/frontend-v2-f4a.md § Watchlists). Watchlists is a REDESIGN wave
// (S3 implements it); this file exercises today's screen against the
// contract's action inventory / primary-action / empty-state rows.
//
// every state below asserts the
// FULL sorted action inventory (`toEqual`), not a `Map` lookup of a handful
// of names — a stray/misclassified/duplicated button now reddens even when
// it isn't one of the specific names a narrower assertion would have
// checked. "No unclassified" is implicit in a full array match (an
// unclassified entry would make the arrays unequal), but is also asserted
// explicitly for readability at the point a failure would matter most.
//
// The default ("minimal") scenario seeds one watchlist ("Main GPW",
// `src/test/scenarios/legacyMinimal.ts`) with one member (CD PROJEKT, ticker
// CDR) — the harness auto-selects it on mount. A list row's accessible name
// is the list name (the member count is visible, not part of the name).

const REGION_NAME = { en: "Watchlists", pl: "Listy obserwowane" } as const;
const ROW_NAME = "Main GPW";

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
  it.each(LOCALES)("Empty (no lists): the header Create form + the invitation's Create, both classified (%s)", async (locale) => {
    appTestState.watchlistsResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists(locale);
    await within(region).findByText(L(locale, "No watchlists yet."));
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Create"), kind: "create" },
        { name: L(locale, "Create your first watchlist"), kind: "create" },
      ]),
    );
  });

  it.each(LOCALES)("Empty (search no match): the header Create form + clear search + the invitation's Create, all classified (%s)", async (locale) => {
    const user = userEvent.setup();
    const region = await openWatchlists(locale);
    const search = await within(region).findByLabelText(L(locale, "Search watchlists"));
    await user.type(search, "zzz-no-such-list");
    await within(region).findByText(L(locale, "No watchlists match this search."));
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Clear watchlist search"), kind: "control" },
        { name: L(locale, "Create"), kind: "create" },
        { name: L(locale, 'Create watchlist "{name}"', { name: "zzz-no-such-list" }), kind: "create" },
      ]),
    );
  });

  it.each(LOCALES)("Empty (selected list has 0 members): inventory drops the member row + Open company, keeps the rest classified (%s)", async (locale) => {
    appTestState.watchlistMembershipsResponse = [];
    const region = await openWatchlists(locale);
    await within(region).findByText(L(locale, "No companies in this watchlist."));
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Add companies"), kind: "add" },
        { name: L(locale, "Create"), kind: "create" },
        { name: ROW_NAME, kind: "destination" },
        { name: L(locale, "Remove"), kind: "remove" },
        { name: L(locale, "Rename"), kind: "rename" },
      ]),
    );
  });
});
