import { useEffect, useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";

import { getReportTaggedFactCoverage } from "../../api/taggedFactPromotion";
import type { TaggedFactCoverageCounts } from "../../api/taggedFactPromotion";
import { useLocale } from "../locale";
import { Button, Hint, InfoGrid, SectionHeader } from "../../ui";
import { CoverageUncrosswalkedConcepts } from "./CoverageUncrosswalkedConcepts";

type Props = {
  companyId: string;
  reloadKey?: number;
};

/// The Layer 1 capture proof, compacted to one line (ADR 0100, epic #398):
/// "every number from the report is either in Fundamentals or has a reason it
/// isn't" is a trust signal the owner reads once, so — unlike the approved
/// mockup's full funnel bar + legend + strip — it does NOT get its own
/// screen; it is one compact grid inside the existing Coverage panel, with a
/// disclosure that links through to the "positions the program doesn't know
/// yet" list (the promote action, ADR 0100 decision 10).
export function CoverageRawCapture({ companyId, reloadKey = 0 }: Props) {
  const { text } = useLocale();
  const [counts, setCounts] = useState<TaggedFactCoverageCounts | null>(null);
  const [expanded, setExpanded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    getReportTaggedFactCoverage(companyId)
      .then((value) => {
        if (!cancelled) setCounts(value);
      })
      .catch(() => {
        if (!cancelled) setCounts(null);
      });
    return () => {
      cancelled = true;
    };
  }, [companyId, reloadKey]);

  // Nothing tagged yet (a company with no ESEF/iXBRL filing captured) — the
  // line has nothing to prove, so it stays silent rather than showing an
  // all-zero grid that reads as a bug.
  if (!counts || counts.rawStored === 0) {
    return null;
  }

  return (
    <div role="group" className="coverage-raw-capture" aria-label={text("Raw report capture")}>
      <SectionHeader level="h3" title={text("What the program read from the report")} />
      <Hint>
        {text("Every number from the report is either in Fundamentals or has a reason it isn't yet.")}
      </Hint>
      <InfoGrid
        ariaLabel={text("Raw report capture")}
        items={[
          { label: text("Numbers in the report"), value: counts.rawStored },
          { label: text("In Fundamentals"), value: counts.projected },
          { label: text("Earlier years, kept as context"), value: counts.comparative },
          { label: text("Split into parts"), value: counts.dimensional },
          { label: text("From notes"), value: counts.noteLevel },
          { label: text("No name yet"), value: counts.awaitingName },
          { label: text("Conflicting values"), value: counts.conflicting },
          // Hidden at zero: an unreadable number is rare, and a permanent
          // zero row would read as noise — but when one exists it MUST be
          // visible, or the "every number has a reason" sentence is false.
          ...(counts.unparsed > 0
            ? [{ label: text("Could not be read"), value: counts.unparsed }]
            : []),
        ]}
      />
      <Button
        variant="minimal"
        aria-expanded={expanded}
        onClick={() => setExpanded((value) => !value)}
      >
        {expanded ? (
          <ChevronDown size={14} aria-hidden="true" />
        ) : (
          <ChevronRight size={14} aria-hidden="true" />
        )}
        {/* A control, not a second title: the section it reveals carries the
            heading (and the count of DISTINCT positions). Repeating the
            heading here rendered the same sentence twice, one line apart.
            No count here either — the InfoGrid above states the raw
            occurrence count ("No name yet"), which is a different number. */}
        {expanded ? text("Hide the unnamed positions") : text("Show the unnamed positions")}
      </Button>
      {expanded ? (
        <CoverageUncrosswalkedConcepts companyId={companyId} reloadKey={reloadKey} />
      ) : null}
    </div>
  );
}
