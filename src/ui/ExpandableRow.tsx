import { ChevronRight } from "lucide-react";
import type { KeyboardEvent, ReactNode } from "react";

export type ExpandableRowProps = {
  children: ReactNode;
  className?: string;
  detail?: ReactNode;
  isExpanded: boolean;
  label: string;
  onToggle: () => void;
};

export function ExpandableRow({
  children,
  className,
  detail,
  isExpanded,
  label,
  onToggle,
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

  return (
    <div>
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
      {isExpanded ? detail : null}
    </div>
  );
}
