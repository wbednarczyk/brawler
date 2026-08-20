import type { ReactNode } from "react";
import { StatusChip } from "../../../ui";
import { useLocale } from "../../locale";
import type { CompanySignal, FeedItem } from "../../../api/types";
import { FeedDetailFiling } from "./FeedDetailFiling";
import { FeedDetailGeneric } from "./FeedDetailGeneric";
import { FeedDetailMedia } from "./FeedDetailMedia";
import { FeedDetailReport } from "./FeedDetailReport";
import { FeedSignalsSection } from "./FeedSignalsSection";

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

type KindChip = { label: string; tone: "media" | "official" };

// Kind chip per presentation kind (mockup artboards InboxMedia/ESPI/Raport).
// `text(...)` calls stay literal here (not a lookup table indexed by a
// variable) so the translation-completeness ratchet's static scan can see them.
function kindChip(kind: FeedItem["presentationKind"], text: (value: string) => string): KindChip | null {
  switch (kind) {
    case "media":
      return { label: text("Media"), tone: "media" };
    case "filing":
      return { label: text("ESPI notice"), tone: "official" };
    case "report":
      return { label: text("Periodic report"), tone: "official" };
    default:
      return null;
  }
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
