import { ArrowRight, ExternalLink, Link } from "lucide-react";
import type { ResearchEvidenceItem } from "../../api/researchTypes";
import { ActionButton, DenseRow, Figure } from "../../ui";
import {
  formatEvidenceAttribution,
  formatEvidenceSummary,
  formatEvidenceTitle,
  formatEvidenceType,
  formatTrustCategory,
} from "./researchFormatters";

type EvidenceRowProps = {
  item: ResearchEvidenceItem;
  changed: boolean;
  text: (value: string) => string;
  onOpen: (item: ResearchEvidenceItem) => void;
  onOpenUrl: (url: string) => void;
  onLink: (item: ResearchEvidenceItem) => void;
  canLink: boolean;
};

export function EvidenceRow({
  item,
  changed,
  text,
  onOpen,
  onOpenUrl,
  onLink,
  canLink,
}: EvidenceRowProps) {
  const summary = formatEvidenceSummary(item);
  const attribution = formatEvidenceAttribution(item);
  // The evidence title, once through `text()` — reused below as the
  // per-row aria-label suffix (sol R2 amendment) so identically-labelled
  // "Open"/"Open source"/"Link to question" buttons across rows stay
  // uniquely named.
  const title = text(formatEvidenceTitle(item));

  return (
    <DenseRow className="research-evidence-row" interactive={false}>
      <div className="research-evidence-marker" aria-hidden="true" />
      <div className="research-evidence-main">
        <div className="research-evidence-meta">
          <span>{text(formatEvidenceType(item.evidenceType))}</span>
          <span>{text(formatTrustCategory(item.trustCategory))}</span>
          {changed ? <span className="research-change-pill">{text("Changed")}</span> : null}
          <time dateTime={item.occurredAt}>
            <Figure kind="datetime" value={item.occurredAt} />
          </time>
        </div>
        <h2>{title}</h2>
        {summary ? <p>{text(summary)}</p> : null}
        {attribution ? <span className="research-attribution">{text(attribution)}</span> : null}
      </div>
      <div className="research-evidence-actions">
        {canLink ? (
          <ActionButton
            className="compact-button"
            onClick={() => onLink(item)}
            verb="link"
            aria-label={`${text("Link to question")}: ${title}`}
          >
            <Link size={15} />
            {text("Link to question")}
          </ActionButton>
        ) : null}
        <ActionButton
          className="compact-button"
          onClick={() => onOpen(item)}
          verb="open"
          aria-label={`${text("Open")}: ${title}`}
        >
          <ArrowRight size={15} />
          {text("Open")}
        </ActionButton>
        {item.sourceUrl ? (
          <ActionButton
            className="compact-button"
            onClick={() => onOpenUrl(item.sourceUrl ?? "")}
            verb="open"
            aria-label={`${text("Open source")}: ${title}`}
          >
            <ExternalLink size={15} />
            {text("Open source")}
          </ActionButton>
        ) : null}
      </div>
    </DenseRow>
  );
}
