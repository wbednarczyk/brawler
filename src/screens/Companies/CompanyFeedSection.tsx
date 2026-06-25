import {
  BookOpenText,
  ExternalLink,
  Inbox,
  Mail,
  MailOpen,
  Save,
} from "lucide-react";
import type { AiAnalysisJob, Company, FeedItem } from "../../api/types";
import { FeedAiAnalysisPanel } from "../../shared/components/FeedAiAnalysisPanel";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import { useAiAnalysisProviderConfigured } from "../../app/state/SettingsContext";
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
  aiAnalysisJobsByFeedItemId: Record<string, AiAnalysisJob[]>;
  aiAnalysisErrorByFeedItemId: Record<string, string | null>;
  aiAnalysisRequestInFlightByFeedItemId: Record<string, boolean>;
  toggleFeedItem: (item: FeedItem) => void;
  selectFeedItemFromKeyboard: (event: React.KeyboardEvent<HTMLElement>, item: FeedItem) => void;
  updateFeedItemState: (item: FeedItem, update: (item: FeedItem) => FeedItem) => void;
  inspectFeedItem: (item: FeedItem) => void;
  openFeedItemNoteDraft: (item: FeedItem) => void;
  startFeedItemAiAnalysis: (item: FeedItem, promptPresetId?: string, customQuestion?: string) => Promise<void>;
  retryFeedItemAiAnalysis: (jobId: string, itemId: string) => Promise<void>;
  openInboxFilter: (company: Company) => void;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  feedItemSummary: (item: FeedItem) => string;
};

export function CompanyFeedSection({
  company,
  feedItems,
  selectedFeedItem,
  aiAnalysisJobsByFeedItemId,
  aiAnalysisErrorByFeedItemId,
  aiAnalysisRequestInFlightByFeedItemId,
  toggleFeedItem,
  selectFeedItemFromKeyboard,
  updateFeedItemState,
  inspectFeedItem,
  openFeedItemNoteDraft,
  startFeedItemAiAnalysis,
  retryFeedItemAiAnalysis,
  openInboxFilter,
  formatTimestamp,
  feedItemSummary,
}: CompanyFeedSectionProps) {
  const { text } = useLocale();
  const aiAnalysisProviderConfigured = useAiAnalysisProviderConfigured();

  return (
    <div
      className="company-tab-panel"
      aria-label={text("Company feed")}
      data-company-feed-list="true"
    >
      {feedItems.map((item) => (
        <div className="company-feed-row-block" key={item.id}>
          <DenseRow
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
            role="button"
            selected={selectedFeedItem?.id === item.id}
            tabIndex={0}
            title={text("Open company feed item details")}
            unread={item.unread}
          >
            <div className="feed-row-main">
              <div className="feed-meta">
                <span>{item.type}</span>
                <span>{item.source}</span>
                <span>{formatTimestamp(item.time, text("Unknown"))}</span>
              </div>
              <h3>{item.title}</h3>
              <p>{feedItemSummary(item)}</p>
            </div>
            {item.saved ? <StatusChip tone="accent">{text("Saved")}</StatusChip> : null}
            {item.unread ? <span className="unread-dot" title={text("Unread")} /> : null}
          </DenseRow>

          {selectedFeedItem?.id === item.id ? (
            <aside className="company-feed-detail" aria-label={text("Company feed item details")}>
              <div>
                <span className="eyebrow">{text("Selected item")}</span>
                <h3>{selectedFeedItem.title}</h3>
                <section className="feed-body-section" aria-label={text("Feed summary")}>
                  <div className="feed-body-heading">
                    <span>{text("Summary")}</span>
                  </div>
                  <p className="feed-detail-body">{feedItemSummary(selectedFeedItem)}</p>
                </section>
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
                <Button
                  className="compact-button"
                  onClick={() => inspectFeedItem(selectedFeedItem)}
                >
                  <Inbox size={15} />
                  {text("Open in Inbox")}
                </Button>
                <Button
                  className="compact-button"
                  onClick={() => openFeedItemNoteDraft(selectedFeedItem)}
                >
                  <BookOpenText size={15} />
                  {text("Note")}
                </Button>
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
              <FeedAiAnalysisPanel
                feedItem={selectedFeedItem}
                jobs={aiAnalysisJobsByFeedItemId[selectedFeedItem.id] ?? []}
                error={aiAnalysisErrorByFeedItemId[selectedFeedItem.id] ?? null}
                providerConfigured={aiAnalysisProviderConfigured}
                requestInFlight={aiAnalysisRequestInFlightByFeedItemId[selectedFeedItem.id] ?? false}
                onStart={startFeedItemAiAnalysis}
                onRetry={retryFeedItemAiAnalysis}
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
          <Button
            className="compact-button"
            onClick={() => openInboxFilter(company)}
          >
            <Inbox size={15} />
            {text("Open filtered Inbox")}
          </Button>
        </EmptyState>
      ) : null}
    </div>
  );
}
