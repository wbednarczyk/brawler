import type { ReactNode } from "react";

export type ErrorTextProps = {
  children: ReactNode;
  className?: string;
};

// Consistent inline error line. Replaces the ad-hoc `<p className="error-text">`
// repeated across screens; reuses the existing `error-text` styling.
export function ErrorText({ children, className }: ErrorTextProps) {
  return (
    <p className={["error-text", className].filter(Boolean).join(" ")} role="alert">
      {children}
    </p>
  );
}
