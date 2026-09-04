import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";

import { ActivityPanel } from "./ActivityPanel";
import { LocaleContext, makeTextTranslator, makeTranslator, type LocaleCode } from "../../locale";
import { COMPANY_SPECS, makeCompany, makeActivityView } from "../../../test/scenarios/entities";
import { collectActionInventory } from "../../../test/uxContracts";
import { plText } from "../../locale/resources/plText";

// `Modal` renders through a portal to `document.body` (never a descendant of
// the launching pane) — every query here reads `document.body`/`screen`, NOT
// the local `render()` container, which stays empty.

const LOCALES: LocaleCode[] = ["en", "pl"];

const companies = COMPANY_SPECS.slice(0, 3).map(makeCompany);
const seededView = makeActivityView(companies);
const emptyView = makeActivityView([]);

afterEach(() => {
  cleanup();
});

function renderPanel(locale: LocaleCode, overrides: Partial<Parameters<typeof ActivityPanel>[0]> = {}) {
  const value = { locale, t: makeTranslator(locale), text: makeTextTranslator(locale) };
  render(
    <LocaleContext.Provider value={value}>
      <ActivityPanel
        open
        onClose={() => {}}
        view={seededView}
        hydrated
        error={null}
        onRetry={() => {}}
        onNavigate={() => {}}
        {...overrides}
      />
    </LocaleContext.Provider>,
  );
  return document.body;
}

describe.each(LOCALES)("ActivityPanel contract (%s)", (locale) => {
  it("renders every family/status label via text() — the seeded families read translated in Polish", () => {
    const root = renderPanel(locale);
    expect(screen.getByRole("dialog")).toBeInTheDocument();

    const expectedFamilies =
      locale === "pl"
        ? ["Czytanie raportów", "Czytanie raportu", "Odświeżanie źródła", "Poranny przegląd"]
        : ["Bulk report reading", "Report reading", "Source refresh", "Morning briefing"];
    for (const label of expectedFamilies) {
      expect(root.textContent).toContain(label);
    }
    // Every EN family source string this seeded view exercises resolves to a
    // real plText entry (a missing one would silently render English).
    for (const literal of ["Bulk report reading", "Report reading", "Source refresh", "Morning briefing"]) {
      expect(plText[literal as keyof typeof plText]).toBeDefined();
    }
  });

  it("exactly one destination action per row", () => {
    const root = renderPanel(locale);
    const rows = root.querySelectorAll(".activity-item");
    const inventory = collectActionInventory(root, locale);
    const destinations = inventory.filter((entry) => entry.kind === "destination");
    expect(rows.length).toBeGreaterThan(0);
    expect(destinations.length).toBe(rows.length);
  });

  it("groups rows under a TickerLabel company heading, non-company rows under Sources and system", () => {
    const root = renderPanel(locale);
    const headings = Array.from(root.querySelectorAll(".activity-group-heading"));
    expect(headings.length).toBeGreaterThan(0);
    const systemHeading = headings.find((node) => node.querySelector(".ticker-label") === null);
    expect(systemHeading).toBeDefined();
    expect(systemHeading?.textContent).toBe(locale === "pl" ? "Źródła i system" : "Sources and system");
  });

  it("expanded failed row shows the raw error text verbatim", () => {
    const root = renderPanel(locale);
    const failedItem = seededView.recent.find((item) => item.status === "failed" && item.error)!;
    expect(failedItem).toBeDefined();
    const row = Array.from(root.querySelectorAll<HTMLElement>(".activity-item")).find((node) =>
      node.textContent?.includes(failedItem.subject),
    );
    expect(row).toBeDefined();
    const toggle = row!.querySelector<HTMLButtonElement>(".expandable-row");
    expect(toggle).toBeDefined();
    fireEvent.click(toggle!);
    expect(within(row!).getByText(failedItem.error!)).toBeInTheDocument();
  });

  it("carries no data-ux-primary-action", () => {
    const root = renderPanel(locale);
    expect(root.querySelectorAll('[data-ux-primary-action="true"]')).toHaveLength(0);
  });

  it("the empty knob renders EmptyState kind=quiet", () => {
    const root = renderPanel(locale, { view: emptyView });
    expect(root.querySelector('[data-empty-kind="quiet"]')).toBeInTheDocument();
  });
});

describe("ActivityPanel — loading and error states", () => {
  it("renders Skeleton rows before hydration", () => {
    const root = renderPanel("en", { hydrated: false, view: null });
    expect(root.querySelector(".ui-skeleton")).toBeInTheDocument();
  });

  it("renders the error strip with a retry control, keeping last-known-good rows", () => {
    renderPanel("en", { error: "boom" });
    expect(screen.getByRole("alert")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Try again" })).toBeInTheDocument();
    expect(within(screen.getByRole("dialog")).getAllByRole("button", { name: /^Open /i }).length).toBeGreaterThan(0);
  });
});
