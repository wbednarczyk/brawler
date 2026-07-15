import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

export type ButtonVariant = "primary" | "secondary" | "minimal" | "icon" | "danger" | "action" | "ghost";

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  icon?: ReactNode;
  variant?: ButtonVariant;
};

const variantClassName: Record<ButtonVariant, string> = {
  primary: "primary-button",
  secondary: "secondary-button",
  minimal: "minimal-button",
  icon: "icon-button",
  // A text-capable danger button (secondary shell + danger color). Icon-only
  // danger actions use variant="icon" with a "danger-button" className so they
  // keep the fixed square icon sizing instead of squeezing a label.
  danger: "secondary-button danger-button",
  action: "action-button",
  ghost: "ghost-button",
};

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { children, className, icon, variant = "secondary", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      className={[variantClassName[variant], className].filter(Boolean).join(" ")}
      // ADR 0081 Q4: stable, semantic hook for scoped interaction-contract
      // helpers (tests/browser/helpers/interactionContracts.ts) — e.g.
      // locating primitive icon buttons for the accessible-name check
      // without falling back to a CSS class selector. Never inferred as
      // "primary action" — callers add data-ux-primary-action explicitly.
      data-ui-button-variant={variant}
      type="button"
      {...props}
    >
      {icon}
      {children}
    </button>
  );
});
