import type { StreamItem } from "../streamModel";
import type { RowContext, StreamPayload } from "./rowContext";
import type { RowDescriptor } from "./streamRowKit";
import { AutopilotRow, autopilotDescriptor } from "./AutopilotRow";
import { AttentionRow, attentionDescriptor } from "./AttentionRow";
import { VerifyRow, verifyDescriptor } from "./VerifyRow";
import { ChangedRow, changedDescriptor } from "./ChangedRow";
import { UpcomingRow, upcomingDescriptor } from "./UpcomingRow";

/** The descriptor for any category payload (used for a group's header row). */
export function describePayload(payload: StreamPayload, ctx: RowContext): RowDescriptor {
  switch (payload.kind) {
    case "autopilot":
      return autopilotDescriptor(payload.run, ctx);
    case "attention":
      return attentionDescriptor(payload.event, ctx);
    case "verify":
      return verifyDescriptor(payload.claim, ctx);
    case "changed":
      return changedDescriptor(payload.item, ctx);
    case "upcoming":
      return upcomingDescriptor(payload.entry, ctx);
  }
}

/** Renders one stream item as its category's row (or a compact group member). */
export function StreamRowView({
  item,
  ctx,
  variant = "row",
}: {
  item: StreamItem<StreamPayload>;
  ctx: RowContext;
  variant?: "row" | "member";
}) {
  const { payload, severity } = item;
  switch (payload.kind) {
    case "autopilot":
      return <AutopilotRow run={payload.run} severity={severity} ctx={ctx} variant={variant} />;
    case "attention":
      return <AttentionRow event={payload.event} severity={severity} ctx={ctx} variant={variant} />;
    case "verify":
      return <VerifyRow item={payload.claim} severity={severity} ctx={ctx} variant={variant} />;
    case "changed":
      return <ChangedRow item={payload.item} severity={severity} ctx={ctx} variant={variant} />;
    case "upcoming":
      return <UpcomingRow entry={payload.entry} severity={severity} ctx={ctx} variant={variant} />;
  }
}
