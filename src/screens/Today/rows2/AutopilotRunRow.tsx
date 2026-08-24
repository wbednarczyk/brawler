import { useState } from "react";
import { Cpu } from "lucide-react";

import type { TodayItem } from "../../../api/generated/TodayItem";
import { useLocale } from "../../../shared/locale";
import { Button, InlineConfirm, StatusChip } from "../../../ui";
import { composeAutopilotRunSummary, isTokenizedSummary, renderAutopilotSummaryTokens } from "../autopilotRunSummary";
import { splitDocumentTitle } from "../documentTitle";
import { RowShell } from "./RowShell";

export type AutopilotRunItem = Extract<TodayItem, { kind: "autopilotRun" }>;

/**
 * `AutopilotRun` carries `companyId` but no `qualifiedTicker` (unlike every
 * other `TodayItem` kind, whose company data is already inlined) — the
 * caller resolves it, same as `AttentionRow`'s ticker. `onUndo`/`onDismiss`
 * are fix wave B finding 7 (ADR 0055 §4 recovery path, restored from the v1
 * row): `onUndo` is `null` when the run produced no facts (nothing to
 * revert) — the two-step `InlineConfirm` guards the destructive call.
 */
export function AutopilotRunRow({
  item,
  qualifiedTicker,
  onOpen,
  onUndo,
  onDismiss,
}: {
  item: AutopilotRunItem;
  qualifiedTicker: string | null;
  onOpen: () => void;
  onUndo: (() => void) | null;
  onDismiss: () => void;
}) {
  const { text, locale } = useLocale();
  // Same anti-filename treatment as `FilingRow` (ADR 0091): a produced
  // report's title may carry a glued filename.
  const { statement } = splitDocumentTitle(item.run.reportDocumentTitle);
  // The token-stream "what changed" summary as the row's meta line
  // (contracts.md § Autonomous Report Pipeline names `renderAutopilotSummaryTokens`
  // as the token-stream consumer; a legacy/non-token `summaryText` falls back
  // to the data-recomposed sentence, same router the v1 row used).
  const summary = isTokenizedSummary(item.run.summaryText)
    ? renderAutopilotSummaryTokens(item.run.summaryText, text, locale)
    : composeAutopilotRunSummary(item.run, text, locale);
  const [confirmingUndo, setConfirmingUndo] = useState(false);

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
      meta={summary}
      actionLabel={text("Read report")}
      onAction={onOpen}
      secondaryActions={
        confirmingUndo ? (
          <InlineConfirm
            cancelLabel={text("Cancel")}
            confirmLabel={text("Undo")}
            onCancel={() => setConfirmingUndo(false)}
            onConfirm={() => {
              setConfirmingUndo(false);
              onUndo?.();
            }}
          >
            {text("Undo this run and revert its facts?")}
          </InlineConfirm>
        ) : (
          <>
            {onUndo ? (
              <Button variant="ghost" type="button" onClick={() => setConfirmingUndo(true)}>
                {text("Undo")}
              </Button>
            ) : null}
            <Button variant="ghost" type="button" onClick={onDismiss}>
              {text("Mark as read")}
            </Button>
          </>
        )
      }
    />
  );
}
