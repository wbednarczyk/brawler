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
  /**
   * Accessible name + hover title for a glyph-only chip (e.g. a bell icon whose
   * text lives only in the label). Pairs with `role="img"` so assistive tech
   * announces the chip as a labelled image rather than reading its icon markup.
   */
  ariaLabel?: string;
  title?: string;
  role?: string;
};

export function StatusChip({
  children,
  className,
  tone = "neutral",
  ariaLabel,
  title,
  role,
}: StatusChipProps) {
  return (
    <span
      className={["ui-status-chip", `ui-status-chip-${tone}`, className].filter(Boolean).join(" ")}
      role={role}
      aria-label={ariaLabel}
      title={title}
    >
      {children}
    </span>
  );
}
