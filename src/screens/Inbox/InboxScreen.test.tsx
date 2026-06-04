import { describe, it } from "vitest";
import { act, fireEvent } from "@testing-library/react";
import type { AiAnalysisJob } from "../../api/types";
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
    expect(screen.getByRole("link", { name: "https://example.local/sample/pkn" })).toHaveAttribute(
      "href",
      "https://example.local/sample/pkn",
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
            url: "https://example.local/report.pdf",
          },
          {
            id: "feed_attachment_sheet_xlsx",
            label: "sheet.xlsx",
            url: "https://example.local/sheet.xlsx",
          },
          {
            id: "feed_attachment_source_terms",
            label: "Regulamin",
            url: "https://example.local/regulamin.pdf",
          },
          {
            id: "feed_attachment_source_privacy",
            label: "Polityka prywatności",
            url: "https://example.local/prywatnosc.pdf",
          },
          {
            id: "feed_attachment_source_cookies",
            label: "Polityka Cookies",
            url: "https://example.local/cookies.pdf",
          },
        ],
      },
    ];

    renderApp();

    const detailPane = within(await screen.findByLabelText("Feed item details"));

    expect(detailPane.getByText("Attachments")).toBeInTheDocument();
    expect(detailPane.getByRole("link", { name: "report.pdf" })).toHaveAttribute(
      "href",
      "https://example.local/report.pdf",
    );
    expect(detailPane.queryByRole("link", { name: "sheet.xlsx" })).not.toBeInTheDocument();
    expect(detailPane.queryByRole("link", { name: "Regulamin" })).not.toBeInTheDocument();
    expect(detailPane.queryByRole("link", { name: "Polityka prywatności" })).not.toBeInTheDocument();
    expect(detailPane.queryByRole("link", { name: "Polityka Cookies" })).not.toBeInTheDocument();
  });

  it("starts AI analysis from a selected feed item", async () => {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      aiProviders: {
        ...appTestState.settingsResponse.aiProviders,
        generalAnalysisProvider: "provider_gemini",
      },
    };

    renderApp();

    await user.click(await screen.findByRole("button", { name: "Summarize impact" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("start_ai_analysis", {
        input: {
          feedItemId: "feed_sample_cdr_report",
          promptPresetId: "default_summary",
          customQuestion: undefined,
        },
      });
    });
    expect(await screen.findByText("AI summary for Current report placeholder for watchlist company")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "GPW ESPI/EBI" })).toHaveAttribute(
      "href",
      "https://www.gpw.pl/komunikaty",
    );
  });

  it("polls visible running AI analysis until the result is stored", async () => {
    vi.useFakeTimers();

    try {
      const createdAt = "2026-01-01T10:00:00.000Z";
      const runningJob: AiAnalysisJob = {
        id: "ai_job_running_cdr",
        feedItemId: "feed_sample_cdr_report",
        promptPresetId: "default_summary",
        customQuestion: null,
        providerId: "provider_gemini",
        model: "gemini-2.5-flash",
        promptVersion: "analysis_v1",
        status: "running",
        errorCode: null,
        error: null,
        createdAt,
        startedAt: createdAt,
        finishedAt: null,
        result: null,
      };
      const completedJob: AiAnalysisJob = {
        ...runningJob,
        status: "succeeded",
        finishedAt: "2026-01-01T10:00:02.000Z",
        result: {
          id: "ai_result_polled_cdr",
          aiAnalysisJobId: runningJob.id,
          feedItemId: runningJob.feedItemId,
          providerId: runningJob.providerId,
          model: runningJob.model,
          promptVersion: runningJob.promptVersion,
          summary: "Polled AI summary for CDR.",
          significance: "medium",
          reasoning: "Completed by the background poll.",
          language: "en",
          tags: ["analysis"],
          sourceReferences: [
            {
              id: "ai_source_polled_cdr",
              sourceUrl: "https://www.gpw.pl/komunikaty",
              label: "GPW ESPI/EBI",
              createdAt,
            },
          ],
          createdAt,
        },
      };

      appTestState.settingsResponse = {
        ...appTestState.settingsResponse,
        aiProviders: {
          ...appTestState.settingsResponse.aiProviders,
          generalAnalysisProvider: "provider_gemini",
        },
      };
      appTestState.aiAnalysisJobsResponse = {
        feed_sample_cdr_report: [runningJob],
      };

      renderApp();

      await act(async () => {
        await Promise.resolve();
      });

      expect(screen.getByText("running")).toBeInTheDocument();

      appTestState.aiAnalysisJobsResponse = {
        feed_sample_cdr_report: [completedJob],
      };

      await act(async () => {
        vi.advanceTimersByTime(1500);
        await Promise.resolve();
      });

      expect(screen.getByText("Polled AI summary for CDR.")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Refresh analysis" })).not.toBeInTheDocument();
    } finally {
      vi.useRealTimers();
    }
  });

  it("opens the matching company workspace from an inbox feed item", async () => {
    const user = userEvent.setup();

    renderApp();

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
    expect(screen.getByRole("link", { name: "https://example.local/sample/pkn" })).toBeInTheDocument();
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
    expect(openUrl).toHaveBeenCalledWith("https://example.local/sample/pkn");

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

    expect(resizer).toHaveAttribute("aria-valuenow", "360");

    resizer.focus();
    await user.keyboard("{ArrowLeft}");

    expect(resizer).toHaveAttribute("aria-valuenow", "384");
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
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

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

    await user.click(screen.getByRole("button", { name: "Delete unsaved" }));

    expect(confirm).toHaveBeenCalledWith("Delete all unsaved feed items? Saved items will stay.");
    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("delete_unsaved_feed_items");
    });
    await within(feedList).findByText("Saved report to keep");
    expect(within(feedList).queryByText("Unsaved report to delete")).not.toBeInTheDocument();
    expect(invoke).not.toHaveBeenCalledWith("refresh_sources", expect.anything());

    confirm.mockRestore();
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
});
