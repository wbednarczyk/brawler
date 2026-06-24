import { describe, it } from "vitest";
import { appTestState, expect, renderApp, screen, userEvent, waitFor } from "../../test/appWorkflowHarness";

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
});
