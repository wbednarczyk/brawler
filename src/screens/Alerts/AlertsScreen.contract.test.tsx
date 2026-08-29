import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, within } from "../../test/appWorkflowHarness";
import { collectActionInventory, collectEmptyStates, expectSinglePrimary } from "../../test/uxContracts";
import type { ActionInventoryEntry } from "../../test/uxContracts";

// F4a S1 contract harness for the Alerts changed workflow
// (docs/plans/frontend-v2-f4a.md § Alerts). S4a/S4b implement the workflow +
// language changes; this file exercises today's screen against the
// contract's action-inventory / primary-action / empty-state rows.
//
// Fix-C guardrail 3 (sol F4a R1 finding 3): every state below asserts the
// FULL sorted action inventory (`toEqual`), not a `Map` lookup of a few
// names. Two entries sharing a name (e.g. "Add alert" appears TWICE in the
// no-rules state — the composer's own quiet button plus the invitation's
// filled one, guardrail 8) are now VISIBLE in the array rather than hidden
// by `Map` key collision.
//
// The default ("minimal") scenario seeds one enabled alert rule and one fired
// attention event (AlertsScreen.test.tsx's existing coverage relies on the
// same seed) — no rule/event creation needed here.

const REGION_NAME = { en: "Alerts", pl: "Alerty" } as const;

async function openAlerts(locale: "en" | "pl") {
  appTestState.settingsResponse = { ...appTestState.settingsResponse, locale };
  renderApp({ section: "Alerts" });
  const region = await screen.findByRole("region", { name: REGION_NAME[locale] });
  // The screen loads its rules + events on mount; the trigger presets are the
  // ready cue (mirrors AlertsScreen.test.tsx's openAlertsScreen helper).
  await within(region).findByRole("button", { name: locale === "pl" ? "Ostrzeżenie o wynikach" : "Profit warning" });
  return region;
}

function sorted(entries: ActionInventoryEntry[]): ActionInventoryEntry[] {
  return [...entries].sort(
    (a, b) => a.name.localeCompare(b.name, "en") || a.kind.localeCompare(b.kind, "en"),
  );
}

// Success state: 8 trigger-preset chips + the Company/Watchlist scope toggle
// + the composer's Add alert + the seeded rule's Pause/Remove + the seeded
// fired event's Open company/Dismiss.
const DEFAULT_EN: ActionInventoryEntry[] = sorted([
  { name: "52-week low", kind: "control" },
  { name: "Add alert", kind: "add" },
  { name: "Analyst recommendation", kind: "control" },
  { name: "Auditor opinion", kind: "control" },
  { name: "Autopilot finished", kind: "control" },
  { name: "Company", kind: "control" },
  { name: "Dismiss", kind: "control" },
  { name: "Insider transactions", kind: "control" },
  { name: "Open company", kind: "destination" },
  { name: "Pause", kind: "pause" },
  { name: "Price range", kind: "control" },
  { name: "Profit warning", kind: "control" },
  { name: "Remove", kind: "remove" },
  { name: "Short position", kind: "control" },
  { name: "Watchlist", kind: "control" },
]);

const DEFAULT_PL: ActionInventoryEntry[] = sorted([
  { name: "Autopilot zakończony", kind: "control" },
  { name: "Dodaj alert", kind: "add" },
  { name: "Lista obserwowanych", kind: "control" },
  { name: "Minimum 52-tygodniowe", kind: "control" },
  { name: "Odrzuć", kind: "control" },
  { name: "Opinia audytora", kind: "control" },
  { name: "Ostrzeżenie o wynikach", kind: "control" },
  { name: "Otwórz spółkę", kind: "destination" },
  { name: "Pozycja krótka", kind: "control" },
  { name: "Rekomendacja analityka", kind: "control" },
  { name: "Spółka", kind: "control" },
  { name: "Transakcje insiderów", kind: "control" },
  { name: "Usuń", kind: "remove" },
  { name: "Wstrzymaj", kind: "pause" },
  { name: "Zakres ceny", kind: "control" },
]);

describe("Alerts action inventory (F4a contract § Alerts, Action inventory)", () => {
  it("the full sorted action inventory matches the contract table (EN)", async () => {
    const region = await openAlerts("en");
    await within(region).findByRole("listitem", { name: /alert rule/i });
    expect(collectActionInventory(region, "en")).toEqual(DEFAULT_EN);
  });

  it("the full sorted action inventory matches the contract table (PL)", async () => {
    const region = await openAlerts("pl");
    await within(region).findByRole("listitem", { name: /reguł/i });
    expect(collectActionInventory(region, "pl")).toEqual(DEFAULT_PL);
  });

  it("today's rule row offers Pause/Resume, not an Enabled checkbox", async () => {
    const region = await openAlerts("en");
    await within(region).findByRole("listitem", { name: /alert rule/i });
    expect(within(region).queryByRole("switch", { name: /enabled/i })).toBeNull();
  });

  it("a fired alert row offers an Open destination (first red journey test target)", async () => {
    const region = await openAlerts("en");
    const firedRow = await within(region).findByRole("listitem", { name: /fired alert/i });
    expect(within(firedRow).queryByRole("button", { name: /^Open/ })).not.toBeNull();
  });

  it("no button in the screen root is left unclassified (default state)", async () => {
    const region = await openAlerts("en");
    await within(region).findByRole("listitem", { name: /alert rule/i });
    const unclassified = collectActionInventory(region, "en").filter((entry) => entry.kind === "unclassified");
    expect(unclassified).toEqual([]);
  });
});

describe("Alerts primary action per state (F4a contract § Alerts, State matrix)", () => {
  it("Success: Add alert is the one filled action", async () => {
    const region = await openAlerts("en");
    expectSinglePrimary(region, 1);
  });

  it("Empty (no rules): Add alert is still the one filled action", async () => {
    appTestState.alertRulesResponse = [];
    const region = await openAlerts("en");
    await within(region).findByText("You don't have any alerts yet");
    expectSinglePrimary(region, 1);
  });

  it("Empty (no rules): the composer's own Add alert goes quiet — only ONE filled (variant=primary) button renders (Fix-C guardrail 8)", async () => {
    appTestState.alertRulesResponse = [];
    const region = await openAlerts("en");
    await within(region).findByText("You don't have any alerts yet");
    const filled = region.querySelectorAll('[data-ui-button-variant="primary"]');
    expect(filled.length).toBe(1);
    // The invitation's action IS that one filled button.
    expect(filled[0]).toHaveAttribute("data-ux-primary-action", "true");
  });
});

describe("Alerts empty states (F4a contract § Alerts, State matrix)", () => {
  it("Empty (no rules): full inventory drops Pause/Remove, gains a SECOND (quiet) Add alert — duplicates visible", async () => {
    appTestState.alertRulesResponse = [];
    const region = await openAlerts("en");
    await within(region).findByText("You don't have any alerts yet");
    expect(collectEmptyStates(region)).toContain("invitation");
    // The composer's own "Add alert" (now quiet) plus the invitation's
    // "Add alert" (filled) — both carry kind="add" regardless of which one
    // is visually filled, so the array legitimately holds the name TWICE.
    expect(collectActionInventory(region, "en")).toEqual(
      sorted([
        ...DEFAULT_EN.filter((entry) => entry.name !== "Pause" && entry.name !== "Remove"),
        { name: "Add alert", kind: "add" },
      ]),
    );
  });

  it("Empty (nothing fired): full inventory drops Dismiss/Open company, renders the quiet kind", async () => {
    appTestState.attentionEventsResponse = [];
    const region = await openAlerts("en");
    await within(region).findByText("All quiet — nothing has fired. That's the point.");
    expect(collectEmptyStates(region)).toContain("quiet");
    expect(collectActionInventory(region, "en")).toEqual(
      sorted(DEFAULT_EN.filter((entry) => entry.name !== "Dismiss" && entry.name !== "Open company")),
    );
  });
});
