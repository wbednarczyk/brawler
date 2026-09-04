import { afterEach, describe, expect, it } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";

import { ActivityPanel } from "./ActivityPanel";
import { LocaleContext, makeTextTranslator, makeTranslator, type LocaleCode } from "../../locale";
import { COMPANY_SPECS, makeCompany, makeActivityView } from "../../../test/scenarios/entities";
import { collectActionInventory } from "../../../test/uxContracts";
import { plText } from "../../locale/resources/plText";
import type { ActivityFamily } from "../../../api/generated/ActivityFamily";
import type { ActivityItem } from "../../../api/generated/ActivityItem";
import type { ActivityView } from "../../../api/generated/ActivityView";
import { familyLabel, statusLabel } from "./activityLabels";

// `Modal` renders through a portal to `document.body` (never a descendant of
// the launching pane) — every query here reads `document.body`/`screen`, NOT
// the local `render()` container, which stays empty.

const LOCALES: LocaleCode[] = ["en", "pl"];

const companies = COMPANY_SPECS.slice(0, 3).map(makeCompany);
const seededView = makeActivityView(companies);
const emptyView = makeActivityView([]);

// Every `ActivityFamily` / `ActivityItem.status` member (sol diff R1 #17):
// the `satisfies Record<…, true>` object fails to compile when the generated
// union gains a member this list does not name (same technique as
// alertLabels.test.ts).
const ALL_FAMILIES = {
  sourceRefresh: true,
  companyRefresh: true,
  registryRefresh: true,
  fxPull: true,
  fundamentalsPull: true,
  briefing: true,
  historyFetch: true,
  reportSweep: true,
  reextraction: true,
  reportReading: true,
  ownershipReading: true,
  managementReading: true,
  priceHistory: true,
  kpiIngest: true,
  transcript: true,
  corrupted: true,
} satisfies Record<ActivityFamily, true>;
const ALL_FAMILY_VALUES = Object.keys(ALL_FAMILIES) as ActivityFamily[];

const ALL_STATUSES = {
  queued: true,
  running: true,
  stalled: true,
  succeeded: true,
  failed: true,
  partial: true,
  interrupted: true,
} satisfies Record<ActivityItem["status"], true>;
const ALL_STATUS_VALUES = Object.keys(ALL_STATUSES) as ActivityItem["status"][];

// Identity `text()`: returns the source string untouched, so calling the
// label function with it recovers the raw EN source constant regardless of
// which locale label() call sites resolve — used to look that constant up
// in `plText` directly (same technique as test #1 below, made exhaustive).
const identityText = (value: string) => value;

describe.each(LOCALES)("ActivityPanel labels — exhaustive family/status coverage (%s, sol diff R1 #17)", (locale) => {
  const text = makeTextTranslator(locale);

  it.each(ALL_FAMILY_VALUES)("familyLabel(%s) resolves a real text()-backed label", (family) => {
    const label = familyLabel(family, text);
    expect(label).toBeTruthy();
    if (locale === "pl") {
      const source = familyLabel(family, identityText);
      expect(plText[source as keyof typeof plText], `missing plText entry for family source "${source}"`).toBeDefined();
    }
  });

  it.each(ALL_STATUS_VALUES)("statusLabel(%s) resolves a real text()-backed label", (status) => {
    const label = statusLabel(status, text);
    expect(label).toBeTruthy();
    if (locale === "pl") {
      const source = statusLabel(status, identityText);
      expect(plText[source as keyof typeof plText], `missing plText entry for status source "${source}"`).toBeDefined();
    }
  });
});

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

// sol diff R1 #14/#15 — a minimal, fully-specified ActivityItem so each test
// below overrides only the field it cares about.
function makeItem(overrides: Partial<ActivityItem>): ActivityItem {
  return {
    id: "job_runs:test-item",
    activityKey: "test-activity-key",
    family: "reportSweep",
    status: "running",
    subject: "TEST",
    companyId: null,
    qualifiedTicker: null,
    progress: null,
    inFlight: null,
    attempt: 1,
    startedAt: "2026-09-04T10:00:00Z",
    finishedAt: null,
    error: null,
    members: [],
    target: { kind: "sources" },
    ...overrides,
  };
}

describe("ActivityPanel — sol diff R1 fixes (#14, #15)", () => {
  afterEach(() => cleanup());

  it("expanded parent row lists members and progress.failed; attempt hidden until it exceeds 1 (#14)", () => {
    const item = makeItem({
      id: "job_runs:sweep-members",
      family: "reportSweep",
      subject: "TEST",
      progress: { done: 3, total: 8, failed: 5 },
      attempt: 3,
      members: ["Raport bieżący Q1 2026.pdf", "Poranny przegląd"],
    });
    const view: ActivityView = { active: [item], queued: [], recent: [], generatedAt: "2026-09-04T12:00:00Z" };
    const root = renderPanel("pl", { view });
    const row = root.querySelector<HTMLElement>(".activity-item")!;
    fireEvent.click(row.querySelector<HTMLButtonElement>(".expandable-row")!);

    expect(within(row).getByText("Raport bieżący Q1 2026.pdf")).toBeInTheDocument();
    expect(within(row).getByText("Poranny przegląd")).toBeInTheDocument();
    // 5 failed -> Polish "many" category -> "nieudanych" (declined, not the
    // hardcoded singular "nieudane").
    expect(row.textContent).toContain("nieudanych");
    // attempt = 3 (> 1) is shown.
    expect(within(row).getByText("Próba")).toBeInTheDocument();
  });

  it("attempt is omitted from the detail when it is exactly 1 (#14)", () => {
    const item = makeItem({ id: "job_runs:first-attempt", attempt: 1 });
    const view: ActivityView = { active: [item], queued: [], recent: [], generatedAt: "2026-09-04T12:00:00Z" };
    const root = renderPanel("en", { view });
    const row = root.querySelector<HTMLElement>(".activity-item")!;
    fireEvent.click(row.querySelector<HTMLButtonElement>(".expandable-row")!);
    expect(within(row).queryByText("Attempt")).not.toBeInTheDocument();
  });

  it("a document subject is mono only when it looks like a filename — a human title is not (#15)", () => {
    const filenameItem = makeItem({
      id: "job_runs:filename-subject",
      family: "reportReading",
      subject: "Raport bieżący Q2 2026.pdf",
      target: { kind: "company", companyId: "c1", tool: { t: "dokumenty", documentId: "doc1" } },
    });
    const humanTitleItem = makeItem({
      id: "job_runs:human-title-subject",
      family: "kpiIngest",
      subject: "Poranny przegląd",
      target: { kind: "company", companyId: "c1", tool: null },
    });
    const view: ActivityView = { active: [filenameItem, humanTitleItem], queued: [], recent: [], generatedAt: "2026-09-04T12:00:00Z" };
    renderPanel("en", { view });

    expect(screen.getByText("Raport bieżący Q2 2026.pdf").className).toContain("activity-subject-mono");
    expect(screen.getByText("Poranny przegląd").className).not.toContain("activity-subject-mono");
  });

  it("In progress / Recent counts decline through pluralNoun in Polish (#15)", () => {
    const active = Array.from({ length: 5 }, (_, i) => makeItem({ id: `job_runs:active-${i}`, status: "running" }));
    const queued = Array.from({ length: 2 }, (_, i) => makeItem({ id: `job_runs:queued-${i}`, status: "queued" }));
    const recent = Array.from({ length: 3 }, (_, i) =>
      makeItem({ id: `job_runs:recent-${i}`, status: "succeeded", finishedAt: "2026-09-04T09:00:00Z" }),
    );
    const view: ActivityView = { active, queued, recent, generatedAt: "2026-09-04T12:00:00Z" };
    const root = renderPanel("pl", { view });

    // 5 active -> Polish "many" -> "aktywnych" (not the invariant "aktywne").
    expect(root.textContent).toContain("5 aktywnych");
    // "queued" stays invariant regardless of count.
    expect(root.textContent).toContain("2 w kolejce");
    // 3 recent -> Polish "few" -> "zadania".
    expect(root.textContent).toContain("3 zadania");
  });

  it("the Modal close control carries a localized accessible name in Polish (#15)", () => {
    renderPanel("pl");
    expect(screen.getByRole("button", { name: "Zamknij okno" })).toBeInTheDocument();
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
