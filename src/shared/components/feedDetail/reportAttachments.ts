import type { FeedItemAttachment } from "../../../api/types";

// Shared by FeedDetailGeneric (redFlag/unknown fallback) and FeedDetailReport:
// a feed item's `attachments` include source-page chrome scraped alongside
// the real document links (a site's ToS/privacy/cookie-policy links) — those
// are never report documents, so both detail bodies filter them out the same
// way rather than each re-deriving the rule.
export function isReportDocumentAttachment(attachment: FeedItemAttachment) {
  const label = attachment.label.trim().toLowerCase();
  const isSourcePageChrome =
    label === "regulamin" ||
    label === "polityka prywatności" ||
    label === "polityka prywatnosci" ||
    label === "polityka cookies";

  // Every document format the adapters accept (bankier emits PDF, XHTML/ZIP
  // ESEF packages, and occasional office formats) — a report whose only
  // attachment is an XHTML must still show its document list.
  const DOC_EXT = /\.(?:pdf|xhtml|html|zip|xml|docx?|xlsx?)(?:$|[?#])/i;
  return !isSourcePageChrome && (DOC_EXT.test(attachment.url) || DOC_EXT.test(attachment.label));
}
