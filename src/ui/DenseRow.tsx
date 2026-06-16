import type { HTMLAttributes, ReactNode } from "react";

type DenseRowProps = HTMLAttributes<HTMLElement> & {
  children: ReactNode;
  className?: string;
  disabled?: boolean;
  interactive?: boolean;
  selected?: boolean;
  unread?: boolean;
};

export function DenseRow({
  children,
  className,
  disabled = false,
  interactive = true,
  selected = false,
  unread = false,
  ...props
}: DenseRowProps) {
  return (
    <article
      aria-disabled={disabled || undefined}
      // `aria-current` is a global ARIA attribute valid on any element; the row
      // has no widget role, so `aria-selected` (option/row/tab-only) would be invalid.
      aria-current={selected ? "true" : undefined}
      className={[
        "ui-dense-row",
        interactive ? "ui-dense-row-interactive" : "",
        selected ? "ui-dense-row-selected" : "",
        unread ? "ui-dense-row-unread" : "",
        disabled ? "ui-dense-row-disabled" : "",
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      data-selected={selected ? "true" : undefined}
      {...props}
    >
      {children}
    </article>
  );
}
