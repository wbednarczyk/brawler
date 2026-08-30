import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import { plText } from "../../shared/locale/resources/plText";

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];
function L(locale: Locale, en: string): string {
  return locale === "pl" ? (plText[en] ?? en) : en;
}

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

// F4b S1 red contract skeleton for the Events redesign
// (docs/plans/frontend-v2-f4b.md § Events, Action inventory). S3 implements
// the redesign; today's screen has no `ActionButton`/`data-action-kind`
// anywhere, so `collectActionInventory` reports every button as
// "unclassified" and every assertion below is expected red — do NOT make
// these green (S3 does).
//
// Default seed: Week view, current-week range, no active filters — one
// event ("Main Market - Corporate actions - Equity - CDR", company CDR).

const REGION_NAME = { en: "Events", pl: "Wydarzenia" } as const;
const EVENT_TITLE = "Main Market - Corporate actions - Equity - CDR";

async function openEvents(locale: Locale) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Events" });
  const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
  await within(region).findByText(EVENT_TITLE);
  return region;
}

function defaultInventory(locale: Locale): ActionInventoryEntry[] {
  return sorted([
    { name: L(locale, "Add event"), kind: "add" },
    { name: L(locale, "Refresh calendar"), kind: "refresh" },
    { name: L(locale, "Week"), kind: "control" },
    { name: L(locale, "List"), kind: "control" },
    { name: L(locale, "Previous week"), kind: "control" },
    { name: L(locale, "Next week"), kind: "control" },
    { name: L(locale, "Current week"), kind: "control" },
    { name: `${L(locale, "Open event")}: ${EVENT_TITLE}`, kind: "control" },
  ]);
}

describe("Events action inventory (F4b contract § Events, Action inventory)", () => {
  for (const locale of LOCALES) {
    it(`the full sorted action inventory matches the contract table, default state (${locale})`, async () => {
      const region = await openEvents(locale);
      expect(collectActionInventory(region, locale)).toEqual(defaultInventory(locale));
    });
  }

  it("no button in the screen root is left unclassified — today every button is (default state)", async () => {
    const region = await openEvents("en");
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});

describe("Events primary action per state (F4b contract § Events, decision 5)", () => {
  it("Success (default, no composer, no proposed selection): `addEvent` is the one filled action, marker and variant on the same element", async () => {
    const region = await openEvents("en");
    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });
});

describe("Events empty states (F4b contract § Events, State matrix)", () => {
  it.each(LOCALES)("Empty (week has no events, a later match exists): invitation with `Show next week with events` (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.companyEventsResponse = [];
    renderApp({ section: "Events" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    expect(collectEmptyStates(region)).toContain("invitation");
    expect(collectActionInventory(region, locale)).toEqual(
      sorted([
        { name: L(locale, "Add event"), kind: "add" },
        { name: L(locale, "Refresh calendar"), kind: "refresh" },
        { name: L(locale, "Week"), kind: "control" },
        { name: L(locale, "List"), kind: "control" },
        { name: L(locale, "Previous week"), kind: "control" },
        { name: L(locale, "Next week"), kind: "control" },
        { name: L(locale, "Current week"), kind: "control" },
        { name: L(locale, "Show next week with events"), kind: "control" },
      ]),
    );
  });
});
