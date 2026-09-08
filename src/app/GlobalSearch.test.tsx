import { describe, it } from "vitest";
import { fireEvent } from "@testing-library/react";
import {
  appTestState,
  expect,
  renderApp,
  screen,
  userEvent,
  vi,
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

  // F4c S2 (ADR 0108 amendment): a `notebook_entry` result lands on the
  // Spółka `notatnik` tool (typed intent) — the `searchFocusSelector` leg it
  // used for the retired Notebooks-global screen is gone.
  it("navigates a notebook_entry result to the Spółka notatnik tool", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "notebook_entry",
          matches: [
            {
              contentType: "notebook_entry",
              sourceId: "note_1",
              companyId: "company_gpw_cdr",
              title: "Profit note",
              snippet: "Possible profit warning next quarter",
              score: 1.5,
            },
          ],
        },
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "profit");

    const results = await screen.findByRole("listbox", { name: "Global search" });
    await user.click(await within(results).findByRole("option", { name: /Profit note/ }));

    const company = await screen.findByRole("region", { name: "Company view" });
    const tool = await within(company).findByRole("group", { name: "Workshop tool" });
    expect(tool).toHaveAttribute("data-tool", "notatnik");
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
  // digest result has no company to open — it lands on the
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

  // #469: GlobalSearch drives its keyboard/ARIA off useComboboxListbox.
  it("has no aria-activedescendant while the results list is closed", async () => {
    renderApp();
    const input = await screen.findByLabelText("Global search");
    expect(input).not.toHaveAttribute("aria-activedescendant");
  });

  it("aria-activedescendant follows ArrowDown to the next result", async () => {
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
              snippet: "Possible profit warning next quarter",
              score: 1.5,
            },
          ],
        },
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "profit");

    const results = await screen.findByRole("listbox", { name: "Global search" });
    const companyOption = await within(results).findByRole("option", { name: /CD PROJEKT S\.A\./ });
    await waitFor(() => expect(input).toHaveAttribute("aria-activedescendant", companyOption.id));

    await user.keyboard("{ArrowDown}");
    const noteOption = within(results).getByRole("option", { name: /Profit note/ });
    expect(input).toHaveAttribute("aria-activedescendant", noteOption.id);
  });

  it("Enter navigates to the active result", async () => {
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

    await user.click(screen.getByRole("button", { name: "Companies" }));
    await screen.findByRole("heading", { name: "Companies" });

    const input = screen.getByLabelText("Global search");
    await user.type(input, "pzu");
    await screen.findByRole("option", { name: /PZU governance report placeholder/ });
    await user.keyboard("{Enter}");

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

  it("Escape closes the open list, then clears the query, then bubbles", async () => {
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
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "cdr");
    const results = await screen.findByRole("listbox", { name: "Global search" });
    expect(await within(results).findByRole("option", { name: /CD PROJEKT S\.A\./ })).toBeInTheDocument();

    const bubbled = vi.fn();
    document.addEventListener("keydown", bubbled);
    try {
      // 1) list open → close-list: the panel closes, the query stays, no bubble.
      await user.keyboard("{Escape}");
      expect(screen.queryByRole("listbox", { name: "Global search" })).not.toBeInTheDocument();
      expect(input).not.toHaveAttribute("aria-activedescendant");
      expect(input).toHaveValue("cdr");
      expect(bubbled).not.toHaveBeenCalled();

      // 2) closed + non-empty query → clear: the query empties, no bubble.
      await user.keyboard("{Escape}");
      expect(input).toHaveValue("");
      expect(bubbled).not.toHaveBeenCalled();

      // 3) closed + empty query → bubble: the event reaches the document.
      await user.keyboard("{Escape}");
      expect(bubbled).toHaveBeenCalled();
    } finally {
      document.removeEventListener("keydown", bubbled);
    }
  });

  it("has no aria-activedescendant while a request is in flight (stale-id guard)", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "company",
          matches: [
            {
              contentType: "company",
              sourceId: "company_alpha",
              companyId: "company_alpha",
              title: "Alpha S.A.",
              snippet: "GPW:ALP",
              score: 2.0,
            },
          ],
        },
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "alpha");
    const results = await screen.findByRole("listbox", { name: "Global search" });
    const optionA = await within(results).findByRole("option", { name: /Alpha S\.A\./ });
    await waitFor(() => expect(input).toHaveAttribute("aria-activedescendant", optionA.id));

    // A fresh keystroke reopens the debounce window — the previous option's
    // id must not linger as aria-activedescendant while nothing referencing
    // it is in the DOM (only the "Searching…" status renders meanwhile).
    await user.type(input, "x");
    expect(input).not.toHaveAttribute("aria-activedescendant");
    // …and the previous query's rows are gone in the very same render — the
    // controller's options are keyed by the query that produced them.
    expect(within(results).queryAllByRole("option")).toHaveLength(0);
  });

  it("an empty query never reports an expanded combobox, and its Escape is left to the app", async () => {
    const user = userEvent.setup();
    renderApp();
    const input = await screen.findByLabelText("Global search");
    await user.click(input);
    expect(input).toHaveAttribute("aria-expanded", "false");
    expect(input).not.toHaveAttribute("aria-controls");
    expect(screen.queryByRole("listbox", { name: "Global search" })).toBeNull();
    // `fireEvent` returns false when a handler called preventDefault — an
    // unconsumed Escape bubbles to whatever the app wants to do with it.
    expect(fireEvent.keyDown(input, { key: "Escape" })).toBe(true);
    expect(input).toHaveAttribute("aria-expanded", "false");
  });

  // #469 sol re-review: the id space is `${contentType}:${sourceId}`, not the
  // bare `sourceId` — a NEW result set that happens to contain the same id
  // must still reset the active option to its own first row, never silently
  // keep pointing at the row that carried that id in the OLD set.
  it("resets the active option to the new result set's first row, even when it repeats a prior id", async () => {
    const user = userEvent.setup();
    appTestState.searchResponse = {
      groups: [
        {
          contentType: "company",
          matches: [
            {
              contentType: "company",
              sourceId: "company_gamma",
              companyId: "company_gamma",
              title: "Gamma S.A.",
              snippet: "GPW:GAM",
              score: 2.0,
            },
            {
              contentType: "company",
              sourceId: "company_shared",
              companyId: "company_shared",
              title: "Shared Co.",
              snippet: "GPW:SHR",
              score: 1.0,
            },
          ],
        },
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "co");
    const resultsA = await screen.findByRole("listbox", { name: "Global search" });
    const sharedOption = await within(resultsA).findByRole("option", { name: /Shared Co\./ });
    await user.keyboard("{ArrowDown}");
    await waitFor(() => expect(input).toHaveAttribute("aria-activedescendant", sharedOption.id));

    appTestState.searchResponse = {
      groups: [
        {
          contentType: "company",
          matches: [
            {
              contentType: "company",
              sourceId: "company_delta",
              companyId: "company_delta",
              title: "Delta S.A.",
              snippet: "GPW:DEL",
              score: 2.0,
            },
            {
              contentType: "company",
              sourceId: "company_shared",
              companyId: "company_shared",
              title: "Shared Co.",
              snippet: "GPW:SHR",
              score: 1.0,
            },
          ],
        },
      ],
    };
    await user.type(input, "x");

    const resultsB = await screen.findByRole("listbox", { name: "Global search" });
    const deltaOption = await within(resultsB).findByRole("option", { name: /Delta S\.A\./ });
    await waitFor(() => expect(input).toHaveAttribute("aria-activedescendant", deltaOption.id));
  });

  // #469 sol re-review: keyboard navigation moves `aria-selected` without
  // moving DOM focus (the input stays focused) — the active row must still
  // be visibly distinct, not only programmatically marked.
  it("paints the active option's background, not only its aria-selected attribute", async () => {
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
      ],
    };

    renderApp();

    const input = await screen.findByLabelText("Global search");
    await user.type(input, "cdr");
    const results = await screen.findByRole("listbox", { name: "Global search" });
    const option = await within(results).findByRole("option", { name: /CD PROJEKT S\.A\./ });
    await waitFor(() => expect(option).toHaveAttribute("aria-selected", "true"));

    // jsdom does not resolve `color-mix()`/CSS custom properties from a
    // stylesheet — assert the paint hook the shell.css rule keys off
    // (`.global-search-result[aria-selected="true"]`), not a computed color.
    expect(option).toHaveAttribute("aria-selected", "true");
    expect(option).toHaveClass("global-search-result");
  });
});
