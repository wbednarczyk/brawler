import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";

import { useFeedController } from "./useFeedController";
import type { Section } from "./navigation";
import type { InboxStatusFilter } from "../screens/Inbox/inboxTypes";
import { COMPANY_SPECS, makeCompany } from "../test/scenarios/entities";

const company = makeCompany(COMPANY_SPECS[0]);
const navigateMock = vi.fn();

function useHarness() {
  const [activeSection, setActiveSection] = useState<Section>("Today");
  const [inboxCompanyFilter, setInboxCompanyFilter] = useState("all");
  const [, setInboxSignalFilter] = useState("all");
  const [, setInboxSourceFilter] = useState("all");
  const [, setInboxStatusFilter] = useState<InboxStatusFilter>("all");
  const [, setInboxTypeFilter] = useState("all");
  const [, setInboxWatchlistFilter] = useState("all");
  const [, setSearchQuery] = useState("");
  const [selectedFeedItemId, setSelectedFeedItemId] = useState<string | null>(null);

  const controller = useFeedController({
    companies: [company],
    filteredFeedItems: [],
    selectedFeedItem: null,
    setActiveSection,
    navigate: navigateMock,
    setFeedError: () => {},
    setFeedState: () => {},
    setInboxCompanyFilter,
    setInboxSignalFilter,
    setInboxSourceFilter,
    setInboxStatusFilter,
    setInboxTypeFilter,
    setInboxWatchlistFilter,
    setSearchQuery,
    setSelectedCompanyFeedItemId: () => {},
    setSelectedFeedItemId,
  });

  return {
    controller,
    activeSection,
    inboxCompanyFilter,
    selectedFeedItemId,
  };
}

describe("useFeedController — openInboxItem (F2 S3 nav seam)", () => {
  it("scopes the Inbox to the company, selects EXACTLY the given feed item, and activates the overlay", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openInboxItem("feed_target", company.id);
    });

    expect(result.current.selectedFeedItemId).toBe("feed_target");
    expect(result.current.inboxCompanyFilter).toBe(company.qualifiedTicker);
    expect(result.current.activeSection).toBe("Inbox");
    expect(result.current.controller.inboxDetailActivationToken).toBe(1);
  });

  it("bumps the activation token on every call, so InboxScreen's effect can re-fire for a second item", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openInboxItem("feed_a", company.id);
    });
    act(() => {
      result.current.controller.openInboxItem("feed_b", company.id);
    });

    expect(result.current.selectedFeedItemId).toBe("feed_b");
    expect(result.current.controller.inboxDetailActivationToken).toBe(2);
  });

  it("never falls back to an unscoped filter for an unknown company id", () => {
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openInboxItem("feed_target", "not_a_real_company");
    });

    // No matching company resolves to "all" rather than a stale/partial
    // filter — the fallback the plan calls out (c80dabe class), never a
    // silently wrong company scope.
    expect(result.current.inboxCompanyFilter).toBe("all");
    expect(result.current.selectedFeedItemId).toBe("feed_target");
  });
});

// F3a (ADR 0107, sol R2 finding 3): opening a company from a feed item is ONE
// guarded transition — the Spółka screen with that item raised in the
// workshop, never company state set ahead of the dirty guard.
describe("useFeedController — openCompanyWorkspaceFromFeedItem", () => {
  it("commits one Spółka transition carrying the feedItem tool", () => {
    navigateMock.mockClear();
    const { result } = renderHook(() => useHarness());

    act(() => {
      result.current.controller.openCompanyWorkspaceFromFeedItem({
        id: "feed_target",
        company: company.qualifiedTicker,
      } as Parameters<typeof result.current.controller.openCompanyWorkspaceFromFeedItem>[0]);
    });

    expect(navigateMock).toHaveBeenCalledTimes(1);
    expect(navigateMock).toHaveBeenCalledWith({
      companyId: company.id,
      section: "Spolka",
      tool: { t: "feedItem", feedItemId: "feed_target" },
    });
    expect(result.current.activeSection).toBe("Today");
  });
});
