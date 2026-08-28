import {
  BookOpenText,
  ExternalLink,
  Inbox,
  Mail,
  MailOpen,
  Save,
} from "lucide-react";
import type { Company, FeedItem } from "../../api/types";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { FeedDetailContent } from "../../shared/components/feedDetail/FeedDetailContent";
import { CompanyContextSection } from "../../shared/components/feedDetail/CompanyContextSection";
import { useLocale } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";
import { ActionRow, Button, DenseRow, EmptyState, StatusChip } from "../../ui";

// No confirm/reject workflow here (typed ESPI signals stay Inbox-only for now)
// — FeedDetailContent's signals slot always gets an empty list + no-ops so
// FeedSignalsSection quietly renders nothing (F1 S5).
const NO_SIGNALS: never[] = [];
const noopSignalHandler = () => {};

// The company-scoped feed list + selected-item detail (read/save, inspect,
// note draft, AI analysis). Extracted from the tabbed CompanyWorkspace so both
// the workspace and the Spółka `feed` workshop tool (ADR 0057, ADR 0107)
// render the same surface from explicit props — no AppStateRoot coupling
// beyond the handlers passed in.
export type CompanyFeedSectionProps = {
  company: Company;
  feedItems: FeedItem[];
  selectedFeedItem: FeedItem | null;
  toggleFeedItem: (item: FeedItem) => void;
  selectFeedItemFromKeyboard: (event: React.KeyboardEvent<HTMLElement>, item: FeedItem) => void;
  updateFeedItemState: (item: FeedItem, update: (item: FeedItem) => FeedItem) => void;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  feedItemSummary: (item: FeedItem) => string;
  // Cross-screen actions are optional: the tabbed workspace wires them to the
  // AppStateRoot navigation, but the self-contained Spółka `feed` tool (ADR
  // 0057) omits them — the same actions stay reachable from the Inbox.
  inspectFeedItem?: (item: FeedItem) => void;
  openFeedItemNoteDraft?: (item: FeedItem) => void;
  openInboxFilter?: (company: Company) => void;
  // Provenance-thread landing for the context block (ADR 0104 dec. 7). Optional:
  // a host with no documents navigation leaves the ticket non-interactive.
  openCompanyReportDocuments?: () => void;
  /** Renders the selected item's detail FIRST, above the list, instead of
   * inline under its row (Spółka `feedItem` workshop tool, owner dogfooding
   * v0.74 item 7 — opened from the Inbox, the item could sit anywhere in a
   * long feed). Default false preserves the existing placement (Companies
   * tab, Spółka `feed` workshop tool). */
  leadWithDetail?: boolean;
};

export function CompanyFeedSection({
  company,
  feedItems,
  selectedFeedItem,
  toggleFeedItem,
  selectFeedItemFromKeyboard,
  updateFeedItemState,
  inspectFeedItem,
  openFeedItemNoteDraft,
  openInboxFilter,
  openCompanyReportDocuments,
  formatTimestamp,
  feedItemSummary,
  leadWithDetail = false,
}: CompanyFeedSectionProps) {
  const { text, locale } = useLocale();

  function renderDetail(item: FeedItem) {
    return (
      <div role="group" className="company-feed-detail" aria-label={text("Company feed item details")}>
        <FeedDetailContent
          item={item}
          signals={NO_SIGNALS}
          feedItemSummary={feedItemSummary}
          formatTimestamp={formatTimestamp}
          onConfirmSignal={noopSignalHandler}
          onRejectSignal={noopSignalHandler}
          actions={
            <ActionRow className="detail-actions" ariaLabel={text("Company feed item actions")}>
              <Button
                className="compact-button"
                onClick={() =>
                  updateFeedItemState(item, (feedItem) => ({
                    ...feedItem,
                    unread: !feedItem.unread,
                  }))
                }
              >
                {item.unread ? <MailOpen size={15} /> : <Mail size={15} />}
                {item.unread ? text("Mark read") : text("Mark unread")}
              </Button>
              <Button
                className="compact-button"
                onClick={() =>
                  updateFeedItemState(item, (feedItem) => ({
                    ...feedItem,
                    saved: !feedItem.saved,
                  }))
                }
              >
                <Save size={15} />
                {item.saved ? text("Unsave") : text("Save")}
              </Button>
              {inspectFeedItem ? (
                <Button className="compact-button" onClick={() => inspectFeedItem(item)}>
                  <Inbox size={15} />
                  {text("Open in Inbox")}
                </Button>
              ) : null}
              {openFeedItemNoteDraft ? (
                <Button className="compact-button" onClick={() => openFeedItemNoteDraft(item)}>
                  <BookOpenText size={15} />
                  {text("Note")}
                </Button>
              ) : null}
              <a
                // The one contracted primary action of this host's detail
                // (experience contract §6); opening the source also marks
                // the item read (§7 exit path).
                className="primary-button compact-button"
                data-ux-primary-action="true"
                href={item.sourceUrl}
                rel="noreferrer"
                target="_blank"
                onClick={() =>
                  updateFeedItemState(item, (feedItem) => ({
                    ...feedItem,
                    unread: false,
                  }))
                }
              >
                <ExternalLink size={15} />
                {text("Open source")}
              </a>
            </ActionRow>
          }
        />
        <div className="detail-context-divider" />
        <CompanyContextSection companyId={company.id} onOpenReportDocuments={openCompanyReportDocuments} />
      </div>
    );
  }

  return (
    <div
      className="company-tab-panel company-feed-panel"
      aria-label={text("Company feed")}
      data-company-feed-list="true"
    >
      {leadWithDetail && selectedFeedItem ? renderDetail(selectedFeedItem) : null}
      {feedItems.map((item) => (
        <div className="company-feed-row-block" key={item.id}>
          <DenseRow
            as="button"
            aria-label={`${text("Open company feed item")}: ${item.title}`}
            className={[
              "company-feed-row",
              item.unread ? "unread" : "",
              selectedFeedItem?.id === item.id ? "company-feed-row-selected" : "",
            ]
              .filter(Boolean)
              .join(" ")}
            data-company-feed-item-id={item.id}
            data-company-feed-row="true"
            onClick={() => toggleFeedItem(item)}
            onKeyDown={(event) => selectFeedItemFromKeyboard(event, item)}
            selected={selectedFeedItem?.id === item.id}
            title={text("Open company feed item details")}
            unread={item.unread}
          >
            <div className="feed-row-main">
              {/* U7-A density row: badge (type) + date stay at every tier; the
                  source folds at S (container query in companies.css). */}
              <div className="feed-meta">
                <span className="feed-meta-type">{item.type}</span>
                <span className="feed-meta-source">{item.source}</span>
                <span className="num-tabular">{formatListTimestamp(item.time, locale, text("Unknown"))}</span>
              </div>
              <h3>{item.title}</h3>
              {/* Summary line: hidden at S, shown from M up. */}
              <p className="feed-row-summary">{feedItemSummary(item)}</p>
            </div>
            {item.saved ? <StatusChip tone="accent">{text("Saved")}</StatusChip> : null}
            {item.unread ? <span className="unread-dot" title={text("Unread")} /> : null}
          </DenseRow>

          {!leadWithDetail && selectedFeedItem?.id === item.id ? renderDetail(selectedFeedItem) : null}
        </div>
      ))}
      {feedItems.length === 0 ? (
        <EmptyState className="company-feed-empty" wrapText={false}>
          <div>
            <strong>{text("No stored feed items for")} <TickerLabel value={company.qualifiedTicker} /> {text("yet.")}</strong>
            <p>
              {text("This company is tracked, but no sample or ingested items are attached to it yet.")}
            </p>
          </div>
          {openInboxFilter ? (
            <Button
              className="compact-button"
              onClick={() => openInboxFilter(company)}
            >
              <Inbox size={15} />
              {text("Open filtered Inbox")}
            </Button>
          ) : null}
        </EmptyState>
      ) : null}
    </div>
  );
}
