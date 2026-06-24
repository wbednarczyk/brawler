import { describe, it } from "vitest";
import { expect, renderApp, screen, userEvent, within } from "../../test/appWorkflowHarness";

describe("Today/Pulse attention home (ADR 0054)", () => {
  it("renders the attention sections and the watchlist conviction rollup", async () => {
    renderApp({ section: "Today" });

    expect(await screen.findByRole("heading", { name: "Today" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "What changed" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "To verify" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Upcoming reports" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Conviction" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Recent activity" })).toBeInTheDocument();

    // The sample scenario pins one company; the rollup reports the tracked count.
    expect(screen.getByText(/Tracking 1 pinned of/)).toBeInTheDocument();
  });

  it("opens the Inbox from the secondary feed peek", async () => {
    const user = userEvent.setup();
    renderApp({ section: "Today" });

    const activity = await screen.findByLabelText("Recent activity");
    await user.click(within(activity).getByRole("button", { name: "Open Inbox" }));

    expect(await screen.findByRole("heading", { name: "Inbox" })).toBeInTheDocument();
  });
});
