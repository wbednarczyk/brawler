import type { ReactNode } from "react";

export type ProvenanceFigureProps = {
  label: ReactNode;
  /** The figure itself (already formatted). Rendered with a dotted provenance underline (ADR 0104 dec. 7). */
  value: ReactNode;
  /**
   * The source ticket under the figure (document/channel/period, mono-style
   * small text) — the thread's landing point. Omit when no source is known
   * yet (the underline still marks provenance intent, just without a ticket).
   */
  sourceTicket?: ReactNode;
  /**
   * When provided, the ticket becomes the thread's landing action (ADR 0104
   * dec. 7 — "a thread without navigation is a defect") and renders as a
   * button instead of static text. Omit when the host has nowhere to send the
   * click — the ticket then stays plain text, never a dead button.
   */
  onSourceTicketClick?: () => void;
  className?: string;
};

// A thesis figure sitting on the signature "provenance thread" (ADR 0104 dec. 7):
// a dotted underline in the official-provenance tone, ending in a small source
// ticket. Only for figures the reader treats as a claim (facts with a document
// behind them) — never decorate an auxiliary number with this.
export function ProvenanceFigure({
  label,
  value,
  sourceTicket,
  onSourceTicketClick,
  className,
}: ProvenanceFigureProps) {
  return (
    <div className={["ui-provenance-figure", className].filter(Boolean).join(" ")}>
      <span className="ui-provenance-figure-label">{label}</span>
      <span className="ui-provenance-figure-value num-tabular">{value}</span>
      {sourceTicket && onSourceTicketClick ? (
        <button className="ui-provenance-figure-ticket" onClick={onSourceTicketClick} type="button">
          {sourceTicket}
        </button>
      ) : sourceTicket ? (
        <span className="ui-provenance-figure-ticket">{sourceTicket}</span>
      ) : null}
    </div>
  );
}
