import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { LocaleContext, makeTextTranslator, makeTranslator } from "../../locale";
import type { FeedItem } from "../../../api/types";
import { FeedDetailContent } from "./FeedDetailContent";

const localeValue = {
  locale: "en" as const,
  t: makeTranslator("en"),
  text: makeTextTranslator("en"),
};

function renderWithLocale(ui: React.ReactElement) {
  return render(<LocaleContext.Provider value={localeValue}>{ui}</LocaleContext.Provider>);
}

function makeItem(overrides: Partial<FeedItem>): FeedItem {
  return {
    id: "f1",
    company: "GPW:PZU",
    type: "Official report",
    presentationKind: "filing",
    source: "Bankier Komunikaty",
    time: "2026-08-19T16:48:00Z",
    title: "Powołanie Członka Zarządu PZU SA",
    unread: true,
    saved: false,
    sourceUrl: "https://example.test/espi/22-2026",
    language: "pl",
    publishedAt: "2026-08-19T16:48:00Z",
    fetchedAt: "2026-08-19T16:50:00Z",
    attribution: "Bankier",
    summary: "Komunikat ESPI/EBI",
    bodyText: "",
    attachments: [],
    ...overrides,
  };
}

const baseProps = {
  signals: [],
  actions: <button type="button">Otwórz komunikat</button>,
  feedItemSummary: (item: FeedItem) => item.summary,
  formatTimestamp: (value: string | null | undefined) => value ?? "—",
  onConfirmSignal: vi.fn(),
  onRejectSignal: vi.fn(),
};

describe("FeedDetailContent dispatcher", () => {
  it("renders the typed filing chip and never the dead 'Komunikat ESPI/EBI' summary literal", () => {
    renderWithLocale(<FeedDetailContent item={makeItem({})} {...baseProps} />);

    expect(screen.getByText("ESPI notice")).toBeInTheDocument();
    expect(screen.queryByText("Komunikat ESPI/EBI")).not.toBeInTheDocument();
  });

  it("dispatches media kind to an excerpt, not the generic fallback markup", () => {
    renderWithLocale(
      <FeedDetailContent
        item={makeItem({
          presentationKind: "media",
          summary: "PZU bije prognozy w drugim kwartale.",
        })}
        {...baseProps}
      />,
    );

    expect(screen.getByText("PZU bije prognozy w drugim kwartale.")).toBeInTheDocument();
    expect(screen.queryByLabelText("Official report body")).not.toBeInTheDocument();
  });

  it("dispatches report kind to a document list with a pluralized count", () => {
    renderWithLocale(
      <FeedDetailContent
        item={makeItem({
          presentationKind: "report",
          attachments: [
            { id: "a1", label: "raport_polroczny_2026.pdf", url: "https://example.test/a1.pdf" },
            { id: "a2", label: "wybrane_dane_finansowe.pdf", url: "https://example.test/a2.pdf" },
          ],
        })}
        {...baseProps}
      />,
    );

    expect(screen.getByText("raport polroczny 2026")).toBeInTheDocument();
    expect(screen.getByText(/2 documents/)).toBeInTheDocument();
  });

  it("falls back to the unchanged generic rendering for redFlag", () => {
    renderWithLocale(
      <FeedDetailContent item={makeItem({ presentationKind: "redFlag" })} {...baseProps} />,
    );

    expect(screen.getByLabelText("Official report body")).toBeInTheDocument();
    expect(screen.getByLabelText("Feed summary")).toBeInTheDocument();
  });
});
