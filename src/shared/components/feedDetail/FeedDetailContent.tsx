import type { ReactNode } from "react";
import { StatusChip } from "../../../ui";
import { useLocale } from "../../locale";
import type { CompanySignal, FeedItem } from "../../../api/types";
import { FeedDetailFiling } from "./FeedDetailFiling";
import { FeedDetailGeneric } from "./FeedDetailGeneric";
import { FeedDetailMedia } from "./FeedDetailMedia";
import { FeedDetailReport } from "./FeedDetailReport";
import { FeedSignalsSection } from "./FeedSignalsSection";
import { feedKindChip, type FeedKindChip } from "./feedPresentation";

export type FeedDetailContentProps = {
  item: FeedItem;
  signals: CompanySignal[];
  // Primary/secondary action buttons, host-composed per kind (ADR 0081 Q4 —
  // the caller marks `data-ux-primary-action`, never inferred here).
  actions: ReactNode;
  feedItemSummary: (item: FeedItem) => string;
  formatTimestamp: (value: string | null | undefined, emptyLabel?: string) => string;
  onConfirmSignal: (signalId: string) => Promise<void> | void;
  onRejectSignal: (signalId: string) => Promise<void> | void;
};

// Kind chip per presentation kind (mockup artboards InboxMedia/ESPI/Raport),
// delegated to the host-neutral `feedPresentation` module (dogfooding #8) so
// every render site agrees on the label/tone. `redFlag` stays routed to the
// unchanged generic fallback below — unlike the row hosts, THIS dispatcher
// still needs a "kind unhandled here" signal (redFlag has no dedicated body
// component), not just a chip.
function kindChip(kind: FeedItem["presentationKind"], text: (value: string) => string): FeedKindChip | null {
  return kind === "redFlag" ? null : feedKindChip(kind, text);
}

// Dispatches the Inbox detail body by `presentationKind` (F1 S4, ADR 0104).
// media/filing/report get the redesigned host-neutral body; redFlag (and any
// future kind this switch doesn't yet cover) falls back to the pre-redesign
// generic rendering, unchanged.
export function FeedDetailContent({
  item,
  signals,
  actions,
  feedItemSummary,
  formatTimestamp,
  onConfirmSignal,
  onRejectSignal,
}: FeedDetailContentProps) {
  const { text } = useLocale();
  const chip = kindChip(item.presentationKind, text);

  if (!chip) {
    return (
      <FeedDetailGeneric
        item={item}
        signals={signals}
        actions={actions}
        feedItemSummary={feedItemSummary}
        formatTimestamp={formatTimestamp}
        onConfirmSignal={onConfirmSignal}
        onRejectSignal={onRejectSignal}
      />
    );
  }

  return (
    <>
      <div className="feed-detail-kind-row">
        <StatusChip tone={chip.tone}>{chip.label}</StatusChip>
        <span className="feed-detail-kind-source">{item.source}</span>
        <span className="feed-detail-kind-time num-tabular">
          {formatTimestamp(item.publishedAt, text("Unknown"))}
        </span>
      </div>
      <h2>{item.title}</h2>
      {item.presentationKind === "media" ? (
        <FeedDetailMedia item={item} feedItemSummary={feedItemSummary} />
      ) : item.presentationKind === "filing" ? (
        <FeedDetailFiling item={item} />
      ) : (
        <FeedDetailReport item={item} />
      )}
      {actions}
      <FeedSignalsSection signals={signals} onConfirmSignal={onConfirmSignal} onRejectSignal={onRejectSignal} />
    </>
  );
}
