import { fireEvent } from "@testing-library/react";
import { describe, it } from "vitest";
import packageJson from "../package.json";
import {
  appTestState,
  expect,
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
    // /^Today/: the ambient-attention badge (ADR 0097) joins the accessible
    // name when unseen events exist — same idiom as the Inbox unread badge.
    expect(within(nav).getByRole("button", { name: /^Today/ })).toBeInTheDocument();
  });

  it("opens the Today mode home", async () => {
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: /^Today/ }));
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

    // Opening a pinned company lands the Spółka screen (F3a S1, ADR 0107).
    expect(await screen.findByRole("region", { name: "Company view" })).toBeInTheDocument();
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

  it("shows the unseen-attention badge on the Today navigation item (ADR 0097 dec. 4)", async () => {
    // The sample dataset carries exactly one unseen non-routine attention event.
    renderApp();

    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    expect(await within(todayNav).findByText("1")).toHaveClass("nav-badge");
    expect(within(todayNav).getByLabelText("1 new important item in Today")).toBeInTheDocument();
  });

  it("clears the Today badge on a visit — seen means 'was on screen' (ADR 0097 dec. 5)", async () => {
    renderApp();

    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    await within(todayNav).findByText("1");

    await userEvent.click(todayNav);
    await screen.findByRole("heading", { name: "Today" });

    // Today's stream batch-marks the loaded unseen events; the optimistic flip
    // empties the badge without waiting for a refetch.
    await waitFor(() => expect(within(todayNav).queryByText("1")).toBeNull());
    expect(invoke).toHaveBeenCalledWith(
      "mark_attention_events_seen",
      expect.objectContaining({ input: expect.objectContaining({ ids: expect.any(Array) }) }),
    );
  });

  it("does not count routine attention events toward the Today badge", async () => {
    appTestState.attentionEventsResponse = appTestState.attentionEventsResponse.map((event) => ({
      ...event,
      severity: "routine" as const,
    }));
    renderApp();

    const todayNav = await screen.findByRole("button", { name: "Today" });
    expect(within(todayNav).queryByText(/^\d+$/)).toBeNull();
  });

  it("announces a count INCREASE politely, but never replays the startup backlog (ADR 0097 dec. 4)", async () => {
    const { container } = renderApp();

    // Hydration: the backlog (1 unseen event) lights the badge…
    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    await within(todayNav).findByText("1");
    const liveRegion = container.querySelector('[aria-live="polite"]') as HTMLElement;
    expect(liveRegion).not.toBeNull();
    // …but is NOT announced.
    expect(liveRegion).toHaveTextContent("");

    // A refresh brings two MORE unseen events (3 total) — one coalesced polite
    // announcement states the new count.
    appTestState.attentionEventsResponse = [
      ...appTestState.attentionEventsResponse,
      { ...appTestState.attentionEventsResponse[0], id: "attn_live_2" },
      { ...appTestState.attentionEventsResponse[0], id: "attn_live_3" },
    ];
    await userEvent.click(screen.getByRole("button", { name: "Refresh sources" }));

    await waitFor(() =>
      expect(liveRegion).toHaveTextContent("3 new important items in Today"),
    );
  });

  it("clears the announcement on a decrease so a repeat cycle re-announces (0→1→0→1)", async () => {
    const { container } = renderApp();
    const todayNav = await screen.findByRole("button", { name: /^Today/ });
    await within(todayNav).findByText("1");
    const liveRegion = container.querySelector('[aria-live="polite"]') as HTMLElement;

    // The refresh control's accessible name tracks its state ("Refreshing…",
    // "Sources refreshed") — capture the node once and click it by reference.
    const refreshButton = screen.getByRole("button", { name: "Refresh sources" });

    // Cycle 1: +1 event (1 → 2) — announced.
    const base = appTestState.attentionEventsResponse[0];
    appTestState.attentionEventsResponse = [base, { ...base, id: "attn_cycle_2" }];
    await userEvent.click(refreshButton);
    await waitFor(() => expect(liveRegion).toHaveTextContent("2 new important items in Today"));

    // Visiting Today clears the count — the region must EMPTY (a later identical
    // announcement needs a real DOM mutation, or aria-live stays silent).
    await userEvent.click(todayNav);
    await screen.findByRole("heading", { name: "Today" });
    await waitFor(() => expect(liveRegion).toHaveTextContent(""));

    // Cycle 2: two fresh events arrive (0 → 2) — the SAME sentence re-announces.
    appTestState.attentionEventsResponse = [
      { ...base, id: "attn_cycle_3" },
      { ...base, id: "attn_cycle_4" },
    ];
    await userEvent.click(refreshButton);
    await waitFor(() => expect(liveRegion).toHaveTextContent("2 new important items in Today"));
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

