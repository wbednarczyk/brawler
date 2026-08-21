import { CheckCircle2 } from "lucide-react";

import type { TodayClaim } from "../../../api/generated/TodayClaim";
import { useLocale } from "../../../shared/locale";
import { StatusChip } from "../../../ui";
import { RowShell } from "./RowShell";

/** "DO WERYFIKACJI" row (Main.dc.html) — lands on the claim itself, highlighted
 * in the company's Claims panel (`openCompanyClaims` seam). */
export function ClaimRow({ entry, onOpen }: { entry: TodayClaim; onOpen: () => void }) {
  const { text } = useLocale();
  const { claim } = entry;
  const period =
    claim.dueFiscalYear && claim.duePeriodType ? `${claim.duePeriodType} ${claim.dueFiscalYear}` : null;

  return (
    <RowShell
      icon={<CheckCircle2 aria-hidden="true" size={18} />}
      ticker={entry.qualifiedTicker}
      chip={
        <StatusChip className="dayq-chip" tone="neutral">
          {period ? `${text("Thesis")} · ${period}` : text("Thesis")}
        </StatusChip>
      }
      title={claim.statement}
      actionLabel={text("Open thesis")}
      onAction={onOpen}
      titleThread
    />
  );
}
