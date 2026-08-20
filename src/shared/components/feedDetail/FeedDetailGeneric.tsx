import { ExternalLink, FileText } from "lucide-react";
import type { ReactNode } from "react";
import { InfoGrid } from "../../../ui";
import { TickerLabel } from "../TickerLabel";
import { useLocale } from "../../locale";
import type { CompanySignal, FeedItem } from "../../../api/types";
import { FeedSignalsSection } from "./FeedSignalsSection";
import { isReportDocumentAttachment } from "./reportAttachments";

export type FeedDetailGenericProps = {
  item: FeedItem;
  signals: CompanySignal[];
  actions: ReactNode;
  feedItemSummary: (item: FeedItem) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  onConfirmSignal: (signalId: string) => Promise<void> | void;
  onRejectSignal: (signalId: string) => Promise<void> | void;
};

// The pre-redesign generic detail body (F1 S4): still used verbatim for
// `redFlag` and any future presentation kind FeedDetailContent does not yet
// have a dedicated design for. Unchanged behavior — media/filing/report get
// FeedDetailMedia/Filing/Report instead.
export function FeedDetailGeneric({
  item,
  signals,
  actions,
  feedItemSummary,
  formatTimestamp,
  onConfirmSignal,
  onRejectSignal,
}: FeedDetailGenericProps) {
  const { text } = useLocale();
  const pdfAttachments = item.attachments.filter(isReportDocumentAttachment);

  return (
    <>
      <div className="detail-icon">
        <FileText size={24} />
      </div>
      <h2>{item.title}</h2>
      <InfoGrid
        ariaLabel={text("Feed item context")}
        className="detail-context"
        items={[
          { label: text("Company"), value: <TickerLabel value={item.company} /> },
          {
            label: text("Source URL"),
            value: (
              <a href={item.sourceUrl} rel="noreferrer" target="_blank">
                {item.sourceUrl}
              </a>
            ),
          },
        ]}
      />
      <section className="feed-body-section" aria-label={text("Feed summary")}>
        <div className="feed-body-heading">
          <span>{text("Summary")}</span>
        </div>
        <p className="feed-detail-body">{feedItemSummary(item)}</p>
      </section>
      <FeedSignalsSection signals={signals} onConfirmSignal={onConfirmSignal} onRejectSignal={onRejectSignal} />
      <details className="feed-body-section feed-body-disclosure" aria-label={text("Official report body")}>
        <summary className="feed-body-heading">
          <span>{text("Official report body")}</span>
          <strong>{item.bodyText ? text("Stored") : text("Not stored")}</strong>
        </summary>
        {item.bodyText ? (
          <p className="feed-detail-body">{item.bodyText}</p>
        ) : (
          <p className="feed-detail-empty">
            {text(
              "No official report body is stored for this item yet. Refresh sources and check Sources for detail warnings if this remains empty.",
            )}
          </p>
        )}
      </details>
      {actions}
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
      <InfoGrid
        ariaLabel={text("Feed item timestamps")}
        className="detail-pane-footer-meta"
        items={[
          { label: text("Published"), value: formatTimestamp(item.publishedAt, text("Unknown")) },
          { label: text("Fetched"), value: formatTimestamp(item.fetchedAt, text("Unknown")) },
        ]}
      />
    </>
  );
}
