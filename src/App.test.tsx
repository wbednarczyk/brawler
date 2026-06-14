import { fireEvent } from "@testing-library/react";
import { describe, it } from "vitest";
import {
  appTestState,
  currentWeekTestDate,
  expect,
  initialCompanies,
  initialFeedItems,
  initialGeminiCredentialStatus,
  initialNotebookEntry,
  invoke,
  openUrl,
  renderApp,
  screen,
  userEvent,
  vi,
  waitFor,
  within,
} from "./test/appWorkflowHarness";

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
    const user = userEvent.setup();

    renderApp();

    const inboxNav = await screen.findByRole("button", { name: /Inbox/ });

    expect(within(inboxNav).getByText("1")).toHaveClass("nav-badge");
    expect(within(inboxNav).getByLabelText("1 unread feed item")).toBeInTheDocument();

    const navigationResizer = screen.getByRole("separator", { name: "Resize navigation" });
    expect(navigationResizer).toHaveAttribute("aria-valuenow", "190");

    navigationResizer.focus();
    await user.keyboard("{ArrowRight}");

    expect(navigationResizer).toHaveAttribute("aria-valuenow", "202");
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
    const navButtons = within(currentNav).getAllByRole("button");
    expect(navButtons[navButtons.length - 1]).toHaveAccessibleName("Diagnostics");
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
