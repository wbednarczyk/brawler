import { CheckCircle2 } from "lucide-react";

import { StatusChip } from "../../../ui";
import type { PulseClaim } from "../useTodayPulse";
import type { Severity } from "../streamModel";
import type { RowContext } from "./rowContext";
import { StreamRow, type RowDescriptor } from "./streamRowKit";

/** The claim-to-verify row descriptor (shared by the single row and a grouped member). */
export function verifyDescriptor(item: PulseClaim, ctx: RowContext): RowDescriptor {
  return {
    key: item.claim.id,
    icon: <CheckCircle2 aria-hidden="true" size={15} />,
    ticker: item.qualifiedTicker,
    typeBadge: (
      <StatusChip tone={item.bucket === "overdue" ? "danger" : "warn"}>
        {item.bucket === "overdue" ? ctx.text("Overdue") : ctx.text("Due")}
      </StatusChip>
    ),
    date: item.claim.madeAt ?? item.claim.createdAt,
    title: item.claim.statement,
    onReview: () => ctx.openCompanyWorkspace(item.companyId, "Claims"),
  };
}

/** A single (ungrouped) claim-to-verify stream row. */
export function VerifyRow({
  item,
  severity,
  ctx,
  variant = "row",
}: {
  item: PulseClaim;
  severity: Severity;
  ctx: RowContext;
  variant?: "row" | "member";
}) {
  return (
    <StreamRow
      category="verify"
      severity={severity}
      descriptor={verifyDescriptor(item, ctx)}
      variant={variant}
    />
  );
}
