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
// the workspace and the cockpit `companyFeed` dashboard panel (ADR 0057) render
// the same surface from explicit props — no AppStateRoot coupling beyond the
// handlers passed in.
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
  // AppStateRoot navigation, but the self-contained cockpit dashboard panel
  // (ADR 0057) omits them — the same actions stay reachable from the Inbox.
  inspectFeedItem?: (item: FeedItem) => void;
  openFeedItemNoteDraft?: (item: FeedItem) => void;
  openInboxFilter?: (company: Company) => void;
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
  formatTimestamp,
  feedItemSummary,
}: CompanyFeedSectionProps) {
  const { text, locale } = useLocale();

  return (
    <div
      className="company-tab-panel company-feed-panel"
      aria-label={text("Company feed")}
      data-company-feed-list="true"
    >
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

          {selectedFeedItem?.id === item.id ? (
            <aside className="company-feed-detail" aria-label={text("Company feed item details")}>
              <FeedDetailContent
                item={selectedFeedItem}
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
                        updateFeedItemState(selectedFeedItem, (feedItem) => ({
                          ...feedItem,
                          unread: !feedItem.unread,
                        }))
                      }
                    >
                      {selectedFeedItem.unread ? <MailOpen size={15} /> : <Mail size={15} />}
                      {selectedFeedItem.unread ? text("Mark read") : text("Mark unread")}
                    </Button>
                    <Button
                      className="compact-button"
                      onClick={() =>
                        updateFeedItemState(selectedFeedItem, (feedItem) => ({
                          ...feedItem,
                          saved: !feedItem.saved,
                        }))
                      }
                    >
                      <Save size={15} />
                      {selectedFeedItem.saved ? text("Unsave") : text("Save")}
                    </Button>
                    {inspectFeedItem ? (
                      <Button
                        className="compact-button"
                        onClick={() => inspectFeedItem(selectedFeedItem)}
                      >
                        <Inbox size={15} />
                        {text("Open in Inbox")}
                      </Button>
                    ) : null}
                    {openFeedItemNoteDraft ? (
                      <Button
                        className="compact-button"
                        onClick={() => openFeedItemNoteDraft(selectedFeedItem)}
                      >
                        <BookOpenText size={15} />
                        {text("Note")}
                      </Button>
                    ) : null}
                    <a
                      className="secondary-button compact-button"
                      href={selectedFeedItem.sourceUrl}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink size={15} />
                      {text("Open source")}
                    </a>
                  </ActionRow>
                }
              />
            </aside>
          ) : null}
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
