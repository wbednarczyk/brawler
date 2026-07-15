import { useMemo } from "react";
import { Sparkles } from "lucide-react";

import type { MorningBriefing } from "../../api/briefing";
import type { MorningBriefingItem } from "../../api/generated/MorningBriefingItem";
import type { Company } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { formatListTimestamp } from "../../shared/format/datetime";
import { MarkdownNoteBody } from "../../shared/components/MarkdownNoteBody";
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

// The composer embeds citations as a bare bracketed `citationKey`, e.g. `[b1]`
// (ADR 0068 T5 backend — never a markdown link), literally inside the
// narrative text. `MarkdownNoteBody` (reused as-is here for headings/lists/
// emphasis) has no notion of citations, so `[b1]` renders as plain text inside
// it; this card additionally extracts the cited keys, in first-appearance
// order, and renders them as a "Sources" list of click-through rows below the
// narrative — the item each key resolves to (via `MorningBriefingItem.
// citationKey`) is exactly the evidence the narrative is citing.
const CITATION_TOKEN = /\[(b\d+)\]/g;

function citationKeysInOrder(markdown: string): string[] {
  const seen = new Set<string>();
  const ordered: string[] = [];
  for (const match of markdown.matchAll(CITATION_TOKEN)) {
    const key = match[1];
    if (!seen.has(key)) {
      seen.add(key);
      ordered.push(key);
    }
  }
  return ordered;
}

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

  const itemsByCitationKey = useMemo(
    () => new Map((briefing?.items ?? []).map((item) => [item.citationKey, item])),
    [briefing],
  );

  const citedItems = useMemo(() => {
    if (!briefing?.narrativeMarkdown) return [];
    return citationKeysInOrder(briefing.narrativeMarkdown)
      .map((key) => itemsByCitationKey.get(key))
      .filter((item): item is MorningBriefingItem => Boolean(item));
  }, [briefing, itemsByCitationKey]);

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
      ) : briefing.narrativeMarkdown ? (
        <div className="today-briefing-narrative">
          <MarkdownNoteBody ariaLabel={text("Briefing narrative")} body={briefing.narrativeMarkdown} />
          {citedItems.length > 0 ? (
            <ul aria-label={text("Sources")} className="today-briefing-sources">
              {citedItems.map((item) => (
                <li key={item.id}>
                  <Button onClick={() => onOpenItem(item)} type="button" variant="ghost">
                    {item.title}
                  </Button>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      ) : briefing.items.length > 0 ? (
        <ul className="today-briefing-items ui-list-rows">{briefing.items.map((item) => itemRow(item))}</ul>
      ) : (
        <EmptyState>{text("Nothing to report since your last briefing.")}</EmptyState>
      )}
    </section>
  );
}
