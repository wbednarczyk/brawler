
import { Sparkles } from "lucide-react";

import type { MorningBriefing } from "../../api/briefing";
import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import type { Company } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";
import { TickerLabel } from "../../shared/components/TickerLabel";
import { Button, EmptyState, ListRow, SectionHeader, Skeleton } from "../../ui";

export type MorningBriefingCardProps = {
  briefing: MorningBriefing | null;
  loading: boolean;
  generating: boolean;
  companyById: Map<string, Company>;
  onGenerate: () => void;
  onOpenItem: (item: MorningBriefingItem) => void;
};


/**
 * The Today briefing card (ADR 0068 decision 4, v0.54 §T5): the latest morning
 * briefing, narrative-with-citations when one was phrased, otherwise the
 * deterministic structured item list — never blocked by a missing provider.
 */
export function MorningBriefingCard({
  briefing,
  loading,
  generating,
  companyById,
  onGenerate,
  onOpenItem,
}: MorningBriefingCardProps) {
  const { text, locale } = useLocale();

  const generateButton = (
    <Button disabled={generating} onClick={onGenerate} type="button" variant="ghost">
      <Sparkles aria-hidden="true" size={14} />
      {generating ? text("Generating…") : text("Generate briefing")}
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
        title={item.detail ? `${item.title} — ${item.detail}` : item.title}
        titleAttr={item.title}
        trailing={
          <Button onClick={() => onOpenItem(item)} type="button" variant="ghost">
            {text("Review")}
          </Button>
        }
      />
    );
  }

  return (
    <section aria-labelledby="today-briefing-title" className="today-briefing-card">
      <SectionHeader actions={generateButton} title={text("Morning briefing")} titleId="today-briefing-title" />
      {loading ? (
        <Skeleton count={2} label={text("Loading the morning briefing…")} variant="list-row" />
      ) : !briefing ? (
        <EmptyState>{text("No briefing yet. Generate one to see what's changed.")}</EmptyState>
      ) : briefing.items.length > 0 ? (
        <ul className="today-briefing-items ui-list-rows">{briefing.items.map((item) => itemRow(item))}</ul>
      ) : (
        <EmptyState>{text("Nothing to report since your last briefing.")}</EmptyState>
      )}
    </section>
  );
}
