import { describe, expect, it, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { InboxScreen } from "./InboxScreen";
import { InboxProvider } from "../../app/state/screenViewModels";
import type { InboxScreenProps } from "./inboxTypes";
import { COMPANY_SPECS, makeCompany, makeFeedItem } from "../../test/scenarios/entities";

// The detail pane renders the company-context block (F1) — resolve its one
// composed call with an empty context so this seam test stays focused on the
// overlay-activation behavior (CompanyFeedSection.test.tsx precedent).
vi.mock("../../api/companyContext", () => ({
  getCompanyContext: vi.fn(() =>
    Promise.resolve({
      companyId: "c1",
      latestPeriodFacts: null,
      upcomingEvents: [],
      notebook: { count: 0, latestAt: null },
      claimsDue: { due: 0, overdue: 0 },
    }),
  ),
}));

const company = makeCompany(COMPANY_SPECS[0]);
const feedItem = makeFeedItem(COMPANY_SPECS[0], 0);

function baseProps(overrides: Partial<InboxScreenProps> = {}): InboxScreenProps {
  return {
    watchlists: [],
    companies: [company],
    feedTypes: [],
    feedSources: [],
    feedSignalCategories: [],
    filteredFeedItems: [feedItem],
    signalsByFeedItemId: {},
    signalsError: null,
    selectedFeedItem: feedItem,
    selectedFeedCompany: company,
    inboxStatusFilter: "all",
    searchQuery: "",
    inboxWatchlistFilter: "all",
    inboxCompanyFilter: "all",
    inboxTypeFilter: "all",
    inboxSignalFilter: "all",
    inboxSourceFilter: "all",
    inboxReviewStats: { visible: 1, unread: 0, saved: 0 },
    inboxEmptyState: null,
    hasActiveInboxFilters: false,
    sourceRefreshState: "idle",
    detailPaneFraction: 0.5,
    detailPaneMinFraction: 0.3,
    detailPaneMaxFraction: 0.7,
    feedError: null,
    sourceRefreshError: null,
    healthError: null,
    databaseError: null,
    setInboxStatusFilter: () => {},
    setSearchQuery: () => {},
    setInboxWatchlistFilter: () => {},
    setInboxCompanyFilter: () => {},
    setInboxTypeFilter: () => {},
    setInboxSignalFilter: () => {},
    setInboxSourceFilter: () => {},
    confirmCompanySignal: () => {},
    rejectCompanySignal: () => {},
    setSelectedFeedItemId: () => {},
    setActiveSection: () => {},
    markVisibleInboxAsRead: () => {},
    clearInboxFilters: () => {},
    refreshSources: () => Promise.resolve(),
    openSourceStatus: () => {},
    toggleFeedItemReadState: () => {},
    selectFeedItemFromKeyboard: () => {},
    updateSelectedFeedItem: () => {},
    openCompanyWorkspaceFromFeedItem: () => {},
    openFeedItemNoteDraft: () => {},
    resizeDetailPaneWithKeyboard: () => {},
    startDetailPaneResize: () => {},
    resizeDetailPane: () => {},
    stopDetailPaneResize: () => {},
    feedItemSummary: () => "",
    formatTimestamp: () => "",
    inboxDetailActivationToken: 0,
    ...overrides,
  };
}

function renderInbox(props: InboxScreenProps) {
  return render(
    <InboxProvider value={props}>
      <InboxScreen />
    </InboxProvider>,
  );
}

describe("InboxScreen — openInboxItem overlay activation (F2 S3 nav seam)", () => {
  it("does not raise the S-overlay on mount when the token starts at 0", () => {
    renderInbox(baseProps());
    expect(screen.getByLabelText("Feed item details")).not.toHaveAttribute("data-detail-open");
  });

  it("raises the S-overlay when Today's openInboxItem bumps the activation token", () => {
    const { rerender } = renderInbox(baseProps({ inboxDetailActivationToken: 0 }));
    expect(screen.getByLabelText("Feed item details")).not.toHaveAttribute("data-detail-open");

    rerender(
      <InboxProvider value={baseProps({ inboxDetailActivationToken: 1 })}>
        <InboxScreen />
      </InboxProvider>,
    );

    expect(screen.getByLabelText("Feed item details")).toHaveAttribute("data-detail-open", "true");
  });

  it("reacts again on a second navigation (token bumps a second time)", () => {
    const { rerender } = renderInbox(baseProps({ inboxDetailActivationToken: 1 }));
    rerender(
      <InboxProvider value={baseProps({ inboxDetailActivationToken: 1 })}>
        <InboxScreen />
      </InboxProvider>,
    );
    expect(screen.getByLabelText("Feed item details")).toHaveAttribute("data-detail-open", "true");

    // Same instance re-render at token 2 (a second `openInboxItem` call).
    rerender(
      <InboxProvider value={baseProps({ inboxDetailActivationToken: 2 })}>
        <InboxScreen />
      </InboxProvider>,
    );
    expect(screen.getByLabelText("Feed item details")).toHaveAttribute("data-detail-open", "true");
  });
});
