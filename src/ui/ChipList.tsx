import type { ReactNode } from "react";

export type ChipListProps = {
  ariaLabel: string;
  children: ReactNode;
  className?: string;
};

export function ChipList({ ariaLabel, children, className }: ChipListProps) {
  return (
    <div className={["ui-chip-list", className].filter(Boolean).join(" ")} aria-label={ariaLabel}>
      {children}
    </div>
  );
}
