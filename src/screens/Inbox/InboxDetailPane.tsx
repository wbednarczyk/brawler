import { BookOpenText, Building2, ExternalLink, FileText, Mail, MailOpen, Save } from "lucide-react";
import { Button } from "../../shared/components/Button";
import { FeedAiAnalysisPanel } from "../../shared/components/FeedAiAnalysisPanel";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import type { InboxScreenProps } from "./inboxTypes";

type InboxDetailPaneProps = Pick<
  InboxScreenProps,
  | "selectedFeedItem"
  | "selectedFeedCompany"
  | "aiAnalysisJobsByFeedItemId"
  | "aiAnalysisErrorByFeedItemId"
  | "aiAnalysisRequestInFlightByFeedItemId"
  | "aiAnalysisProviderConfigured"
  | "healthError"
  | "databaseError"
  | "updateSelectedFeedItem"
  | "openCompanyWorkspaceFromFeedItem"
  | "openFeedItemNoteDraft"
  | "startFeedItemAiAnalysis"
  | "retryFeedItemAiAnalysis"
  | "feedItemSummary"
  | "formatTimestamp"
>;

function isPdfAttachment(attachment: { label: string; url: string }) {
  const label = attachment.label.trim().toLowerCase();
  const isSourcePageChrome =
    label === "regulamin" ||
    label === "polityka prywatności" ||
    label === "polityka prywatnosci" ||
    label === "polityka cookies";

  return (
    !isSourcePageChrome &&
    (/\.pdf(?:$|[?#])/i.test(attachment.url) || /\.pdf(?:$|[?#])/i.test(attachment.label))
  );
}

export function InboxDetailPane({
  selectedFeedItem,
  selectedFeedCompany,
  aiAnalysisJobsByFeedItemId,
  aiAnalysisErrorByFeedItemId,
  aiAnalysisRequestInFlightByFeedItemId,
  aiAnalysisProviderConfigured,
  healthError,
  databaseError,
  updateSelectedFeedItem,
  openCompanyWorkspaceFromFeedItem,
  openFeedItemNoteDraft,
  startFeedItemAiAnalysis,
  retryFeedItemAiAnalysis,
  feedItemSummary,
  formatTimestamp,
}: InboxDetailPaneProps) {
  const { text } = useLocale();
  const pdfAttachments = selectedFeedItem?.attachments.filter(isPdfAttachment) ?? [];

  return (
    <aside className="detail-pane" aria-label={text("Feed item details")}>
      <div className="detail-icon">
        <FileText size={24} />
      </div>
      {selectedFeedItem ? (
        <>
          <h2>{selectedFeedItem.title}</h2>
          <dl className="detail-context" aria-label={text("Feed item context")}>
            <div>
              <dt>{text("Company")}</dt>
              <dd>
                <TickerLabel value={selectedFeedItem.company} />
              </dd>
            </div>
            <div>
              <dt>{text("Source URL")}</dt>
              <dd>
                <a href={selectedFeedItem.sourceUrl} rel="noreferrer" target="_blank">
                  {selectedFeedItem.sourceUrl}
                </a>
              </dd>
            </div>
          </dl>
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
          <div className="detail-actions" aria-label={text("Feed item actions")}>
            <Button
              className="compact-button"
              onClick={() =>
                updateSelectedFeedItem((item) => ({
                  ...item,
                  unread: !item.unread,
                }))
              }
            >
              {selectedFeedItem.unread ? <MailOpen size={15} /> : <Mail size={15} />}
              {selectedFeedItem.unread ? text("Mark read") : text("Mark unread")}
            </Button>
            <Button
              className="compact-button"
              onClick={() =>
                updateSelectedFeedItem((item) => ({
                  ...item,
                  saved: !item.saved,
                }))
              }
            >
              <Save size={15} />
              {selectedFeedItem.saved ? text("Unsave") : text("Save")}
            </Button>
            {selectedFeedCompany ? (
              <Button
                className="compact-button"
                onClick={() => openCompanyWorkspaceFromFeedItem(selectedFeedItem)}
              >
                <Building2 size={15} />
                {text("Open company")}
              </Button>
            ) : null}
            {selectedFeedCompany ? (
              <Button
                className="compact-button"
                onClick={() => openFeedItemNoteDraft(selectedFeedItem)}
              >
                <BookOpenText size={15} />
                {text("Note")}
              </Button>
            ) : null}
          </div>
          {pdfAttachments.length > 0 ? (
            <div className="feed-attachment-list" aria-label={text("Feed attachments")}>
              <span className="feed-attachment-heading">{text("Attachments")}</span>
              {pdfAttachments.map((attachment) => (
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
          <dl className="detail-pane-footer-meta" aria-label={text("Feed item timestamps")}>
            <div>
              <dt>{text("Published")}</dt>
              <dd>{formatTimestamp(selectedFeedItem.publishedAt, text("Unknown"))}</dd>
            </div>
            <div>
              <dt>{text("Fetched")}</dt>
              <dd>{formatTimestamp(selectedFeedItem.fetchedAt, text("Unknown"))}</dd>
            </div>
          </dl>
        </>
      ) : (
        <>
          <h2>{text("No item selected")}</h2>
          <p>{text("Select a feed item to inspect source details and origin links.")}</p>
        </>
      )}
      {healthError ? <p className="error-text">{text("Health command failed")}: {healthError}</p> : null}
      {databaseError ? <p className="error-text">{text("Local data refresh failed")}: {databaseError}</p> : null}
    </aside>
  );
}
