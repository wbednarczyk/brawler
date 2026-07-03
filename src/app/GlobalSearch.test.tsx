import { describe, it } from "vitest";
import {
  appTestState,
  expect,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../test/appWorkflowHarness";

const HIGHLIGHT_START = String.fromCharCode(2);
const HIGHLIGHT_END = String.fromCharCode(3);

describe("Global search", () => {
  it("shows grouped results and navigates to the owning screen", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "company",
          matches: [
            {
              contentType: "company",
              sourceId: "company_gpw_cdr",
              companyId: "company_gpw_cdr",
              title: "CD PROJEKT S.A.",
              snippet: "GPW:CDR",
              score: 2.0,
            },
          ],
        },
        {
          contentType: "notebook_entry",
          matches: [
            {
              contentType: "notebook_entry",
              sourceId: "note_1",
              companyId: "company_gpw_cdr",
              title: "Profit note",
              snippet: `Possible ${HIGHLIGHT_START}profit${HIGHLIGHT_END} warning next quarter`,
              score: 1.5,
            },
          ],
        },
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "profit");

    // Grouped results render with localized group titles, scoped to the results panel.
    const results = await screen.findByRole("listbox", { name: "Global search" });
    expect(await within(results).findByText("Companies")).toBeInTheDocument();
    expect(within(results).getByText("Notes")).toBeInTheDocument();
    expect(within(results).getByText("CD PROJEKT S.A.")).toBeInTheDocument();

    // The highlighted term renders as a <mark> (plain text, not HTML).
    expect(within(results).getByText("profit", { selector: "mark" })).toBeInTheDocument();

    // Selecting a company result opens the curated cockpit dashboard (ADR 0057).
    await user.click(within(results).getByRole("option", { name: /CD PROJEKT S\.A\./ }));

    await waitFor(() => {
      expect(screen.getByLabelText("Research cockpit")).toBeInTheDocument();
    });
  });

  it("navigates a feed-item result to the Inbox and selects that item", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "feed_item",
          matches: [
            {
              contentType: "feed_item",
              sourceId: "feed_sample_pzu_report",
              companyId: null,
              title: "PZU governance report placeholder",
              snippet: "PZU governance report placeholder",
              score: 1.0,
            },
          ],
        },
      ],
    };

    renderApp();

    // Start away from the Inbox so navigation is observable.
    await user.click(screen.getByRole("button", { name: "Companies" }));
    await screen.findByRole("heading", { name: "Companies" });

    const input = screen.getByLabelText("Global search");
    await user.type(input, "pzu");

    await user.click(await screen.findByRole("option", { name: /PZU governance report placeholder/ }));

    // Navigates to the Inbox and selects that specific item.
    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Inbox" })).toBeInTheDocument();
    });
    const selectedRow = await screen.findByRole("button", {
      name: "Select feed item: PZU governance report placeholder",
    });
    await waitFor(() => {
      expect(selectedRow).toHaveAttribute("aria-current", "true");
    });
  });
});
