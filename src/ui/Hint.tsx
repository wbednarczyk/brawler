import type { ReactNode } from "react";

export type HintProps = {
  children: ReactNode;
  className?: string;
};

// Muted helper/hint text under a control or action. Same font size as body
// text, muted color — a consistent home for the per-screen `*-hint` paragraphs.
export function Hint({ children, className }: HintProps) {
  return <p className={["ui-hint", className].filter(Boolean).join(" ")}>{children}</p>;
}
