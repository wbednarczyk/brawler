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

    // Selecting a company result opens the Spółka screen (F3a S1, ADR 0107).
    await user.click(within(results).getByRole("option", { name: /CD PROJEKT S\.A\./ }));

    await waitFor(() => {
      expect(screen.getByRole("region", { name: "Company view" })).toBeInTheDocument();
    });
  });

  // Regression for bug c80dabe: cross-navigation into the Inbox promised to
  // clear filters "so the selected item is not hidden by an active filter",
  // but the signal filter was omitted from the reset — a stale signal filter
  // silently hid the whole feed (0 items on "All") after navigation.
  it("clears a stale signal filter so a searched feed item is visible", async () => {
    const user = userEvent.setup();
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
    ];
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

    // Narrow the feed to one signal category; the PZU item carries no signals.
    await screen.findByRole("button", {
      name: "Select feed item: Current report placeholder for watchlist company",
    });
    await user.selectOptions(screen.getByLabelText("Inbox signal type"), "insider_transaction");
    const feedList = screen.getByLabelText("Feed items");
    expect(
      within(feedList).queryByText("PZU governance report placeholder"),
    ).not.toBeInTheDocument();

    const input = screen.getByLabelText("Global search");
    await user.type(input, "pzu");
    await user.click(await screen.findByRole("option", { name: /PZU governance report placeholder/ }));

    // The navigation contract: the target item is visible, no stale filter hides it.
    expect(
      await screen.findByRole("button", { name: "Select feed item: PZU governance report placeholder" }),
    ).toBeInTheDocument();
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

  // ADR 0108: with the docking engine gone, a companyless research brief/
  // digest result has no cockpit dashboard to open — it lands on the
  // standalone Research screen instead.
  it("a companyless research brief result opens the Research screen", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "research_brief",
          matches: [
            {
              contentType: "research_brief",
              sourceId: "brief_1",
              companyId: null,
              title: "Weekly market brief",
              snippet: "Weekly market brief",
              score: 1.0,
            },
          ],
        },
      ],
    };

    renderApp();

    const input = screen.getByLabelText("Global search");
    await user.type(input, "weekly");

    await user.click(await screen.findByRole("option", { name: /Weekly market brief/ }));

    expect(await screen.findByRole("heading", { name: "Research" })).toBeInTheDocument();
  });
});
