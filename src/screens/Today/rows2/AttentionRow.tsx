import { BellRing } from "lucide-react";

import type { AlertRule, AttentionEvent } from "../../../api/attention";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { attentionEventBadgeText, attentionEventTitleText } from "../attentionEventLabels";
import { RowShell } from "./RowShell";

/**
 * A root-fed attention event (ADR 0106 dec. 5) doclejed to its local day by
 * `dayQueueModel`. Reuses the existing badge/title composers
 * (`attentionEventLabels.ts`, shared with the Alerts stream) so the "what
 * fired" sentence stays identical everywhere — never re-derived here.
 * `qualifiedTicker`/`rule`/`actionLabel` are caller-resolved: the event DTO
 * carries neither a ticker nor its rule, and the landing destination varies
 * by `evidenceType` (mirrors the existing `openAttentionEvidence` routing,
 * owned by the screen wiring, not this row).
 */
export function AttentionRow({
  event,
  qualifiedTicker,
  rule,
  actionLabel,
  onOpen,
}: {
  event: AttentionEvent;
  qualifiedTicker: string | null;
  rule?: AlertRule;
  actionLabel: string;
  onOpen: () => void;
}) {
  const { text } = useLocale();

  return (
    <RowShell
      icon={<BellRing aria-hidden="true" size={18} />}
      ticker={qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="accent">
          {attentionEventBadgeText(event, text)}
        </StatusChip>
      }
      title={attentionEventTitleText(event, rule, text)}
      actionLabel={actionLabel}
      onAction={onOpen}
      emphasis={event.severity !== "routine"}
      accent={event.severity === "urgent"}
    />
  );
}
