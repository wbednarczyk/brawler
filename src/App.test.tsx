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
    id: "feed_fixture_kgh_transcript",
    company: "GPW:KGH",
    type: "Transcript",
    source: "Local fixture",
    time: "Mon",
    title: "Transcript-derived note candidate waits for future provider work",
    unread: false,
    saved: false,
    sourceUrl: "https://example.local/fixture/kgh-transcript",
    language: "en",
    publishedAt: "Mon",
    fetchedAt: "Mon",
    attribution: "Fixture",
    summary: "Transcript placeholder for future video and notebook workflows.",
  },
  {
    id: "feed_fixture_pzu_report",
    company: "GPW:PZU",
    type: "Official report",
    source: "GPW ESPI/EBI",
    time: "Fri",
    title: "PZU governance report placeholder",
    unread: false,
    saved: false,
    sourceUrl: "https://www.gpw.pl/komunikaty",
    language: "pl",
    publishedAt: "Fri",
    fetchedAt: "Fri",
    attribution: "GPW",
    summary: "Fourth fixture item keeps the sample feed aligned with local GPW lookup companies.",
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

const initialCompanies = [
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
  {
    id: "company_gpw_pkn",
    exchange: "GPW",
    ticker: "PKN",
    qualifiedTicker: "GPW:PKN",
    displayName: "ORLEN S.A.",
    isin: "PLPKN0000018",
    cik: null,
    lei: null,
  },
  {
    id: "company_gpw_kgh",
    exchange: "GPW",
    ticker: "KGH",
    qualifiedTicker: "GPW:KGH",
    displayName: "KGHM POLSKA MIEDZ S.A.",
    isin: "PLKGHM000017",
    cik: null,
    lei: null,
  },
  {
    id: "company_gpw_pzu",
    exchange: "GPW",
    ticker: "PZU",
    qualifiedTicker: "GPW:PZU",
    displayName: "PZU S.A.",
    isin: "PLPZU0000011",
    cik: null,
    lei: null,
  },
];

type TestNotebookEntry = {
  id: string;
  companyId: string;
  title: string;
  body: string;
  bodyFormat: string;
  tags: string[];
  kind: string;
  claimStatus: string | null;
  eventDate: string | null;
  followUpAfter: string | null;
  followUpDate: string | null;
  createdAt: string;
  updatedAt: string;
  origins: Array<{
    id: string;
    sourceType: string;
    sourceId: string | null;
    sourceUrl: string | null;
    label: string | null;
    createdAt: string;
  }>;
};

const initialNotebookEntry: TestNotebookEntry = {
  id: "note_company_gpw_cdr_release_schedule",
  companyId: "company_gpw_cdr",
  title: "Release schedule promise",
  body: "Management promised a release milestone in the next two quarters.",
  bodyFormat: "markdown",
  tags: ["management-guidance", "product"],
  kind: "claim",
  claimStatus: "open",
  eventDate: "2026-05-29",
  followUpAfter: "2026-Q4",
  followUpDate: "2026-11-30",
  createdAt: "2026-05-29T10:00:00Z",
  updatedAt: "2026-05-29T10:00:00Z",
  origins: [
    {
      id: "note_origin_release_schedule_manual_1",
      sourceType: "manual",
      sourceId: null,
      sourceUrl: null,
      label: "Manual note",
      createdAt: "2026-05-29T10:00:00Z",
    },
  ],
};

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

describe("App", () => {
  let companiesResponse = initialCompanies;
  let feedItemsResponse = initialFeedItems;
  let notebookEntriesResponse: TestNotebookEntry[] = [];

  beforeEach(() => {
    companiesResponse = initialCompanies;
    feedItemsResponse = initialFeedItems;
    notebookEntriesResponse = [];
    vi.mocked(invoke).mockClear();
    vi.mocked(invoke).mockImplementation((command: string, args?: unknown) => {
      if (command === "health") {
        return Promise.resolve({ status: "ok", version: "0.3.0" });
      }

      if (command === "database_status") {
        return Promise.resolve({
          appliedMigrations: 3,
          companies: 0,
          sourceAdapters: 1,
          settings: 7,
        });
      }

      if (command === "list_companies") {
        return Promise.resolve(companiesResponse);
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
        return Promise.resolve(feedItemsResponse);
      }

      if (command === "list_notebook_entries") {
        const companyId = (args as { companyId: string }).companyId;

        return Promise.resolve(
          notebookEntriesResponse.filter((entry) => entry.companyId === companyId),
        );
      }

      if (command === "create_notebook_entry") {
        const input = (args as {
          input: {
            companyId: string;
            title: string;
            body: string;
            bodyFormat: string;
            tags: string[];
            kind: string;
            claimStatus: string | null;
            eventDate: string | null;
            followUpAfter: string | null;
            followUpDate: string | null;
            origins: Array<{
              sourceType: string;
              sourceId: string | null;
              sourceUrl: string | null;
              label: string | null;
            }>;
          };
        }).input;
        const created = {
          id: `note_${input.companyId}_${input.title.toLowerCase().replace(/\s+/g, "_")}`,
          companyId: input.companyId,
          title: input.title,
          body: input.body,
          bodyFormat: input.bodyFormat,
          tags: input.tags.map((tag) => tag.toLowerCase()).sort(),
          kind: input.kind,
          claimStatus: input.claimStatus,
          eventDate: input.eventDate,
          followUpAfter: input.followUpAfter,
          followUpDate: input.followUpDate,
          createdAt: "2026-05-29T10:00:00Z",
          updatedAt: "2026-05-29T10:00:00Z",
          origins: input.origins.map((item, index) => ({
            id: `note_origin_${index}`,
            sourceType: item.sourceType,
            sourceId: item.sourceId,
            sourceUrl: item.sourceUrl,
            label: item.label,
            createdAt: "2026-05-29T10:00:00Z",
          })),
        };

        notebookEntriesResponse = [created, ...notebookEntriesResponse];

        return Promise.resolve(created);
      }

      if (command === "update_notebook_entry") {
        const input = (args as {
          input: {
            id: string;
            title: string;
            body: string;
            tags: string[];
            kind: string;
            claimStatus: string | null;
            eventDate: string | null;
            followUpAfter: string | null;
            followUpDate: string | null;
          };
        }).input;
        const existing = notebookEntriesResponse.find((entry) => entry.id === input.id);
        const updated = {
          ...(existing ?? initialNotebookEntry),
          id: input.id,
          title: input.title,
          body: input.body,
          tags: input.tags.map((tag) => tag.toLowerCase()).sort(),
          kind: input.kind,
          claimStatus: input.claimStatus,
          eventDate: input.eventDate,
          followUpAfter: input.followUpAfter,
          followUpDate: input.followUpDate,
          updatedAt: "2026-05-29T10:05:00Z",
        };

        notebookEntriesResponse = notebookEntriesResponse.map((entry) =>
          entry.id === updated.id ? updated : entry,
        );

        return Promise.resolve(updated);
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
        const item =
          feedItemsResponse.find((feedItem) => feedItem.id === input.id) ??
          initialFeedItems.find((feedItem) => feedItem.id === input.id) ??
          initialFeedItems[0];

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
    expect(await screen.findByText("ok 0.3.0")).toBeInTheDocument();
    expect(screen.getByText("DB")).toBeInTheDocument();
    expect(screen.getByLabelText("Database connection active")).toBeInTheDocument();
  });

  it("shows unread feed count in the Inbox navigation item", async () => {
    render(<App />);

    const inboxNav = await screen.findByRole("button", { name: /Inbox/ });

    expect(within(inboxNav).getByText("1")).toHaveClass("nav-badge");
    expect(within(inboxNav).getByLabelText("1 unread feed item")).toBeInTheDocument();
  });

  it("shows selected feed item details", async () => {
    const user = userEvent.setup();

    render(<App />);

    const selectedRow = await screen.findByRole("button", {
      name: "Select feed item: Fixture item proving the inbox layout can scan dense rows",
    });

    await user.click(selectedRow);

    expect(selectedRow).toHaveClass("feed-row-selected");
    expect(selectedRow).toHaveAttribute("aria-current", "true");
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

  it("opens the matching company workspace from an inbox feed item", async () => {
    const user = userEvent.setup();

    render(<App />);

    await screen.findByLabelText("Feed item details");
    await user.click(screen.getByRole("button", { name: "Open company" }));

    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Company workspace")).toBeInTheDocument();
    expect(screen.getByLabelText("Company feed item details")).toBeInTheDocument();
    expect(screen.getByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    })).toHaveClass("company-feed-row-selected");
  });

  it("creates a notebook draft from an inbox feed item with feed origins", async () => {
    const user = userEvent.setup();

    render(<App />);

    await screen.findByLabelText("Feed item details");
    await user.click(screen.getByRole("button", { name: "Note" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(screen.getByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(screen.getByLabelText("Notebook screen note title")).toHaveValue(
      "Current report placeholder for watchlist company",
    );
    expect(screen.getByLabelText("Notebook screen note body")).toHaveValue(
      "Fixture official report used to validate feed filtering and detail rendering.",
    );
    expect(screen.getByLabelText("Notebook screen note tags")).toHaveValue(
      "feed, official-report, gpw-espi/ebi",
    );

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Current report placeholder for watchlist company",
          body: "Fixture official report used to validate feed filtering and detail rendering.",
          bodyFormat: "markdown",
          tags: ["feed", "official-report", "gpw-espi/ebi"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "feed_item",
              sourceId: "feed_fixture_cdr_report",
              sourceUrl: "https://www.gpw.pl/komunikaty",
              label: "GPW ESPI/EBI: Current report placeholder for watchlist company",
            },
          ],
        },
      });
    });

    const originFeedButton = await within(notebooksWorkspace).findByRole("button", {
      name: "Open origin feed item: GPW ESPI/EBI: Current report placeholder for watchlist company",
    });
    expect(
      within(notebooksWorkspace).getByRole("link", {
        name: "Open origin source: GPW ESPI/EBI: Current report placeholder for watchlist company",
      }),
    ).toHaveAttribute("href", "https://www.gpw.pl/komunikaty");

    await user.click(originFeedButton);

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Feed item details")).getByRole("heading", {
        name: "Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
  });

  it("selects inbox feed items with the keyboard", async () => {
    const user = userEvent.setup();

    render(<App />);

    const feedItem = await screen.findByRole("button", {
      name: "Select feed item: Fixture item proving the inbox layout can scan dense rows",
    });

    feedItem.focus();
    await user.keyboard("{Enter}");

    expect(screen.getByText("Saved fixture item used to validate the saved filter before real ingestion exists.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "https://example.local/fixture/pkn" })).toBeInTheDocument();
  });

  it("moves through inbox feed items with arrow keys", async () => {
    const user = userEvent.setup();

    render(<App />);

    const firstFeedItem = await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });

    firstFeedItem.focus();
    await user.keyboard("{ArrowDown}");

    expect(
      screen.getByRole("button", {
        name: "Select feed item: Fixture item proving the inbox layout can scan dense rows",
      }),
    ).toHaveFocus();
    expect(screen.getByText("Saved fixture item used to validate the saved filter before real ingestion exists.")).toBeInTheDocument();

    await user.keyboard("{ArrowUp}");

    expect(firstFeedItem).toHaveFocus();
    expect(screen.getByText("Fixture official report used to validate feed filtering and detail rendering.")).toBeInTheDocument();
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

  it("moves selection to the next unread item after marking the current unread item read", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_fixture_cdr_second_unread",
        title: "Second unread report for review flow",
        summary: "Second unread item should become selected after the first one is marked read.",
        unread: true,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Unread" }));

    const firstUnreadRow = await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });

    expect(firstUnreadRow).toHaveClass("feed-row-selected");

    await user.click(screen.getByRole("button", { name: "Mark read" }));

    await waitFor(() => {
      expect(
        screen.getByRole("button", {
          name: "Select feed item: Second unread report for review flow",
        }),
      ).toHaveClass("feed-row-selected");
    });
    expect(screen.queryByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    })).not.toBeInTheDocument();
    expect(screen.getByText("Second unread item should become selected after the first one is marked read.")).toBeInTheDocument();
  });

  it("summarizes the current inbox review set", async () => {
    const user = userEvent.setup();

    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    const summary = screen.getByLabelText("Inbox review summary");

    expect(within(summary).getByText("4")).toBeInTheDocument();
    expect(within(summary).getByText("visible")).toBeInTheDocument();
    expect(within(summary).getAllByText("1")).toHaveLength(2);
    expect(within(summary).getByText("unread")).toBeInTheDocument();
    expect(within(summary).getByText("saved")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Unread" }));

    expect(within(summary).getAllByText("1")).toHaveLength(2);
    expect(within(summary).getByText("visible")).toBeInTheDocument();
    expect(within(summary).getByText("unread")).toBeInTheDocument();
    expect(within(summary).getByText("0")).toBeInTheDocument();
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

  it("shows a first-run inbox empty state when no companies are tracked", async () => {
    const user = userEvent.setup();

    companiesResponse = [];
    feedItemsResponse = [];

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");

    expect(await within(feedList).findByText("No companies tracked yet.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add company" }));

    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
  });

  it("shows a no-feed inbox empty state with a source status path when companies exist", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [];

    render(<App />);

    const feedList = screen.getByLabelText("Feed items");

    expect(await within(feedList).findByText("No stored feed items yet.")).toBeInTheDocument();
    expect(within(feedList).getByRole("button", { name: "Refresh pending" })).toBeDisabled();

    await user.click(within(feedList).getByRole("button", { name: "Open Sources" }));

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Source adapter details")).toBeInTheDocument();
  });

  it("keeps source refresh as a disabled placeholder", async () => {
    render(<App />);

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    const sourceRefresh = screen.getByRole("button", { name: "Refresh sources unavailable" });

    expect(sourceRefresh).toBeDisabled();
  });

  it("opens source status from the topbar source pill", async () => {
    const user = userEvent.setup();

    render(<App />);

    const sourceStatus = await screen.findByRole("button", { name: "Open source status" });

    expect(sourceStatus).toHaveTextContent("Sources");
    expect(sourceStatus).toHaveTextContent("1/1");

    await user.click(sourceStatus);

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const sourceRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW ESPI/EBI",
    });

    expect(sourceRow).toHaveClass("source-row-selected");
    expect(within(sourceAdaptersRegion).getByLabelText("Source adapter details")).toBeInTheDocument();
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
    const sourceRow = await within(sourceAdaptersRegion).findByRole("button", {
      name: "Open source adapter: GPW ESPI/EBI",
    });

    expect(within(sourceAdaptersRegion).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("gpw-espi-ebi")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("official_report · public_page")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Ready")).toBeInTheDocument();

    await user.click(sourceRow);

    expect(sourceRow).toHaveClass("source-row-selected");
    expect(within(await screen.findByLabelText("Source adapter details")).getByText("15 min")).toBeInTheDocument();

    await user.click(sourceRow);

    expect(screen.queryByLabelText("Source adapter details")).not.toBeInTheDocument();
  });

  it("expands and collapses source adapter details with keyboard controls", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceRow = await screen.findByRole("button", {
      name: "Open source adapter: GPW ESPI/EBI",
    });

    sourceRow.focus();
    await user.keyboard("{Enter}");

    expect(await screen.findByLabelText("Source adapter details")).toBeInTheDocument();

    await user.keyboard(" ");

    expect(screen.queryByLabelText("Source adapter details")).not.toBeInTheDocument();
  });

  it("shows the notebooks workspace and planned transcript placeholder screen", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [initialNotebookEntry];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Notebooks" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(screen.getByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Open notebook company: GPW:CDR",
      }),
    ).toBeInTheDocument();
    const notebookCompanyButton = within(notebooksWorkspace).getByRole("button", {
      name: "Open notebook company: GPW:CDR",
    });
    expect(notebookCompanyButton).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", { name: "Show open claims for GPW:CDR" }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", { name: "Show follow-ups for GPW:CDR" }),
    ).toBeInTheDocument();
    const notebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    expect(notebookRow).toBeInTheDocument();
    expect(screen.queryByLabelText("Notebook screen selected body")).not.toBeInTheDocument();

    await user.click(notebookRow);

    expect(screen.getByLabelText("Notebook screen selected body")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "New note" }));
    await user.type(screen.getByLabelText("Notebook screen note title"), "Notebook desk note");
    await user.type(screen.getByLabelText("Notebook screen note body"), "Created from the main notebooks pane.");
    await user.type(screen.getByLabelText("Notebook screen note tags"), "desk, workflow");
    await user.selectOptions(screen.getByLabelText("Notebook screen note kind"), "observation");
    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Notebook desk note",
          body: "Created from the main notebooks pane.",
          bodyFormat: "markdown",
          tags: ["desk", "workflow"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "manual",
              sourceId: null,
              sourceUrl: null,
              label: "Manual note",
            },
          ],
        },
      });
    });

    const createdNotebookRow = await within(notebooksWorkspace).findByRole("button", {
      name: "Select notebook screen entry: Notebook desk note",
    });
    expect(createdNotebookRow).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Transcripts" }));

    const transcriptsPlaceholder = await screen.findByLabelText("Transcripts placeholder");

    expect(screen.getByRole("heading", { name: "Transcripts" })).toBeInTheDocument();
    expect(within(transcriptsPlaceholder).getByText("Gemini for YouTube transcription only")).toBeInTheDocument();
  });

  it("filters notebook screen entries by kind, status, tag, and follow-up scheduling", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [
      initialNotebookEntry,
      {
        ...initialNotebookEntry,
        id: "note_company_gpw_cdr_margin_observation",
        title: "Margin observation",
        body: "Desk note about margin pressure after the call.",
        tags: ["desk", "margin"],
        kind: "observation",
        claimStatus: null,
        followUpAfter: null,
        followUpDate: null,
      },
      {
        ...initialNotebookEntry,
        id: "note_company_gpw_cdr_capex_claim",
        title: "Capex delivery claim",
        body: "Management claimed capex should normalize.",
        tags: ["capex", "management-guidance"],
        kind: "claim",
        claimStatus: "delivered",
        followUpAfter: "2026-Q3",
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Notebooks" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(
      await within(notebooksWorkspace).findByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Show open claims for GPW:CDR" }));

    expect(screen.getByLabelText("Notebook claim status filter")).toHaveValue("open");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Show follow-ups for GPW:CDR" }));

    expect(screen.getByLabelText("Notebook follow-up filter")).toHaveValue("has_follow_up");
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).not.toBeInTheDocument();
    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));

    await user.selectOptions(screen.getByLabelText("Notebook kind filter"), "observation");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.selectOptions(screen.getByLabelText("Notebook claim status filter"), "delivered");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Capex delivery claim",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.type(screen.getByLabelText("Notebook tag filter"), "desk");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Capex delivery claim",
      }),
    ).not.toBeInTheDocument();

    await user.click(within(notebooksWorkspace).getByRole("button", { name: "Clear filters" }));
    await user.selectOptions(screen.getByLabelText("Notebook follow-up filter"), "no_follow_up");

    expect(
      within(notebooksWorkspace).getByRole("button", {
        name: "Select notebook screen entry: Margin observation",
      }),
    ).toBeInTheDocument();
    expect(
      within(notebooksWorkspace).queryByRole("button", {
        name: "Select notebook screen entry: Release schedule promise",
      }),
    ).not.toBeInTheDocument();
  });

  it("renders notebook Markdown in read mode", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [
      {
        ...initialNotebookEntry,
        body: "# Release checklist\n- **Milestone** shipped\n- `Patch` ready",
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Notebooks" }));
    const notebookRow = await screen.findByRole("button", {
      name: "Select notebook screen entry: Release schedule promise",
    });

    expect(within(notebookRow).queryByText("# Release checklist")).not.toBeInTheDocument();

    await user.click(notebookRow);

    const notebookBody = screen.getByLabelText("Notebook screen selected body");

    expect(within(notebookBody).getByRole("heading", { name: "Release checklist" })).toBeInTheDocument();
    expect(within(notebookBody).getByText("Milestone")).toHaveProperty("tagName", "STRONG");
    expect(within(notebookBody).getByText("Patch")).toHaveProperty("tagName", "CODE");

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Notebook screen selected follow-up date"));
    await user.type(screen.getByLabelText("Notebook screen selected follow-up date"), "2026-12-15");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "# Release checklist\n- **Milestone** shipped\n- `Patch` ready",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "open",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-12-15",
        },
      });
    });
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

  it("opens a company workspace with company-scoped feed and metadata tabs", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    const companyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(companyRow);

    const workspace = await screen.findByLabelText("Company workspace");

    expect(companyRow).toHaveClass("company-row-selected");
    expect(companyRow.compareDocumentPosition(workspace) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(workspace).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(workspace).getByText("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(within(screen.getByLabelText("Selected company metadata")).getByText("1 feed")).toBeInTheDocument();
    expect(within(screen.getByLabelText("Selected company metadata")).getByText("1 unread")).toBeInTheDocument();
    expect(within(screen.getByLabelText("Selected company metadata")).getByText("0 saved")).toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Company feed")).getByRole("button", {
        name: "Open company feed item: Current report placeholder for watchlist company",
      }),
    ).toBeInTheDocument();
    expect(within(screen.getByLabelText("Company feed")).queryByText("Fixture item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();

    await user.click(within(workspace).getByRole("button", { name: "Metadata" }));

    expect(within(screen.getByLabelText("Company metadata")).getByText("PLOPTTC00011")).toBeInTheDocument();
  });

  it("lists and creates company notebook entries", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [initialNotebookEntry];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(screen.getByRole("button", { name: "Notebook" }));

    const notebook = await screen.findByLabelText("Company notebook");

    expect(
      within(notebook).getByRole("button", { name: "Select notebook entry: Release schedule promise" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Selected notebook body")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );
    expect(within(notebook).getAllByText("management-guidance").length).toBeGreaterThan(0);
    expect(within(notebook).getAllByText("2026-Q4").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "Edit" }));
    await user.clear(screen.getByLabelText("Selected notebook body"));
    await user.type(screen.getByLabelText("Selected notebook body"), "Management shifted the release language.");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "Management shifted the release language.",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "open",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-11-30",
        },
      });
    });

    await user.click(screen.getByRole("button", { name: "New note" }));
    await user.clear(screen.getByLabelText("Notebook note title"));
    await user.type(screen.getByLabelText("Notebook note title"), "Conference note");
    await user.type(screen.getByLabelText("Notebook note body"), "Board mentioned margin pressure.");
    await user.type(screen.getByLabelText("Notebook note tags"), "conference, margin");
    await user.selectOptions(screen.getByLabelText("Notebook note kind"), "observation");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_notebook_entry", {
        input: {
          companyId: "company_gpw_cdr",
          title: "Conference note",
          body: "Board mentioned margin pressure.",
          bodyFormat: "markdown",
          tags: ["conference", "margin"],
          kind: "observation",
          claimStatus: null,
          eventDate: null,
          followUpAfter: null,
          followUpDate: null,
          origins: [
            {
              sourceType: "manual",
              sourceId: null,
              sourceUrl: null,
              label: "Manual note",
            },
          ],
        },
      });
    });
    const conferenceNoteRow = await within(notebook).findByRole("button", {
      name: "Select notebook entry: Conference note",
    });
    expect(conferenceNoteRow).toBeInTheDocument();
    await user.click(conferenceNoteRow);

    expect(screen.getByLabelText("Selected notebook body")).toHaveTextContent("Board mentioned margin pressure.");
  });

  it("lists company claims and updates claim status", async () => {
    const user = userEvent.setup();

    notebookEntriesResponse = [initialNotebookEntry];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(screen.getByRole("button", { name: "Claims" }));

    const claims = await screen.findByLabelText("Company claims");
    const claimRow = within(claims).getByRole("button", {
      name: "Open claim: Release schedule promise",
    });

    expect(claimRow).toBeInTheDocument();
    expect(within(claims).getByText("1 follow-up item for GPW:CDR")).toBeInTheDocument();

    await user.click(claimRow);

    expect(screen.getByLabelText("Claim detail")).toHaveTextContent(
      "Management promised a release milestone in the next two quarters.",
    );

    await user.selectOptions(screen.getByLabelText("Claim status"), "delivered");
    await user.click(within(screen.getByLabelText("Claim detail")).getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("update_notebook_entry", {
        input: {
          id: "note_company_gpw_cdr_release_schedule",
          title: "Release schedule promise",
          body: "Management promised a release milestone in the next two quarters.",
          tags: ["management-guidance", "product"],
          kind: "claim",
          claimStatus: "delivered",
          eventDate: "2026-05-29",
          followUpAfter: "2026-Q4",
          followUpDate: "2026-11-30",
        },
      });
    });
  });

  it("hides the company workspace when the open company row is clicked again", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const companyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(companyRow);
    expect(await screen.findByLabelText("Company workspace")).toBeInTheDocument();

    await user.click(companyRow);
    expect(screen.queryByLabelText("Company workspace")).not.toBeInTheDocument();
  });

  it("moves through company rows with arrow keys without expanding a collapsed workspace", async () => {
    const user = userEvent.setup();

    companiesResponse = [
      initialCompanies[0],
      {
        id: "company_gpw_pkn",
        exchange: "GPW",
        ticker: "PKN",
        qualifiedTicker: "GPW:PKN",
        displayName: "ORLEN S.A.",
        isin: "PLPKN0000018",
        cik: null,
        lei: null,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const firstCompanyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    firstCompanyRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyRow = screen.getByRole("button", { name: "Open GPW:PKN workspace" });

    expect(secondCompanyRow).toHaveFocus();
    expect(secondCompanyRow).not.toHaveClass("company-row-selected");
    expect(screen.queryByLabelText("Company workspace")).not.toBeInTheDocument();
  });

  it("moves an already-open company workspace with company row arrow keys", async () => {
    const user = userEvent.setup();

    companiesResponse = [
      initialCompanies[0],
      {
        id: "company_gpw_pkn",
        exchange: "GPW",
        ticker: "PKN",
        qualifiedTicker: "GPW:PKN",
        displayName: "ORLEN S.A.",
        isin: "PLPKN0000018",
        cik: null,
        lei: null,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const firstCompanyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(firstCompanyRow);
    firstCompanyRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyRow = screen.getByRole("button", { name: "Open GPW:PKN workspace" });

    expect(secondCompanyRow).toHaveFocus();
    expect(secondCompanyRow).toHaveClass("company-row-selected");
    expect(within(await screen.findByLabelText("Company workspace")).getByText("GPW:PKN")).toBeInTheDocument();
  });

  it("shows an actionable company feed empty state for tracked companies without feed items", async () => {
    const user = userEvent.setup();

    companiesResponse = [
      {
        id: "company_gpw_lpp",
        exchange: "GPW",
        ticker: "LPP",
        qualifiedTicker: "GPW:LPP",
        displayName: "LPP S.A.",
        isin: "PLLPP0000011",
        cik: null,
        lei: null,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:LPP workspace" }));

    const companyFeed = await screen.findByLabelText("Company feed");

    expect(within(companyFeed).getByText("No stored feed items for GPW:LPP yet.")).toBeInTheDocument();
    expect(
      within(companyFeed).getByText(
        "This company is tracked locally, but no fixture or ingested items are attached to it yet.",
      ),
    ).toBeInTheDocument();

    await user.click(within(companyFeed).getByRole("button", { name: "Open filtered Inbox" }));

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:LPP");
    expect(screen.getByText("No feed items for selected filters.")).toBeInTheDocument();
  });

  it("shows company feed item details inline and can open the item in the inbox", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(companyFeedRow);

    const companyFeedDetail = await screen.findByLabelText("Company feed item details");

    expect(companyFeedRow.compareDocumentPosition(companyFeedDetail) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(companyFeedDetail).getByText("Fixture official report used to validate feed filtering and detail rendering.")).toBeInTheDocument();
    expect(within(companyFeedDetail).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(companyFeedDetail).getByRole("link", { name: "Open source" })).toHaveAttribute(
      "href",
      "https://www.gpw.pl/komunikaty",
    );

    await user.click(within(companyFeedDetail).getByRole("button", { name: "Open in Inbox" }));

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
    expect(screen.getByLabelText("Feed item details")).toBeInTheDocument();
  });

  it("uses inbox unread and saved visual state in company feed rows", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    expect(companyFeedRow).toHaveClass("unread");
    expect(within(companyFeedRow).getByTitle("Unread")).toBeInTheDocument();
  });

  it("hides company feed item details when the open feed row is clicked again", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(companyFeedRow);
    expect(await screen.findByLabelText("Company feed item details")).toBeInTheDocument();

    await user.click(companyFeedRow);
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("moves through collapsed company feed rows without expanding details", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_fixture_cdr_second_report",
        title: "Second CDR report for company feed keyboard navigation",
        summary: "Second company-scoped fixture item.",
        unread: false,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const firstCompanyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    firstCompanyFeedRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyFeedRow = screen.getByRole("button", {
      name: "Open company feed item: Second CDR report for company feed keyboard navigation",
    });

    expect(secondCompanyFeedRow).toHaveFocus();
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("moves expanded company feed details with arrow keys and toggles details with space", async () => {
    const user = userEvent.setup();

    feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_fixture_cdr_second_report",
        title: "Second CDR report for company feed keyboard navigation",
        summary: "Second company-scoped fixture item.",
        unread: false,
      },
    ];

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    const firstCompanyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(firstCompanyFeedRow);
    firstCompanyFeedRow.focus();
    await user.keyboard("{ArrowDown}");

    const secondCompanyFeedRow = screen.getByRole("button", {
      name: "Open company feed item: Second CDR report for company feed keyboard navigation",
    });

    expect(secondCompanyFeedRow).toHaveFocus();
    expect(
      within(await screen.findByLabelText("Company feed item details")).getByText(
        "Second company-scoped fixture item.",
      ),
    ).toBeInTheDocument();

    await user.keyboard(" ");
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("updates company feed item read and saved state from inline details", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    }));

    const companyFeedDetail = await screen.findByLabelText("Company feed item details");

    await user.click(within(companyFeedDetail).getByRole("button", { name: "Mark read" }));
    await user.click(within(companyFeedDetail).getByRole("button", { name: "Save" }));

    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_fixture_cdr_report",
        read: true,
        saved: false,
      },
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_fixture_cdr_report",
        read: true,
        saved: true,
      },
    });
  });

  it("creates a watchlist and assigns a company", async () => {
    const user = userEvent.setup();

    render(<App />);

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.type(screen.getByLabelText("Watchlist name"), "Main GPW");
    await user.click(screen.getByRole("button", { name: "Create" }));
    await user.click(
      within(await screen.findByRole("button", { name: "Open GPW:CDR workspace" })).getByRole(
        "button",
        { name: "Assign" },
      ),
    );

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
    await user.click(
      within(await screen.findByRole("button", { name: "Open GPW:CDR workspace" })).getByRole(
        "button",
        { name: "Remove" },
      ),
    );

    expect(await screen.findByRole("status", { name: "Removed from Main GPW" })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("remove_company_from_watchlist", {
      input: {
        watchlistId: "watchlist_main_gpw",
        companyId: "company_gpw_cdr",
      },
    });
  });
});
