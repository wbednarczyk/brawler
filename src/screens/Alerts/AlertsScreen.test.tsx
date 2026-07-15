import { describe, it } from "vitest";
import { axe } from "jest-axe";
import {
  expect,
  invoke,
  renderApp,
  screen,
  userEvent,
  within,
} from "../../test/appWorkflowHarness";

// T3 (ADR 0068), relocated 2026-07-15 (owner decision, v0.54): the Alerts
// manager is a standalone Library sidebar screen, not a Settings section — the
// UI entry point for the CRUD commands (create/list/update/enable/delete) plus
// the fired-event review (list/dismiss). Full-app workflow tests against the
// stateful mock runtime (ADR 0048) so they exercise the real command surface.

async function openAlertsScreen(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Alerts" }));
  const region = await screen.findByRole("region", { name: "Alerts" });
  // The screen loads its rules + events on mount. The trigger presets render as
  // selectable chips (redesign v0.54) — wait for the first chip as the ready cue.
  await within(region).findByRole("button", { name: "Profit warning" });
  return region;
}

describe("Alerts screen — rules manager (T3, ADR 0068; relocated v0.54)", () => {
  it("renders as its own Library sidebar destination, not a Settings section", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Alerts" });

    await expect(screen.getByRole("heading", { name: /^Alerts$/ })).toBeVisible();

    // The Settings screen no longer hosts an "Alerts" section tab.
    await user.click(screen.getByRole("button", { name: "Settings" }));
    const settingsRegion = await screen.findByLabelText("Application settings");
    expect(within(settingsRegion).queryByRole("button", { name: "Alerts" })).not.toBeInTheDocument();
  });

  it("shows a live preview sentence rebuilt from the current selections", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    // The preview restates the draft rule in plain language (redesign v0.54).
    const preview = within(region).getByRole("note", { name: "Alert preview" });
    expect(preview).toHaveTextContent(/profit warning/i);

    // Switching the trigger and entering a price range rebuilds the sentence.
    await user.click(within(region).getByRole("button", { name: "Price range" }));
    await user.type(within(region).getByLabelText<HTMLInputElement>("Minimum price"), "80");
    await user.type(within(region).getByLabelText<HTMLInputElement>("Maximum price"), "100");
    expect(preview).toHaveTextContent(/80.*100/);
  });

  it("creates a rule from a preset chip and a scope target", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    await user.click(within(region).getByRole("button", { name: "Insider transactions" }));
    await user.click(within(region).getByRole("button", { name: "Add alert" }));

    expect(invoke).toHaveBeenCalledWith(
      "create_alert_rule",
      expect.objectContaining({
        input: expect.objectContaining({
          triggerType: "signal_category",
          signalCategory: "insider_transaction",
        }),
      }),
    );
  });

  it("creates a price-range rule with the min/max inputs bound two-way", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    await user.click(within(region).getByRole("button", { name: "Price range" }));
    const min = within(region).getByLabelText<HTMLInputElement>("Minimum price");
    const max = within(region).getByLabelText<HTMLInputElement>("Maximum price");
    await user.clear(min);
    await user.type(min, "20");
    await user.clear(max);
    await user.type(max, "30");
    await user.click(within(region).getByRole("button", { name: "Add alert" }));

    expect(invoke).toHaveBeenCalledWith(
      "create_alert_rule",
      expect.objectContaining({
        input: expect.objectContaining({
          triggerType: "price_enters_range",
          priceMin: 20,
          priceMax: 30,
        }),
      }),
    );
  });

  it("edits an existing price-range rule's bounds through update_alert_rule", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    // Create a price-range rule so a row with editable bounds exists.
    await user.click(within(region).getByRole("button", { name: "Price range" }));
    await user.type(within(region).getByLabelText<HTMLInputElement>("Minimum price"), "20");
    await user.type(within(region).getByLabelText<HTMLInputElement>("Maximum price"), "30");
    await user.click(within(region).getByRole("button", { name: "Add alert" }));

    const row = await within(region).findByRole("listitem", { name: /price range/i });
    const rowMin = within(row).getByLabelText<HTMLInputElement>(/minimum price/i);
    await user.clear(rowMin);
    await user.type(rowMin, "25");
    await user.tab();

    expect(invoke).toHaveBeenCalledWith(
      "update_alert_rule",
      expect.objectContaining({ input: expect.objectContaining({ priceMin: 25 }) }),
    );
  });

  it("disables a rule through its enable toggle", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    // The minimal scenario seeds one enabled rule.
    const toggle = await within(region).findByRole("switch", { name: /enabled/i });
    expect(toggle).toBeChecked();
    await user.click(toggle);

    expect(invoke).toHaveBeenCalledWith(
      "set_alert_rule_enabled",
      expect.objectContaining({ input: expect.objectContaining({ enabled: false }) }),
    );
  });

  it("deletes a rule with an undo toast", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    const rule = await within(region).findByRole("listitem", { name: /alert rule/i });
    await user.click(within(rule).getByRole("button", { name: "Delete" }));

    expect(invoke).toHaveBeenCalledWith(
      "delete_alert_rule",
      expect.objectContaining({ input: expect.objectContaining({ id: expect.any(String) }) }),
    );
    // The reversible-delete pattern offers an undo (ADR 0076 D5).
    expect(await screen.findByRole("button", { name: "Undo" })).toBeInTheDocument();
  });

  it("dismisses a fired alert event", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    const event = await within(region).findByRole("listitem", { name: /fired alert/i });
    await user.click(within(event).getByRole("button", { name: "Dismiss" }));

    expect(invoke).toHaveBeenCalledWith(
      "dismiss_attention_event",
      expect.objectContaining({ input: expect.objectContaining({ id: expect.any(String) }) }),
    );
  });

  it("rejects an identical duplicate rule with an inline error", async () => {
    const user = userEvent.setup();
    renderApp();
    const region = await openAlertsScreen(user);

    await user.click(within(region).getByRole("button", { name: "Insider transactions" }));
    await user.click(within(region).getByRole("button", { name: "Add alert" }));
    await user.click(within(region).getByRole("button", { name: "Add alert" }));

    // Backend parity (storage/attention.rs DuplicateAlertRule): the mock rejects
    // the twin and the screen surfaces it inline instead of inserting a duplicate.
    expect(
      await within(region).findByText(/identical alert rule already exists/i),
    ).toBeInTheDocument();
    expect(within(region).getAllByRole("button", { name: "Delete" })).toHaveLength(2);
  });

  it("has no accessibility violations", async () => {
    const user = userEvent.setup();
    const { container } = renderApp();
    await openAlertsScreen(user);
    await user.click(screen.getByRole("button", { name: "Price range" }));

    const results = await axe(container, { rules: { region: { enabled: false } } });
    expect(results.violations.map((violation) => violation.id)).toEqual([]);
  });
});
