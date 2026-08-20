import { describe, it, expect, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { Company, FeedItem } from "../../api/types";
import { CompanyFeedSection } from "./CompanyFeedSection";

const company = { id: "c1", qualifiedTicker: "GPW:CDR", displayName: "CD PROJEKT S.A." } as Company;

function makeItem(overrides: Partial<FeedItem>): FeedItem {
  return {
    id: "f1",
    company: "GPW:CDR",
    type: "report",
    presentationKind: "report",
    source: "GPW ESPI",
    time: "2026-06-01T00:00:00Z",
    title: "Quarterly report",
    unread: true,
    saved: false,
    sourceUrl: "https://example.test/report",
    language: "pl",
    publishedAt: "2026-06-01T00:00:00Z",
    fetchedAt: "2026-06-01T00:00:00Z",
    attribution: "GPW",
    summary: "Summary of the report",
    bodyText: "",
    attachments: [],
    ...overrides,
  };
}

const noop = () => {};
const baseProps = {
  company,
  aiAnalysisJobsByFeedItemId: {},
  aiAnalysisErrorByFeedItemId: {},
  selectFeedItemFromKeyboard: noop,
  formatTimestamp: (value: string | null | undefined) => value ?? "—",
  feedItemSummary: (item: FeedItem) => item.summary || item.title,
};

describe("CompanyFeedSection (ADR 0057 dashboard panel)", () => {
  it("selects an item and persists a read toggle through updateFeedItemState", async () => {
    const user = userEvent.setup();
    const toggleFeedItem = vi.fn();
    const updateFeedItemState = vi.fn();
    const item = makeItem({});

    render(
      <CompanyFeedSection
        {...baseProps}
        feedItems={[item]}
        selectedFeedItem={item}
        toggleFeedItem={toggleFeedItem}
        updateFeedItemState={updateFeedItemState}
      />,
    );

    const detail = screen.getByLabelText("Company feed item details");
    await user.click(within(detail).getByRole("button", { name: "Mark read" }));

    expect(updateFeedItemState).toHaveBeenCalledTimes(1);
    const [passedItem, transform] = updateFeedItemState.mock.calls[0];
    expect(transform(passedItem).unread).toBe(false);
  });

  it("omits the cross-screen Inbox/Note actions when their handlers are not provided", () => {
    const item = makeItem({});
    render(
      <CompanyFeedSection
        {...baseProps}
        feedItems={[item]}
        selectedFeedItem={item}
        toggleFeedItem={noop}
        updateFeedItemState={noop}
      />,
    );

    const detail = screen.getByLabelText("Company feed item details");
    expect(within(detail).queryByRole("button", { name: "Open in Inbox" })).not.toBeInTheDocument();
    expect(within(detail).queryByRole("button", { name: "Note" })).not.toBeInTheDocument();
    // The external source link is always available (self-contained).
    expect(within(detail).getByRole("link", { name: "Open source" })).toBeInTheDocument();
  });

  it("shows the tracked-but-empty state when the company has no feed items", () => {
    render(
      <CompanyFeedSection
        {...baseProps}
        feedItems={[]}
        selectedFeedItem={null}
        toggleFeedItem={noop}
        updateFeedItemState={noop}
      />,
    );

    expect(screen.getByText(/No stored feed items for/)).toBeInTheDocument();
  });
});
