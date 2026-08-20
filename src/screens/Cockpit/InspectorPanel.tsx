import type { FeedItem } from "../../api/types";
import { feedItemSummary } from "../../app/useNotebookController";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { formatListTimestamp } from "../../shared/format/datetime";
import { useLocale } from "../../shared/locale";
import { EmptyState, SectionHeader, StatusChip } from "../../ui";

export function InspectorPanel({
  item,
  text,
}: {
  item: FeedItem | null;
  text: (s: string) => string;
}) {
  const { locale } = useLocale();
  if (!item) {
    return <EmptyState>{text("Select a feed item to inspect it.")}</EmptyState>;
  }
  // Cockpit-native, read-rich inspector: company/source context, summary, body,
  // and source attachments — no controller-bound actions (AI analysis, mark-read,
  // notes) here; those arrive when the cockpit becomes the host (phase 6).
  const pdfAttachments = item.attachments.filter((attachment) =>
    /\.pdf(?:$|[?#])/i.test(attachment.url),
  );
  return (
    <div role="group" className="cockpit-inspector" aria-label={text("Feed item inspector")}>
      <header className="cockpit-inspector-head">
        <TickerLabel value={item.company} />
        <h3 className="cockpit-inspector-title">{item.title}</h3>
        <div className="cockpit-inspector-tags">
          <StatusChip>{item.source}</StatusChip>
          <StatusChip>{item.type}</StatusChip>
          {item.language ? <StatusChip>{item.language.toUpperCase()}</StatusChip> : null}
          <span className="cockpit-inspector-time num-tabular">{formatListTimestamp(item.time, locale)}</span>
        </div>
        {item.attribution ? (
          <p className="cockpit-inspector-attribution">{item.attribution}</p>
        ) : null}
      </header>
      {/* Routed through the shared guard so a filing's dead boilerplate summary
          never renders here either (F1 S1; sol round-1 finding 3). */}
      {feedItemSummary(item) ? (
        <p className="cockpit-inspector-summary">{feedItemSummary(item)}</p>
      ) : null}
      {item.bodyText ? <p className="cockpit-inspector-body">{item.bodyText}</p> : null}
      {pdfAttachments.length > 0 ? (
        <div role="group" className="cockpit-inspector-attachments" aria-label={text("Attachments")}>
          <SectionHeader level="h4" title={text("Attachments")} />
          <ul>
            {pdfAttachments.map((attachment) => (
              <li key={attachment.url}>
                <a href={attachment.url} target="_blank" rel="noreferrer">
                  {attachment.label}
                </a>
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      <a className="secondary-button compact-button cockpit-inspector-open" href={item.sourceUrl} target="_blank" rel="noreferrer">
        {text("Open source")}
      </a>
    </div>
  );
}
