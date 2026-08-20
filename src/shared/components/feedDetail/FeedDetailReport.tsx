import { useLocale } from "../../locale";
import { DOCUMENT_FORMS, pluralNoun } from "../../locale/plural";
import type { FeedItem } from "../../../api/types";
import { isReportDocumentAttachment } from "./reportAttachments";

export type FeedDetailReportProps = {
  item: FeedItem;
};

// Display-only transform (F1 S4 scope note: don't invent data the backend
// hasn't classified) — strips the extension and turns separators into spaces
// so the human eye scans a title, not a raw filename. The real filename stays
// visible right below it (dec. 6 — human title first, filename in mono meta).
function humanTitleFromLabel(label: string): string {
  // A URL-shaped label yields its decoded basename, not the mangled full URL.
  let base = label;
  if (/^https?:\/\//i.test(label)) {
    try {
      const path = new URL(label).pathname;
      base = decodeURIComponent(path.split("/").filter(Boolean).pop() ?? label);
    } catch {
      base = label;
    }
  }
  const withoutExt = base.replace(/\.[a-z0-9]{1,5}$/i, "");
  const spaced = withoutExt.replace(/[_-]+/g, " ").trim();
  return spaced || base;
}

// Report kind (an official filing with attachments): a document list — human
// title first, filename in mono metadata below — plus the not-yet-built
// preview placeholder from InboxRaport.dc.html (#86 owns the real viewer).
export function FeedDetailReport({ item }: FeedDetailReportProps) {
  const { text, locale } = useLocale();
  const docs = item.attachments.filter(isReportDocumentAttachment);
  const countLabel = `${docs.length} ${pluralNoun(locale, docs.length, DOCUMENT_FORMS)}`;
  const readLabel = item.unread ? text("not read yet") : text("read");

  return (
    <>
      {docs.length > 0 ? <p className="feed-report-count">{`${countLabel} · ${readLabel}`}</p> : null}
      {docs.length > 0 ? (
        <ul className="feed-doc-list" aria-label={text("Report documents")}>
          {docs.map((doc) => (
            <li className="feed-doc-row" key={doc.id}>
              <span className="feed-doc-row-title">{humanTitleFromLabel(doc.label)}</span>
              <span className="feed-doc-row-file">{doc.label}</span>
            </li>
          ))}
        </ul>
      ) : null}
      <div className="feed-doc-preview-placeholder">
        {text("Document preview lands here — in progress.")}
      </div>
    </>
  );
}
