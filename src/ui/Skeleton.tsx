// Loading placeholder (ADR 0076 — feedback surfaces). A shimmer block stands in
// for content while an existing loading state resolves. Two variants: `block`
// (a single bar, sized by CSS/utility class) and `list-row` (repeated rows that
// echo a list). The shimmer is disabled under prefers-reduced-motion (CSS).
// Announced once as a polite live region so assistive tech reports "loading".

export type SkeletonProps = {
  variant?: "block" | "list-row";
  /** Number of rows for the list-row variant (ignored by `block`). */
  count?: number;
  className?: string;
  /** Accessible loading label; defaults to a generic loading announcement. */
  label?: string;
};

export function Skeleton({ variant = "block", count = 3, className, label = "Loading…" }: SkeletonProps) {
  const rows = variant === "list-row" ? Math.max(1, count) : 1;
  const classes = ["ui-skeleton", `ui-skeleton-${variant}`, className].filter(Boolean).join(" ");
  return (
    <div className={classes} role="status" aria-busy="true" aria-label={label}>
      {Array.from({ length: rows }, (_, index) => (
        <span key={index} className="ui-skeleton-bar" aria-hidden="true" />
      ))}
    </div>
  );
}
