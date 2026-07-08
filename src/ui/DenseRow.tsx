import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

type DenseRowProps = HTMLAttributes<HTMLElement> &
  Pick<ButtonHTMLAttributes<HTMLButtonElement>, "onDoubleClick"> & {
    children: ReactNode;
    className?: string;
    disabled?: boolean;
    interactive?: boolean;
    selected?: boolean;
    unread?: boolean;
    /**
     * Element rendered for the row. Default `"article"` is a non-interactive
     * container — pair it with an inner primary `<button>` when the row also holds
     * secondary controls (a whole-row `role="button"` on an <article> is invalid
     * ARIA and nests interactives). Pass `"button"` for a whole-row click target
     * with no nested controls: a real button is keyboard-operable and axe-clean
     * (ADR 0076 D9), so the consumer drops `role`/`tabIndex`/Enter-Space handling.
     */
    as?: "article" | "button";
  };

export function DenseRow({
  as = "article",
  children,
  className,
  disabled = false,
  interactive = true,
  selected = false,
  unread = false,
  ...props
}: DenseRowProps) {
  const classNames = [
    "ui-dense-row",
    interactive ? "ui-dense-row-interactive" : "",
    as === "button" ? "ui-dense-row-button" : "",
    selected ? "ui-dense-row-selected" : "",
    unread ? "ui-dense-row-unread" : "",
    disabled ? "ui-dense-row-disabled" : "",
    className,
  ]
    .filter(Boolean)
    .join(" ");

  // `aria-current` is a global ARIA attribute valid on any element; the row has
  // no widget role, so `aria-selected` (option/row/tab-only) would be invalid.
  const ariaCurrent = selected ? "true" : undefined;
  const dataSelected = selected ? "true" : undefined;

  if (as === "button") {
    return (
      <button
        type="button"
        aria-disabled={disabled || undefined}
        aria-current={ariaCurrent}
        className={classNames}
        data-selected={dataSelected}
        {...(props as ButtonHTMLAttributes<HTMLButtonElement>)}
      >
        {children}
      </button>
    );
  }

  return (
    <article
      aria-disabled={disabled || undefined}
      aria-current={ariaCurrent}
      className={classNames}
      data-selected={dataSelected}
      {...props}
    >
      {children}
    </article>
  );
}
