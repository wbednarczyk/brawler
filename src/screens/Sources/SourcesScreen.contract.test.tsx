import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import { makeSourceAdapters } from "../../test/scenarios/entities";

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

// F4b S4 (docs/plans/frontend-v2-f4b.md § Sources, contract-exempt short
// form): brought to the F4a full sorted-array shape per the S1 integrator
// note — the 5-adapter mock seed (`makeSourceAdapters`) is small enough for
// an exhaustive inventory, unlike the real DB's 19-row Dense state.

const REGION_NAME = { en: "Sources", pl: "Źródła" } as const;

// SOURCE_ADAPTER_SPECS order (src/test/scenarios/entities.ts): GPW Company
// Directory (company_registry), Bankier Company Komunikaty (official_report),
// Bankier Giełda RSS (public_media), Bankier Wiadomosci RSS (public_media),
// Portal Analiz (authenticated_research, `visibility: "developer"` —
// developer-only, hidden in the default non-developer mode this test runs
// in; see `SourcesScreen.test.tsx` "shows normal-user source status and
// hides developer-only candidates"). Display names are proper nouns —
// identical in both locales.
const ADAPTER_NAMES = [
  "GPW Company Directory",
  "Bankier Company Komunikaty",
  "Bankier Giełda RSS",
  "Bankier Wiadomosci RSS",
] as const;

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

function restInventory(locale: Locale): ActionInventoryEntry[] {
  const refreshLabel = locale === "pl" ? "Odśwież źródła" : "Refresh sources";
  const openPrefix = locale === "pl" ? "Otwórz źródło" : "Open source";
  return sorted([
    { name: refreshLabel, kind: "refresh" },
    ...ADAPTER_NAMES.map((name) => ({ name: `${openPrefix}: ${name}`, kind: "control" })),
  ]);
}

// Expanding the company_registry adapter (GPW Company Directory) adds its
// detail-panel actions: `Open source page` (open), the registry's own
// `Refresh company directory` (refresh), and the collapsed `Companies`
// disclosure (control) — the richest branch, exercising every non-row kind
// in one state without also expanding the nested company list.
function expandedInventory(locale: Locale): ActionInventoryEntry[] {
  return sorted([
    ...restInventory(locale),
    { name: locale === "pl" ? "Otwórz stronę źródła" : "Open source page", kind: "open" },
    { name: locale === "pl" ? "Odśwież katalog spółek" : "Refresh company directory", kind: "refresh" },
    { name: locale === "pl" ? "Spółki" : "Companies", kind: "control" },
  ]);
}

async function openSources(locale: Locale) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  appTestState.sourceAdaptersResponse = makeSourceAdapters();
  renderApp({ section: "Sources" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

describe("Sources action inventory (F4b contract § Sources)", () => {
  it.each(LOCALES)("the full sorted action inventory matches the contract table, rest state (%s)", async (locale) => {
    const region = await openSources(locale);
    await within(region).findAllByRole("button", { name: new RegExp(locale === "pl" ? "^Otwórz źródło:" : "^Open source:") });
    expect(collectActionInventory(region, locale)).toEqual(restInventory(locale));
  });

  it.each(LOCALES)("the full sorted action inventory matches the contract table, one row expanded (%s)", async (locale) => {
    const user = userEvent.setup();
    const region = await openSources(locale);
    const openPrefix = locale === "pl" ? "Otwórz źródło" : "Open source";
    const row = await within(region).findByRole("button", { name: `${openPrefix}: GPW Company Directory` });
    await user.click(row);
    await within(region).findByRole("button", { name: locale === "pl" ? "Otwórz stronę źródła" : "Open source page" });
    expect(collectActionInventory(region, locale)).toEqual(expandedInventory(locale));
  });

  it("no button in the screen root is left unclassified — every action is now classified", async () => {
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

  it("still no primary once a row is expanded — Sources never has a filled action", async () => {
    const user = userEvent.setup();
    const region = await openSources("en");
    const row = await within(region).findByRole("button", { name: "Open source: GPW Company Directory" });
    await user.click(row);
    await within(region).findByRole("button", { name: "Open source page" });
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
    await within(region).findByText(locale === "pl" ? "Brak źródeł" : "No sources");
    expect(collectEmptyStates(region)).toContain("invitation");
  });
});
