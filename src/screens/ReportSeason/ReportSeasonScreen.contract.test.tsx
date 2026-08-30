import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";
import { COMPANY_SPECS, makeReportSeasonEntry } from "../../test/scenarios/entities";
import type { ReportSeasonEntry } from "../../api/reportSeason";

// F4b S4 (docs/plans/frontend-v2-f4b.md § Report Season, short form): brought
// to the F4a full-array shape per the S1 integrator note. `reportSeasonPast`
// has no `appTestState` override (only `reportSeasonUpcomingResponse` is
// exposed by the shared scenario harness), so the "no button unclassified" /
// "no duplicate name" checks below run against the real seed's rows too —
// still meaningful (they catch a classification regression on ANY row) —
// while the exact-name assertions are scoped to the one controlled upcoming
// entry this file seeds itself. The "expectation exists → `Mark as
// prepared` is primary" state needs `listReportExpectations`/
// `expectationReview` seeded, which this shared harness cannot do (no
// `appTestState` field) — that state is instead covered by
// `ReportSeasonScreen.test.tsx`'s direct-mock render.

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

const REGION_NAME = { en: "Report Season", pl: "Sezon raportów" } as const;
const cdr = COMPANY_SPECS.find((spec) => spec.key === "cdr")!;
const controlledEntry = makeReportSeasonEntry(cdr, true);

async function openReportSeason(locale: Locale, upcoming: ReportSeasonEntry[] = [controlledEntry]) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  appTestState.reportSeasonUpcomingResponse = upcoming;
  renderApp({ section: "ReportSeason" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

describe("Report Season action inventory (F4b contract § Report Season)", () => {
  it.each(LOCALES)("no button in the screen root is left unclassified (%s)", async (locale) => {
    const region = await openReportSeason(locale);
    await within(region).findAllByRole("button", { expanded: false });
    const unclassified = collectActionInventory(region, locale).filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });

  it.each(LOCALES)("no two buttons share an accessible name at rest (%s)", async (locale) => {
    const region = await openReportSeason(locale);
    await within(region).findAllByRole("button", { expanded: false });
    const names = collectActionInventory(region, locale).map((entry) => entry.name);
    expect(new Set(names).size).toBe(names.length);
  });

  it.each(LOCALES)(
    "the controlled upcoming row's toggle is `control`, named `Open report: <company> <date>` (%s)",
    async (locale) => {
      const region = await openReportSeason(locale);
      const prefix = locale === "pl" ? "Otwórz raport" : "Open report";
      const row = await within(region).findByRole("button", {
        name: `${prefix}: ${controlledEntry.displayName} ${controlledEntry.eventDate}`,
      });
      expect(row.getAttribute("data-action-kind")).toBe("control");
    },
  );
});

describe("Report Season primary action per state (F4b contract § Report Season, expanded card)", () => {
  it("Expanded card, no expectations: `Add expectations` is the one filled action, marker+variant on the same element", async () => {
    const region = await openReportSeason("en");
    const row = await within(region).findByRole("button", {
      name: `Open report: ${controlledEntry.displayName} ${controlledEntry.eventDate}`,
    });
    const user = userEvent.setup();
    await user.click(row);
    await within(region).findByRole("button", { name: "Add expectations" });

    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
    const primary = region.querySelector('[data-ux-primary-action="true"]');
    expect(primary).toHaveTextContent("Add expectations");
  });

  it("no two buttons share an accessible name once the card is expanded", async () => {
    const region = await openReportSeason("en");
    const row = await within(region).findByRole("button", {
      name: `Open report: ${controlledEntry.displayName} ${controlledEntry.eventDate}`,
    });
    const user = userEvent.setup();
    await user.click(row);
    await within(region).findByRole("button", { name: "Add expectations" });
    const names = collectActionInventory(region, "en").map((entry) => entry.name);
    expect(new Set(names).size).toBe(names.length);
  });
});

describe("Report Season empty states (F4b contract § Report Season, State table)", () => {
  it.each(LOCALES)("Empty (scope has no upcoming reports): an invitation, not a bare sentence (%s)", async (locale) => {
    const region = await openReportSeason(locale, []);
    await within(region).findByText(
      locale === "pl"
        ? "Brak nadchodzących raportów w tym zakresie. Poszerz zakres listy, aby zobaczyć więcej."
        : "No upcoming reports in scope. Widen the watchlist scope to see more.",
    );
    expect(collectEmptyStates(region)).toContain("invitation");
  });
});
