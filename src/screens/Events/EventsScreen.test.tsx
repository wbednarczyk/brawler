import { describe, it } from "vitest";
import {
  currentWeekTestDate,
  expect,
  invoke,
  renderApp,
  screen,
  userEvent,
  waitFor,
  within,
} from "../../test/appWorkflowHarness";
import { resolveEventViewMode } from "./eventTypes";

// U7-D density (ADR 0076 D6): at S/short the week grid is not offered and the
// list view renders regardless of the persisted mode preference — a presentation
// override that never mutates the stored setting. jsdom has no container queries,
// so the tier switch itself is browser-tested; this asserts the pure resolver.
describe("resolveEventViewMode (density list-mode override)", () => {
  it("forces list when the pane is compact, whatever the stored preference", () => {
    expect(resolveEventViewMode("week", true)).toBe("list");
    expect(resolveEventViewMode("list", true)).toBe("list");
  });

  it("preserves the stored preference when the pane is not compact", () => {
    expect(resolveEventViewMode("week", false)).toBe("week");
    expect(resolveEventViewMode("list", false)).toBe("list");
  });
});

describe("Events screen workflows", () => {
  it("shows upcoming company events from real source-backed event data", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Events" });


    expect(screen.getByRole("heading", { name: "Events" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Week" })).toHaveClass("segment-active");
    const cdrEventRow = await screen.findByRole("button", {
      name: "Open event: Main Market - Corporate actions - Equity - CDR",
    });
    expect(within(cdrEventRow).getByText("Corporate Action")).toBeInTheDocument();
    expect(within(cdrEventRow).getByText("GPW:CDR")).toBeInTheDocument();
    expect(within(cdrEventRow).getByText("Today")).toBeInTheDocument();

    await user.click(cdrEventRow);

    const eventDetails = screen.getByLabelText("Event details");
    expect(eventDetails).toBeInTheDocument();
    expect(within(eventDetails).getByText("Official Calendar")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open source" })).toBeInTheDocument();
  });

  it("filters company events by company and type", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Events" });

    await user.click(screen.getByRole("button", { name: "List" }));
    expect(screen.getByText("Main Market - Corporate actions - Equity - CDR")).toBeInTheDocument();
    expect(
      screen.getByText("Main Market - End of market making activities - Equity - PZU"),
    ).toBeInTheDocument();

    await user.selectOptions(screen.getByLabelText("Event company filter"), "company_gpw_pzu");

    await waitFor(() => {
      expect(screen.queryByText("Main Market - Corporate actions - Equity - CDR")).not.toBeInTheDocument();
    });
    expect(
      screen.getByText("Main Market - End of market making activities - Equity - PZU"),
    ).toBeInTheDocument();
    expect(invoke).toHaveBeenCalledWith("list_company_events", {
      input: expect.objectContaining({
        companyId: "company_gpw_pzu",
        eventType: null,
        mode: "upcoming",
      }),
    });

    await user.selectOptions(screen.getByLabelText("Event type filter"), "market_making");

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("list_company_events", {
        input: expect.objectContaining({
          companyId: "company_gpw_pzu",
          eventType: "market_making",
          mode: "upcoming",
        }),
      });
    });
  });

  it("creates a manual company event from the Events screen", async () => {
    const user = userEvent.setup();

    renderApp({ section: "Events" });

    await user.click(screen.getByRole("button", { name: "Add event" }));

    await user.selectOptions(screen.getByLabelText("Manual event company"), "company_gpw_pzu");
    await user.selectOptions(screen.getByLabelText("Manual event type"), "dividend");
    await user.clear(screen.getByLabelText("Manual event date"));
    await user.type(screen.getByLabelText("Manual event date"), currentWeekTestDate(3));
    await user.type(screen.getByLabelText("Manual event title"), "Dividend decision expected");
    await user.click(screen.getByRole("button", { name: "Save" }));

    await waitFor(() => {
      expect(invoke).toHaveBeenCalledWith("create_company_event", {
        input: expect.objectContaining({
          companyId: "company_gpw_pzu",
          eventType: "dividend",
          title: "Dividend decision expected",
          sourceType: "manual",
        }),
      });
    });
    expect((await screen.findAllByText("Dividend decision expected")).length).toBeGreaterThan(0);
    expect(screen.getAllByText("Manual").length).toBeGreaterThan(0);
  });
});
