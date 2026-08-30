import { ChevronRight } from "lucide-react";
import type { ReactNode } from "react";

export type ExpandableRowProps = {
  children: ReactNode;
  className?: string;
  detail?: ReactNode;
  isExpanded: boolean;
  label: string;
  onToggle: () => void;
  // Interactive controls for the row (e.g. delete / re-run buttons). They render
  // as SIBLINGS of the clickable `role="button"` article, never inside it — a
  // button nested in a button is an axe `nested-interactive` (WCAG 4.1.2)
  // violation. Keep non-interactive trailing content (chips, measured values) in
  // `children`; only real controls belong here.
  actions?: ReactNode;
};

export function ExpandableRow({
  children,
  className,
  detail,
  isExpanded,
  label,
  onToggle,
  actions,
}: ExpandableRowProps) {
  // The disclosure chevron is rendered by the primitive so every consumer gets
  // a consistent, non-ARIA-only affordance (ADR 0076 D9). The lucide
  // ChevronRight (▸) rotates to ▾ via the `expandable-row-open` modifier; it is
  // decorative because the row already exposes aria-expanded.
  const rowClassName = ["expandable-row", isExpanded ? "expandable-row-open" : "", className]
    .filter(Boolean)
    .join(" ");

  // A real `<button>` (F4b S1, #417/a11y class — `role="button"` on an
  // `<article>` is invalid ARIA, same defect `DenseRow`'s `as="button"`
  // exists to avoid): natively keyboard-operable, so no manual
  // tabIndex/onKeyDown. `actions` still renders as a SIBLING (never inside),
  // so this stays free of nested-interactive.
  const rowArticle = (
    <button
      aria-expanded={isExpanded}
      aria-label={label}
      className={rowClassName}
      // Every consumer is a disclosure toggle (ADR 0104 dec 3: an
      // expand/collapse affordance is a filter-like control, never a
      // dictionary-verb command) — classified once here rather than at each
      // call site (F4b S4, Report Season row toggle).
      data-action-kind="control"
      onClick={onToggle}
      type="button"
    >
      <span aria-hidden="true" className="expandable-row-chevron">
        <ChevronRight size={15} />
      </span>
      <span className="expandable-row-content">{children}</span>
    </button>
  );

  return (
    <div className="expandable-row-shell">
      {actions ? (
        <div className="expandable-row-line">
          {rowArticle}
          <span className="expandable-row-actions">{actions}</span>
        </div>
      ) : (
        rowArticle
      )}
      {isExpanded ? detail : null}
    </div>
  );
}
