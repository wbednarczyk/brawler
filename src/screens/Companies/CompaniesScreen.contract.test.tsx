import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, within } from "../../test/appWorkflowHarness";
import { collectActionInventory, collectEmptyStates } from "../../test/uxContracts";

// F4a S1 contract harness for the Companies library language pass
// (docs/plans/frontend-v2-f4a.md § Companies library — contract-exempt: no
// storyboard/state-matrix, but the action inventory + empty-state-as-invitation
// rules still apply). S2 implements the language pass; this file exercises
// today's screen against those two rows so the gaps are named and provable.
// `it` where today's screen already matches, `it.fails` naming the exact
// contract row where it does not yet, `it.todo` for a state that has no
// rendering path at all yet (S2 builds it) — S2 flips them to `it`.

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

  it.fails("Lookup carries kind=control and Add carries kind=add", async () => {
    const region = await openCompanies("en");
    const inventory = collectActionInventory(region, "en");
    const byName = new Map(inventory.map((entry) => [entry.name, entry.kind]));
    expect(byName.get("Lookup")).toBe("control");
    expect(byName.get("Add")).toBe("add");
  });

  it("a company row's Open destination already carries the contract name shape", async () => {
    const region = await openCompanies("en");
    expect(within(region).getAllByRole("button", { name: /^Open / }).length).toBeGreaterThan(0);
  });

  it.fails("a company row's Open destination carries kind=destination", async () => {
    const region = await openCompanies("en");
    const inventory = collectActionInventory(region, "en");
    const openRow = inventory.find((entry) => /^Open /.test(entry.name));
    expect(openRow?.kind).toBe("destination");
  });

  it.fails("a watchlist-membership chip names itself \"Open watchlist …\", not just the list name", async () => {
    const region = await openCompanies("en");
    expect(within(region).queryByRole("button", { name: /^Open watchlist / })).not.toBeNull();
  });

  it.fails("Manage settings destination carries kind=destination", async () => {
    const region = await openCompanies("en");
    const inventory = collectActionInventory(region, "en");
    const manageSettings = inventory.find((entry) => entry.name === "Manage settings");
    expect(manageSettings?.kind).toBe("destination");
  });

  it.fails("a company row's remove control reads \"Remove\", not \"Delete\" (dec. 3 amendment)", async () => {
    const region = await openCompanies("en");
    expect(within(region).queryByRole("button", { name: /^Remove / })).not.toBeNull();
  });

  it.fails("no button in the screen root is left unclassified", async () => {
    const region = await openCompanies("en");
    const unclassified = collectActionInventory(region, "en").filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });
});

describe("Companies empty states are invitations (F4a contract § Companies library)", () => {
  it.fails("Empty (no companies) renders the invitation kind focusing the lookup, not the legacy shape", async () => {
    appTestState.companiesResponse = [];
    appTestState.watchlistMembershipsResponse = [];
    const region = await openCompanies("en");
    await within(region).findByText("No companies yet.");
    expect(collectEmptyStates(region)).toContain("invitation");
  });

  // The registry no-match empty state ("Nie znaleziono w rejestrze" + "Dodaj
  // ręcznie") has no rendering path today at all — the registry-suggestions
  // block only renders when there IS at least one match
  // (CompaniesScreen.tsx: `companyFormRegistryMatches.length > 0 ? … : null`).
  // S2 builds the no-match branch; this row is a todo, not a red assertion,
  // until there is a DOM state to assert against.
  it.todo("Empty (registry no-match) renders the invitation kind with 'Dodaj ręcznie'");
});
