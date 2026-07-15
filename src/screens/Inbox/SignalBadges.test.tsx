import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";

import { SignalBadges } from "./SignalBadges";
import { LocaleContext, makeTextTranslator, makeTranslator } from "../../shared/locale";
import type { CompanySignal, CompanySignalCategory } from "../../api/types";

function signal(category: CompanySignalCategory, displayName: string): CompanySignal {
  return {
    id: `signal_${category}`,
    companyId: "company_gpw_cdr",
    company: "GPW:CDR",
    companyName: "CD PROJEKT S.A.",
    feedItemId: "feed_1",
    category,
    categoryDisplayName: displayName,
    confidence: 0.93,
    classifiedBy: "rule",
    status: "confirmed",
    signalDate: "2026-05-28",
    providerId: null,
    modelId: null,
    derivedEventId: null,
    title: "Raport niezależnego biegłego rewidenta z zastrzeżeniem",
    sourceUrl: "https://example.test/source",
    createdAt: "2026-05-28T12:00:00Z",
    updatedAt: "2026-05-28T12:00:00Z",
  };
}

describe("SignalBadges", () => {
  it("renders the auditor_opinion badge with the danger red-flag tone", () => {
    render(<SignalBadges signals={[signal("auditor_opinion", "Auditor opinion")]} />);
    const badge = screen.getByText("Auditor opinion");
    expect(badge.closest(".ui-status-chip")).toHaveClass("ui-status-chip-danger");
  });

  it("renders the translated Polish label for auditor_opinion", () => {
    render(
      <LocaleContext.Provider
        value={{ locale: "pl", t: makeTranslator("pl"), text: makeTextTranslator("pl") }}
      >
        <SignalBadges signals={[signal("auditor_opinion", "Auditor opinion")]} />
      </LocaleContext.Provider>,
    );
    expect(screen.getByText("Opinia audytora")).toBeInTheDocument();
  });
});
