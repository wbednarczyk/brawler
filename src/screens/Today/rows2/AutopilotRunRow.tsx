import { Cpu } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { splitDocumentTitle } from "../documentTitle";
import { RowShell } from "./RowShell";

export type AutopilotRunItem = Extract<TodayItem, { kind: "autopilotRun" }>;

/**
 * `AutopilotRun` carries `companyId` but no `qualifiedTicker` (unlike every
 * other `TodayItem` kind, whose company data is already inlined) — the
 * caller resolves it, same as `AttentionRow`'s ticker.
 */
export function AutopilotRunRow({
  item,
  qualifiedTicker,
  onOpen,
}: {
  item: AutopilotRunItem;
  qualifiedTicker: string | null;
  onOpen: () => void;
}) {
  const { text } = useLocale();
  // Same anti-filename treatment as `FilingRow` (ADR 0091): a produced
  // report's title may carry a glued filename.
  const { statement, filename } = splitDocumentTitle(item.run.reportDocumentTitle);

  return (
    <RowShell
      icon={<Cpu aria-hidden="true" size={18} />}
      ticker={qualifiedTicker}
      chip={
        // A failed run must be visibly failed ON ITS OWN ROW (ADR 0091 dec. 3)
        // — never only via the paired attention event, which lives in a
        // separate root-fed subsystem.
        item.run.status === "failed" ? (
          <StatusChip className="dayq-chip" tone="danger">
            {text("Autopilot failed")}
          </StatusChip>
        ) : (
          <StatusChip className="dayq-chip" tone="accent">
            {text("Autopilot")}
          </StatusChip>
        )
      }
      title={statement ?? text("Autopilot finished")}
      meta={filename}
      actionLabel={text("Read report")}
      onAction={onOpen}
    />
  );
}
