import { describe, it, expect } from "vitest";
import type { FeedItem } from "../api/types";
import { feedItemSummary } from "./useNotebookController";

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
