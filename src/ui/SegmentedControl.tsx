import type { ButtonHTMLAttributes, ReactNode } from "react";

export type SegmentedControlProps = {
  ariaLabel: string;
  children: ReactNode;
  className?: string;
};

export type SegmentedControlOptionProps = {
  active?: boolean;
  children: ReactNode;
  onClick: () => void;
} & Omit<ButtonHTMLAttributes<HTMLButtonElement>, "className" | "onClick" | "type">;

export function SegmentedControl({ ariaLabel, children, className }: SegmentedControlProps) {
  return (
    <div className={["segmented-control", className].filter(Boolean).join(" ")} role="group" aria-label={ariaLabel}>
      {children}
    </div>
  );
}

// `...rest` forwards e.g. `data-action-kind` (ADR 0104 dec. 3 amendment: a
// segment toggle is a filter/selection, never a command — a caller marks it
// `data-action-kind="control"` per the per-screen action-inventory contract,
// `src/test/uxContracts.tsx` `collectActionInventory`) without every
// consumer re-implementing this button.
export function SegmentedControlOption({ active = false, children, onClick, ...rest }: SegmentedControlOptionProps) {
  return (
    <button
      // `...rest` spreads FIRST (Fix-C guardrail 5, sol F4a R1 finding 6): a
      // caller-supplied `aria-pressed` must never win over the controlled
      // state below it, and `type`/`onClick` stay this primitive's own even
      // though TS already excludes them from `rest`'s type.
      {...rest}
      className={active ? "segment-active" : undefined}
      // A single-select toggle group: `aria-pressed` exposes which segment is
      // active to assistive tech.
      aria-pressed={active}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}
