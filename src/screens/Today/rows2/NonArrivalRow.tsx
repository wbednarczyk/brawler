import { AlertCircle } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { RowShell } from "./RowShell";

export type NonArrivalItem = Extract<TodayItem, { kind: "nonArrival" }>;

/**
 * A non-arrival row exists exactly while the shared `report_delay` predicate
 * says "missed" but the flag has not fired yet (plan decision 2) — its own
 * action is "Odśwież źródła" (Main.dc.html DNP row), the only lever the user
 * has while waiting.
 */
export function NonArrivalRow({ item, onOpen }: { item: NonArrivalItem; onOpen: () => void }) {
  const { text } = useLocale();

  return (
    <RowShell
      icon={<AlertCircle aria-hidden="true" size={18} />}
      ticker={item.qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="warn">
          {text("Not received")}
        </StatusChip>
      }
      title={item.title}
      actionLabel={text("Refresh sources")}
      onAction={onOpen}
      emphasis
      accent
    />
  );
}
