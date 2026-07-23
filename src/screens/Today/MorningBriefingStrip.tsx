import { useState } from "react";
import { ChevronRight, Sparkles } from "lucide-react";

import type { MorningBriefing } from "../../api/briefing";
import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import type { Company } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { Button, ListRow } from "../../ui";
import { briefingItemText } from "./briefingItemText";

export type MorningBriefingStripProps = {
  briefing: MorningBriefing | null;
  loading: boolean;
  generating: boolean;
  companyById: Map<string, Company>;
  onGenerate: () => void;
  onOpenItem: (item: MorningBriefingItem) => void;
};

const SUMMARY_LABEL: Record<MorningBriefingItem["itemType"], string> = {
  autopilot_run: "Autopilot",
  attention_event: "Signal",
  signal: "Signal",
  claim_due: "To verify",
  report_date: "Upcoming",
};

/** Grouped one-line counts summary — "3 Autopilot · 2 Signal · 1 To verify". */
function summarize(items: MorningBriefingItem[], text: (k: string) => string): string {
  const counts = new Map<string, number>();
  for (const item of items) {
    const label = SUMMARY_LABEL[item.itemType] ?? "Signal";
    counts.set(label, (counts.get(label) ?? 0) + 1);
  }
  return [...counts.entries()].map(([label, n]) => `${n} ${text(label)}`).join(" · ");
}

/**
 * The compact morning-briefing STRIP (ADR 0087 dec. 5 mockup amendment):
 * replaces the card wall of near-identical rows with a single line — label +
 * "since" timestamp + a grouped counts summary — that expands IN PLACE to the
 * deterministic item list (D2 renderers, ADR 0084) and carries a secondary
 * "Generate" action. Never blocked by a missing provider.
 */
export function MorningBriefingStrip({
  briefing,
  loading,
  generating,
  companyById,
  onGenerate,
  onOpenItem,
}: MorningBriefingStripProps) {
  const { text, locale } = useLocale();
  const [expanded, setExpanded] = useState(false);

  const generateButton = (
    <Button
      className="today-brief-generate"
      disabled={generating}
      onClick={onGenerate}
      type="button"
      variant="ghost"
    >
      {generating ? text("Composing the review…") : text("Generate")}
    </Button>
  );

  function itemRow(item: MorningBriefingItem) {
    const company = companyById.get(item.companyId);
    return (
      <ListRow
        key={item.id}
        icon={<Sparkles aria-hidden="true" size={14} />}
        meta={
          <>
            {company ? <TickerLabel value={company.qualifiedTicker} /> : null}{" "}
            {formatListTimestamp(item.domainDate, locale)}
          </>
        }
        title={briefingItemText(item, text, locale)}
        titleAttr={item.title}
        trailing={
          <Button onClick={() => onOpenItem(item)} type="button" variant="ghost">
            {text("Review")}
          </Button>
        }
      />
    );
  }

  const hasItems = briefing !== null && briefing.items.length > 0;

  return (
    <section aria-labelledby="today-briefing-title" className="today-briefing-strip">
      <div className="today-brief-line">
        <Sparkles aria-hidden="true" size={14} className="today-brief-icon" />
        <span className="today-brief-label" id="today-briefing-title">
          {text("Morning briefing")}
        </span>
        {briefing ? (
          <span className="today-brief-since num-tabular">
            {formatListTimestamp(briefing.composedAt, locale)}
          </span>
        ) : null}
        {loading ? (
          <span className="today-brief-summary">{text("Loading the morning briefing…")}</span>
        ) : hasItems ? (
          <span className="today-brief-summary">{summarize(briefing.items, text)}</span>
        ) : briefing ? (
          <span className="today-brief-summary">
            {text("Nothing to report since your last briefing.")}
          </span>
        ) : (
          <span className="today-brief-summary">
            {text("No briefing yet. Generate one to see what's changed.")}
          </span>
        )}
        {hasItems ? (
          <button
            type="button"
            className={["today-brief-expand", expanded ? "today-brief-expand-open" : ""]
              .filter(Boolean)
              .join(" ")}
            aria-expanded={expanded}
            onClick={() => setExpanded((prior) => !prior)}
          >
            {text("Expand")}
            <ChevronRight aria-hidden="true" size={14} />
          </button>
        ) : null}
        {generateButton}
      </div>
      {hasItems && expanded ? (
        <ul className="today-briefing-items ui-list-rows">
          {briefing.items.map((item) => itemRow(item))}
        </ul>
      ) : null}
    </section>
  );
}
