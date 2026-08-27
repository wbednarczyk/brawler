import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, waitFor, within } from "../../test/appWorkflowHarness";

// Component-level coverage for the Watchlists empty and no-search-match states.
// These are deliberately NOT covered by the Playwright CRUD journey
// (tests/browser/watchlists.spec.ts), which exercises the happy path — here we
// fill the cheap-layer gaps (empty state, search filter) per ADR 0048.

describe("Watchlists screen states", () => {
  it("shows the empty state when no watchlists exist", async () => {
    appTestState.watchlistsResponse = [];
    appTestState.watchlistMembershipsResponse = [];

    renderApp({ section: "Watchlists" });

    expect(await screen.findByText("No watchlists yet.")).toBeInTheDocument();
  });

  it("shows a no-match message when the search excludes every watchlist", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Watchlists" });
    // The default state seeds one watchlist ("Main GPW"); a non-matching search
    // must surface the distinct "no match" message (vs the "no watchlists" empty
    // state), and hide the seeded watchlist row.
    const search = await screen.findByLabelText("Search watchlists");
    await user.type(search, "zzz-no-such-list");

    await waitFor(() => {
      expect(screen.getByText("No watchlists match this search.")).toBeInTheDocument();
    });
  });

  // Owner decision 2026-08-26 (ADR 0107): watchlist rows get an explicit
  // "Open company" action that lands on that company's Spółka screen through
  // the guarded entry (`openCompanyWorkspaceById`), never a direct state set.
  it("Open company on a watchlist row lands on Spółka", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Watchlists" });

    // The default scenario seeds one watchlist ("Main GPW") with one member
    // (CDR), auto-selected by the screen's own effect.
    const members = await screen.findByLabelText("Companies in watchlist");
    const cdrRow = within(members).getByText("CD PROJEKT S.A.").closest(".watchlist-member-row")!;
    await user.click(within(cdrRow as HTMLElement).getByRole("button", { name: "Open company" }));

    const spolka = await screen.findByRole("region", { name: "Company view" });
    expect(spolka).toHaveAttribute("data-company-id", "company_gpw_cdr");
  });
});
