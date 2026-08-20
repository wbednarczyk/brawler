import { Check, X } from "lucide-react";
import { ActionRow, Button, StatusChip } from "../../../ui";
import { useLocale } from "../../locale";
import type { CompanySignal } from "../../../api/types";

export type FeedSignalsSectionProps = {
  signals: CompanySignal[];
  onConfirmSignal: (signalId: string) => Promise<void> | void;
  onRejectSignal: (signalId: string) => Promise<void> | void;
};

// Typed ESPI/EBI signals (classify_filing) attached to a feed item — shared
// across every presentation kind that can carry them (filing/report today,
// plus the redFlag/unknown fallback) so the confirm/reject workflow never
// silently disappears behind a per-kind redesign.
export function FeedSignalsSection({ signals, onConfirmSignal, onRejectSignal }: FeedSignalsSectionProps) {
  const { text } = useLocale();
  if (signals.length === 0) return null;

  return (
    <section className="feed-body-section feed-signals-section" aria-label={text("Typed filing signals")}>
      <div className="feed-body-heading">
        <span>{text("Typed signals")}</span>
      </div>
      <ul className="feed-signals-list">
        {signals.map((signal) => (
          <li className="feed-signal-row" key={signal.id}>
            <div className="feed-signal-meta">
              <StatusChip tone={signal.status === "proposed" ? "warn" : "accent"}>
                {text(signal.categoryDisplayName)}
              </StatusChip>
              <span className="feed-signal-source">
                {signal.classifiedBy === "ai"
                  ? text("AI proposal — confirm to apply")
                  : signal.classifiedBy === "agent"
                    ? text("Agent-classified")
                    : text("Rule-classified")}
              </span>
            </div>
            {signal.status === "proposed" ? (
              <ActionRow className="feed-signal-actions" ariaLabel={text("Signal proposal actions")}>
                <Button
                  className="compact-button"
                  onClick={() => {
                    void onConfirmSignal(signal.id);
                  }}
                >
                  <Check size={15} />
                  {text("Confirm")}
                </Button>
                <Button
                  className="compact-button danger-subtle-button"
                  onClick={() => {
                    void onRejectSignal(signal.id);
                  }}
                >
                  <X size={15} />
                  {text("Reject")}
                </Button>
              </ActionRow>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
