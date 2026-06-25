import { describe, it } from "vitest";
import {
  expect,
  invoke,
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

  it("opens the curated cockpit dashboard from a company row (ADR 0057)", async () => {
    const user = userEvent.setup();

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    const companyRow = await screen.findByRole("button", { name: "Open GPW:CDR dashboard" });
    await user.click(companyRow);

    // The deep-dive is the cockpit dashboard now, not a tabbed workspace inside
    // the Companies screen.
    expect(await screen.findByLabelText("Research cockpit")).toBeInTheDocument();
    expect(screen.queryByLabelText("Company workspace")).not.toBeInTheDocument();
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

});
