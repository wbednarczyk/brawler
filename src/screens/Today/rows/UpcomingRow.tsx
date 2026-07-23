import { CalendarClock } from "lucide-react";

import type { ReportSeasonEntry } from "../../../api/generated/ReportSeasonEntry";
import { StatusChip } from "../../../ui";
import type { Severity } from "../streamModel";
import type { RowContext } from "./rowContext";
import { StreamRow, type RowDescriptor } from "./streamRowKit";

/** The upcoming-report row descriptor. */
export function upcomingDescriptor(entry: ReportSeasonEntry, ctx: RowContext): RowDescriptor {
  return {
    key: `${entry.companyId}::${entry.eventKey}`,
    icon: <CalendarClock aria-hidden="true" size={15} />,
    ticker: entry.qualifiedTicker,
    typeBadge: <StatusChip tone="neutral">{ctx.text("Upcoming")}</StatusChip>,
    date: entry.eventDate,
    title: entry.displayName,
    onReview: () => ctx.openCompanyWorkspace(entry.companyId, "Feed"),
  };
}

/** A single (ungrouped) upcoming-report stream row. */
export function UpcomingRow({
  entry,
  severity,
  ctx,
  variant = "row",
}: {
  entry: ReportSeasonEntry;
  severity: Severity;
  ctx: RowContext;
  variant?: "row" | "member";
}) {
  return (
    <StreamRow
      category="upcoming"
      severity={severity}
      descriptor={upcomingDescriptor(entry, ctx)}
      variant={variant}
    />
  );
}
