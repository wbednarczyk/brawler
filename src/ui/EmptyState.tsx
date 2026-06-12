import type { ReactNode } from "react";

export type EmptyStateProps = {
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  wrapText?: boolean;
};

export function EmptyState({ actions, children, className, wrapText = true }: EmptyStateProps) {
  return (
    <div className={["empty-state", className].filter(Boolean).join(" ")}>
      {wrapText ? <span>{children}</span> : children}
      {actions}
    </div>
  );
}
