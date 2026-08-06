import { useState } from "react";
import { BellRing } from "lucide-react";

import type { AttentionEvent } from "../../../api/attention";
import { Button, ClearButton, InlineConfirm, StatusChip } from "../../../ui";
import { attentionEventBadgeText, attentionEventTitleText } from "../attentionEventLabels";
import { splitDocumentTitle } from "../documentTitle";
import type { Severity } from "../streamModel";
import type { RowContext, StreamPayload } from "./rowContext";
import { StreamRow, type RowDescriptor } from "./streamRowKit";

/**
 * Evidence click-through for a fired attention event (ADR 0068 T4): mark it seen,
 * then jump to its evidence surface — a signal opens the company Feed, a
 * reconciliation opens the missed report itself in the system browser (ADR 0097
 * dec. 8: the report is NEVER in the feed — witness items are not ingested, so
 * feed navigation cannot reach it), an autopilot-run/daily-quote opens
 * Fundamentals; an evidence company outside the registry falls back to the
 * Inbox. Shared with the briefing strip.
 */
export function openAttentionEvidence(event: AttentionEvent, ctx: RowContext) {
  ctx.attention.markAttentionEventSeenRow(event.id);
  // The missed report's own URL — the only surface that actually shows it.
  if (event.evidenceType === "source_reconciliation" && event.witnessUrl) {
    ctx.openExternalUrl(event.witnessUrl);
    return;
  }
  // A system event may carry NO company at all (a workspace-wide background job
  // that failed, ADR 0091 dec. 2) — there is no company workspace to open, so it
  // resolves like an out-of-registry evidence company: the Inbox.
  const company = event.companyId ? ctx.companyById.get(event.companyId) : undefined;
  if (!company) {
    ctx.openInbox();
    return;
  }
  switch (event.evidenceType) {
    // `source_reconciliation` here = a legacy row without a stored URL: the
    // company Feed is the best remaining anchor (company context, not the
    // report itself).
    case "company_signal":
    case "source_reconciliation":
      ctx.openCompanyWorkspace(company.id, "Feed");
      return;
    case "autopilot_run":
    case "daily_quote":
      ctx.openCompanyWorkspace(company.id, "Fundamentals");
      return;
    default:
      ctx.openInbox();
  }
}

/**
 * The attention-event row descriptor (shared by the single row and a grouped
 * member). `readOnly` (the Archive view, owner 2026-07-23) drops the trailing
 * Dismiss control — an archived event is already dismissed, nothing to acknowledge
 * — while keeping the full row anatomy (severity chip, title, alert-origin bell)
 * and Review as pure navigation.
 */
export function attentionDescriptor(
  event: AttentionEvent,
  ctx: RowContext,
  options?: { readOnly?: boolean },
): RowDescriptor {
  const rule = event.ruleId ? ctx.attention.attentionRulesById.get(event.ruleId) : undefined;
  const company = event.companyId ? ctx.companyById.get(event.companyId) : undefined;
  // A report-document evidence title may be a filename glued onto the human
  // title. Split it: the human statement flows into the composed sentence; the
  // filename moves to a quiet secondary link line. A
  // filename-only title leaves `statement` null, so the composer falls back to its
  // generic copy — mirroring the autopilot row.
  const { statement, filename } = splitDocumentTitle(event.evidenceTitle);
  const titleEvent = filename ? { ...event, evidenceTitle: statement } : event;
  return {
    key: event.id,
    icon: <BellRing aria-hidden="true" size={15} />,
    ticker: company?.qualifiedTicker ?? event.companyId,
    typeBadge: <StatusChip tone="warn">{attentionEventBadgeText(event, ctx.text)}</StatusChip>,
    date: event.firedAt,
    title: attentionEventTitleText(titleEvent, rule, ctx.text),
    // The event exists because the user's own alert rule fired (non-null ruleId);
    // system-raised events (reconciliation) have no rule and stay unmarked.
    ruleOrigin: event.ruleId != null,
    documentLink: filename ? { filename, onOpen: () => openAttentionEvidence(event, ctx) } : null,
    onReview: () => openAttentionEvidence(event, ctx),
    trailing: options?.readOnly ? undefined : (
      <ClearButton
        label={ctx.text("Dismiss")}
        onClick={() => ctx.attention.dismissAttentionEventRow(event.id)}
      />
    ),
  };
}

/** A single read-only archived (dismissed) attention row — no Dismiss control. */
export function ArchivedAttentionRow({ event, ctx }: { event: AttentionEvent; ctx: RowContext }) {
  return (
    <StreamRow
      category="attention"
      severity={event.severity}
      descriptor={attentionDescriptor(event, ctx, { readOnly: true })}
      variant="row"
    />
  );
}

/**
 * Does EVERY member payload come from a user alert rule? True only for a non-empty
 * set of attention events that all carry a non-null `ruleId`. A collapsed
 * group/aggregate header shows the alert-rule indicator only when this holds — a
 * single system member (reconciliation) suppresses it.
 */
export function everyMemberFromAlertRule(payloads: StreamPayload[]): boolean {
  return (
    payloads.length > 0 &&
    payloads.every((payload) => payload.kind === "attention" && payload.event.ruleId != null)
  );
}

/** Attention-event ids among a group/aggregate's member payloads (non-attention ignored). */
export function attentionEventIds(payloads: StreamPayload[]): string[] {
  return payloads
    .filter((payload): payload is Extract<StreamPayload, { kind: "attention" }> =>
      payload.kind === "attention",
    )
    .map((payload) => payload.event.id);
}

/**
 * The group-level "Dismiss all" secondary action (ADR 0087 amendment 2026-07-23):
 * a two-step InlineConfirm (like Undo) that dispatches the existing per-event
 * dismiss for every member — optimistic, and the toast wiring syncs off the same
 * per-event dismiss. Only rendered for attention groups/aggregates (there is
 * nothing to bulk-dismiss otherwise).
 */
export function AttentionDismissAll({ eventIds, ctx }: { eventIds: string[]; ctx: RowContext }) {
  const [confirming, setConfirming] = useState(false);
  if (eventIds.length === 0) return null;
  return (
    <div className="today-group-dismiss-all">
      {confirming ? (
        <InlineConfirm
          cancelLabel={ctx.text("Cancel")}
          confirmLabel={ctx.text("Dismiss all")}
          onCancel={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            for (const id of eventIds) ctx.attention.dismissAttentionEventRow(id);
          }}
        >
          {ctx.text("Dismiss all alerts in this group?")}
        </InlineConfirm>
      ) : (
        <Button onClick={() => setConfirming(true)} type="button" variant="ghost">
          {ctx.text("Dismiss all")}
        </Button>
      )}
    </div>
  );
}

/** A single (ungrouped) fired-alert stream row. */
export function AttentionRow({
  event,
  severity,
  ctx,
  variant = "row",
}: {
  event: AttentionEvent;
  severity: Severity;
  ctx: RowContext;
  variant?: "row" | "member";
}) {
  return (
    <StreamRow
      category="attention"
      severity={severity}
      descriptor={attentionDescriptor(event, ctx)}
      variant={variant}
    />
  );
}
