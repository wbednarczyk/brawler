import type { ReactNode } from "react";
import { ActionButton, Figure } from "../../ui";

// U7-D density (ADR 0076 D6): Research secondary sections (review queue, questions)
// fold to a clickable count chip below their reveal tier and when the pane is
// short — "Review queue 3" — that expands the section in place (aria-expanded).
// The tier switch is pure CSS (container queries key off `data-fold`); only the
// expand/collapse is React state (surfaced as `data-expanded`). The section body
// stays in the DOM at every tier so its content is always reachable and testable.
type ResearchFoldSectionProps = {
  // The width tier from which the section shows expanded by default: "m" reveals
  // at M+ (>=420px), "l" reveals at L only (>=760px). Below that, and when short,
  // the count chip stands in.
  foldTier: "m" | "l";
  bodyId: string;
  label: string;
  count: number;
  expanded: boolean;
  onToggle: () => void;
  toggleTitle: string;
  children: ReactNode;
};

export function ResearchFoldSection({
  foldTier,
  bodyId,
  label,
  count,
  expanded,
  onToggle,
  toggleTitle,
  children,
}: ResearchFoldSectionProps) {
  return (
    <div className="research-fold" data-fold={foldTier} data-expanded={expanded || undefined}>
      <ActionButton
        aria-controls={bodyId}
        aria-expanded={expanded}
        className="research-fold-chip"
        kind="control"
        onClick={onToggle}
        title={toggleTitle}
      >
        <span aria-hidden="true" className="research-fold-chip-caret">
          {expanded ? "▾" : "▸"}
        </span>
        <span className="research-fold-chip-label">{label}</span>
        <span className="research-fold-chip-count">
          <Figure value={count} />
        </span>
      </ActionButton>
      <div className="research-fold-body" id={bodyId}>
        {children}
      </div>
    </div>
  );
}
