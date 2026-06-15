import { Tag } from "lucide-react";
import type { CompanySignal, CompanySignalCategory } from "../../api/types";
import { useLocale } from "../../shared/locale";
import { StatusChip } from "../../ui";

type StatusChipTone = "neutral" | "accent" | "ok" | "warn" | "danger";

// Category → badge tone. Proposed (AI) signals always render in the "warn" tone
// regardless of category, so an unconfirmed type is visually distinct.
const CATEGORY_TONE: Record<CompanySignalCategory, StatusChipTone> = {
  insider_transaction: "accent",
  dividend: "ok",
  profit_warning: "danger",
  significant_contract: "accent",
  own_shares: "accent",
  guidance_change: "warn",
  general_meeting: "neutral",
  other: "neutral",
};

type SignalBadgesProps = {
  signals: CompanySignal[] | undefined;
};

export function SignalBadges({ signals }: SignalBadgesProps) {
  const { text } = useLocale();

  if (!signals || signals.length === 0) {
    return null;
  }

  return (
    <div className="feed-row-signals" aria-label={text("Typed filing signals")}>
      {signals.map((signal) => {
        const proposed = signal.status === "proposed";
        const tone: StatusChipTone = proposed ? "warn" : CATEGORY_TONE[signal.category];
        const label = text(signal.categoryDisplayName);

        return (
          <StatusChip key={signal.id} tone={tone} className="signal-badge">
            <Tag size={11} aria-hidden />
            {proposed ? `${label} · ${text("Proposed")}` : label}
          </StatusChip>
        );
      })}
    </div>
  );
}
