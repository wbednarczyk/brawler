import { useState, type KeyboardEvent, type ReactNode } from "react";
import { ChevronRight, BellRing } from "lucide-react";

import { useLocale } from "../../../shared/locale";
import { formatListTimestamp } from "../../../shared/format/datetime";
import { TickerLabel } from "../../../shared/components/TickerLabel";
import { Button, StatusChip } from "../../../ui";
import type { Severity, StreamCategory } from "../streamModel";

/**
 * Shared anatomy for a Today attention-stream row (ADR 0087 + the D1 experience
 * contract §4/§6): severity left-border cue · leading icon · full ticker · a
 * severity chip (only when non-routine) · type badge · optional group ×N chip ·
 * full date · a 2-line title · exactly ONE `Przejrzyj` primary action. Every
 * category row and every group member composes this — no hand-rolled row shells.
 */

/** A category row reduced to render inputs; the model payload never reaches here typed. */
export type RowDescriptor = {
  key: string;
  icon: ReactNode;
  /** Full ticker (never truncated), or null to omit. */
  ticker: string | null;
  /** The category type badge (StatusChip). */
  typeBadge: ReactNode;
  /** Extra chips after the type badge (autopilot status, group count, …). */
  extraChips?: ReactNode;
  /**
   * A quiet indicator that THIS event exists because the user's own alert rule
   * fired (owner dogfooding 2026-07-23): a bell glyph in a fixed chip slot. On a
   * collapsed group/aggregate header it is set only when EVERY member is
   * rule-fired; expanded members carry their own. System events never set it.
   */
  ruleOrigin?: boolean;
  date: string | null;
  title: string;
  /** Optional dimmer second line (mockup `.sub`). */
  subtitle?: string | null;
  /**
   * A leading filename split off the document title (owner dogfooding
   * 2026-07-23): metadata, not prose, so it renders as a quiet secondary link
   * line rather than in the statement. `onOpen` reveals the document's surface.
   */
  documentLink?: { filename: string; onOpen: () => void } | null;
  onReview: () => void;
  /** A non-expand trailing control (e.g. attention Dismiss). */
  trailing?: ReactNode;
};

// Roving focus across the stream's primary actions (ADR 0076 U-Rb D4, extended
// to group members): ArrowUp/Down and j/k move; Enter/Space fire natively. It
// walks every `[data-today-row]` inside the enclosing `.today-stream` in DOM
// order, so an expanded group's members are traversed inline between rows.
export function onStreamRowKeyDown(event: KeyboardEvent<HTMLElement>) {
  const isDown = event.key === "ArrowDown" || event.key === "j";
  const isUp = event.key === "ArrowUp" || event.key === "k";
  if (!isDown && !isUp) return;
  event.preventDefault();
  const list = event.currentTarget.closest(".today-stream");
  const buttons = Array.from(list?.querySelectorAll<HTMLElement>('[data-today-row="true"]') ?? []);
  const currentIndex = buttons.indexOf(event.currentTarget);
  const nextIndex = Math.min(Math.max(currentIndex + (isDown ? 1 : -1), 0), buttons.length - 1);
  buttons[nextIndex]?.focus();
}

/** The row's single roving primary action (ADR 0081 §6: one per row). */
export function RowAction({ onClick, label }: { onClick: () => void; label?: string }) {
  const { text } = useLocale();
  return (
    <Button
      className="today-row-review"
      data-today-row="true"
      data-ux-primary-action="true"
      onClick={onClick}
      onKeyDown={onStreamRowKeyDown}
      type="button"
      variant="ghost"
    >
      {label ?? text("Review")}
    </Button>
  );
}

/** Severity cue chip — shown only for non-routine rows (routine is the dimmed baseline). */
export function SeverityChip({ severity }: { severity: Severity }) {
  const { text } = useLocale();
  if (severity === "urgent") {
    return (
      <StatusChip className="today-sev-chip" tone="danger">
        {text("Urgent")}
      </StatusChip>
    );
  }
  if (severity === "notable") {
    return (
      <StatusChip className="today-sev-chip" tone="warn">
        {text("Notable")}
      </StatusChip>
    );
  }
  return null;
}

function DisclosureButton({
  expanded,
  onToggle,
  label,
}: {
  expanded: boolean;
  onToggle: () => void;
  label: string;
}) {
  return (
    <button
      type="button"
      className={["today-row-disclosure", expanded ? "today-row-disclosure-open" : ""]
        .filter(Boolean)
        .join(" ")}
      aria-expanded={expanded}
      aria-label={label}
      onClick={onToggle}
    >
      <ChevronRight aria-hidden="true" size={15} />
    </button>
  );
}

export type StreamRowProps = {
  category: StreamCategory;
  severity: Severity;
  descriptor: RowDescriptor;
  /** ×N group chip node when this row heads a collapsed group. */
  groupChip?: ReactNode;
  /** Content revealed by the disclosure chevron (autopilot detail OR group members). */
  expandable?: ReactNode;
  /** Accessible label for the disclosure toggle. */
  disclosureLabel?: string;
  /** `row` = top-level `<li>`; `member` = compact `<div>` inside a group. */
  variant?: "row" | "member";
};

/** Renders one row (or one group member) from a descriptor. */
export function StreamRow({
  category,
  severity,
  descriptor,
  groupChip,
  expandable,
  disclosureLabel,
  variant = "row",
}: StreamRowProps) {
  const { text, locale } = useLocale();
  const [expanded, setExpanded] = useState(false);
  const isMember = variant === "member";

  const body = (
    <>
      <div className="today-row-line">
        <span className="today-row-icon">{descriptor.icon}</span>
        <div className="today-row-body">
          <div className="today-row-head">
            {descriptor.ticker ? <TickerLabel value={descriptor.ticker} /> : null}
            <SeverityChip severity={severity} />
            {descriptor.typeBadge}
            {groupChip}
            {descriptor.extraChips}
            {descriptor.ruleOrigin ? (
              <StatusChip
                className="today-rule-origin"
                tone="accent"
                role="img"
                ariaLabel={text("From your alert rule")}
                title={text("From your alert rule")}
              >
                <BellRing aria-hidden="true" size={11} />
              </StatusChip>
            ) : null}
            <span className="today-row-date num-tabular">
              {formatListTimestamp(descriptor.date, locale)}
            </span>
          </div>
          <p className="today-row-title" title={descriptor.title}>
            {descriptor.title}
          </p>
          {descriptor.subtitle ? <p className="today-row-sub">{descriptor.subtitle}</p> : null}
          {descriptor.documentLink ? (
            <Button
              className="today-row-doc-link"
              variant="minimal"
              type="button"
              onClick={descriptor.documentLink.onOpen}
              aria-label={`${text("Open document")}: ${descriptor.documentLink.filename}`}
              title={descriptor.documentLink.filename}
            >
              {descriptor.documentLink.filename}
            </Button>
          ) : null}
        </div>
        <div className="today-row-actions">
          <RowAction onClick={descriptor.onReview} />
          {descriptor.trailing}
          {expandable ? (
            <DisclosureButton
              expanded={expanded}
              onToggle={() => setExpanded((prior) => !prior)}
              label={disclosureLabel ?? text("Details")}
            />
          ) : null}
        </div>
      </div>
      {expandable && expanded ? <div className="today-row-expand">{expandable}</div> : null}
    </>
  );

  const className = [
    "today-stream-row",
    `today-sev-${severity}`,
    isMember ? "today-stream-member" : "",
  ]
    .filter(Boolean)
    .join(" ");

  if (isMember) {
    return (
      <div className={className} data-member-category={category} data-severity={severity}>
        {body}
      </div>
    );
  }
  return (
    <li className={className} data-category={category} data-severity={severity}>
      {body}
    </li>
  );
}
