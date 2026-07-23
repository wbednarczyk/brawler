import { FileText } from "lucide-react";

import type { FeedItem } from "../../../api/types";
import { StatusChip } from "../../../ui";
import type { Severity } from "../streamModel";
import type { RowContext } from "./rowContext";
import { StreamRow, type RowDescriptor } from "./streamRowKit";

/** The "what changed" report-disclosure row descriptor. */
export function changedDescriptor(item: FeedItem, ctx: RowContext): RowDescriptor {
  const company = ctx.companyByTicker.get(item.company);
  return {
    key: item.id,
    icon: <FileText aria-hidden="true" size={15} />,
    ticker: item.company,
    typeBadge: <StatusChip tone="official">{ctx.text("Report")}</StatusChip>,
    date: item.publishedAt || item.time,
    title: item.title,
    onReview: () =>
      company ? ctx.openCompanyWorkspace(company.id, "Feed") : ctx.openInbox(),
  };
}

/** A single (ungrouped) new-report-disclosure stream row. */
export function ChangedRow({
  item,
  severity,
  ctx,
  variant = "row",
}: {
  item: FeedItem;
  severity: Severity;
  ctx: RowContext;
  variant?: "row" | "member";
}) {
  return (
    <StreamRow
      category="changed"
      severity={severity}
      descriptor={changedDescriptor(item, ctx)}
      variant={variant}
    />
  );
}
