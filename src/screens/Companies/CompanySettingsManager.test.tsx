import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";

// CompanySettingsManager workflows (ADR 0056): the master-detail bulk
// per-company settings surface entered via the Companies screen's "Manage
// settings" toggle. Pins the v1 scope: multi-select (checkboxes/select-all/
// watchlist-scope), the mixed-value "—" indicator, and bulk autopilot writes
// that fire only on an explicit change, never on mount or on selecting rows.

async function openSettingsMode(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Companies" }));
  await user.click(screen.getByRole("button", { name: "Manage settings" }));
  await screen.findByLabelText("Company settings");
}

function rowFor(companyName: string): HTMLElement {
  return screen.getByText(companyName).closest("li") as HTMLElement;
}

describe("CompanySettingsManager workflows (ADR 0056)", () => {
  it("enters settings mode from the Companies screen toggle and leaves it again", async () => {
    const user = userEvent.setup();
    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));
    expect(screen.queryByLabelText("Company settings")).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Manage settings" }));
    expect(await screen.findByLabelText("Company settings")).toBeInTheDocument();
    expect(
      screen.getByText("Select companies on the left to edit their settings."),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_company_autopilot_modes");

    await user.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.queryByLabelText("Company settings")).not.toBeInTheDocument();
  });

  it("multi-selects companies with row checkboxes and select-all", async () => {
    const user = userEvent.setup();
    renderApp();
    await openSettingsMode(user);

    await user.click(within(rowFor("CD PROJEKT S.A.")).getByRole("checkbox"));
    expect(screen.getByText("1 selected")).toBeInTheDocument();

    await user.click(within(rowFor("ORLEN S.A.")).getByRole("checkbox"));
    expect(screen.getByText("2 selected")).toBeInTheDocument();

    await user.click(screen.getByLabelText("Select all"));
    expect(screen.getByText("4 selected")).toBeInTheDocument();
    expect(within(rowFor("PZU S.A.")).getByRole("checkbox")).toBeChecked();

    // Unchecking "select all" clears the whole selection, back to the empty state.
    await user.click(screen.getByLabelText("Select all"));
    expect(screen.getByText("0 selected")).toBeInTheDocument();
    expect(
      screen.getByText("Select companies on the left to edit their settings."),
    ).toBeInTheDocument();
  });

  it("selects every member of a watchlist via the by-watchlist scope control", async () => {
    const user = userEvent.setup();
    renderApp();
    await openSettingsMode(user);

    // Fixture: "Main GPW" contains only CD PROJEKT among the four tracked companies.
    await user.selectOptions(screen.getByLabelText("By watchlist"), "watchlist_main_gpw");

    expect(screen.getByText("1 selected")).toBeInTheDocument();
    expect(within(rowFor("CD PROJEKT S.A.")).getByRole("checkbox")).toBeChecked();
    expect(within(rowFor("ORLEN S.A.")).getByRole("checkbox")).not.toBeChecked();
  });

  it("shows the mixed-value indicator when the selection has different autopilot modes", async () => {
    appTestState.companyAutopilotModesResponse = [
      { companyId: "company_gpw_cdr", mode: "assist" },
      { companyId: "company_gpw_pkn", mode: "autopilot" },
    ];
    const user = userEvent.setup();
    renderApp();
    await openSettingsMode(user);

    await user.click(within(rowFor("CD PROJEKT S.A.")).getByRole("checkbox"));
    await user.click(within(rowFor("ORLEN S.A.")).getByRole("checkbox"));

    expect(await screen.findByLabelText("Autopilot mode")).toHaveValue("");
    expect(screen.getByText("— Mixed —")).toBeInTheDocument();
  });

  it("bulk-changes the autopilot mode for exactly the selected companies, writes only on the explicit change", async () => {
    const user = userEvent.setup();
    renderApp();
    await openSettingsMode(user);

    await user.click(within(rowFor("CD PROJEKT S.A.")).getByRole("checkbox"));
    await user.click(within(rowFor("KGHM POLSKA MIEDZ S.A.")).getByRole("checkbox"));

    // Selecting rows must not itself write anything.
    expect(invoke).not.toHaveBeenCalledWith("set_companies_autopilot", expect.anything());

    await user.selectOptions(screen.getByLabelText("Autopilot mode"), "autopilot");

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("set_companies_autopilot", {
        input: { companyIds: ["company_gpw_cdr", "company_gpw_kgh"], mode: "autopilot" },
      }),
    );
    // Neither the unselected company nor the pre-selection call carried a mode write.
    expect(invoke).not.toHaveBeenCalledWith("set_companies_autopilot", {
      input: { companyIds: expect.arrayContaining(["company_gpw_pzu"]), mode: expect.anything() },
    });
    // The write reflects immediately in both selected rows' badges (the "refresh").
    expect(await within(rowFor("CD PROJEKT S.A.")).findByText("autopilot")).toBeInTheDocument();
    expect(within(rowFor("KGHM POLSKA MIEDZ S.A.")).getByText("autopilot")).toBeInTheDocument();
  });

  it("filters the master company list by search", async () => {
    const user = userEvent.setup();
    renderApp();
    await openSettingsMode(user);

    await user.type(screen.getByLabelText("Search companies"), "pzu");

    expect(screen.getByText("PZU S.A.")).toBeInTheDocument();
    expect(screen.queryByText("CD PROJEKT S.A.")).not.toBeInTheDocument();

    await user.type(screen.getByLabelText("Search companies"), "xyz-no-match");
    expect(screen.getByText("No companies match.")).toBeInTheDocument();
  });
});
