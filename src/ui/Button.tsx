import type { ButtonHTMLAttributes, ReactNode } from "react";

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
  danger: "icon-button danger-button",
  action: "action-button",
  ghost: "ghost-button",
};

export function Button({ children, className, icon, variant = "secondary", ...props }: ButtonProps) {
  return (
    <button
      className={[variantClassName[variant], className].filter(Boolean).join(" ")}
      type="button"
      {...props}
    >
      {icon}
      {children}
    </button>
  );
}
