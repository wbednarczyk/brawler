import type { ReactNode } from "react";

export type FilterToolbarProps = {
  ariaLabel: string;
  children: ReactNode;
  className?: string;
};

export function FilterToolbar({ ariaLabel, children, className }: FilterToolbarProps) {
  return (
    <div className={["filter-toolbar", className].filter(Boolean).join(" ")} aria-label={ariaLabel}>
      {children}
    </div>
  );
}
