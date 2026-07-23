import type { ReactNode } from "react";

export type SegmentedControlProps = {
  ariaLabel: string;
  children: ReactNode;
  className?: string;
};

export type SegmentedControlOptionProps = {
  active?: boolean;
  children: ReactNode;
  onClick: () => void;
};

export function SegmentedControl({ ariaLabel, children, className }: SegmentedControlProps) {
  return (
    <div className={["segmented-control", className].filter(Boolean).join(" ")} role="group" aria-label={ariaLabel}>
      {children}
    </div>
  );
}

export function SegmentedControlOption({ active = false, children, onClick }: SegmentedControlOptionProps) {
  return (
    <button
      className={active ? "segment-active" : undefined}
      // A single-select toggle group: `aria-pressed` exposes which segment is
      // active to assistive tech (the active state was previously class-only).
      aria-pressed={active}
      onClick={onClick}
      type="button"
    >
      {children}
    </button>
  );
}
