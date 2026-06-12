import { ArrowRight, ExternalLink, Link } from "lucide-react";
import type { ResearchEvidenceItem } from "../../api/researchTypes";
import { Button, DenseRow } from "../../ui";
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
  formatTimestamp: (value: string | null | undefined) => string;
  onOpen: (item: ResearchEvidenceItem) => void;
  onOpenUrl: (url: string) => void;
  onLink: (item: ResearchEvidenceItem) => void;
  canLink: boolean;
};

export function EvidenceRow({
  item,
  changed,
  text,
  formatTimestamp,
  onOpen,
  onOpenUrl,
  onLink,
  canLink,
}: EvidenceRowProps) {
  const summary = formatEvidenceSummary(item);
  const attribution = formatEvidenceAttribution(item);

  return (
    <DenseRow className="research-evidence-row" interactive={false}>
      <div className="research-evidence-marker" aria-hidden="true" />
      <div className="research-evidence-main">
        <div className="research-evidence-meta">
          <span>{text(formatEvidenceType(item.evidenceType))}</span>
          <span>{text(formatTrustCategory(item.trustCategory))}</span>
          {changed ? <span className="research-change-pill">{text("Changed")}</span> : null}
          <time dateTime={item.occurredAt}>{formatTimestamp(item.occurredAt)}</time>
        </div>
        <h2>{text(formatEvidenceTitle(item))}</h2>
        {summary ? <p>{text(summary)}</p> : null}
        {attribution ? <span className="research-attribution">{text(attribution)}</span> : null}
      </div>
      <div className="research-evidence-actions">
        {canLink ? (
          <Button className="icon-button" onClick={() => onLink(item)} title={text("Link evidence")}>
            <Link size={16} />
          </Button>
        ) : null}
        <Button className="icon-button" onClick={() => onOpen(item)} title={text("Open evidence")}>
          <ArrowRight size={16} />
        </Button>
        {item.sourceUrl ? (
          <Button
            className="icon-button"
            onClick={() => onOpenUrl(item.sourceUrl ?? "")}
            title={text("Open source URL")}
          >
            <ExternalLink size={16} />
          </Button>
        ) : null}
      </div>
    </DenseRow>
  );
}
