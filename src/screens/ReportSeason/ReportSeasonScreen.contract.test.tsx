import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";
import {
  collectActionInventory,
  collectEmptyStates,
  expectPrimaryMarkerMatchesVariant,
  expectSinglePrimary,
  expectPhrasingOnlyExpandableRows,
} from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";
import {
  COMPANY_SPECS,
  makeFinancialFact,
  makeFinancialPeriod,
  makeReportSeasonEntry,
} from "../../test/scenarios/entities";
import type { ReportSeasonEntry } from "../../api/reportSeason";

// F4b S4 (docs/plans/frontend-v2-f4b.md § Report Season, short form), sol R1
// fix wave: table-drives every reachable substate in both locales with exact
// sorted inventories + `expectPrimaryMarkerMatchesVariant` — default, card
// expanded without expectations, with expectations (unfrozen), composer
// open, frozen-unresolved review. `reportExpectationsResponse`/
// `financialPeriodsResponse` (appWorkflowHarness.tsx, MockRuntime) were added
// rather than skipping the expectation-seeded states. `reportSeasonPast` has
// no override, but "minimal"'s 3 default past rows are the STATIC (non-
// expandable) branch — a plain ListRow with no button inside — so they never
// contribute to the action inventory; the exact-name assertions stay exact.

const LOCALES = ["en", "pl"] as const;
type Locale = (typeof LOCALES)[number];

const REGION_NAME = { en: "Report Season", pl: "Sezon raportów" } as const;
const cdr = COMPANY_SPECS.find((spec) => spec.key === "cdr")!;
const controlledEntry = makeReportSeasonEntry(cdr, true);
const rowLabel = { en: "Open report", pl: "Otwórz raport" } as const;

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

function rowToggleName(locale: Locale): string {
  return `${rowLabel[locale]}: ${controlledEntry.displayName} ${controlledEntry.eventDate}`;
}

function restInventory(locale: Locale): ActionInventoryEntry[] {
  return sorted([{ name: rowToggleName(locale), kind: "control" }]);
}

const LABELS = {
  en: {
    markAsPrepared: "Mark as prepared",
    markAsReviewed: "Mark as reviewed",
    company: "Company",
    claims: "Claims",
    addExpectations: "Add expectations",
    editExpectations: "Edit expectations",
    addMetric: "Add metric",
    save: "Save",
    cancel: "Cancel",
    saveVerdict: "Save verdict",
  },
  pl: {
    markAsPrepared: "Oznacz jako przygotowane",
    markAsReviewed: "Oznacz jako przejrzane",
    company: "Spółka",
    claims: "Tezy",
    addExpectations: "Dodaj oczekiwania",
    editExpectations: "Zmień oczekiwania",
    addMetric: "Dodaj miarę",
    save: "Zapisz",
    cancel: "Anuluj",
    saveVerdict: "Zapisz ocenę",
  },
} as const;

// The prep-checklist ActionRow, present in every expanded state regardless
// of expectation/composer state.
function cardChecklistInventory(locale: Locale): ActionInventoryEntry[] {
  const t = LABELS[locale];
  return [
    { name: t.markAsPrepared, kind: "markAs" },
    { name: t.markAsReviewed, kind: "markAs" },
    { name: t.company, kind: "destination" },
    { name: t.claims, kind: "destination" },
  ];
}

async function openReportSeason(locale: Locale, upcoming: ReportSeasonEntry[] = [controlledEntry]) {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  appTestState.reportSeasonUpcomingResponse = upcoming;
  renderApp({ section: "ReportSeason" });
  return screen.findByRole("region", { name: REGION_NAME[locale] });
}

async function expandCard(locale: Locale, region: HTMLElement, user: ReturnType<typeof userEvent.setup>) {
  const row = await within(region).findByRole("button", { name: rowToggleName(locale) });
  await user.click(row);
  await within(region).findByRole("button", { name: LABELS[locale].markAsPrepared });
}

function seedExpectation(locale: Locale, overrides: Record<string, unknown> = {}) {
  appTestState.reportExpectationsResponse = [
    {
      id: "report_expectation_contract_1",
      companyId: controlledEntry.companyId,
      eventKey: controlledEntry.eventKey,
      fiscalYear: 2026,
      periodType: "H1",
      stanceMd: "Contract-test stance",
      frozenAt: null,
      resolutionNoteMd: null,
      resolvedAt: null,
      createdAt: "2026-06-01T00:00:00Z",
      updatedAt: "2026-06-01T00:00:00Z",
      metrics: [],
      ...overrides,
    },
  ];
  void locale;
}

describe("Report Season action inventory (F4b contract § Report Season)", () => {
  it.each(LOCALES)("the full sorted action inventory matches the contract table, rest state (%s)", async (locale) => {
    const region = await openReportSeason(locale);
    await within(region).findByRole("button", { name: rowToggleName(locale) });
    expect(collectActionInventory(region, locale)).toEqual(restInventory(locale));
    expectPrimaryMarkerMatchesVariant(region);
    expectSinglePrimary(region, 0);
  });

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, card expanded without expectations (%s)",
    async (locale) => {
      const user = userEvent.setup();
      const region = await openReportSeason(locale);
      await expandCard(locale, region, user);
      await within(region).findByRole("button", { name: LABELS[locale].addExpectations });

      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...restInventory(locale),
          ...cardChecklistInventory(locale),
          { name: LABELS[locale].addExpectations, kind: "add" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(LABELS[locale].addExpectations);
    },
  );

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, card expanded with an unfrozen expectation (%s)",
    async (locale) => {
      seedExpectation(locale);
      const user = userEvent.setup();
      const region = await openReportSeason(locale);
      await expandCard(locale, region, user);
      await within(region).findByRole("button", { name: LABELS[locale].editExpectations });

      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...restInventory(locale),
          ...cardChecklistInventory(locale),
          { name: LABELS[locale].editExpectations, kind: "edit" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(LABELS[locale].markAsPrepared);
    },
  );

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, composer open (%s)",
    async (locale) => {
      const user = userEvent.setup();
      const region = await openReportSeason(locale);
      await expandCard(locale, region, user);
      await user.click(await within(region).findByRole("button", { name: LABELS[locale].addExpectations }));
      await within(region).findByRole("button", { name: LABELS[locale].save });

      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...restInventory(locale),
          ...cardChecklistInventory(locale),
          { name: LABELS[locale].addMetric, kind: "add" },
          { name: LABELS[locale].save, kind: "save" },
          { name: LABELS[locale].cancel, kind: "control" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(LABELS[locale].save);
    },
  );

  it.each(LOCALES)(
    "the full sorted action inventory matches the contract table, frozen-unresolved review (%s)",
    async (locale) => {
      seedExpectation(locale, { fiscalYear: 2026, periodType: "FY" });
      appTestState.financialPeriodsResponse = [makeFinancialPeriod(cdr, 2026)];
      appTestState.financialFactsResponse = [makeFinancialFact(cdr, 2026)];
      const user = userEvent.setup();
      const region = await openReportSeason(locale);
      await expandCard(locale, region, user);
      await within(region).findByRole("button", { name: LABELS[locale].saveVerdict });

      expect(collectActionInventory(region, locale)).toEqual(
        sorted([
          ...restInventory(locale),
          ...cardChecklistInventory(locale),
          { name: LABELS[locale].saveVerdict, kind: "save" },
        ]),
      );
      expectPrimaryMarkerMatchesVariant(region);
      expectSinglePrimary(region, 1);
      const primary = region.querySelector('[data-ux-primary-action="true"]');
      expect(primary).toHaveTextContent(LABELS[locale].saveVerdict);
    },
  );

  it("no button in the screen root is left unclassified", async () => {
    const region = await openReportSeason("en");
    await within(region).findByRole("button", { name: rowToggleName("en") });
    const unclassified = collectActionInventory(region, "en").filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });

  // Sol R2: consumer guard — the ExpandableRow summary is a real <button>,
  // so the rendered rows must stay phrasing-only (no <ul>/<li>/<div>).
  it("expandable report rows contain only phrasing content", async () => {
    const region = await openReportSeason("en");
    await within(region).findByRole("button", { name: rowToggleName("en") });
    expectPhrasingOnlyExpandableRows(region);
  });

  it.each(LOCALES)("no two buttons share an accessible name in any state (%s)", async (locale) => {
    const user = userEvent.setup();
    const region = await openReportSeason(locale);
    await expandCard(locale, region, user);
    await within(region).findByRole("button", { name: LABELS[locale].addExpectations });
    const names = collectActionInventory(region, locale).map((entry) => entry.name);
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
    expectPrimaryMarkerMatchesVariant(region);
  });
});
