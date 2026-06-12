import type { ReactNode } from "react";

export type StatusPillTone = "neutral" | "ok" | "warn" | "danger";

export type StatusPillProps = {
  children: ReactNode;
  tone?: StatusPillTone;
};

export function StatusPill({ children, tone = "neutral" }: StatusPillProps) {
  return (
    <span className={["ui-status-pill", tone === "neutral" ? "" : `ui-status-pill-${tone}`].filter(Boolean).join(" ")}>
      {children}
    </span>
  );
}
