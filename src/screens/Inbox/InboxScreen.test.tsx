import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  appTestState,
  expect,
  initialFeedItems,
  invoke,
  openUrl,
  renderApp,
  screen,
  userEvent,
  vi,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

describe("Inbox screen workflows", () => {
  it("shows selected feed item details", async () => {
    const user = userEvent.setup();

    renderApp();

    const selectedRow = await screen.findByRole("button", {
      name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
    });

    await user.click(selectedRow);

    expect(selectedRow).toHaveClass("feed-row-selected");
    expect(selectedRow).toHaveAttribute("aria-current", "true");
    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "https://example.test/sample/pkn" })).toHaveAttribute(
      "href",
      "https://example.test/sample/pkn",
    );
    expect(screen.queryByRole("link", { name: "Open source" })).not.toBeInTheDocument();
    const detailPane = within(screen.getByLabelText("Feed item details"));
    expect(detailPane.queryByText("Sample feed")).not.toBeInTheDocument();
    expect(detailPane.queryByText("Sample")).not.toBeInTheDocument();

    const timestampMeta = within(screen.getByLabelText("Feed item timestamps"));
    expect(timestampMeta.getByText("Published")).toBeInTheDocument();
    expect(timestampMeta.getByText("Fetched")).toBeInTheDocument();
    expect(timestampMeta.getAllByText("Yesterday")).toHaveLength(2);
  });

  it("shows only PDF attachments in feed item details", async () => {
    appTestState.feedItemsResponse = [
      {
        ...initialFeedItems[0],
        attachments: [
          {
            id: "feed_attachment_report_pdf",
            label: "report.pdf",
            url: "https://example.test/report.pdf",
          },
          {
            id: "feed_attachment_sheet_xlsx",
            label: "sheet.xlsx",
            url: "https://example.test/sheet.xlsx",
          },
          {
            id: "feed_attachment_source_terms",
            label: "Regulamin",
            url: "https://example.test/regulamin.pdf",
          },
          {
            id: "feed_attachment_source_privacy",
            label: "Polityka prywatności",
            url: "https://example.test/prywatnosc.pdf",
          },
          {
            id: "feed_attachment_source_cookies",
            label: "Polityka Cookies",
            url: "https://example.test/cookies.pdf",
          },
        ],
      },
    ];

    renderApp();

    const detailPane = within(await screen.findByLabelText("Feed item details"));

    expect(detailPane.getByText("Attachments")).toBeInTheDocument();
    expect(detailPane.getByRole("link", { name: "report.pdf" })).toHaveAttribute(
      "href",
      "https://example.test/report.pdf",
    );
    expect(detailPane.queryByRole("link", { name: "sheet.xlsx" })).not.toBeInTheDocument();
    expect(detailPane.queryByRole("link", { name: "Regulamin" })).not.toBeInTheDocument();
    expect(detailPane.queryByRole("link", { name: "Polityka prywatności" })).not.toBeInTheDocument();
    expect(detailPane.queryByRole("link", { name: "Polityka Cookies" })).not.toBeInTheDocument();
  });

  // ADR 0084 decision 5 (clean cut, revised 2026-07-20): the AI analysis tables
  // are DROPPED, so the read-only viewer the earlier slice kept has nothing to
  // display and is gone too. The detail rail must render no AI-analysis surface
  // at all, and no retired AI command may be reachable from it.
  it("renders no AI-analysis surface in the detail rail (ADR 0084 clean cut)", async () => {
    renderApp();

    // A feed item is selected and its detail rail is rendered.
    expect((await screen.findAllByText(initialFeedItems[0].title)).length).toBeGreaterThan(0);

    // Every AI-analysis affordance — read-only viewer included — is gone.
    for (const name of [
      "Analyze with AI",
      "Analyze",
      "Summarize impact",
      "Find risks",
      "Retry",
      "View analysis",
    ]) {
      expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
    }
    expect(screen.queryByLabelText("AI custom question")).not.toBeInTheDocument();
    expect(screen.queryByText("AI analysis")).not.toBeInTheDocument();

    // Nothing reached a retired AI command.
    const commands = vi.mocked(invoke).mock.calls.map(([command]) => command);
    for (const retired of ["list_ai_analysis", "start_ai_analysis", "retry_ai_analysis"]) {
      expect(commands).not.toContain(retired);
    }
  });

  it("opens the matching company dashboard from an inbox feed item (ADR 0057)", async () => {
    const user = userEvent.setup();

    renderApp();

    await screen.findByLabelText("Feed item details");
    await user.click(screen.getByRole("button", { name: "Open company" }));

    // "Open company" lands the curated cockpit dashboard scoped to the company.
    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
  });

  it("creates a notebook draft from an inbox feed item with feed origins", async () => {
    const user = userEvent.setup();

    renderApp();

    await screen.findByLabelText("Feed item details");
    await user.click(screen.getByRole("button", { name: "Note" }));

    const notebooksWorkspace = await screen.findByLabelText("Notebooks workspace");

    expect(screen.getByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(screen.getByLabelText("Notebook screen note title")).toHaveValue(
      "Current report placeholder for watchlist company",
    );
    expect(screen.getByLabelText("Notebook screen note body")).toHaveValue(
      "Sample official report used to validate feed filtering and detail rendering.",
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
          body: "Sample official report used to validate feed filtering and detail rendering.",
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
              sourceId: "feed_sample_cdr_report",
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

    renderApp();

    const feedItem = await screen.findByRole("button", {
      name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
    });

    feedItem.focus();
    await user.keyboard("{Enter}");

    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);
    expect(screen.getByRole("link", { name: "https://example.test/sample/pkn" })).toBeInTheDocument();
  });

  it("moves through inbox feed items with arrow keys", async () => {
    const user = userEvent.setup();

    renderApp();

    const firstFeedItem = await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });

    firstFeedItem.focus();
    await user.keyboard("{ArrowDown}");

    expect(
      screen.getByRole("button", {
        name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
      }),
    ).toHaveFocus();
    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);

    await user.keyboard("{ArrowUp}");

    expect(firstFeedItem).toHaveFocus();
    expect(
      screen.getAllByText("Sample official report used to validate feed filtering and detail rendering.").length,
    ).toBeGreaterThan(0);
  });

  it("runs inbox workflow shortcuts for selection, state, source, and notes", async () => {
    renderApp();

    expect(await screen.findByLabelText("Feed item details")).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "J", code: "KeyJ" });
    expect(
      screen.getAllByText("Saved sample item used to validate the saved filter before real ingestion exists.").length,
    ).toBeGreaterThan(0);

    fireEvent.keyDown(document, { key: "M", code: "KeyM" });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_pkn_news",
        read: false,
        saved: true,
      },
    });

    fireEvent.keyDown(document, { key: "S", code: "KeyS" });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_pkn_news",
        read: true,
        saved: false,
      },
    });

    fireEvent.keyDown(document, { key: "O", code: "KeyO" });
    expect(openUrl).toHaveBeenCalledWith("https://example.test/sample/pkn");

    fireEvent.keyDown(document, { key: "N", code: "KeyN" });
    expect(await screen.findByRole("heading", { name: "Notebooks" })).toBeInTheDocument();
    expect(screen.getByLabelText("Notebook screen note title")).toHaveValue(
      "Sample item proving the inbox layout can scan dense rows",
    );
  });

  it("shows feed details only in the inbox", async () => {
    const user = userEvent.setup();

    renderApp();

    expect(await screen.findByLabelText("Feed item details")).toBeInTheDocument();
    expect(screen.getByRole("separator", { name: "Resize feed details" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    expect(screen.queryByLabelText("Feed item details")).not.toBeInTheDocument();
    expect(screen.queryByRole("separator", { name: "Resize feed details" })).not.toBeInTheDocument();
  });

  it("resizes the inbox detail pane with keyboard controls", async () => {
    const user = userEvent.setup();

    renderApp();

    const resizer = await screen.findByRole("separator", { name: "Resize feed details" });

    // The feed and detail panes split 50/50 by default (ADR 0047); ArrowLeft
    // moves the divider left, widening the detail pane by 3 percentage points.
    expect(resizer).toHaveAttribute("aria-valuenow", "50");

    resizer.focus();
    await user.keyboard("{ArrowLeft}");

    expect(resizer).toHaveAttribute("aria-valuenow", "53");
  });

  it("filters inbox sample items by watchlist", async () => {
    const user = userEvent.setup();

    renderApp();

    const feedList = screen.getByLabelText("Feed items");

    expect(
      await within(feedList).findByText("Sample item proving the inbox layout can scan dense rows"),
    ).toBeInTheDocument();

    await user.selectOptions(await screen.findByLabelText("Inbox watchlist"), "watchlist_main_gpw");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Sample item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();
  });

  it("filters inbox sample items by status", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Unread" }));

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).queryByText("Sample item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Saved" }));

    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
    expect(within(feedList).getByText("Sample item proving the inbox layout can scan dense rows")).toBeInTheDocument();
  });

  it("moves selection to the next unread item after marking the current unread item read", async () => {
    const user = userEvent.setup();

    appTestState.feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_unread",
        title: "Second unread report for review flow",
        summary: "Second unread item should become selected after the first one is marked read.",
        unread: true,
      },
    ];

    renderApp();

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
    expect(
      screen.getAllByText("Second unread item should become selected after the first one is marked read.").length,
    ).toBeGreaterThan(0);
  });

  it("keeps keyboard focus in the feed when a row is hidden by marking it read (ADR 0076 D9)", async () => {
    const user = userEvent.setup();

    appTestState.feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_unread",
        title: "Second unread report for focus flow",
        summary: "Second unread item should receive focus after the first is hidden.",
        unread: true,
      },
    ];

    renderApp();

    await user.click(screen.getByRole("button", { name: "Unread" }));

    const firstRow = await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });
    const secondRow = screen.getByRole("button", {
      name: "Select feed item: Second unread report for focus flow",
    });

    // Double-click the focused row to mark it read; under the "unread" filter it
    // leaves the list. Focus must move to the row that slid into its slot, not
    // fall back to <body>.
    firstRow.focus();
    await user.dblClick(firstRow);

    await waitFor(() => {
      expect(
        screen.queryByRole("button", {
          name: "Select feed item: Current report placeholder for watchlist company",
        }),
      ).not.toBeInTheDocument();
    });
    expect(document.activeElement).toBe(secondRow);
  });

  it("marks all visible inbox items as read", async () => {
    const user = userEvent.setup();

    appTestState.feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_unread",
        title: "Second unread report for bulk read flow",
        summary: "Second unread item should be marked read with the bulk action.",
        unread: true,
      },
    ];

    renderApp();

    await user.click(screen.getByRole("button", { name: "Mark all read" }));

    await waitFor(() => {
      expect(screen.getByLabelText("Inbox review summary")).toHaveTextContent("0 unread");
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_report",
        read: true,
        saved: false,
      },
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_second_unread",
        read: true,
        saved: false,
      },
    });
  });

  it("confirms and deletes unsaved feed items without refreshing sources", async () => {
    const user = userEvent.setup();

    appTestState.feedItemsResponse = [
      {
        ...initialFeedItems[0],
        id: "feed_unsaved_delete_candidate",
        title: "Unsaved report to delete",
        saved: false,
      },
      {
        ...initialFeedItems[1],
        id: "feed_saved_to_keep",
        title: "Saved report to keep",
        saved: true,
      },
    ];

    renderApp();

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Unsaved report to delete");

    // Bulk data clear (ADR 0076 D5): confirm in place, no native dialog.
    await user.click(screen.getByRole("button", { name: "Delete unsaved" }));
    expect(invoke).not.toHaveBeenCalledWith("delete_unsaved_feed_items");
    expect(screen.getByText("Delete all unsaved feed items? Saved items will stay.")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete unsaved" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_unsaved_feed_items");
    });
    await within(feedList).findByText("Saved report to keep");
    expect(within(feedList).queryByText("Unsaved report to delete")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("refresh_sources", expect.anything());
  });

  it("summarizes the current inbox review set", async () => {
    const user = userEvent.setup();

    renderApp();

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

  it("filters inbox sample items by search query", async () => {
    const user = userEvent.setup();

    renderApp();

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.type(screen.getByLabelText("Search feed"), "transcript");

    expect(within(feedList).getByText("Transcript-derived note candidate waits for future provider work")).toBeInTheDocument();
    expect(within(feedList).queryByText("Current report placeholder for watchlist company")).not.toBeInTheDocument();
  });

  it("filters inbox sample items by type and source", async () => {
    const user = userEvent.setup();

    renderApp();

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

    renderApp();

    const feedList = screen.getByLabelText("Feed items");
    await within(feedList).findByText("Current report placeholder for watchlist company");

    await user.type(screen.getByLabelText("Search feed"), "does-not-match");

    expect(within(feedList).getByText("No feed items for selected filters.")).toBeInTheDocument();

    await user.click(screen.getAllByRole("button", { name: "Clear filters" })[0]);

    expect(screen.getByLabelText("Search feed")).toHaveValue("");
    expect(within(feedList).getByText("Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(within(feedList).getByText("Sample item proving the inbox layout can scan dense rows")).toBeInTheDocument();
  });

  it("shows a first-run inbox empty state when no companies are tracked", async () => {
    const user = userEvent.setup();

    appTestState.companiesResponse = [];
    appTestState.feedItemsResponse = [];

    renderApp();

    const feedList = screen.getByLabelText("Feed items");

    expect(await within(feedList).findByText("No companies tracked yet.")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add company" }));

    expect(screen.getByRole("heading", { name: "Companies" })).toBeInTheDocument();
  });

  it("shows a no-feed inbox empty state with a source status path when companies exist", async () => {
    const user = userEvent.setup();

    appTestState.feedItemsResponse = [];

    renderApp();

    const feedList = screen.getByLabelText("Feed items");

    expect(await within(feedList).findByText("No stored feed items yet.")).toBeInTheDocument();
    expect(within(feedList).getByRole("button", { name: "Refresh sources" })).toBeEnabled();

    await user.click(within(feedList).getByRole("button", { name: "Open Sources" }));

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Source details")).toBeInTheDocument();
  });

  it("updates sample read and saved state from the detail pane", async () => {
    const user = userEvent.setup();

    renderApp();

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

    renderApp();

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

  it("shows typed signal badges, filters by signal type, and confirms an AI proposal", async () => {
    appTestState.companySignalsResponse = [
      {
        id: "signal_feed_sample_cdr_report_insider_transaction",
        companyId: "company_gpw_cdr",
        company: "GPW:CDR",
        companyName: "CD PROJEKT S.A.",
        feedItemId: "feed_sample_cdr_report",
        category: "insider_transaction",
        categoryDisplayName: "Insider transaction",
        confidence: 0.95,
        classifiedBy: "rule",
        status: "confirmed",
        signalDate: "2026-05-28",
        providerId: null,
        modelId: null,
        derivedEventId: null,
        title: "Current report placeholder for watchlist company",
        sourceUrl: "https://example.test/source",
        createdAt: "2026-05-28T12:00:00Z",
        updatedAt: "2026-05-28T12:00:00Z",
      },
      {
        id: "signal_feed_sample_pkn_news_guidance_change",
        companyId: "company_gpw_pkn",
        company: "GPW:PKN",
        companyName: "PKN ORLEN S.A.",
        feedItemId: "feed_sample_pkn_news",
        category: "guidance_change",
        categoryDisplayName: "Guidance change",
        confidence: 0.72,
        classifiedBy: "ai",
        status: "proposed",
        signalDate: "2026-05-28",
        providerId: "provider_gemini",
        modelId: "gemini-2.5-flash",
        derivedEventId: null,
        title: "Sample item proving the inbox layout can scan dense rows",
        sourceUrl: "https://example.test/source",
        createdAt: "2026-05-28T12:00:00Z",
        updatedAt: "2026-05-28T12:00:00Z",
      },
    ];

    const user = userEvent.setup();
    renderApp();

    // Wait for the feed (and signals) to load.
    await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });
    const feedList = screen.getByLabelText("Feed items");
    // A confirmed rule signal is visually distinguishable as a feed badge.
    expect((await within(feedList).findAllByText("Insider transaction")).length).toBeGreaterThan(0);
    // The AI proposal renders with the "Proposed" marker, never silently applied.
    expect(within(feedList).getByText("Guidance change · Proposed")).toBeInTheDocument();

    // Filtering by signal type narrows the feed to matching filings.
    await user.selectOptions(screen.getByLabelText("Inbox signal type"), "insider_transaction");
    expect(
      within(feedList).getByText("Current report placeholder for watchlist company"),
    ).toBeInTheDocument();
    expect(
      within(feedList).queryByText("Sample item proving the inbox layout can scan dense rows"),
    ).not.toBeInTheDocument();

    // Clear the filter and confirm the AI proposal from the detail pane.
    await user.selectOptions(screen.getByLabelText("Inbox signal type"), "all");
    await user.click(
      await screen.findByRole("button", {
        name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
      }),
    );
    const detailPane = within(screen.getByLabelText("Feed item details"));
    await user.click(detailPane.getByRole("button", { name: "Confirm" }));

    // Confirmation transitions the proposal to confirmed: the proposed marker is gone.
    await waitFor(() => {
      expect(within(feedList).queryByText("Guidance change · Proposed")).not.toBeInTheDocument();
    });
    expect(within(feedList).getAllByText("Guidance change").length).toBeGreaterThan(0);
  });

  // U7-E1 density contract (ADR 0076 D6): at the S width tier the Inbox detail is
  // a full-pane overlay over the list, raised presentation-only when a row is
  // activated (`data-detail-open`) and lowered by a back control — the list keeps
  // its selection. jsdom has no container queries, so the tier switch itself is
  // browser-tested; here we assert the affordance's DOM + toggle behavior.
  it("raises and lowers the detail overlay from the back control (S overlay affordance)", async () => {
    const user = userEvent.setup();

    renderApp();

    // The detail starts lowered even though the list defaults to a selection.
    expect(screen.getByLabelText("Feed item details")).not.toHaveAttribute("data-detail-open");

    const selectedRow = await screen.findByRole("button", {
      name: "Select feed item: Sample item proving the inbox layout can scan dense rows",
    });
    await user.click(selectedRow);
    expect(selectedRow).toHaveClass("feed-row-selected");
    expect(screen.getByLabelText("Feed item details")).toHaveAttribute("data-detail-open", "true");

    await user.click(
      within(screen.getByLabelText("Feed item details")).getByRole("button", { name: "Back to list" }),
    );

    // The overlay is lowered while the selection is preserved.
    expect(screen.getByLabelText("Feed item details")).not.toHaveAttribute("data-detail-open");
    expect(selectedRow).toHaveClass("feed-row-selected");
  });
});
