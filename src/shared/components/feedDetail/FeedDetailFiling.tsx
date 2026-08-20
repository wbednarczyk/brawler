import { useLocale } from "../../locale";
import type { FeedItem } from "../../../api/types";
import { parseFilingBody } from "./parseFilingBody";

export type FeedDetailFilingProps = {
  item: FeedItem;
};

// Filing kind (a bare ESPI/EBI notice, no attachments): report number + legal
// basis meta line, then a "Content" section parsed from the stored body text —
// only when it parses non-empty (state matrix § Partial, InboxESPI.dc.html).
// No `body_text` at all renders compact: chip/title/date + actions only.
export function FeedDetailFiling({ item }: FeedDetailFilingProps) {
  const { text } = useLocale();
  const parsed = parseFilingBody(item.bodyText);
  const metaParts = [
    parsed.reportNumber ? text("RB {n}").replace("{n}", parsed.reportNumber) : null,
    parsed.legalBasis,
  ].filter((part): part is string => Boolean(part));

  return (
    <>
      {metaParts.length > 0 ? <p className="feed-filing-meta">{metaParts.join(" · ")}</p> : null}
      {parsed.content ? (
        <section className="feed-body-section" aria-label={text("Filing content")}>
          <div className="feed-body-heading">
            <span>{text("Content")}</span>
          </div>
          <p className="feed-detail-body">{parsed.content}</p>
        </section>
      ) : null}
    </>
  );
}
