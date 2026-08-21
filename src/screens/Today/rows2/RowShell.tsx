import type { KeyboardEvent, ReactNode } from "react";

import { TickerLabel } from "../../../shared/components/TickerLabel";
import { Button } from "../../../ui";

// Roving focus across the queue's row actions (ADR 0076 U-Rb D4, carried over
// from the v1 stream kit — losing it in the F2 rebuild was a keyboard
// regression): ArrowUp/Down and j/k move; Enter/Space fire natively. Walks
// every `[data-dayq-action]` inside the screen body in DOM order, so focus
// crosses day-section boundaries inline.
function onRowActionKeyDown(event: KeyboardEvent<HTMLElement>) {
  const isDown = event.key === "ArrowDown" || event.key === "j";
  const isUp = event.key === "ArrowUp" || event.key === "k";
  if (!isDown && !isUp) return;
  event.preventDefault();
  const list = event.currentTarget.closest(".dayq-screen-body");
  const buttons = Array.from(list?.querySelectorAll<HTMLElement>('[data-dayq-action="true"]') ?? []);
  const currentIndex = buttons.indexOf(event.currentTarget);
  const nextIndex = Math.min(Math.max(currentIndex + (isDown ? 1 : -1), 0), buttons.length - 1);
  buttons[nextIndex]?.focus();
}

/**
 * Shared row anatomy for the Dziś v2 day queue (F2 S3, screen-local — plan
 * decision 9, NOT a `src/ui` primitive). Matches Wiersze.dc.html: leading
 * icon · ticker · a quiet provenance chip · a human title · optional mono
 * meta · exactly ONE quiet action button naming its destination. Every
 * per-kind row (`FilingRow`, `MediaClusterRow`, …) composes this — no
 * hand-rolled row shells.
 */
export type RowShellProps = {
  /** A stable identifier for browser-test/DOM lookup of THIS row (e.g. the
   * feed item id) — never used for React `key` (callers own that). */
  id?: string;
  icon: ReactNode;
  /** Full qualified ticker, or `null` for a system-wide row with no company
   * (an attention event outside the registry). */
  ticker: string | null;
  /** The provenance/status chip (StatusChip) — official cyan / media magenta /
   * agent violet / caution amber per ADR 0104. */
  chip: ReactNode;
  title: string;
  /** A quiet mono metadata line under the title (e.g. an event date). */
  meta?: string | null;
  /** Omitted entirely for a row with no landing destination (a calendar
   * "zapowiedź" row — Wiersze.dc.html "bez akcji — czeka"). */
  actionLabel?: string;
  onAction?: () => void;
  /** Bold (700) title weight for the header's primary-candidate row; the
   * default (500) otherwise (plan decision 8's "waga tytułu" expression). */
  emphasis?: boolean;
  /** The severity outline ring — the OTHER half of decision 8's expression
   * (no extra color, just weight + obrys). */
  accent?: boolean;
  /** The signature provenance-thread underline (ADR 0104) — thesis/claim
   * titles only. */
  titleThread?: boolean;
};

export function RowShell({
  id,
  icon,
  ticker,
  chip,
  title,
  meta,
  actionLabel,
  onAction,
  emphasis,
  accent,
  titleThread,
}: RowShellProps) {
  return (
    <div
      className={["dayq-row", accent ? "dayq-row-accent" : ""].filter(Boolean).join(" ")}
      data-dayq-row="true"
      data-dayq-row-id={id}
    >
      <span className="dayq-row-icon" aria-hidden="true">
        {icon}
      </span>
      <div className="dayq-row-body">
        <div className="dayq-row-head">
          {ticker ? <TickerLabel value={ticker} /> : null}
          {chip}
        </div>
        <p
          className={["dayq-row-title", emphasis ? "dayq-row-title-strong" : "", titleThread ? "dayq-row-title-thread" : ""]
            .filter(Boolean)
            .join(" ")}
        >
          {title}
        </p>
        {meta ? <p className="dayq-row-meta">{meta}</p> : null}
      </div>
      {actionLabel && onAction ? (
        <Button
          className="dayq-row-action"
          data-dayq-action="true"
          variant="ghost"
          type="button"
          onClick={onAction}
          onKeyDown={onRowActionKeyDown}
        >
          {actionLabel}
        </Button>
      ) : null}
    </div>
  );
}
