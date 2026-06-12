import {
  Activity,
  CheckCircle2,
  Inbox,
  Mail,
  MailOpen,
  Plus,
  RefreshCw,
  Save,
  Trash2,
  X,
} from "lucide-react";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import {
  Button,
  DenseRow,
  EmptyState,
  FilterToolbar,
  PanelHeader,
  SearchField,
  SegmentedControl,
  SegmentedControlOption,
  StatusChip,
} from "../../ui";
import { InboxDetailPane } from "./InboxDetailPane";
import type { InboxScreenProps } from "./inboxTypes";

export function InboxScreen({
  watchlists,
  companies,
  feedTypes,
  feedSources,
  filteredFeedItems,
  selectedFeedItem,
  selectedFeedCompany,
  aiAnalysisJobsByFeedItemId,
  aiAnalysisErrorByFeedItemId,
  aiAnalysisRequestInFlightByFeedItemId,
  aiAnalysisProviderConfigured,
  inboxStatusFilter,
  searchQuery,
  inboxWatchlistFilter,
  inboxCompanyFilter,
  inboxTypeFilter,
  inboxSourceFilter,
  inboxReviewStats,
  inboxEmptyState,
  hasActiveInboxFilters,
  deleteUnsavedFeedState,
  sourceRefreshState,
  detailPaneWidth,
  detailPaneMinWidth,
  detailPaneMaxWidth,
  feedError,
  deleteUnsavedFeedError,
  sourceRefreshError,
  healthError,
  databaseError,
  setInboxStatusFilter,
  setSearchQuery,
  setInboxWatchlistFilter,
  setInboxCompanyFilter,
  setInboxTypeFilter,
  setInboxSourceFilter,
  setSelectedFeedItemId,
  setActiveSection,
  markVisibleInboxAsRead,
  deleteUnsavedFeedItems,
  clearInboxFilters,
  refreshSources,
  openSourceStatus,
  toggleFeedItemReadState,
  selectFeedItemFromKeyboard,
  updateSelectedFeedItem,
  openCompanyWorkspaceFromFeedItem,
  openFeedItemNoteDraft,
  startFeedItemAiAnalysis,
  retryFeedItemAiAnalysis,
  resizeDetailPaneWithKeyboard,
  startDetailPaneResize,
  resizeDetailPane,
  stopDetailPaneResize,
  feedItemSummary,
  formatTimestamp,
}: InboxScreenProps) {
  const { t, text } = useLocale();

  return (
    <>
      <section className="feed-panel" aria-labelledby="inbox-title">
        <PanelHeader
          title={t("inbox.title")}
          description={t("inbox.description")}
          titleId="inbox-title"
          actions={
            <SegmentedControl ariaLabel={text("Feed status filter")}>
              <SegmentedControlOption active={inboxStatusFilter === "all"} onClick={() => setInboxStatusFilter("all")}>
                <Inbox size={14} />
                {text("All")}
              </SegmentedControlOption>
              <SegmentedControlOption active={inboxStatusFilter === "unread"} onClick={() => setInboxStatusFilter("unread")}>
                <Mail size={14} />
                {text("Unread")}
              </SegmentedControlOption>
              <SegmentedControlOption active={inboxStatusFilter === "saved"} onClick={() => setInboxStatusFilter("saved")}>
                <Save size={14} />
                {text("Saved")}
              </SegmentedControlOption>
            </SegmentedControl>
          }
        />

        <div className="filter-reset-row" aria-label={text("Inbox filter reset")}>
          <div className="inbox-review-summary" aria-label={text("Inbox review summary")}>
            <span>
              <strong>{inboxReviewStats.visible}</strong> {text("visible")}
            </span>
            <span>
              <strong>{inboxReviewStats.unread}</strong> {text("unread")}
            </span>
            <span>
              <strong>{inboxReviewStats.saved}</strong> {text("saved")}
            </span>
          </div>
          <Button
            className="compact-button"
            disabled={inboxReviewStats.unread === 0}
            onClick={markVisibleInboxAsRead}
          >
            <MailOpen size={15} />
            {text("Mark all read")}
          </Button>
          <Button
            className="compact-button"
            disabled={!hasActiveInboxFilters}
            onClick={clearInboxFilters}
          >
            <X size={15} />
            {text("Clear filters")}
          </Button>
        </div>

        <FilterToolbar ariaLabel={text("Inbox filters")}>
          <label className="inbox-search-field">
            {text("Search")}
            <SearchField
              ariaLabel={t("app.search.ariaLabel")}
              as="span"
              className="search-box"
              clearLabel={text("Clear inbox search")}
              iconSize={16}
              inputProps={{ "data-inbox-search-input": "true" }}
              onChange={setSearchQuery}
              onClear={() => setSearchQuery("")}
              placeholder={t("app.search.placeholder")}
              type="text"
              value={searchQuery}
            />
          </label>
          <label>
            {text("Watchlist")}
            <select
              aria-label={text("Inbox watchlist")}
              value={inboxWatchlistFilter}
              onChange={(event) => setInboxWatchlistFilter(event.target.value)}
            >
              <option value="all">{text("All watchlists")}</option>
              {watchlists.map((watchlist) => (
                <option key={watchlist.id} value={watchlist.id}>
                  {watchlist.name}
                </option>
              ))}
            </select>
          </label>
          <label>
            {text("Company")}
            <select
              aria-label={text("Inbox company")}
              value={inboxCompanyFilter}
              onChange={(event) => setInboxCompanyFilter(event.target.value)}
            >
              <option value="all">{text("All companies")}</option>
              {companies.map((company) => (
                <option key={company.id} value={company.qualifiedTicker}>
                  {company.qualifiedTicker}
                </option>
              ))}
            </select>
          </label>
          <label>
            {text("Type")}
            <select
              aria-label={text("Inbox type")}
              value={inboxTypeFilter}
              onChange={(event) => setInboxTypeFilter(event.target.value)}
            >
              <option value="all">{text("All types")}</option>
              {feedTypes.map((type) => (
                <option key={type} value={type}>
                  {type}
                </option>
              ))}
            </select>
          </label>
          <label>
            {text("Source")}
            <select
              aria-label={text("Inbox source")}
              value={inboxSourceFilter}
              onChange={(event) => setInboxSourceFilter(event.target.value)}
            >
              <option value="all">{text("All sources")}</option>
              {feedSources.map((source) => (
                <option key={source} value={source}>
                  {source}
                </option>
              ))}
            </select>
          </label>
        </FilterToolbar>

        <div className="feed-list" aria-label={text("Feed items")}>
          {filteredFeedItems.map((item) => (
            <DenseRow
              aria-label={`${text("Select feed item")}: ${item.title}`}
              aria-current={selectedFeedItem?.id === item.id ? "true" : undefined}
              className={[
                "feed-row",
                item.unread ? "unread" : "",
                selectedFeedItem?.id === item.id ? "feed-row-selected" : "",
              ]
                .filter(Boolean)
                .join(" ")}
              key={item.id}
              data-feed-item-id={item.id}
              data-feed-row="true"
              onClick={() => setSelectedFeedItemId(item.id)}
              onDoubleClick={() => toggleFeedItemReadState(item)}
              onKeyDown={(event) => selectFeedItemFromKeyboard(event, item)}
              role="button"
              selected={selectedFeedItem?.id === item.id}
              tabIndex={0}
              title={text("Select feed item")}
              unread={item.unread}
            >
              <div className="feed-row-main">
                <div className="feed-row-topline">
                  <strong><TickerLabel value={item.company} /></strong>
                  <span>{formatTimestamp(item.time, text("Unknown"))}</span>
                </div>
                <div className="feed-row-source-line">
                  <span>{item.type}</span>
                  <span>{item.source}</span>
                </div>
                <h2>{item.title}</h2>
                <p>{feedItemSummary(item)}</p>
              </div>
              {item.saved ? <StatusChip tone="accent">{text("Saved")}</StatusChip> : null}
              {item.unread ? <span className="unread-dot" title={text("Unread")} /> : null}
            </DenseRow>
          ))}
          {inboxEmptyState ? (
            <EmptyState wrapText={false}>
              {inboxEmptyState === "no-companies" ? (
                <>
                  <span>{text("No companies tracked yet.")}</span>
                  <Button
                    className="compact-button"
                    onClick={() => setActiveSection("Companies")}
                  >
                    <Plus size={15} />
                    {text("Add company")}
                  </Button>
                </>
              ) : null}

              {inboxEmptyState === "no-feed" ? (
                <>
                  <span>{text("No stored feed items yet.")}</span>
                  <div className="empty-state-actions">
                    <Button
                      className="compact-button"
                      disabled={sourceRefreshState === "refreshing"}
                      onClick={() => {
                        void refreshSources("manual");
                      }}
                      title={text("Fetch GPW ESPI/EBI public listings")}
                    >
                      {sourceRefreshState === "done" ? <CheckCircle2 size={15} /> : <RefreshCw size={15} />}
                      {sourceRefreshState === "refreshing" ? text("Refreshing") : text("Refresh sources")}
                    </Button>
                    <Button
                      className="compact-button"
                      onClick={openSourceStatus}
                    >
                      <Activity size={15} />
                      {text("Open Sources")}
                    </Button>
                  </div>
                </>
              ) : null}

              {inboxEmptyState === "no-matches" ? (
                <>
                  <span>{text("No feed items for selected filters.")}</span>
                  {hasActiveInboxFilters ? (
                    <Button
                      className="compact-button"
                      onClick={clearInboxFilters}
                    >
                      <X size={15} />
                      {text("Clear filters")}
                    </Button>
                  ) : null}
                </>
              ) : null}
            </EmptyState>
          ) : null}
          {feedError ? <p className="error-text">{text("Feed command failed")}: {feedError}</p> : null}
          {deleteUnsavedFeedError ? (
            <p className="error-text">{text("Delete unsaved failed")}: {deleteUnsavedFeedError}</p>
          ) : null}
          {sourceRefreshError ? (
            <p className="error-text">{text("Source refresh failed")}: {sourceRefreshError}</p>
          ) : null}
        </div>

        <div className="inbox-maintenance-row" aria-label={text("Inbox maintenance")}>
          <span>{text("Feed cleanup")}</span>
          <Button
            className="compact-button danger-subtle-button"
            disabled={deleteUnsavedFeedState === "refreshing"}
            onClick={deleteUnsavedFeedItems}
          >
            {deleteUnsavedFeedState === "done" ? <CheckCircle2 size={15} /> : <Trash2 size={15} />}
            {deleteUnsavedFeedState === "refreshing" ? text("Deleting") : text("Delete unsaved")}
          </Button>
        </div>
      </section>

      <div
        aria-label={text("Resize feed details")}
        aria-orientation="vertical"
        aria-valuemax={detailPaneMaxWidth}
        aria-valuemin={detailPaneMinWidth}
        aria-valuenow={detailPaneWidth}
        className="pane-resizer"
        onKeyDown={resizeDetailPaneWithKeyboard}
        onPointerDown={startDetailPaneResize}
        onPointerMove={resizeDetailPane}
        onPointerUp={stopDetailPaneResize}
        role="separator"
        tabIndex={0}
        title={text("Drag to resize feed details")}
      />

        <InboxDetailPane
          selectedFeedItem={selectedFeedItem}
          selectedFeedCompany={selectedFeedCompany}
          aiAnalysisJobsByFeedItemId={aiAnalysisJobsByFeedItemId}
          aiAnalysisErrorByFeedItemId={aiAnalysisErrorByFeedItemId}
          aiAnalysisRequestInFlightByFeedItemId={aiAnalysisRequestInFlightByFeedItemId}
          aiAnalysisProviderConfigured={aiAnalysisProviderConfigured}
          healthError={healthError}
          databaseError={databaseError}
          updateSelectedFeedItem={updateSelectedFeedItem}
          openCompanyWorkspaceFromFeedItem={openCompanyWorkspaceFromFeedItem}
          openFeedItemNoteDraft={openFeedItemNoteDraft}
          startFeedItemAiAnalysis={startFeedItemAiAnalysis}
          retryFeedItemAiAnalysis={retryFeedItemAiAnalysis}
          feedItemSummary={feedItemSummary}
          formatTimestamp={formatTimestamp}
        />
    </>
  );
}
