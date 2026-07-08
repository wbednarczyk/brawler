import type { ReactNode } from "react";

// Semantic tone vocabulary (ADR 0076 D3). Meaning-bearing tones map 1:1 onto the
// `--tone-*` design tokens resolved per palette × mode in themes.css:
//   ok→positive · warn→caution · danger→negative · neutral→neutral · accent→agent
// plus the source-trust tones official/media/user for provenance badges.
type StatusChipTone =
  | "neutral"
  | "accent"
  | "ok"
  | "warn"
  | "danger"
  | "official"
  | "media"
  | "user";

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
