import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

// F4b S1 red contract skeleton for Sources — a language-pass, NOT a redesign
// (docs/plans/frontend-v2-f4b.md § Sources, contract-exempt short form): the
// screen keeps its journey/primitive shape, only copy + classification
// change (S4). Today's screen has no `ActionButton`/`data-action-kind`
// anywhere, so the action-inventory assertion below is expected red — do NOT
// make it green (S4 does). Scoped to a spot-check (not the sorted full-array
// shape the redesign files use): the real DB seed's row count (§ Sources
// "Dense 19 rows") makes an exhaustive per-row array fragile for a
// classification-only pass — S4 is free to promote this to a full array once
// every row carries `data-action-kind`.

const REGION_NAME = { en: "Sources", pl: "Źródła" } as const;

async function openSources(locale: Locale) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Sources" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

describe("Sources action inventory (F4b contract § Sources)", () => {
  it.each(LOCALES)("the header refresh action is classified `refresh` (%s)", async (locale) => {
    const region = await openSources(locale);
    const refreshLabel = locale === "pl" ? "Odśwież źródła" : "Refresh sources";
    const inventory = collectActionInventory(region, locale);
    expect(inventory.find((entry) => entry.name === refreshLabel)).toEqual({
      name: refreshLabel,
      kind: "refresh",
    });
  });

  it("no button in the screen root is left unclassified — today several are", async () => {
    const region = await openSources("en");
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});

describe("Sources primary action per state (F4b contract § Sources: `expectSinglePrimary(root, 0)`)", () => {
  it("the screen has no primary at rest, and every marked primary carries variant=\"primary\"", async () => {
    const region = await openSources("en");
    expectSinglePrimary(region, 0);
    expectPrimaryMarkerMatchesVariant(region);
  });
});

describe("Sources empty states (F4b contract § Sources, State table)", () => {
  it.each(LOCALES)("Empty (no sources configured): an invitation with the refresh action (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.sourceAdaptersResponse = [];
    renderApp({ section: "Sources" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    await within(region).findByText(
      locale === "pl" ? "Brak skonfigurowanych źródeł." : "No sources configured.",
    );
    expect(collectEmptyStates(region)).toContain("invitation");
  });
});
