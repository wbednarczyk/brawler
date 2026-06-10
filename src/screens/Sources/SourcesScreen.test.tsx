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
    await screen.findByLabelText("Source list");
    expect(screen.queryByLabelText("Developer source refresh summary")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Last source refresh summary")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Unmatched source item diagnostics")).not.toBeInTheDocument();
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
    await user.click(await screen.findByRole("button", { name: "Open source: Bankier Giełda RSS" }));

    expect(
      within(await screen.findByLabelText("Source details")).getByText("In-app · 15 min · backoff 30 min"),
    ).toBeInTheDocument();
  });

  it("refreshes source-backed feed items from the Sources screen", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const sourcesPanel = screen.getByRole("region", { name: "Sources" });
    await user.click(within(sourcesPanel).getByRole("button", { name: "Refresh sources" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_sources", { input: { trigger: "manual" } }),
    );
    expect(screen.queryByLabelText("Developer source refresh summary")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Last source refresh summary")).not.toBeInTheDocument();
  });

  it("does not show a duplicate source refresh button in source details", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourcesRegion = await screen.findByLabelText("Source list");
    await user.click(within(sourcesRegion).getByRole("button", {
      name: "Open source: Bankier Giełda RSS",
    }));

    expect(within(await screen.findByLabelText("Source details")).getByText("Status")).toBeInTheDocument();
    expect(
      within(await screen.findByLabelText("Source details")).queryByRole("button", {
        name: "Refresh source",
      }),
    ).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Developer source refresh summary")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Last source refresh summary")).not.toBeInTheDocument();
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

      const sourcesRegion = await screen.findByLabelText("Source list");
      await user.click(within(sourcesRegion).getByRole("button", {
        name: "Open source: Bankier Company Komunikaty",
      }));
      const bankierCompanyNextPoll =
        within(await screen.findByLabelText("Source details")).getByText(/^In 15 min \d+s$|^In 16 min$/)
          .textContent;

      await user.click(within(sourcesRegion).getByRole("button", {
        name: "Open source: Bankier Giełda RSS",
      }));
      const bankierNextPoll =
        within(await screen.findByLabelText("Source details")).getByText(/^In 15 min \d+s$|^In 16 min$/)
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
    expect(sourceStatus).toHaveTextContent("6/6");

    await user.click(sourceStatus);

    expect(screen.getByRole("heading", { name: "Sources" })).toBeInTheDocument();
    const sourcesRegion = await screen.findByLabelText("Source list");
    const sourceRow = within(sourcesRegion).getByRole("button", {
      name: "Open source: GPW Company Directory",
    });

    expect(sourceRow).toHaveClass("source-row-selected");
    expect(within(sourcesRegion).getByLabelText("Source details")).toBeInTheDocument();
  });

  it("shows normal-user source status and hides developer-only candidates", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourcesRegion = await screen.findByLabelText("Source list");
    const sourceRow = await within(sourcesRegion).findByRole("button", {
      name: "Open source: Bankier Company Komunikaty",
    });

    expect(within(sourcesRegion).queryByText("GPW ESPI/EBI")).not.toBeInTheDocument();
    expect(within(sourcesRegion).queryByText("Portal Analiz")).not.toBeInTheDocument();
    expect(within(sourcesRegion).queryByText("bankier-market-rss")).not.toBeInTheDocument();
    expect(within(sourcesRegion).getByText("Bankier Giełda RSS")).toBeInTheDocument();
    expect(within(sourcesRegion).getByText("Public Media")).toBeInTheDocument();
    expect(within(sourcesRegion).getByText("Bankier Company Komunikaty")).toBeInTheDocument();
    expect(within(sourcesRegion).getByText("Official Reports")).toBeInTheDocument();
    expect(within(sourcesRegion).getByText("NewConnect Company Directory")).toBeInTheDocument();
    expect(within(sourcesRegion).getByText("Company directory · NewConnect company list")).toBeInTheDocument();
    expect(within(sourceRow).getByText("Not refreshed yet")).toBeInTheDocument();
    expect(
      within(sourceRow).getByRole("switch", { name: "Turn off Bankier Company Komunikaty" }),
    ).toBeChecked();

    await user.click(sourceRow);

    expect(sourceRow).toHaveClass("source-row-selected");
    const sourceDetails = await screen.findByLabelText("Source details");
    expect(within(sourceDetails).getByText("Next poll")).toBeInTheDocument();
    expect(
      within(await screen.findByLabelText("Source details")).getByText(
        "2 fetched · 1 created · 1 matched · 0 unmatched",
      ),
    ).toBeInTheDocument();
    const sourcePageButton = within(await screen.findByLabelText("Source details")).getByRole("button", {
      name: "Open source page for Bankier Company Komunikaty",
    });
    await user.click(sourcePageButton);
    expect(openUrl).toHaveBeenCalledWith("https://www.bankier.pl/gielda/notowania/akcje/{TICKER}/komunikaty");
  });

  it("persists optional source enablement and protects required sources", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourcesRegion = await screen.findByLabelText("Source list");
    const bankierRow = within(sourcesRegion).getByRole("button", {
      name: "Open source: Bankier Giełda RSS",
    });
    const bankierToggle = within(bankierRow).getByRole("switch", {
      name: "Turn off Bankier Giełda RSS",
    });

    await user.click(bankierToggle);

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("set_source_adapter_enabled", {
        input: { adapterId: "bankier-market-rss", enabled: false },
      }),
    );
    expect(
      within(bankierRow).getByRole("switch", { name: "Turn on Bankier Giełda RSS" }),
    ).not.toBeChecked();
    expect(within(bankierRow).getAllByText("Off").length).toBeGreaterThan(0);

    const directoryRow = within(sourcesRegion).getByRole("button", {
      name: "Open source: GPW Company Directory",
    });
    expect(within(directoryRow).queryByRole("checkbox")).not.toBeInTheDocument();
  });

  it("keeps source order stable when an optional source is disabled", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const calendarGroup = await screen.findByRole("region", { name: "Calendar and events" });
    const sourceNamesBefore = within(calendarGroup)
      .getAllByRole("button", { name: /Open source:/ })
      .map((row) => row.getAttribute("aria-label"));

    const bankierCalendarRow = within(calendarGroup).getByRole("button", {
      name: "Open source: Bankier Kalendarium",
    });
    await user.click(within(bankierCalendarRow).getByRole("switch", {
      name: "Turn off Bankier Kalendarium",
    }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("set_source_adapter_enabled", {
        input: { adapterId: "bankier-kalendarium-html", enabled: false },
      }),
    );

    const sourceNamesAfter = within(calendarGroup)
      .getAllByRole("button", { name: /Open source:/ })
      .map((row) => row.getAttribute("aria-label"));

    expect(sourceNamesAfter).toEqual(sourceNamesBefore);
  });

  it("shows source candidates only in Developer diagnostics", async () => {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };

    renderApp();

    await user.click(await screen.findByRole("button", { name: "Sources" }));
    const sourcesRegion = await screen.findByLabelText("Source list");
    expect(within(sourcesRegion).queryByText("GPW ESPI/EBI")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Diagnostics" }));
    await user.click(await screen.findByRole("button", { name: /Source candidates/i }));

    expect(await screen.findByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(await screen.findByText("Portal Analiz")).toBeInTheDocument();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("list_source_adapters", {
        input: { includeDeveloperOnly: true },
      }),
    );
  });

  it("shows source refresh summary only in Developer mode at the end of Sources", async () => {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      developerMode: true,
    };

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));
    const sourcesPanel = screen.getByRole("region", { name: "Sources" });
    const sourcesRegion = await screen.findByLabelText("Source list");
    await user.click(within(sourcesPanel).getByRole("button", { name: "Refresh sources" }));

    const developerSummary = await within(sourcesRegion).findByLabelText("Developer source refresh summary");
    const refreshSummary = within(developerSummary).getByLabelText("Last source refresh summary");

    expect(within(refreshSummary).getByLabelText("Fetched source items")).toHaveTextContent("2");
    expect(within(refreshSummary).getByLabelText("Matched source items")).toHaveTextContent("1");
    expect(sourcesRegion.lastElementChild).toBe(developerSummary);
  });

  it("refreshes the GPW company registry from source details", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourcesRegion = await screen.findByLabelText("Source list");
    const registryRow = within(sourcesRegion).getByRole("button", {
      name: "Open source: GPW Company Directory",
    });

    expect(within(registryRow).getByText("Company directory · GPW company list")).toBeInTheDocument();

    await user.click(registryRow);
    const registryDetails = await screen.findByLabelText("Source details");
    expect(within(registryDetails).getByText("Next poll")).toBeInTheDocument();
    expect(within(registryDetails).getByText(/In 23h|In 1 day/)).toBeInTheDocument();
    expect(within(registryDetails).getByText("Directory result")).toBeInTheDocument();
    expect(within(registryDetails).getByText("400 directory entries · 400 refreshed or updated")).toBeInTheDocument();
    expect(within(registryDetails).queryByText("Detail warning")).not.toBeInTheDocument();
    await user.click(within(await screen.findByLabelText("Company directory refresh")).getByRole("button", {
      name: "Refresh company directory",
    }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("refresh_gpw_company_registry", {
        input: { trigger: "manual" },
      }),
    );
    expect(await screen.findByText("750/750 saved entries")).toBeInTheDocument();
  });

  it("keeps company directory lists separated by source and adds companies from each", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourcesRegion = await screen.findByLabelText("Source list");
    await user.click(within(sourcesRegion).getByRole("button", {
      name: "Open source: GPW Company Directory",
    }));

    const registryPanel = await screen.findByLabelText("Company directory entries");
    expect(within(registryPanel).queryByText("DINO POLSKA S.A.")).not.toBeInTheDocument();
    expect(within(registryPanel).queryByText("4MOBILITY SPÓŁKA AKCYJNA")).not.toBeInTheDocument();

    await user.click(within(registryPanel).getByRole("button", { name: /Companies/i }));

    expect(await within(registryPanel).findByText("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(await within(registryPanel).findByText("DINO POLSKA S.A.")).toBeInTheDocument();
    expect(within(registryPanel).queryByText("4MOBILITY SPÓŁKA AKCYJNA")).not.toBeInTheDocument();

    await user.type(within(registryPanel).getByLabelText("Search company directory"), "dnp");

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

    await user.click(within(sourcesRegion).getByRole("button", {
      name: "Open source: NewConnect Company Directory",
    }));
    const newConnectPanel = await screen.findByLabelText("Company directory entries");
    const newConnectToggle = within(newConnectPanel).getByRole("button", { name: /Companies/i });
    if (newConnectToggle.getAttribute("aria-expanded") !== "true") {
      await user.click(newConnectToggle);
    }
    const newConnectSearch = within(newConnectPanel).getByLabelText("Search company directory");
    await user.clear(newConnectSearch);

    expect(await within(newConnectPanel).findByText("4MOBILITY SPÓŁKA AKCYJNA")).toBeInTheDocument();
    expect(within(newConnectPanel).queryByText("CD PROJEKT S.A.")).not.toBeInTheDocument();

    await user.type(newConnectSearch, "4mb");
    await user.click(within(newConnectPanel).getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("create_company", {
        input: {
          exchange: "NC",
          ticker: "4MB",
          displayName: "4MOBILITY SPÓŁKA AKCYJNA",
          isin: "PLESLTN00010",
          cik: null,
          lei: null,
        },
      }),
    );
  });

  it("expands and collapses source details with keyboard controls", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Sources" }));

    const sourceRow = await screen.findByRole("button", {
      name: "Open source: Bankier Giełda RSS",
    });

    sourceRow.focus();
    await user.keyboard("{Enter}");

    expect(await screen.findByLabelText("Source details")).toBeInTheDocument();

    await user.keyboard(" ");

    expect(screen.queryByLabelText("Source details")).not.toBeInTheDocument();
  });
});
