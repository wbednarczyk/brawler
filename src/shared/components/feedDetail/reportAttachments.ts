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

  return (
    !isSourcePageChrome &&
    (/\.pdf(?:$|[?#])/i.test(attachment.url) || /\.pdf(?:$|[?#])/i.test(attachment.label))
  );
}
