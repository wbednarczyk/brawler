import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  initialCompanies,
  renderApp,
  screen,
  userEvent,
  within,
} from "../../test/appWorkflowHarness";

describe("Fundamentals manual entry workflow", () => {
  it("shows fundamentals tab in company workspace", async () => {
    const user = userEvent.setup();

    appTestState.companiesResponse = initialCompanies;

    renderApp();

    await user.click(screen.getByRole("button", { name: "Companies" }));

    // Find first company row and click to open workspace
    const companiesList = await screen.findByLabelText("Companies list");
    const firstCompanyRow = within(companiesList).getAllByRole("button", {
      name: /workspace/,
    })[0];

    await user.click(firstCompanyRow);

    // Look for Fundamentals tab in workspace
    const fundamentalsTab = await screen.findByRole("button", {
      name: /Fundamentals/,
    });
    expect(fundamentalsTab).toBeInTheDocument();

    await user.click(fundamentalsTab);

    // Verify fundamentals panel is visible
    const fundamentalsPanel = await screen.findByLabelText("Company fundamentals");
    expect(fundamentalsPanel).toBeInTheDocument();

    // Verify empty state text
    expect(
      within(fundamentalsPanel).getByText(/No reporting periods yet/),
    ).toBeInTheDocument();
  });
});
