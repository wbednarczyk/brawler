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
import { Button } from "../../shared/components/Button";
import { EmptyState } from "../../shared/components/EmptyState";
import { useLocale } from "../../shared/locale";
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
  inboxStatusFilter,
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
        <div className="panel-header">
          <div>
            <h1 id="inbox-title">{t("inbox.title")}</h1>
            <p>{t("inbox.description")}</p>
          </div>
          <div className="segmented-control" aria-label={text("Feed status filter")}>
            <button
              type="button"
              className={inboxStatusFilter === "all" ? "segment-active" : undefined}
              onClick={() => setInboxStatusFilter("all")}
            >
              <Inbox size={14} />
              {text("All")}
            </button>
            <button
              type="button"
              className={inboxStatusFilter === "unread" ? "segment-active" : undefined}
              onClick={() => setInboxStatusFilter("unread")}
            >
              <Mail size={14} />
              {text("Unread")}
            </button>
            <button
              type="button"
              className={inboxStatusFilter === "saved" ? "segment-active" : undefined}
              onClick={() => setInboxStatusFilter("saved")}
            >
              <Save size={14} />
              {text("Saved")}
            </button>
          </div>
        </div>

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
            disabled={deleteUnsavedFeedState === "refreshing"}
            onClick={deleteUnsavedFeedItems}
          >
            {deleteUnsavedFeedState === "done" ? <CheckCircle2 size={15} /> : <Trash2 size={15} />}
            {deleteUnsavedFeedState === "refreshing" ? text("Deleting") : text("Delete unsaved")}
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

        <div className="filter-toolbar" aria-label={text("Inbox filters")}>
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
        </div>

        <div className="feed-list" aria-label={text("Feed items")}>
          {filteredFeedItems.map((item) => (
            <article
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
              tabIndex={0}
              title={text("Select feed item")}
            >
              <div className="feed-row-main">
                <div className="feed-meta">
                  <span>{item.company}</span>
                  <span>{item.type}</span>
                  <span>{item.source}</span>
                  <span>{formatTimestamp(item.time, text("Unknown"))}</span>
                </div>
                <h2>{item.title}</h2>
                <p>{feedItemSummary(item)}</p>
              </div>
              {item.saved ? <span className="saved-pill">{text("Saved")}</span> : null}
              {item.unread ? <span className="unread-dot" title={text("Unread")} /> : null}
            </article>
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
        healthError={healthError}
        databaseError={databaseError}
        updateSelectedFeedItem={updateSelectedFeedItem}
        openCompanyWorkspaceFromFeedItem={openCompanyWorkspaceFromFeedItem}
        openFeedItemNoteDraft={openFeedItemNoteDraft}
        feedItemSummary={feedItemSummary}
        formatTimestamp={formatTimestamp}
      />
    </>
  );
}
