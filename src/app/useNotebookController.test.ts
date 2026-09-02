import { describe, it, expect } from "vitest";
import type { FeedItem } from "../api/types";
import { buildFeedItemNoteDraft, feedItemSummary } from "./useNotebookController";

function makeItem(overrides: Partial<FeedItem>): FeedItem {
  return {
    id: "f1",
    company: "GPW:CDR",
    type: "Official report",
    source: "GPW ESPI/EBI",
    time: "2026-06-01T00:00:00Z",
    title: "Raport bieżący 7/2026",
    unread: true,
    saved: false,
    sourceUrl: "https://example.test/report",
    language: "pl",
    publishedAt: "2026-06-01T00:00:00Z",
    fetchedAt: "2026-06-01T00:00:00Z",
    attribution: "GPW",
    summary: "Komunikat ESPI/EBI",
    bodyText: "",
    attachments: [],
    presentationKind: "filing",
    ...overrides,
  };
}

describe("feedItemSummary (shared render-site root, ADR F1 S1)", () => {
  it("suppresses the dead ESPI/EBI summary literal for a filing-kind item", () => {
    const item = makeItem({});

    expect(feedItemSummary(item)).toBe("");
  });

  it("still returns the trimmed summary for a non-filing item", () => {
    const item = makeItem({
      presentationKind: "report",
      summary: "Q1 2026 results beat expectations",
    });

    expect(feedItemSummary(item)).toBe("Q1 2026 results beat expectations");
  });
});

// sol fix1 item 3: extracted so both the cross-company `openFeedItemNoteDraft`
// entry point and a same-company caller (the Spółka feed panel's Note
// action) build the identical draft — one origin-attribution fork, not two.
describe("buildFeedItemNoteDraft (sol fix1 item 3)", () => {
  it("attributes the draft to the feed item with a feed_item origin", () => {
    const item = makeItem({
      presentationKind: "report",
      title: "Report 0",
      summary: "Sample feed item summary.",
      bodyText: "",
      type: "Official report",
      source: "GPW ESPI/EBI",
      id: "feed_0",
      sourceUrl: "https://example.test/feed/0",
    });

    const draft = buildFeedItemNoteDraft(item);

    expect(draft.form.title).toBe("Report 0");
    expect(draft.form.body).toBe("Sample feed item summary.");
    expect(draft.origins).toEqual([
      {
        sourceType: "feed_item",
        sourceId: "feed_0",
        sourceUrl: "https://example.test/feed/0",
        label: "GPW ESPI/EBI: Report 0",
      },
    ]);
  });
});
