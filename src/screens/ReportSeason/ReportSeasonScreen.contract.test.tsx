import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
} from "../../test/uxContracts";

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

// F4b S1 red contract skeleton for Report Season — a language pass + nav
// entry + `useCommandQuery` migration, NOT a journey redesign
// (docs/plans/frontend-v2-f4b.md § Report Season, short form). Today's
// screen has no `ActionButton`/`data-action-kind` anywhere, so the
// assertions below are expected red — do NOT make them green (S4 does).
// Scoped to a spot-check (not the sorted full-array shape the redesign
// files use): the real DB seed (multiple upcoming companies) makes an
// exhaustive per-row array fragile for a classification-only pass.

const REGION_NAME = { en: "Report Season", pl: "Sezon raportów" } as const;

async function openReportSeason(locale: Locale) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "ReportSeason" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

describe("Report Season action inventory (F4b contract § Report Season)", () => {
  it("no button in the screen root is left unclassified — today several are", async () => {
    const region = await openReportSeason("en");
    // Wait for at least one upcoming-report row to render before scanning.
    await within(region).findAllByRole("button", { expanded: false });
    const unclassified = collectActionInventory(region, "en").filter(
      (entry) => entry.kind === "unclassified",
    );
    expect(unclassified).toEqual([]);
  });
});

describe("Report Season primary action per state (F4b contract § Report Season, expanded card)", () => {
  it("Expanded card: `markAs`/`add` is the one filled action, marker and variant on the same element", async () => {
    const user = userEvent.setup();
    const region = await openReportSeason("en");
    const [firstRow] = await within(region).findAllByRole("button", { expanded: false });
    await user.click(firstRow);

    expectSinglePrimary(region, 1);
    expectPrimaryMarkerMatchesVariant(region);
  });
});

describe("Report Season empty states (F4b contract § Report Season, State table)", () => {
  it.each(LOCALES)("Empty (scope has no upcoming reports): an invitation, not a bare sentence (%s)", async (locale) => {
    appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
    appTestState.reportSeasonUpcomingResponse = [];
    renderApp({ section: "ReportSeason" });
    const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
    await within(region).findByText(
      locale === "pl"
        ? "Brak nadchodzących raportów w tym zakresie. Poszerz zakres listy, aby zobaczyć więcej."
        : "No upcoming reports in scope. Widen the watchlist scope to see more.",
    );
    expect(collectEmptyStates(region)).toContain("invitation");
  });
});
