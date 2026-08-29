import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, within } from "../../test/appWorkflowHarness";
import { collectActionInventory, collectEmptyStates, expectSinglePrimary } from "../../test/uxContracts";

// F4a S1 contract harness for the Alerts changed workflow
// (docs/plans/frontend-v2-f4a.md § Alerts). S4a/S4b implement the workflow +
// language changes; this file exercises today's screen against the
// contract's action-inventory / primary-action / empty-state rows so the
// gaps are named and provable. `it` where today's screen already matches,
// `it.fails` naming the exact contract row where it does not yet — S4a/S4b
// flip them to `it` as they land (never delete a row, flip it).
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

describe("Alerts action inventory (F4a contract § Alerts, Action inventory)", () => {
  it("Add alert already carries the primary marker (but not yet its dictionary kind)", async () => {
    const region = await openAlerts("en");
    const addAlert = within(region).getByRole("button", { name: "Add alert" });
    expect(addAlert).toHaveAttribute("data-ux-primary-action", "true");
  });

  it("Add alert carries kind=add", async () => {
    const region = await openAlerts("en");
    const inventory = collectActionInventory(region, "en");
    const addAlert = inventory.find((entry) => entry.name === "Add alert");
    expect(addAlert?.kind).toBe("add");
  });

  it("Dodaj alert carries kind=add (PL)", async () => {
    const region = await openAlerts("pl");
    const inventory = collectActionInventory(region, "pl");
    const addAlert = inventory.find((entry) => entry.name === "Dodaj alert");
    expect(addAlert?.kind).toBe("add");
  });

  it("today's rule row shows Remove, not Delete (dec. 3 amendment: remove is the only collection-removal verb)", async () => {
    const region = await openAlerts("en");
    await within(region).findByRole("listitem", { name: /alert rule/i });
    expect(within(region).queryByRole("button", { name: "Remove" })).not.toBeNull();
    expect(within(region).queryByRole("button", { name: "Delete" })).toBeNull();
  });

  it("today's rule row offers Pause/Resume, not an Enabled checkbox", async () => {
    const region = await openAlerts("en");
    await within(region).findByRole("listitem", { name: /alert rule/i });
    expect(within(region).queryByRole("switch", { name: /enabled/i })).toBeNull();
    expect(
      within(region).queryByRole("button", { name: "Pause" }) ??
        within(region).queryByRole("button", { name: "Resume" }),
    ).not.toBeNull();
  });

  it("a fired alert row offers an Open destination (first red journey test target)", async () => {
    const region = await openAlerts("en");
    const firedRow = await within(region).findByRole("listitem", { name: /fired alert/i });
    expect(within(firedRow).queryByRole("button", { name: /^Open/ })).not.toBeNull();
  });

  it("no button in the screen root is left unclassified", async () => {
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
});

describe("Alerts empty states (F4a contract § Alerts, State matrix)", () => {
  it("Empty (no rules) renders the invitation kind, not the legacy shape", async () => {
    appTestState.alertRulesResponse = [];
    const region = await openAlerts("en");
    await within(region).findByText("You don't have any alerts yet");
    expect(collectEmptyStates(region)).toContain("invitation");
  });

  it("Empty (nothing fired) renders the quiet kind, not the legacy shape", async () => {
    appTestState.attentionEventsResponse = [];
    const region = await openAlerts("en");
    await within(region).findByText("All quiet — nothing has fired. That's the point.");
    expect(collectEmptyStates(region)).toContain("quiet");
  });
});
