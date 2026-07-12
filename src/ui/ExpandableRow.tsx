import { ChevronRight } from "lucide-react";
import type { KeyboardEvent, ReactNode } from "react";

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
  function toggleFromKeyboard(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onToggle();
    }
  }

  // The disclosure chevron is rendered by the primitive so every consumer gets
  // a consistent, non-ARIA-only affordance (ADR 0076 D9). The lucide
  // ChevronRight (▸) rotates to ▾ via the `expandable-row-open` modifier; it is
  // decorative because the article already exposes aria-expanded.
  const rowClassName = ["expandable-row", isExpanded ? "expandable-row-open" : "", className]
    .filter(Boolean)
    .join(" ");

  const rowArticle = (
    <article
      aria-expanded={isExpanded}
      aria-label={label}
      className={rowClassName}
      onClick={onToggle}
      onKeyDown={toggleFromKeyboard}
      role="button"
      tabIndex={0}
    >
      <span aria-hidden="true" className="expandable-row-chevron">
        <ChevronRight size={15} />
      </span>
      <span className="expandable-row-content">{children}</span>
    </article>
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
