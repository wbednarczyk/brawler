import type { ReactNode } from "react";

type StatusChipTone = "neutral" | "accent" | "ok" | "warn" | "danger";

type StatusChipProps = {
  children: ReactNode;
  className?: string;
  tone?: StatusChipTone;
};

export function StatusChip({ children, className, tone = "neutral" }: StatusChipProps) {
  return (
    <span className={["ui-status-chip", `ui-status-chip-${tone}`, className].filter(Boolean).join(" ")}>
      {children}
    </span>
  );
}
