import { BellRing } from "lucide-react";

import type { AlertRule, AttentionEvent } from "../../../api/attention";
import { useLocale } from "../../../shared/locale";
import { ClearButton, StatusChip } from "../../../ui";
import { attentionEventBadgeText, attentionEventTitleText } from "../attentionEventLabels";
import { splitDocumentTitle } from "../documentTitle";
import { RowShell } from "./RowShell";

/**
 * A root-fed attention event (ADR 0106 dec. 5) doclejed to its local day by
 * `dayQueueModel`. Reuses the existing badge/title composers
 * (`attentionEventLabels.ts`, shared with the Alerts stream) so the "what
 * fired" sentence stays identical everywhere — never re-derived here.
 * `qualifiedTicker`/`rule`/`actionLabel` are caller-resolved: the event DTO
 * carries neither a ticker nor its rule, and the landing destination varies
 * by `evidenceType` (mirrors the existing `openAttentionEvidence` routing,
 * owned by the screen wiring, not this row). `onDismiss` (fix wave B finding
 * 7, ADR 0097 dec. 5) is DISTINCT from "seen" — dismiss moves the event to
 * Archive; `null` for a read-only context (Archive itself has no dismiss).
 */
export function AttentionRow({
  event,
  qualifiedTicker,
  rule,
  actionLabel,
  onOpen,
  onDismiss = null,
}: {
  event: AttentionEvent;
  qualifiedTicker: string | null;
  rule?: AlertRule;
  actionLabel: string;
  onOpen: () => void;
  onDismiss?: (() => void) | null;
}) {
  const { text } = useLocale();
  // Same anti-filename treatment as `FilingRow` (ADR 0091): `evidenceTitle`
  // may carry a glued filename (a report-document evidence link).
  const { statement, filename } = splitDocumentTitle(event.evidenceTitle);
  const titleEvent = filename ? { ...event, evidenceTitle: statement } : event;

  return (
    <RowShell
      icon={<BellRing aria-hidden="true" size={18} />}
      ticker={qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="accent">
          {attentionEventBadgeText(event, text)}
        </StatusChip>
      }
      title={attentionEventTitleText(titleEvent, rule, text)}
      meta={filename ? <span className="dayq-row-meta-mono">{filename}</span> : null}
      actionLabel={actionLabel}
      onAction={onOpen}
      emphasis={event.severity !== "routine"}
      accent={event.severity === "urgent"}
      secondaryActions={onDismiss ? <ClearButton label={text("Dismiss")} onClick={onDismiss} /> : null}
    />
  );
}
