import { fireEvent } from "@testing-library/react";
import { describe, it } from "vitest";
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
    expect(within(nav).getByRole("button", { name: "Today" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Compare" })).toBeInTheDocument();
  });

  it("opens the Today and Compare mode homes", async () => {
    renderApp();

    await userEvent.click(await screen.findByRole("button", { name: "Today" }));
    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Compare" }));
    expect(await screen.findByRole("heading", { name: "Compare" })).toBeInTheDocument();
  });

  it("creates a named view and lists it as a Modes nav destination (ADR 0057)", async () => {
    const user = userEvent.setup();
    renderApp();

    const nav = await screen.findByRole("navigation", { name: "Primary navigation" });
    await user.click(within(nav).getByRole("button", { name: "New view" }));

    await user.type(await screen.findByLabelText("View name"), "Earnings");
    await user.click(screen.getByRole("button", { name: /Create view/i }));

    // The saved view appears in the Modes group and the empty view prompts to add panels.
    expect(await within(nav).findByRole("button", { name: "Earnings" })).toBeInTheDocument();
    expect(await screen.findByText("This view is empty.")).toBeInTheDocument();

    // The view can be deleted from its sidebar entry.
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await user.click(within(nav).getByRole("button", { name: "Delete view: Earnings" }));
    await waitFor(() =>
      expect(within(nav).queryByRole("button", { name: "Earnings" })).not.toBeInTheDocument(),
    );
    confirm.mockRestore();
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
    expect(await screen.findByText("v0.3.0")).toBeInTheDocument();
    expect(screen.queryByText("ok 0.3.0")).not.toBeInTheDocument();
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

    // Ctrl+K focuses global search without leaving the current section.
    fireEvent.keyDown(document, { key: "K", code: "KeyK", ctrlKey: true });
    const searchInput = screen.getByLabelText("Global search");
    await waitFor(() => expect(searchInput).toHaveFocus());
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();

    fireEvent.keyDown(searchInput, { key: "3", code: "Digit3", ctrlKey: true });
    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();

    searchInput.blur();
    fireEvent.keyDown(document, { key: "8", code: "Digit8", ctrlKey: true });
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
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
