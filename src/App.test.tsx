import { fireEvent } from "@testing-library/react";
import { describe, it } from "vitest";
import packageJson from "../package.json";
import type { AttentionEvent } from "./api/attention";
import {
  appTestState,
  expect,
  handleAppCommand,
  initialCompanies,
  invoke,
  renderApp,
  screen,
  userEvent,
  vi,
  waitFor,
  within,
} from "./test/appWorkflowHarness";

describe("Sidebar IA spine (ADR 0054)", () => {
  it("groups navigation into modes, library and utilities with mode destinations", async () => {
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    expect(within(nav).getByText("Modes")).toBeInTheDocument();
    expect(within(nav).getByText("Library")).toBeInTheDocument();
    expect(within(nav).getByText("Utilities")).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Today" })).toBeInTheDocument();
    // Compare is a live mode destination again (ADR 0089, v0.61): the
    // comparison read model + FX substrate now give the mode real content, so
    // the reserved slot under Dashboard is restored to the spine.
    expect(within(nav).getByRole("button", { name: "Compare" })).toBeInTheDocument();
  });

  it("opens the Today mode home", async () => {
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Today" }));
    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
  });

  it("creates a named view and lists it as a Modes nav destination (ADR 0057)", async () => {
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "New view" }));

    await user.type(await screen.findByLabelText("View name"), "Earnings");
    // Choose the 3×3 grid preset so the chosen dimensions flow end-to-end.
    await user.click(screen.getByRole("button", { name: "3×3" }));
    await user.click(screen.getByRole("button", { name: /Create view/i }));

    // The saved view appears in the Modes group and opens pre-split into a real
    // 3×3 grid — nine cells, each with a "Pick a panel" button (ADR 0057).
    expect(await within(nav).findByRole("button", { name: "Earnings" })).toBeInTheDocument();
    const cells = await screen.findAllByRole("button", { name: "Pick a panel" });
    expect(cells.length).toBe(9);

    // Since card 106f8a7 the add surface offers GENERIC company-scoped panel
    // types bound to the view company — so the flow picks the view company
    // first, then fills cells (the per-company palette entries are gone).
    const viewCompany = screen.getByLabelText("View company");
    const cdrOption = (
      within(viewCompany).getAllByRole("option") as HTMLOptionElement[]
    ).find((option) => option.textContent?.includes("GPW:CDR"));
    await user.selectOptions(viewCompany, cdrOption?.value ?? "");

    // Picking a panel for a cell fills it in place (one fewer empty cell, a new
    // panel tab appears).
    await user.click(cells[0]);
    await user.type(await screen.findByLabelText("Search commands"), "Fundamentals");
    await user.keyboard("{Enter}");
    await waitFor(() =>
      expect(screen.getAllByRole("button", { name: "Pick a panel" }).length).toBe(cells.length - 1),
    );

    // The view can be deleted from its sidebar entry.
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await user.click(within(nav).getByRole("button", { name: "Delete view: Earnings" }));
    await waitFor(() =>
      expect(within(nav).queryByRole("button", { name: "Earnings" })).not.toBeInTheDocument(),
    );
    confirm.mockRestore();
  });

  // Issue #89: a saved view renames in place from its sidebar row (pencil →
  // inline TextField → Enter), through the real rename_cockpit_layout command.
  it("renames a saved view inline from its sidebar row", async () => {
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "New view" }));
    await user.type(await screen.findByLabelText("View name"), "Morning");
    await user.click(screen.getByRole("button", { name: /Create view/i }));
    expect(await within(nav).findByRole("button", { name: "Morning" })).toBeInTheDocument();

    await user.click(within(nav).getByRole("button", { name: "Rename view: Morning" }));
    const field = within(nav).getByLabelText("View name");
    await user.clear(field);
    await user.type(field, "Evening{Enter}");

    expect(await within(nav).findByRole("button", { name: "Evening" })).toBeInTheDocument();
    expect(within(nav).queryByRole("button", { name: "Morning" })).not.toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("rename_cockpit_layout", {
        input: expect.objectContaining({ name: "Evening" }),
      });
    });
  });

  it("lists pinned companies in the spine and opens the company workspace", async () => {
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    expect(within(nav).getByText("Pinned companies")).toBeInTheDocument();
    const pinned = within(nav).getByRole("button", { name: "CDR" });
    await userEvent.click(pinned);

    // Opening a pinned company lands the curated cockpit dashboard (ADR 0057).
    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
  });

  it("unpins a company from the spine and persists via update_settings", async () => {
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    const unpin = within(nav).getByRole("button", { name: /Unpin from sidebar/ });
    await userEvent.click(unpin);

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith(
        "update_settings",
        expect.objectContaining({ input: expect.objectContaining({ pinnedCompanyIds: [] }) }),
      );
    });
  });
});

describe("App shell", () => {
  it("renders the investor inbox shell", async () => {
    renderApp();

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(
      (await screen.findAllByText("Current report placeholder for watchlist company")).length,
    ).toBeGreaterThan(0);
    expect(within(screen.getByLabelText("Feed items")).getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
    expect(
      screen.getAllByText("Sample official report used to validate feed filtering and detail rendering.").length,
    ).toBeGreaterThan(0);
    // The brand chip shows the health-reported version; the mock mirrors
    // package.json so this can never rot behind the real app (audit K12).
    expect(await screen.findByText(`v${packageJson.version}`)).toBeInTheDocument();
    expect(screen.queryByText(`ok ${packageJson.version}`)).not.toBeInTheDocument();
    expect(screen.queryByText("AI")).not.toBeInTheDocument();
    expect(screen.queryByText("Data")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open source status" })).toBeInTheDocument();
  });

  it("shows unread feed count in the Inbox navigation item", async () => {
    renderApp();

    const inboxNav = await screen.findByRole("button", { name: /Inbox/ });

    expect(within(inboxNav).getByText("1")).toHaveClass("nav-badge");
    expect(within(inboxNav).getByLabelText("1 unread feed item")).toBeInTheDocument();
  });

  it("shows Diagnostics navigation only in Developer mode", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Diagnostics" })).not.toBeInTheDocument();

    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };

    renderApp();

    expect(await screen.findByRole("button", { name: "Diagnostics" })).toBeInTheDocument();
    const navContainers = screen.getAllByLabelText("Primary navigation");
    const currentNav = navContainers[navContainers.length - 1];
    // Diagnostics is the developer-gated utility in the sidebar spine (ADR 0054).
    expect(within(currentNav).getByRole("button", { name: "Diagnostics" })).toBeInTheDocument();
  });

  it("shows Developer-mode local metrics in Diagnostics", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };

    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Diagnostics" }));

    expect(await screen.findByRole("heading", { name: "Metrics" })).toBeInTheDocument();
    expect(await screen.findByText("brawler_source_refresh_total")).toBeInTheDocument();
    expect(screen.getByText("adapter_id=bankier-company-komunikaty · status=succeeded")).toBeInTheDocument();
    expect(screen.getByText("512 KiB")).toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_local_metrics_snapshot");
    });
  });

  it("registers app-level shortcuts and suppresses them while searching", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "2", code: "Digit2", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Companies" })).toBeInTheDocument();

    // Ctrl+F focuses global search without leaving the current section.
    fireEvent.keyDown(document, { key: "F", code: "KeyF", ctrlKey: true });
    const searchInput = screen.getByLabelText("Global search");
    await waitFor(() => expect(searchInput).toHaveFocus());
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();

    fireEvent.keyDown(searchInput, { key: "3", code: "Digit3", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();

    searchInput.blur();
    fireEvent.keyDown(document, { key: "8", code: "Digit8", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("opens the global command palette on Ctrl+K", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    // Ctrl+K opens the global command palette (v0.50 U6), not focusing search.
    fireEvent.keyDown(document, { key: "K", code: "KeyK", ctrlKey: true });
    const palette = await screen.findByRole("dialog", { name: "Command palette" });
    // App-level commands (derived from the shortcut registry) are listed.
    expect(within(palette).getByRole("button", { name: "Open Settings" })).toBeInTheDocument();

    // Running a listed command navigates and closes the palette.
    await userEvent.click(within(palette).getByRole("button", { name: "Open Settings" }));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Command palette" })).not.toBeInTheDocument();
  });

  it("also opens the command palette on Meta+K (macOS ⌘K twin)", async () => {
    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "K", code: "KeyK", metaKey: true });
    expect(await screen.findByRole("dialog", { name: "Command palette" })).toBeInTheDocument();
  });

  it("uses configured shortcut bindings from settings", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      shortcutBindings: {
        "app.openCompanies": {
          key: "C",
          ctrlKey: true,
        },
        "app.openInbox": {
          key: "1",
          ctrlKey: true,
          disabled: true,
        },
      },
    };

    renderApp();

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("get_settings");
    });

    fireEvent.keyDown(document, { key: "C", code: "KeyC", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Companies" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "1", code: "Digit1", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
  });
});

// Live-defect fix (v0.57 fix wave 2): the persistent-toast overflow summary
// ("+N more") bridges its translated label from AppStateRoot into App.tsx via
// a ref (see App.tsx's `persistentOverflowRef` doc comment) so ToastProvider
// can mount above LocaleContext. A ref write alone does not force the label
// closure that already rendered to be re-evaluated with the Polish text, so
// the summary row kept showing the English "+9 more" literal in a Polish app
// (owner screenshot: 27-toast-stack.png). The fix must make the label
// reactive, not patch around the ref.
describe("Persistent-toast overflow summary — locale reactivity (D1 fix)", () => {
  const signalRule = {
    id: "alert_rule_overflow_1",
    triggerType: "signal_category" as const,
    signalCategory: "profit_warning",
    priceMin: null,
    priceMax: null,
    scopeType: "company" as const,
    scopeRef: initialCompanies[0].id,
    enabled: true,
    createdAt: "2026-01-01T00:00:00Z",
    updatedAt: "2026-01-01T00:00:00Z",
  };

  function overflowAttentionEvent(id: string): AttentionEvent {
    return {
      id,
      ruleId: signalRule.id,
      triggerType: "signal_category",
      companyId: initialCompanies[0].id,
      evidenceType: "company_signal",
      evidenceRef: `signal_${id}`,
      firedAt: "2026-06-10T09:00:00Z",
      seen: false,
      dismissed: false,
      // signal_category + profit_warning → urgent, the only level that raises a
      // PERSISTENT toast (ADR 0087 dec. 3) — exactly what the overflow cap exercises.
      severity: "urgent",
      evidenceTitle: null,
      evidenceDetail: null,
    };
  }

  it("renders the '+N more' summary in Polish once the Polish locale is active", async () => {
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      locale: "pl",
    };
    appTestState.autopilotRunsResponse = [];
    appTestState.alertRulesResponse = [signalRule];
    // PERSISTENT_VISIBLE_CAP is 3 — 5 unseen events leave 2 collapsed into the
    // overflow row.
    appTestState.attentionEventsResponse = Array.from({ length: 5 }, (_, i) =>
      overflowAttentionEvent(`attn_overflow_${i}`),
    );

    renderApp({ section: "Today" });

    const summary = await screen.findByRole("button", { name: /^\+\d+ więcej$/ });
    expect(summary).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /more$/ })).not.toBeInTheDocument();
  });

  it("still switches the already-rendered summary to Polish when the locale settles AFTER the toast burst (the real race)", async () => {
    // The live defect: the attention-events/alert-rules round trip can settle
    // (and fire the whole persistent-toast burst) before the settings fetch
    // that carries `locale` resolves. With no further toast/dismiss after
    // that, the ref-bridge version never re-rendered the summary once the
    // Polish label was finally bound — it stayed on the English literal that
    // was current when the burst rendered. Reproduce that ordering exactly:
    // hold `get_settings` open until the English summary has already painted,
    // then release it and assert the SAME summary element updates in place.
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      locale: "pl",
    };
    appTestState.autopilotRunsResponse = [];
    appTestState.alertRulesResponse = [signalRule];
    // Distinct ids from the previous test: `toastedAttentionEventIds` is a
    // module-scoped "this app session" dedup Set (TodayScreen.tsx) that
    // outlives a single test's render — reused ids would already be marked
    // toasted and silently produce no new toasts at all here.
    appTestState.attentionEventsResponse = Array.from({ length: 5 }, (_, i) =>
      overflowAttentionEvent(`attn_overflow_race_${i}`),
    );

    let releaseSettings: (() => void) | undefined;
    const settingsGate = new Promise<void>((resolve) => {
      releaseSettings = resolve;
    });
    vi.mocked(invoke).mockImplementation(async (command, args) => {
      if (command === "get_settings") {
        await settingsGate;
      }
      return handleAppCommand(command as string, args as Record<string, unknown> | undefined);
    });

    renderApp({ section: "Today" });

    // The toast burst fires and renders before settings/locale ever resolves.
    const summary = await screen.findByRole("button", { name: /^\+\d+ more$/ });

    // Now let the delayed `get_settings` (locale: "pl") resolve — with no new
    // toast and no dismissal, the fix must still flip this same element to
    // the Polish label.
    releaseSettings?.();
    await waitFor(() => expect(summary).toHaveAccessibleName(/^\+\d+ więcej$/));
  });
});
