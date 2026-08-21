import { Button } from "../../../ui";

export type DeltaAction = { label: string; onClick: () => void };

export type DeltaHeaderProps = {
  /** Mono eyebrow line, e.g. "DZIŚ · CZWARTEK 21 SIERPNIA 2026 · 07:25". */
  eyebrow: string;
  /** `formatDeltaHeadline`'s sentence, or the clean-morning copy. */
  headline: string;
  /** Optional secondary muted line (e.g. the media-count note). */
  note?: string | null;
  /** The CTA slot (plan decision 8/§6 — the screen's ONE primary action, the
   * most-urgent `pickPrimary` candidate). `null`/omitted on a clean morning
   * — no primary action is deliberate, not a loading state. */
  primaryAction?: DeltaAction | null;
  /** A quiet secondary action alongside it (e.g. "Otwórz Inbox (26)"). */
  secondaryAction?: DeltaAction | null;
};

/** The Dziś v2 header (F2 S3): eyebrow + delta sentence + the screen's one
 * primary CTA. Carries `data-ux-primary-action` through to the primary
 * button so the interaction-hierarchy contract (ADR 0081 Q4) can assert it. */
export function DeltaHeader({ eyebrow, headline, note, primaryAction, secondaryAction }: DeltaHeaderProps) {
  return (
    <div className="dayq-delta-header">
      <span className="dayq-delta-eyebrow">{eyebrow}</span>
      <h2 className="dayq-delta-headline">{headline}</h2>
      {note ? <p className="dayq-delta-note">{note}</p> : null}
      {primaryAction || secondaryAction ? (
        <div className="dayq-delta-actions">
          {primaryAction ? (
            <Button
              variant="primary"
              type="button"
              data-ux-primary-action="true"
              onClick={primaryAction.onClick}
            >
              {primaryAction.label}
            </Button>
          ) : null}
          {secondaryAction ? (
            <Button variant="secondary" type="button" onClick={secondaryAction.onClick}>
              {secondaryAction.label}
            </Button>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
