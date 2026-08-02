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
import { useLocale } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";
import { ActionRow, Button, DenseRow, EmptyState, InfoGrid, StatusChip } from "../../ui";

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
              <div>
                <span className="eyebrow">{text("Selected item")}</span>
                <h3>{selectedFeedItem.title}</h3>
                <div role="group" className="feed-body-section" aria-label={text("Feed summary")}>
                  <div className="feed-body-heading">
                    <span>{text("Summary")}</span>
                  </div>
                  <p className="feed-detail-body">{feedItemSummary(selectedFeedItem)}</p>
                </div>
                <details className="feed-body-section feed-body-disclosure" aria-label={text("Official report body")}>
                  <summary className="feed-body-heading">
                    <span>{text("Official report body")}</span>
                    <strong>{selectedFeedItem.bodyText ? text("Stored") : text("Not stored")}</strong>
                  </summary>
                  {selectedFeedItem.bodyText ? (
                    <p className="feed-detail-body">{selectedFeedItem.bodyText}</p>
                  ) : (
                    <p className="feed-detail-empty">
                      {text("No official report body is stored for this item yet. Refresh sources and check Sources for detail warnings if this remains empty.")}
                    </p>
                  )}
                </details>
              </div>
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
              <InfoGrid
                className="metadata-grid"
                items={[
                  { label: text("Source"), value: selectedFeedItem.source },
                  { label: text("Type"), value: selectedFeedItem.type },
                  {
                    label: text("Published"),
                    value: formatTimestamp(selectedFeedItem.publishedAt, text("Unknown")),
                  },
                  {
                    label: text("Fetched"),
                    value: formatTimestamp(selectedFeedItem.fetchedAt, text("Unknown")),
                  },
                  { label: text("Attribution"), value: selectedFeedItem.attribution },
                  { label: text("Language"), value: selectedFeedItem.language },
                ]}
              />
              {selectedFeedItem.attachments.length > 0 ? (
                <div className="feed-attachment-list" aria-label={text("Company feed attachments")}>
                  {selectedFeedItem.attachments.map((attachment) => (
                    <a
                      className="feed-attachment-link"
                      href={attachment.url}
                      key={attachment.id}
                      rel="noreferrer"
                      target="_blank"
                    >
                      <ExternalLink size={14} />
                      {attachment.label}
                    </a>
                  ))}
                </div>
              ) : null}
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
