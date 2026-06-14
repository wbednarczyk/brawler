import { describe, it } from "vitest";
import { act, fireEvent } from "@testing-library/react";
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

describe("Companies screen workflows", () => {
  it("fills company form from the GPW registry lookup", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.clear(screen.getByLabelText("Ticker"));
    await user.type(screen.getByLabelText("Ticker"), "CDR");
    await user.click(screen.getByRole("button", { name: "Lookup" }));

    expect(await screen.findByDisplayValue("CD PROJEKT S.A.")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLOPTTC00011")).toBeInTheDocument();
    expect(screen.getByText("Filled from company_directory: GPW:CDR")).toBeInTheDocument();
  });

  it("fills company form from NewConnect lookup while the default exchange is GPW", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    expect(screen.getByLabelText("Exchange")).toHaveValue("GPW");

    await user.clear(screen.getByLabelText("Ticker"));
    await user.type(screen.getByLabelText("Ticker"), "4MB");
    await user.click(screen.getByRole("button", { name: "Lookup" }));

    expect(await screen.findByDisplayValue("NC")).toBeInTheDocument();
    expect(screen.getByDisplayValue("4MB")).toBeInTheDocument();
    expect(screen.getByDisplayValue("4MOBILITY SPÓŁKA AKCYJNA")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLESLTN00010")).toBeInTheDocument();
    expect(screen.getByText("Filled from company_directory: NC:4MB")).toBeInTheDocument();
  });

  it("adds a NewConnect company after lookup fills the company form", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.selectOptions(screen.getByLabelText("Company watchlist filter"), "watchlist_main_gpw");
    expect(screen.getByLabelText("Company watchlist filter")).toHaveValue("watchlist_main_gpw");

    await user.clear(screen.getByLabelText("Ticker"));
    await user.type(screen.getByLabelText("Ticker"), "4MB");
    await user.click(screen.getByRole("button", { name: "Lookup" }));
    expect(await screen.findByDisplayValue("NC")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add" }));

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
    expect(screen.getByLabelText("Company watchlist filter")).toHaveValue("all");
    expect(within(screen.getByLabelText("Companies list")).getByText("NC:4MB")).toBeInTheDocument();
  });

  it("selects a company from local GPW registry suggestions", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "DINO");

    const suggestions = await screen.findByLabelText("Company registry suggestions");
    await user.click(within(suggestions).getByRole("button", { name: /GPW:DNP/ }));

    expect(screen.getByDisplayValue("DNP")).toBeInTheDocument();
    expect(screen.getByDisplayValue("DINO POLSKA S.A.")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLDINPL00011")).toBeInTheDocument();
    expect(screen.getByText("Selected from company directory: GPW:DNP")).toBeInTheDocument();
    expect(screen.queryByLabelText("Company registry suggestions")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear ticker" }));

    expect(screen.getByLabelText("Exchange")).toHaveValue("GPW");
    expect(screen.getByLabelText("Ticker")).toHaveValue("");
    expect(screen.getByLabelText("Name")).toHaveValue("DINO POLSKA S.A.");
    expect(screen.getByLabelText("ISIN")).toHaveValue("PLDINPL00011");
    expect(screen.queryByText("Selected from company directory: GPW:DNP")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Ticker"), "DNP");

    await user.click(screen.getByRole("button", { name: "Add" }));

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
  });

  it("shows NewConnect registry suggestions from the default company form", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    expect(screen.getByLabelText("Exchange")).toHaveValue("GPW");

    await user.clear(screen.getByLabelText("Name"));
    await user.type(screen.getByLabelText("Name"), "4MOBILITY");

    const suggestions = await screen.findByLabelText("Company registry suggestions");
    await user.click(within(suggestions).getByRole("button", { name: /NC:4MB/ }));

    expect(screen.getByLabelText("Exchange")).toHaveValue("NC");
    expect(screen.getByDisplayValue("4MB")).toBeInTheDocument();
    expect(screen.getByDisplayValue("4MOBILITY SPÓŁKA AKCYJNA")).toBeInTheDocument();
    expect(screen.getByDisplayValue("PLESLTN00010")).toBeInTheDocument();
    expect(screen.getByText("Selected from company directory: NC:4MB")).toBeInTheDocument();
  });

  it("adds a company from a future directory entry through the shared lookup flow", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    expect(screen.getByLabelText("Exchange")).toHaveValue("GPW");

    await user.clear(screen.getByLabelText("Ticker"));
    await user.type(screen.getByLabelText("Ticker"), "SAP");
    await user.click(screen.getByRole("button", { name: "Lookup" }));

    expect(await screen.findByDisplayValue("XETRA")).toBeInTheDocument();
    expect(screen.getByDisplayValue("SAP")).toBeInTheDocument();
    expect(screen.getByDisplayValue("SAP SE")).toBeInTheDocument();
    expect(screen.getByDisplayValue("DE0007164600")).toBeInTheDocument();
    expect(screen.getByText("Filled from company_directory: XETRA:SAP")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Add" }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("create_company", {
        input: {
          exchange: "XETRA",
          ticker: "SAP",
          displayName: "SAP SE",
          isin: "DE0007164600",
          cik: null,
          lei: null,
        },
      }),
    );
    expect(within(screen.getByLabelText("Companies list")).getByText("XETRA:SAP")).toBeInTheDocument();
  });

  it("filters the tracked companies list", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const companyList = await screen.findByLabelText("Companies list");
    expect(within(companyList).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(companyList).getByText("GPW:PZU")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Search tracked companies"), "pzu");

    expect(within(companyList).queryByText("GPW:CDR")).not.toBeInTheDocument();
    expect(within(companyList).getByText("GPW:PZU")).toBeInTheDocument();
    expect(screen.getByText("1/4 companies")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Clear company search" }));

    expect(within(companyList).getByText("GPW:CDR")).toBeInTheDocument();
    expect(screen.getByText("4/4 companies")).toBeInTheDocument();
  });

  it("filters the tracked companies list by watchlist", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const companyList = await screen.findByLabelText("Companies list");

    expect(within(companyList).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(companyList).getByText("GPW:PZU")).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Company watchlist filter"), "watchlist_main_gpw");

    expect(within(companyList).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(companyList).queryByText("GPW:PZU")).not.toBeInTheDocument();
    expect(screen.getByText("1/4 companies")).toBeInTheDocument();
  });

  it("opens the matching Watchlists panel from a company watchlist pill", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(
      within(await screen.findByLabelText("Watchlist memberships for GPW:CDR")).getByRole("button", {
        name: "Main GPW",
      }),
    );

    expect(await screen.findByRole("heading", { name: "Watchlists" })).toBeInTheDocument();
    expect(
      within(screen.getByLabelText("Selected watchlist")).getByRole("heading", { name: "Main GPW" }),
    ).toBeInTheDocument();
  });

  it("confirms and deletes a company", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByTitle("Delete GPW:CDR"));

    expect(confirm).toHaveBeenCalledWith("Delete GPW:CDR from tracked companies?");

    confirm.mockRestore();
  });

  it("opens a company workspace with company-scoped feed and metadata tabs", async () => {
    const user = userEvent.setup();

    renderApp();

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
    expect(within(screen.getByLabelText("Company feed")).queryByText("Sample item proving the inbox layout can scan dense rows")).not.toBeInTheDocument();

    await user.click(within(workspace).getByRole("button", { name: "Metadata" }));

    expect(within(screen.getByLabelText("Company metadata")).getByText("PLOPTTC00011")).toBeInTheDocument();
  });

  it("runs company workspace navigation shortcuts", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    expect(await screen.findByLabelText("Company workspace")).toBeInTheDocument();
    expect(screen.getByLabelText("Company workspace")).toHaveTextContent("CD PROJEKT S.A.");

    await act(async () => {
      fireEvent.keyDown(document, { key: "L", code: "KeyL" });
    });
    expect(await screen.findByLabelText("Company notebook")).toBeInTheDocument();

    await act(async () => {
      fireEvent.keyDown(document, { key: "H", code: "KeyH" });
    });
    expect(await screen.findByLabelText("Company feed")).toBeInTheDocument();

    await act(async () => {
      fireEvent.keyDown(document, { key: "J", code: "KeyJ", shiftKey: true });
    });
    expect(screen.getByLabelText("Company workspace")).toHaveTextContent("ORLEN S.A.");

    await act(async () => {
      fireEvent.keyDown(document, { key: "K", code: "KeyK", shiftKey: true });
    });
    expect(screen.getByLabelText("Company workspace")).toHaveTextContent("CD PROJEKT S.A.");
  });

  it("lists and creates company notebook entries", async () => {
    const user = userEvent.setup();

    appTestState.notebookEntriesResponse = [initialNotebookEntry];

    renderApp();

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

    appTestState.notebookEntriesResponse = [initialNotebookEntry];

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    await user.click(screen.getByRole("button", { name: "Claims" }));

    const claims = await screen.findByLabelText("Company claims");
    const claimRow = within(claims).getByRole("button", {
      name: "Open claim: Release schedule promise",
    });

    expect(claimRow).toBeInTheDocument();
    expect(claims).toHaveTextContent("1 follow-up item for GPW:CDR");

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

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    const companyRow = await screen.findByRole("button", { name: "Open GPW:CDR workspace" });

    await user.click(companyRow);
    expect(await screen.findByLabelText("Company workspace")).toBeInTheDocument();

    await user.click(companyRow);
    expect(screen.queryByLabelText("Company workspace")).not.toBeInTheDocument();
  });

  it("moves through company rows with arrow keys without expanding a collapsed workspace", async () => {
    const user = userEvent.setup();

    appTestState.companiesResponse = [
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

    renderApp();

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

    appTestState.companiesResponse = [
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

    renderApp();

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

    appTestState.companiesResponse = [
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

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:LPP workspace" }));

    const companyFeed = await screen.findByLabelText("Company feed");

    expect(companyFeed).toHaveTextContent("No stored feed items for GPW:LPP yet.");
    expect(
      within(companyFeed).getByText(
        "This company is tracked, but no sample or ingested items are attached to it yet.",
      ),
    ).toBeInTheDocument();

    await user.click(within(companyFeed).getByRole("button", { name: "Open filtered Inbox" }));

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:LPP");
    expect(screen.getByText("No feed items for selected filters.")).toBeInTheDocument();
  });

  it("shows company feed item details inline and can open the item in the inbox", async () => {
    const user = userEvent.setup();
    appTestState.settingsResponse = {
      ...appTestState.settingsResponse,
      aiProviders: {
        ...appTestState.settingsResponse.aiProviders,
        generalAnalysisProvider: "provider_gemini",
      },
    };

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));
    const companyFeedRow = await screen.findByRole("button", {
      name: "Open company feed item: Current report placeholder for watchlist company",
    });

    await user.click(companyFeedRow);

    const companyFeedDetail = await screen.findByLabelText("Company feed item details");

    expect(companyFeedRow.compareDocumentPosition(companyFeedDetail) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(companyFeedDetail).getByLabelText("Feed summary")).toHaveTextContent(
      "Sample official report used to validate feed filtering and detail rendering.",
    );
    const companyOfficialBody = within(companyFeedDetail).getByLabelText("Official report body");
    expect(companyOfficialBody).toHaveTextContent("Not stored");
    expect(companyOfficialBody).not.toHaveAttribute("open");
    await user.click(within(companyOfficialBody).getByText("Official report body"));
    expect(companyOfficialBody).toHaveAttribute("open");
    expect(companyOfficialBody).toHaveTextContent(/No official report body is stored/);
    expect(within(companyFeedDetail).getByText("GPW ESPI/EBI")).toBeInTheDocument();
    expect(within(companyFeedDetail).getByRole("link", { name: "Open source" })).toHaveAttribute(
      "href",
      "https://www.gpw.pl/komunikaty",
    );
    // AI analysis controls live in a modal launched from the detail panel; the
    // modal portals to <body>, so query its contents at screen level.
    await user.click(within(companyFeedDetail).getByRole("button", { name: "Analyze with AI" }));
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
    // The analysis result renders in the modal, which portals to <body> (not
    // inside the company feed detail subtree).
    expect(
      await screen.findByText("AI summary for Current report placeholder for watchlist company"),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Close dialog" }));
    await user.click(within(companyFeedDetail).getByRole("button", { name: "Open in Inbox" }));

    expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    expect(screen.getByLabelText("Inbox company")).toHaveValue("GPW:CDR");
    expect(screen.getByLabelText("Feed item details")).toBeInTheDocument();
  });

  it("uses inbox unread and saved visual state in company feed rows", async () => {
    const user = userEvent.setup();

    renderApp();

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

    renderApp();

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

    appTestState.feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_report",
        title: "Second CDR report for company feed keyboard navigation",
        summary: "Second company-scoped sample item.",
        unread: false,
      },
    ];

    renderApp();

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

    appTestState.feedItemsResponse = [
      initialFeedItems[0],
      {
        ...initialFeedItems[0],
        id: "feed_sample_cdr_second_report",
        title: "Second CDR report for company feed keyboard navigation",
        summary: "Second company-scoped sample item.",
        unread: false,
      },
    ];

    renderApp();

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
        "Second company-scoped sample item.",
      ),
    ).toBeInTheDocument();

    await user.keyboard(" ");
    expect(screen.queryByLabelText("Company feed item details")).not.toBeInTheDocument();
  });

  it("updates company feed item read and saved state from inline details", async () => {
    const user = userEvent.setup();

    renderApp();

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
        id: "feed_sample_cdr_report",
        read: true,
        saved: false,
      },
    });
    expect(invoke).toHaveBeenCalledWith("update_feed_item_state", {
      input: {
        id: "feed_sample_cdr_report",
        read: true,
        saved: true,
      },
    });
  });

  it("creates a watchlist and assigns an already-tracked company from the Watchlists panel", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Watchlists" }));
    await user.type(screen.getByLabelText("Watchlist name"), "Growth GPW");
    await user.click(screen.getByRole("button", { name: "Create" }));
    await user.click(await screen.findByRole("button", { name: /Growth GPW/ }));
    await user.click(screen.getByRole("button", { name: "Add companies" }));
    await user.click(within(screen.getByLabelText("Add companies")).getByRole("button", { name: /GPW:CDR/ }));
    await user.click(screen.getByRole("button", { name: "Add selected" }));

    expect(await within(screen.getByLabelText("Companies in watchlist")).findByText("GPW:CDR")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Companies" }));
    expect(
      await within(screen.getByLabelText("Watchlist memberships for GPW:CDR")).findByText("Growth GPW"),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("create_watchlist", {
      input: {
        name: "Growth GPW",
        description: null,
      },
    });
    expect(invoke).toHaveBeenCalledWith("add_company_to_watchlist", {
      input: {
        watchlistId: "watchlist_growth_gpw",
        companyId: "company_gpw_cdr",
      },
    });
  });

  it("renames a watchlist without changing its stable id", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Watchlists" }));
    await user.click(screen.getByRole("button", { name: "Rename" }));
    await user.clear(screen.getByLabelText("Rename watchlist"));
    await user.type(screen.getByLabelText("Rename watchlist"), "Long-term GPW");
    await user.click(screen.getByRole("button", { name: "Save" }));

    expect(await screen.findByRole("button", { name: /Long-term GPW/ })).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("rename_watchlist", {
      input: {
        id: "watchlist_main_gpw",
        name: "Long-term GPW",
        description: null,
      },
    });
  });

  it("removes a company from a selected watchlist in the Watchlists panel", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Watchlists" }));
    await user.click(within(await screen.findByLabelText("Companies in watchlist")).getByRole("button", { name: /Remove/ }));

    expect(invoke).toHaveBeenCalledWith("remove_company_from_watchlist", {
      input: {
        watchlistId: "watchlist_main_gpw",
        companyId: "company_gpw_cdr",
      },
    });
  });

  it("deletes a watchlist, keeps companies, and resets an active company watchlist filter", async () => {
    const user = userEvent.setup();
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.selectOptions(screen.getByLabelText("Company watchlist filter"), "watchlist_main_gpw");

    expect(screen.getByText("1/4 companies")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Watchlists" }));
    await user.click(within(screen.getByLabelText("Selected watchlist")).getByRole("button", { name: "Delete" }));

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await waitFor(() => expect(screen.getByText("4/4 companies")).toBeInTheDocument());
    expect(screen.getByLabelText("Company watchlist filter")).toHaveValue("all");
    expect(invoke).toHaveBeenCalledWith("delete_watchlist", {
      watchlistId: "watchlist_main_gpw",
    });

    confirm.mockRestore();
  });

  it("shows watchlist memberships in the company workspace without mutation controls", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await user.click(await screen.findByRole("button", { name: "Open GPW:CDR workspace" }));

    expect(within(await screen.findByLabelText("Company workspace")).getByText("Main GPW")).toBeInTheDocument();
    expect(screen.queryByLabelText("Manage watchlists for GPW:CDR")).not.toBeInTheDocument();
  });
});
