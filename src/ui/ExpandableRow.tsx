import type { KeyboardEvent, ReactNode } from "react";

export type ExpandableRowProps = {
  children: ReactNode;
  className?: string;
  detail?: ReactNode;
  isExpanded: boolean;
  label: string;
  onToggle: () => void;
};

export function ExpandableRow({
  children,
  className,
  detail,
  isExpanded,
  label,
  onToggle,
}: ExpandableRowProps) {
  function toggleFromKeyboard(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Enter" || event.key === " ") {
      event.preventDefault();
      onToggle();
    }
  }

  return (
    <div>
      <article
        aria-expanded={isExpanded}
        aria-label={label}
        className={className}
        onClick={onToggle}
        onKeyDown={toggleFromKeyboard}
        role="button"
        tabIndex={0}
      >
        {children}
      </article>
      {isExpanded ? detail : null}
    </div>
  );
}
