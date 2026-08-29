import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import { collectActionInventory, collectEmptyStates, expectSinglePrimary } from "../../test/uxContracts";

// F4a S2 contract harness for the Companies library language pass
// (docs/plans/frontend-v2-f4a.md § Companies library — contract-exempt: no
// storyboard/state-matrix, but the action inventory + empty-state-as-invitation
// rules still apply). Originally an S1 red harness (`it` where the screen
// already matched, `it.fails`/`it.todo` naming the gaps); S2 built the
// missing behavior and flips every row to `it`.

const REGION_NAME = { en: "Companies", pl: "Spółki" } as const;

async function openCompanies(locale: "en" | "pl") {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Companies" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

describe("Companies action inventory (F4a contract § Companies library)", () => {
  it("Lookup and Add already carry their contract names", async () => {
    const region = await openCompanies("en");
    expect(within(region).getByRole("button", { name: "Lookup" })).toBeInTheDocument();
    expect(within(region).getByRole("button", { name: "Add" })).toBeInTheDocument();
  });

  it("Lookup carries kind=control and Add carries kind=add", async () => {
    const region = await openCompanies("en");
    const inventory = collectActionInventory(region, "en");
    const byName = new Map(inventory.map((entry) => [entry.name, entry.kind]));
    expect(byName.get("Lookup")).toBe("control");
    expect(byName.get("Add")).toBe("add");
  });

  it("Add is the only primary action at rest", async () => {
    const region = await openCompanies("en");
    expectSinglePrimary(region);
  });

  it("a company row's Open destination already carries the contract name shape", async () => {
    const region = await openCompanies("en");
    expect(within(region).getAllByRole("button", { name: /^Open / }).length).toBeGreaterThan(0);
  });

  it("a company row's Open destination carries kind=destination", async () => {
    const region = await openCompanies("en");
    const inventory = collectActionInventory(region, "en");
    const openRow = inventory.find((entry) => /^Open /.test(entry.name));
    expect(openRow?.kind).toBe("destination");
  });

  it("a watchlist-membership chip names itself \"Open watchlist …\", not just the list name", async () => {
    const region = await openCompanies("en");
    expect(within(region).queryByRole("button", { name: /^Open watchlist / })).not.toBeNull();
  });

  it("Manage settings destination carries kind=destination", async () => {
    const region = await openCompanies("en");
    const inventory = collectActionInventory(region, "en");
    const manageSettings = inventory.find((entry) => entry.name === "Manage settings");
    expect(manageSettings?.kind).toBe("destination");
  });

  it("a company row's remove control reads \"Remove\", not \"Delete\" (dec. 3 amendment)", async () => {
    const region = await openCompanies("en");
    // Every seeded row carries its own "Remove <ticker>" control (getAllByRole,
    // not getByRole/queryByRole — the default fixture tracks more than one
    // company).
    expect(within(region).getAllByRole("button", { name: /^Remove / }).length).toBeGreaterThan(0);
  });

  it("no button in the screen root is left unclassified", async () => {
    const region = await openCompanies("en");
    const unclassified = collectActionInventory(region, "en").filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });
});

describe("Companies empty states are invitations (F4a contract § Companies library)", () => {
  it("Empty (no companies) renders the invitation kind and its action focuses the lookup", async () => {
    appTestState.companiesResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openCompanies("en");
    await within(region).findByText("No companies in your library yet");
    expect(collectEmptyStates(region)).toContain("invitation");

    const user = userEvent.setup();
    await user.click(within(region).getByRole("button", { name: "Add your first company" }));
    expect(within(region).getByLabelText("Ticker")).toHaveFocus();
  });

  it("Empty (no filter match) renders the invitation kind and its action clears the filters", async () => {
    const region = await openCompanies("en");
    const user = userEvent.setup();
    await user.type(within(region).getByLabelText("Search tracked companies"), "zzz-no-match-zzz");

    await within(region).findByText("No companies match your filters");
    expect(collectEmptyStates(region)).toContain("invitation");

    await user.click(within(region).getByRole("button", { name: "Clear filters" }));
    expect(within(region).getByLabelText("Search tracked companies")).toHaveValue("");
  });

  it("Empty (registry no-match) renders the invitation kind with 'Add manually'", async () => {
    const region = await openCompanies("en");
    const user = userEvent.setup();
    await user.clear(within(region).getByLabelText("Name"));
    await user.type(within(region).getByLabelText("Name"), "Definitely Not In The Registry Zzz");

    await within(region).findByText("No match in the registry");
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(within(region).getByRole("button", { name: "Add manually" })).toBeInTheDocument();
  });
});
