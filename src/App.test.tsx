import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

const initialFeedItems = [
  {
    id: "feed_fixture_cdr_report",
    company: "GPW:CDR",
    type: "Official report",
    source: "GPW ESPI/EBI",
    time: "Today 09:12",
    title: "Current report placeholder for watchlist company",
    unread: true,
    saved: false,
    sourceUrl: "https://www.gpw.pl/komunikaty",
    language: "pl",
    publishedAt: "Today 09:12",
    fetchedAt: "Today 09:15",
    attribution: "GPW",
    summary: "Fixture official report used to validate feed filtering and detail rendering.",
  },
  {
    id: "feed_fixture_pkn_news",
    company: "GPW:PKN",
    type: "News",
    source: "Fixture feed",
    time: "Yesterday",
    title: "Fixture item proving the inbox layout can scan dense rows",
    unread: false,
    saved: true,
    sourceUrl: "https://example.local/fixture/pkn",
    language: "en",
    publishedAt: "Yesterday",
    fetchedAt: "Yesterday",
    attribution: "Fixture",
    summary: "Saved fixture item used to validate the saved filter before real ingestion exists.",
  },
  {
    id: "feed_fixture_msft_transcript",
    company: "NASDAQ:MSFT",
    type: "Transcript",
    source: "Local fixture",
    time: "Mon",
    title: "Transcript-derived note candidate waits for future provider work",
    unread: false,
    saved: false,
    sourceUrl: "https://example.local/fixture/msft-transcript",
    language: "en",
    publishedAt: "Mon",
    fetchedAt: "Mon",
    attribution: "Fixture",
    summary: "Transcript placeholder for future video and notebook workflows.",
  },
];

const sourceAdapters = [
  {
    id: "gpw-espi-ebi",
    displayName: "GPW ESPI/EBI",
    sourceType: "official_report",
    fetchMode: "public_page",
    enabled: true,
    defaultPollIntervalSeconds: 900,
    lastSuccessAt: null,
    lastErrorAt: null,
    lastError: null,
    markets: ["GPW"],
  },
];

const initialSettings = {
  theme: "dark",
  accentPalette: "night-neon",
  pollIntervalSeconds: 900,
  settingsSource: "sqlite",
  settingsImportExportFormat: "yaml",
  yamlImportExportStatus: "accepted_deferred",
  aiProviders: {
    youtubeTranscriptionProvider: "gemini",
    generalAnalysisProvider: null,
  },
  aiAnalysisMode: "source_grounded",
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("App", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
      if (command === "health") {
        return Promise.resolve({ status: "ok", version: "0.1.0" });
      }

      if (command === "database_status") {
        return Promise.resolve({
          appliedMigrations: 1,
          companies: 0,
          sourceAdapters: 1,
          settings: 7,
        });
      }

      if (command === "list_companies") {
        return Promise.resolve([
          {
            id: "company_gpw_cdr",
            exchange: "GPW",
            ticker: "CDR",
            qualifiedTicker: "GPW:CDR",
            displayName: "CD PROJEKT S.A.",
            isin: "PLOPTTC00011",
            cik: null,
            lei: null,
          },
        ]);
      }

      if (command === "lookup_company") {
        return Promise.resolve({
          exchange: "GPW",
          ticker: "CDR",
          qualifiedTicker: "GPW:CDR",
          displayName: "CD PROJEKT S.A.",
          isin: "PLOPTTC00011",
          source: "local_fixture",
        });
      }

      if (command === "delete_company") {
        return Promise.resolve();
      }

      if (command === "list_watchlists") {
        return Promise.resolve([
          {
            id: "watchlist_main_gpw",
            name: "Main GPW",
            description: null,
            companyCount: 1,
          },
        ]);
      }

      if (command === "list_watchlist_memberships") {
        return Promise.resolve([
          {
            watchlistId: "watchlist_main_gpw",
            watchlistName: "Main GPW",
            companyId: "company_gpw_cdr",
          },
        ]);
      }

      if (command === "create_watchlist") {
        return Promise.resolve({
          id: "watchlist_main_gpw",
          name: "Main GPW",
          description: null,
          companyCount: 0,
        });
      }

      if (command === "add_company_to_watchlist") {
        return Promise.resolve();
      }

      if (command === "remove_company_from_watchlist") {
        return Promise.resolve();
      }

      if (command === "list_feed_items") {
        return Promise.resolve(initialFeedItems);
      }

      if (command === "list_source_adapters") {
        return Promise.resolve(sourceAdapters);
      }

      if (command === "get_settings") {
        return Promise.resolve(initialSettings);
      }

      if (command === "update_settings") {
        const input = (args as { input: { theme?: string } }).input;

        return Promise.resolve({
          ...initialSettings,
          theme: input.theme ?? initialSettings.theme,
        });
      }

      if (command === "update_feed_item_state") {
        const input = (args as { input: { id: string; read?: boolean; saved?: boolean } }).input;
        const item = initialFeedItems.find((feedItem) => feedItem.id === input.id) ?? initialFeedItems[0];

        return Promise.resolve({
          ...item,
          unread: input.read === undefined ? item.unread : !input.read,
          saved: input.saved ?? item.saved,
        });
      }

      return Promise.reject(new Error(`Unexpected command: ${command}`));
    });
  });

  it("renders the investor inbox shell", async () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(
      (await screen.findAllByText("Current report placeholder for watchlist company")).length,
    ).toBeGreaterThan(0);
    expect(within(screen.getByLabelText("Feed items")).getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getAllByText("Current report placeholder for watchlist company").length).toBeGreaterThan(0);
    expect(screen.getByText("Fixture official report used to validate feed filtering and detail rendering.")).toBeInTheDocument();
    expect(await screen.findByText("ok 0.1.0")).toBeInTheDocument();
    expect(screen.getByText("DB")).toBeInTheDocument();
    expect(screen.getByLabelText("Database connection active")).toBeInTheDocument();
  });

  it("shows selected feed item details", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(await screen.findByText("Fixture item proving the inbox layout can scan dense rows"));

    expect(screen.getByText("Saved fixture item used to validate the saved filter before real ingestion exists.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Open source" })).toHaveAttribute(
      "href",
      "https://example.local/fixture/pkn",
    );
    expect(screen.getByRole("link", { name: "https://example.local/fixture/pkn" })).toHaveAttribute(
      "href",
      "https://example.local/fixture/pkn",
    );
    expect(screen.getByText("Fixture")).toBeInTheDocument();
  });

  it("shows feed details only in the inbox", async () => {
    const user = userEvent.setup();

    render(<App />);

    expect(await screen.findByLabelText("Feed item details")).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize feed details" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    expect(screen.queryByLabelText("Feed item details")).not.toBeInTheDocument();
    expect(screen.queryByRole("separator", { name: "Resize feed details" })).not.toBeInTheDocument();
  });

  it("resizes the inbox detail pane with keyboard controls", async () => {
    const user = userEvent.setup();

    render(<App />);

    const resizer = await screen.findByRole("separator", { name: "Resize feed details" });

    expect(resizer).toHaveAttribute("aria-valuenow", "360");

    resizer.focus();
    await user.keyboard("{ArrowLeft}");

    expect(resizer).toHaveAttribute("aria-valuenow", "384");
  });

  it("filters inbox fixture items by watchlist", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");

    expect(
      await within(feedList).findByText("Fixture item proving the inbox layout can scan dense rows"),
    ).toBeInTheDocument();

    await user.selectOptions(await screen.findByLabelText("Inbox watchlist"), "watchlist_main_gpw");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Fixture item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();
  });

  it("filters inbox fixture items by status", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Unread" }));

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Fixture item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Saved" }));

    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("Fixture item proving the inbox layout can scan dense rows")).toBeInTheDocument();
  });

  it("filters inbox fixture items by search query", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.type(screen.getByLabelText("Search feed"), "transcript");

    expect(within(feedList).getByText("Transcript-derived note candidate waits for future provider work")).toBeInTheDocument();
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
  });

  it("filters inbox fixture items by type and source", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.selectOptions(screen.getByLabelText("Inbox type"), "Transcript");

    expect(within(feedList).getByText("Transcript-derived note candidate waits for future provider work")).toBeInTheDocument();
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Inbox type"), "all");
    await user.selectOptions(screen.getByLabelText("Inbox source"), "GPW ESPI/EBI");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Transcript-derived note candidate waits for future provider work")).not.toBeInTheDocument();
  });

  it("clears active inbox filters", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.type(screen.getByLabelText("Search feed"), "does-not-match");

    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Clear filters" })[0]);

    expect(screen.getByLabelText("Search feed")).toHaveValue("");
    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).getByText("Fixture item proving the inbox layout can scan dense rows")).toBeInTheDocument();
  });

  it("keeps source refresh as a disabled placeholder", async () => {
    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    const sourceRefresh = screen.getByRole("button", { name: "Refresh sources unavailable" });

    expect(sourceRefresh).toBeDisabled();
  });

  it("refreshes database-backed views from the DB status pill", async () => {
    const user = userEvent.setup();

    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    vi.mocked(invoke).mockClear();

    await user.click(screen.getByRole("button", { name: "Refresh database-backed views" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("database_status");
      expect(invoke).toHaveBeenCalledWith("list_companies");
      expect(invoke).toHaveBeenCalledWith("list_watchlists");
      expect(invoke).toHaveBeenCalledWith("list_watchlist_memberships");
      expect(invoke).toHaveBeenCalledWith("list_feed_items");
      expect(invoke).toHaveBeenCalledWith("list_source_adapters");
      expect(invoke).toHaveBeenCalledWith("get_settings");
    });
  });

  it("shows SQLite-backed settings and persists theme changes", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Settings" }));

    const settingsRegion = await screen.findByLabelText("Application settings");

    expect(within(settingsRegion).getByText("sqlite")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("night-neon")).toBeInTheDocument();
    expect(within(settingsRegion).getByText("accepted_deferred")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Settings theme"), "light");

    expect(invoke).toHaveBeenCalledWith("update_settings", {
      input: {
        theme: "light",
      },
    });
    expect(screen.getByLabelText("Settings theme")).toHaveValue("light");
  });

  it("shows source adapter status", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");

    expect(within(sourceAdaptersRegion).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("gpw-espi-ebi")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("official_report · public_page")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("15 min")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Ready")).toBeInTheDocument();
  });

  it("updates fixture read and saved state from the detail pane", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.click(screen.getByRole("button", { name: "Unread" }));
    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Mark read" }));
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "All" }));
    await user.click(within(feedList).getByText("Current report placeholder for watchlist company"));
    await user.click(screen.getByRole("button", { name: "Save" }));
    await user.click(screen.getByRole("button", { name: "Saved" }));

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
  });

  it("toggles feed item read state on double click", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");
    const feedTitle = await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.dblClick(feedTitle);
    await user.click(screen.getByRole("button", { name: "Unread" }));

    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "All" }));
    await user.dblClick(within(feedList).getByText("Current report placeholder for watchlist company"));
    await user.click(screen.getByRole("button", { name: "Unread" }));

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
  });

  it("fills company form from lookup fixtures", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.clear(screen.getByLabelText("Ticker"));
    await user.type(screen.getByLabelText("Ticker"), "CDR");
    await user.click(screen.getByRole("button", { name: "Lookup" }));

    expect(await screen.findByDisplayValue("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLOPTTC00011")).toBeInTheDocument();
    expect(screen.getByText("Filled from local_fixture: GPW:CDR")).toBeInTheDocument();
  });

  it("confirms and deletes a company", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByTitle("Delete GPW:CDR"));

    expect(confirm).toHaveBeenCalledWith("Delete GPW:CDR from your local registry?");

    confirm.mockRestore();
  });

  it("creates a watchlist and assigns a company", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.type(screen.getByLabelText("Watchlist name"), "Main GPW");
    await user.click(screen.getByRole("button", { name: "Create" }));
    await user.click(await screen.findByRole("button", { name: "Assign" }));

    expect(await within(screen.getByLabelText("Watchlist chips")).findByText("Main GPW")).toBeInTheDocument();
    expect(
      await within(screen.getByLabelText("Watchlist memberships for GPW:CDR")).findByText("Main GPW"),
    ).toBeInTheDocument();
    expect(await screen.findByRole("status", { name: "Assigned to Main GPW" })).toBeInTheDocument();
  });

  it("removes a company from a selected watchlist", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Remove" }));

    expect(await screen.findByRole("status", { name: "Removed from Main GPW" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("remove_company_from_watchlist", {
      input: {
        watchlistId: "watchlist_main_gpw",
        companyId: "company_gpw_cdr",
      },
    });
  });
});
