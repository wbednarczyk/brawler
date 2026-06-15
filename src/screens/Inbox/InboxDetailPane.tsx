import {
  BookOpenText,
  Building2,
  Check,
  ExternalLink,
  FileText,
  Mail,
  MailOpen,
  Save,
  X,
} from "lucide-react";
import { ActionRow, Button, InfoGrid, StatusChip } from "../../ui";
import { FeedAiAnalysisPanel } from "../../shared/components/FeedAiAnalysisPanel";
import { FeedKpiExtractionPanel } from "../../shared/components/FeedKpiExtractionPanel";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { useLocale } from "../../shared/locale";
import type { CompanySignal } from "../../api/types";
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
  | "confirmCompanySignal"
  | "rejectCompanySignal"
  | "feedItemSummary"
  | "formatTimestamp"
> & {
  selectedFeedSignals: CompanySignal[] | undefined;
};

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
  selectedFeedSignals,
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
  confirmCompanySignal,
  rejectCompanySignal,
  feedItemSummary,
  formatTimestamp,
}: InboxDetailPaneProps) {
  const { text } = useLocale();
  const pdfAttachments = selectedFeedItem?.attachments.filter(isPdfAttachment) ?? [];
  const signals = selectedFeedSignals ?? [];

  return (
    <aside className="detail-pane" aria-label={text("Feed item details")}>
      <div className="detail-icon">
        <FileText size={24} />
      </div>
      {selectedFeedItem ? (
        <>
          <h2>{selectedFeedItem.title}</h2>
          <InfoGrid
            ariaLabel={text("Feed item context")}
            className="detail-context"
            items={[
              { label: text("Company"), value: <TickerLabel value={selectedFeedItem.company} /> },
              {
                label: text("Source URL"),
                value: (
                  <a href={selectedFeedItem.sourceUrl} rel="noreferrer" target="_blank">
                    {selectedFeedItem.sourceUrl}
                  </a>
                ),
              },
            ]}
          />
          <section className="feed-body-section" aria-label={text("Feed summary")}>
            <div className="feed-body-heading">
              <span>{text("Summary")}</span>
            </div>
            <p className="feed-detail-body">{feedItemSummary(selectedFeedItem)}</p>
          </section>
          {signals.length > 0 ? (
            <section
              className="feed-body-section feed-signals-section"
              aria-label={text("Typed filing signals")}
            >
              <div className="feed-body-heading">
                <span>{text("Typed signals")}</span>
              </div>
              <ul className="feed-signals-list">
                {signals.map((signal) => (
                  <li className="feed-signal-row" key={signal.id}>
                    <div className="feed-signal-meta">
                      <StatusChip tone={signal.status === "proposed" ? "warn" : "accent"}>
                        {text(signal.categoryDisplayName)}
                      </StatusChip>
                      <span className="feed-signal-source">
                        {signal.classifiedBy === "ai"
                          ? text("AI proposal — confirm to apply")
                          : text("Rule-classified")}
                      </span>
                    </div>
                    {signal.status === "proposed" ? (
                      <ActionRow className="feed-signal-actions" ariaLabel={text("Signal proposal actions")}>
                        <Button
                          className="compact-button"
                          onClick={() => {
                            void confirmCompanySignal(signal.id);
                          }}
                        >
                          <Check size={15} />
                          {text("Confirm")}
                        </Button>
                        <Button
                          className="compact-button danger-subtle-button"
                          onClick={() => {
                            void rejectCompanySignal(signal.id);
                          }}
                        >
                          <X size={15} />
                          {text("Reject")}
                        </Button>
                      </ActionRow>
                    ) : null}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}
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
          <ActionRow className="detail-actions" ariaLabel={text("Feed item actions")}>
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
          </ActionRow>
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
          {pdfAttachments.length > 0 || /report|raport/i.test(selectedFeedItem.type) ? (
            <FeedKpiExtractionPanel
              key={selectedFeedItem.id}
              feedItem={selectedFeedItem}
              providerConfigured={aiAnalysisProviderConfigured}
            />
          ) : null}
          <InfoGrid
            ariaLabel={text("Feed item timestamps")}
            className="detail-pane-footer-meta"
            items={[
              { label: text("Published"), value: formatTimestamp(selectedFeedItem.publishedAt, text("Unknown")) },
              { label: text("Fetched"), value: formatTimestamp(selectedFeedItem.fetchedAt, text("Unknown")) },
            ]}
          />
        </>
      ) : (
        <>
          <h2>{text("No item selected")}</h2>
          <p>{text("Select a feed item to inspect source details and origin links.")}</p>
        </>
      )}
      {healthError ? <p className="error-text">{text("Health command failed")}: {healthError}</p> : null}
          {databaseError ? <p className="error-text">{text("Workspace refresh failed")}: {databaseError}</p> : null}
    </aside>
  );
}
