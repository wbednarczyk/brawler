import type { ReactNode } from "react";

export type ActionRowProps = {
  children: ReactNode;
  ariaLabel?: string;
  className?: string;
};

export function ActionRow({ ariaLabel, children, className }: ActionRowProps) {
  return (
    <div
      aria-label={ariaLabel}
      className={["ui-action-row", className].filter(Boolean).join(" ")}
    >
      {children}
    </div>
  );
}
