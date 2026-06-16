import type { ReactNode } from "react";

export type ErrorTextProps = {
  children: ReactNode;
  className?: string;
  /** Element to render. Defaults to a block `<p>`; use `"span"` for an inline error inside a row. */
  as?: "p" | "span";
};

// Consistent inline error line. Replaces the ad-hoc `<p className="error-text">`
// repeated across screens; reuses the existing `error-text` styling and adds
// `role="alert"` so assistive tech announces it.
export function ErrorText({ children, className, as: Tag = "p" }: ErrorTextProps) {
  return (
    <Tag className={["error-text", className].filter(Boolean).join(" ")} role="alert">
      {children}
    </Tag>
  );
}
