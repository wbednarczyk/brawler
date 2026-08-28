import { useState } from "react";

import type { Company } from "../../../api/types";
import type { RedFlag, RedFlagsView } from "../../../api/redFlags";
import { TickerLabel } from "../../../shared/components/TickerLabel";
import { useLocale } from "../../../shared/locale";
import {
  Button,
  EmptyState,
  ErrorText,
  ExpandableRow,
  Hint,
  InlineConfirm,
  SectionHeader,
  StatusChip,
} from "../../../ui";

// The company-scoped red-flags panel (v0.57 T7, ADR 0083 Decision 8/9):
// active flags with a fixed-slot severity chip, a per-row acknowledge (inline
// confirm), and a collapsed acknowledged-history group. Decision support only —
// the app raises "something smells here" from reports, ownership, health scores,
// the auditor opinion, and short selling; never advice language. A calm explicit
// empty state, never blank. `onOpenEvidence` selects the underlying feed item —
// the caller (the Spółka `sygnaly` tool) owns where that navigates.
export type RedFlagsSectionProps = {
  company: Company;
  view: RedFlagsView | null;
  error: string | null;
  onAcknowledge: (flagId: string) => void;
  onOpenEvidence?: (feedItemId: string) => void;
};

type ChipTone = "danger" | "warn" | "neutral";

function severityTone(severity: string): ChipTone {
  if (severity === "high") return "danger";
  if (severity === "medium") return "warn";
  return "neutral";
}

function severityLabel(severity: string, text: (value: string) => string): string {
  if (severity === "high") return text("High");
  if (severity === "medium") return text("Medium");
  return severity;
}

function flagTypeLabel(flagType: string, text: (value: string) => string): string {
  switch (flagType) {
    case "auditor_red_flag":
      return text("Auditor red flag");
    case "report_delay":
      return text("Report delay");
    case "fund_exit":
      return text("Fund exit");
    case "score_deterioration":
      return text("Score deterioration");
    case "short_spike":
      return text("Short-selling spike");
    default:
      return flagType;
  }
}

export function RedFlagsSection({
  company,
  view,
  error,
  onAcknowledge,
  onOpenEvidence,
}: RedFlagsSectionProps) {
  const { text } = useLocale();
  const [confirmingId, setConfirmingId] = useState<string | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);

  const active = view?.active ?? [];
  const history = view?.history ?? [];

  function renderActiveRow(flag: RedFlag) {
    const confirming = confirmingId === flag.flagId;
    return (
      <li key={flag.flagId} className="red-flags-row">
        <span className="red-flags-severity-slot">
          <StatusChip tone={severityTone(flag.severity)}>
            {severityLabel(flag.severity, text)}
          </StatusChip>
        </span>
        <span className="red-flags-row-main">
          <span className="red-flags-type">{flagTypeLabel(flag.flagType, text)}</span>
          <span className="red-flags-title">{flag.title}</span>
        </span>
        <time className="red-flags-date num-tabular" dateTime={flag.raisedDate}>
          {flag.raisedDate}
        </time>
        <span className="red-flags-actions">
          {flag.evidenceFeedItemId && onOpenEvidence ? (
            <Button
              className="compact-button"
              onClick={() => onOpenEvidence(flag.evidenceFeedItemId as string)}
            >
              {text("Open")}
            </Button>
          ) : null}
          {confirming ? (
            <InlineConfirm
              confirmLabel={text("Yes")}
              cancelLabel={text("Cancel")}
              onConfirm={() => {
                onAcknowledge(flag.flagId);
                setConfirmingId(null);
              }}
              onCancel={() => setConfirmingId(null)}
            >
              {text("Acknowledge this flag?")}
            </InlineConfirm>
          ) : (
            <Button className="compact-button" onClick={() => setConfirmingId(flag.flagId)}>
              {text("Acknowledge")}
            </Button>
          )}
        </span>
      </li>
    );
  }

  return (
    <div className="company-tab-panel red-flags-panel" aria-label={text("Warning signals")}>
      <SectionHeader level="h3" paneLead title={text("Warning signals")} />
      <p className="red-flags-attr">
        {text("Derived signals for")} <TickerLabel value={company.qualifiedTicker} /> —{" "}
        {text("deteriorating health, delayed reports, fund exits, auditor opinion, short selling.")}
      </p>

      {error ? (
        <ErrorText>
          {text("Could not load warning signals")}: {error}
        </ErrorText>
      ) : null}

      {active.length === 0 ? (
        <EmptyState className="red-flags-empty" wrapText={false}>
          <span className="red-flags-empty-title">{text("No active warning signals")}</span>
          <span>
            {text(
              "Nothing to flag right now — the app watches reports, ownership, scores, the auditor, and short selling.",
            )}
          </span>
        </EmptyState>
      ) : (
        <>
          <SectionHeader level="h4" title={text("Active")} meta={active.length} />
          <ul className="red-flags-list" aria-label={text("Active")}>
            {active.map(renderActiveRow)}
          </ul>
        </>
      )}

      {history.length > 0 ? (
        <ExpandableRow
          className="red-flags-history-toggle"
          isExpanded={historyOpen}
          label={text("Acknowledged history")}
          onToggle={() => setHistoryOpen((open) => !open)}
          detail={
            <ul className="red-flags-history" aria-label={text("Acknowledged history")}>
              {history.map((flag) => (
                <li key={flag.flagId} className="red-flags-row red-flags-row-acked">
                  <span className="red-flags-severity-slot">
                    <StatusChip tone="neutral">{severityLabel(flag.severity, text)}</StatusChip>
                  </span>
                  <span className="red-flags-row-main">
                    <span className="red-flags-type">{flagTypeLabel(flag.flagType, text)}</span>
                    <span className="red-flags-title">{flag.title}</span>
                  </span>
                  {flag.ackedAt ? (
                    <time className="red-flags-date num-tabular" dateTime={flag.ackedAt}>
                      {flag.ackedAt.slice(0, 10)}
                    </time>
                  ) : null}
                </li>
              ))}
            </ul>
          }
        >
          <span className="red-flags-history-label">
            {text("Acknowledged history")} ({history.length})
          </span>
        </ExpandableRow>
      ) : null}

      <Hint className="red-flags-foot">
        {text(
          "Each raised flag also posts a signal in the company feed — attach an alert rule to be notified.",
        )}
      </Hint>
    </div>
  );
}
