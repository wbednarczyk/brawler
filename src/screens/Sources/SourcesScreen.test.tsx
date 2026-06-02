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
} from "../../test/appWorkflowHarness";

describe("Sources screen workflows", () => {
  it("refreshes source-backed feed items from the topbar", async () => {
    const user = userEvent.setup();

    renderApp();

    await within(screen.getByLabelText("Feed items")).findByText(
      "Current report placeholder for watchlist company",
    );

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));

    const refreshedFeedItem = await screen.findByRole("button", {
      name: "Select feed item: Refreshed GPW report from sample source",
    });
    await user.click(refreshedFeedItem);
    expect(screen.getByLabelText("Feed summary")).toHaveTextContent(
      "Refreshed GPW report from sample source",
    );
    expect(await screen.findAllByText("2026-05-30 17:13:31")).not.toHaveLength(0);
    expect(screen.getByText("2026-05-30 17:30:00")).toBeInTheDocument();
    const officialBody = screen.getByLabelText("Official report body");
    expect(officialBody).toHaveTextContent("Stored");
    expect(officialBody).not.toHaveAttribute("open");
    await user.click(within(officialBody).getByText("Official report body"));
    expect(officialBody).toHaveAttribute("open");
    expect(officialBody).toHaveTextContent(
      "Official GPW body text fetched from the detail page.",
    );
    expect(screen.getByText("report.pdf")).toBeInTheDocument();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_sources", { input: { trigger: "manual" } }),
    );

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const refreshSummary = await screen.findByLabelText("Last source refresh summary");

    expect(within(refreshSummary).getByLabelText("Fetched source items")).toHaveTextContent("2");
    expect(within(refreshSummary).getByLabelText("Matched source items")).toHaveTextContent("1");

    const unmatchedItems = await screen.findByLabelText("Unmatched source item diagnostics");
    expect(within(unmatchedItems).queryByText("LUBAWA S.A.")).not.toBeInTheDocument();

    await user.click(within(unmatchedItems).getByRole("button", { name: /Unmatched/i }));

    expect(within(unmatchedItems).getByText("LUBAWA S.A.")).toBeInTheDocument();
    expect(within(unmatchedItems).getByText("Unmatched GPW report from sample source")).toBeInTheDocument();
    expect(within(unmatchedItems).getByText("2026-05-30 17:13:31")).toBeInTheDocument();
  });

  it("shows source refresh failures in the topbar refresh control", async () => {
    const user = userEvent.setup();
    appTestState.refreshSourcesError = "GPW HTTP request failed";

    renderApp();

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));

    const failedRefresh = await screen.findByRole("button", { name: "Source refresh failed" });
    expect(failedRefresh).toHaveAttribute("title", "Source refresh failed: Error: GPW HTTP request failed");
    expect(failedRefresh).toHaveClass("source-refresh-button-danger");
  });

  it("backs off scheduled source polling after repeated refresh failures", async () => {
    const user = userEvent.setup();
    appTestState.refreshSourcesError = "GPW HTTP request failed";

    renderApp();

    await user.click(screen.getByRole("button", { name: "Refresh sources" }));
    await user.click(await screen.findByRole("button", { name: "Source refresh failed" }));
    await user.click(screen.getByRole("button", { name: "Sources" }));
    await user.click(await screen.findByRole("button", { name: "Open source adapter: Bankier Giełda RSS" }));

    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText("In-app · 15 min · backoff 30 min"),
    ).toBeInTheDocument();
  });

  it("refreshes source-backed feed items from the Sources screen", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const sourcesPanel = screen.getByRole("region", { name: "Sources" });
    await user.click(within(sourcesPanel).getByRole("button", { name: "Refresh sources" }));

    expect(await screen.findByLabelText("Last source refresh summary")).toBeInTheDocument();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_sources", { input: { trigger: "manual" } }),
    );
  });

  it("refreshes a single enabled source from source details", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    await user.click(within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: Bankier Giełda RSS",
    }));

    expect(within(await screen.findByLabelText("Source adapter details")).getByText("Public RSS")).toBeInTheDocument();

    await user.click(within(await screen.findByLabelText("Source adapter details")).getByRole("button", {
      name: "Refresh source",
    }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_source", {
        input: {
          adapterId: "bankier-market-rss",
          trigger: "manual",
        },
      }),
    );
    expect(await screen.findByLabelText("Last source refresh summary")).toBeInTheDocument();
  });

  it("shows independently jittered next poll times for enabled feed sources", async () => {
    const user = userEvent.setup();
    const randomSpy = vi
      .spyOn(Math, "random")
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(0.5)
      .mockReturnValueOnce(0.9)
      .mockReturnValue(0.1);

    try {
      renderApp();

      await user.click(screen.getByRole("button", { name: "Sources" }));

      const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
      await user.click(within(sourceAdaptersRegion).getByRole("button", {
        name: "Open source adapter: Bankier Company Komunikaty",
      }));
      const bankierCompanyNextPoll =
        within(await screen.findByLabelText("Source adapter details")).getByText(/^In 15 min \d+s$|^In 16 min$/)
          .textContent;

      await user.click(within(sourceAdaptersRegion).getByRole("button", {
        name: "Open source adapter: Bankier Giełda RSS",
      }));
      const bankierNextPoll =
        within(await screen.findByLabelText("Source adapter details")).getByText(/^In 15 min \d+s$|^In 16 min$/)
          .textContent;

      expect(bankierNextPoll).not.toEqual(bankierCompanyNextPoll);
    } finally {
      randomSpy.mockRestore();
    }
  });

  it("opens source status from the topbar source pill", async () => {
    const user = userEvent.setup();

    renderApp();

    const sourceStatus = await screen.findByRole("button", { name: "Open source status" });

    expect(sourceStatus).toHaveTextContent("Sources");
    expect(sourceStatus).toHaveTextContent("3/7");

    await user.click(sourceStatus);

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const sourceRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW Company Registry",
    });

    expect(sourceRow).toHaveClass("source-row-selected");
    expect(within(sourceAdaptersRegion).getByLabelText("Source adapter details")).toBeInTheDocument();
  });

  it("refreshes database-backed views from the DB status pill", async () => {
    const user = userEvent.setup();

    renderApp();

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

  it("shows source adapter status", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const sourceRow = await within(sourceAdaptersRegion).findByRole("button", {
      name: "Open source adapter: GPW ESPI/EBI",
    });

    expect(within(sourceAdaptersRegion).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("gpw-espi-ebi")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Official Reports · Public Page")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Bankier Giełda RSS")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getAllByText("Public Media · RSS")).toHaveLength(3);
    expect(within(sourceAdaptersRegion).getByText("Bankier Company Komunikaty")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Official Reports · Public JSON")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Bankier Firma RSS")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Bankier Wiadomosci RSS")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Portal Analiz")).toBeInTheDocument();
    expect(within(sourceAdaptersRegion).getByText("Authenticated Research · Authenticated")).toBeInTheDocument();
    expect(within(sourceRow).getByText("Disabled")).toBeInTheDocument();

    await user.click(sourceRow);

    expect(sourceRow).toHaveClass("source-row-selected");
    const sourceDetails = await screen.findByLabelText("Source adapter details");
    expect(within(sourceDetails).getAllByText("Off")).not.toHaveLength(0);
    expect(within(sourceDetails).getByText("Next poll")).toBeInTheDocument();
    expect(within(sourceDetails).getAllByText("Off")).not.toHaveLength(0);
    expect(within(sourceDetails).getByText("Access")).toBeInTheDocument();
    expect(within(sourceDetails).getAllByText("Disabled")).not.toHaveLength(0);
    expect(within(sourceDetails).getByText("Manual")).toBeInTheDocument();
    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText(
        "2 fetched · 1 created · 1 matched · 1 unmatched · details 1/1 stored · 0 failed",
      ),
    ).toBeInTheDocument();
    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText(
        "Disabled while Bankier Company Komunikaty is the active official-report source",
      ),
    ).toBeInTheDocument();

    expect(
      within(await screen.findByLabelText("Source adapter details")).getByText(/Registered for later revisit/),
    ).toBeInTheDocument();
    const sourcePageButton = within(await screen.findByLabelText("Source adapter details")).getByRole("button", {
      name: "Open source page for GPW ESPI/EBI",
    });
    await user.click(sourcePageButton);
    expect(openUrl).toHaveBeenCalledWith("https://www.gpw.pl/komunikaty");

    const portalAnalizRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: Portal Analiz",
    });
    expect(within(portalAnalizRow).getByText("Disabled")).toBeInTheDocument();
    await user.click(portalAnalizRow);
    const portalAnalizDetails = await screen.findByLabelText("Source adapter details");
    expect(within(portalAnalizDetails).getAllByText("Off")).not.toHaveLength(0);
    expect(within(portalAnalizDetails).getByText("Access")).toBeInTheDocument();
    expect(within(portalAnalizDetails).getAllByText("Disabled")).not.toHaveLength(0);
    expect(within(portalAnalizDetails).getByRole("button", { name: "Refresh source" })).toBeDisabled();
    expect(
      within(portalAnalizDetails).getByText(
        "Late-v1 disabled placeholder; no automated access until the authenticated-source implementation is explicitly built",
      ),
    ).toBeInTheDocument();

    await user.click(portalAnalizRow);

    expect(screen.queryByLabelText("Source adapter details")).not.toBeInTheDocument();
  });

  it("refreshes the GPW company registry from source details", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    const registryRow = within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW Company Registry",
    });

    expect(within(registryRow).getByText("Company registry · Public GPW company list")).toBeInTheDocument();

    await user.click(registryRow);
    const registryDetails = await screen.findByLabelText("Source adapter details");
    expect(within(registryDetails).getByText("Next poll")).toBeInTheDocument();
    expect(within(registryDetails).getByText(/In 23h|In 1 day/)).toBeInTheDocument();
    expect(within(registryDetails).getByText("Cache result")).toBeInTheDocument();
    expect(within(registryDetails).getByText("400 cached entries · 400 refreshed or updated")).toBeInTheDocument();
    expect(within(registryDetails).getByText("Refresh policy")).toBeInTheDocument();
    expect(within(registryDetails).queryByText("Detail warning")).not.toBeInTheDocument();
    await user.click(within(await screen.findByLabelText("Company registry refresh")).getByRole("button", {
      name: "Refresh registry",
    }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_gpw_company_registry", {
        input: { trigger: "manual" },
      }),
    );
    expect(await screen.findByText("400/400 cached")).toBeInTheDocument();
  });

  it("lists cached GPW registry companies and adds an untracked company", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceAdaptersRegion = await screen.findByLabelText("Source adapters");
    await user.click(within(sourceAdaptersRegion).getByRole("button", {
      name: "Open source adapter: GPW Company Registry",
    }));

    const registryPanel = await screen.findByLabelText("GPW company registry entries");
    expect(within(registryPanel).queryByText("DINO POLSKA S.A.")).not.toBeInTheDocument();

    await user.click(within(registryPanel).getByRole("button", { name: /Companies/i }));

    expect(await within(registryPanel).findByText("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(await within(registryPanel).findByText("DINO POLSKA S.A.")).toBeInTheDocument();

    await user.type(within(registryPanel).getByLabelText("Search GPW company registry"), "dnp");

    expect(within(registryPanel).queryByText("CD PROJEKT S.A.")).not.toBeInTheDocument();
    expect(await within(registryPanel).findByText("DINO POLSKA S.A.")).toBeInTheDocument();

    await user.click(within(registryPanel).getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("create_company", {
        input: {
          exchange: "GPW",
          ticker: "DNP",
          displayName: "DINO POLSKA S.A.",
          isin: "PLDINPL00011",
          cik: null,
          lei: null,
        },
      }),
    );
    expect(await within(registryPanel).findByTitle("GPW:DNP already added")).toBeDisabled();
  });

  it("expands and collapses source adapter details with keyboard controls", async () => {
    const user = userEvent.setup();

    renderApp();

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
});
