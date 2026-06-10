import type { ReactNode } from "react";

type StatusPillTone = "neutral" | "ok" | "warn" | "danger";

type StatusPillProps = {
  children: ReactNode;
  tone?: StatusPillTone;
};

export function StatusPill({ children, tone = "neutral" }: StatusPillProps) {
  return <span className={["membership-chip", tone === "neutral" ? "" : `status-${tone}`].filter(Boolean).join(" ")}>{children}</span>;
}
