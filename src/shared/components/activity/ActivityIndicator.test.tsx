import { describe, expect, expectTypeOf, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { ActivityIndicator } from "./ActivityIndicator";
import { LocaleContext, makeTextTranslator, makeTranslator } from "../../locale";
import type { ActivitySummary } from "../../../api/generated/ActivitySummary";

// ADR 0109 dec. 5: the topbar indicator is a status ledger, never a failure
// counter. Two pins: the summary DTO carries exactly the three fields the
// indicator may render (adding a failure count reddens this at compile time),
// and the rendered control never uses the danger tone.
describe("ActivityIndicator", () => {
  it("the summary DTO has exactly active, queued and lastFinishedAt", () => {
    expectTypeOf<ActivitySummary>().toEqualTypeOf<{ active: number; queued: number; lastFinishedAt: string | null }>();
  });

  it("renders the queued count without a danger tone and names the last finished time when idle", () => {
    const value = { locale: "pl" as const, t: makeTranslator("pl"), text: makeTextTranslator("pl") };
    const { container, rerender } = render(
      <LocaleContext.Provider value={value}>
        <ActivityIndicator summary={{ active: 0, queued: 3, lastFinishedAt: null }} onOpen={() => {}} />
      </LocaleContext.Provider>,
    );
    const button = screen.getByRole("button", { name: "Otwórz aktywność" });
    expect(button).toHaveTextContent("3");
    expect(container.querySelector('[data-tone="danger"], .is-danger, .ui-figure-danger')).toBeNull();

    rerender(
      <LocaleContext.Provider value={value}>
        <ActivityIndicator summary={{ active: 0, queued: 0, lastFinishedAt: "2026-09-08T08:00:00Z" }} onOpen={() => {}} />
      </LocaleContext.Provider>,
    );
    expect(screen.getByRole("button", { name: "Otwórz aktywność" })).not.toHaveTextContent(/\d/);
    expect(screen.getByRole("button", { name: "Otwórz aktywność" }).getAttribute("title")).toMatch(/Ostatnio|Last/);
  });
});
