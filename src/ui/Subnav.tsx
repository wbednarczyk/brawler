import type { ReactNode } from "react";

export type SubnavItem<Id extends string> = {
  id: Id;
  icon?: ReactNode;
  label: ReactNode;
};

type SubnavProps<Id extends string> = {
  activeId: Id;
  ariaLabel: string;
  className?: string;
  items: Array<SubnavItem<Id>>;
  onSelect: (id: Id) => void;
};

export function Subnav<Id extends string>({
  activeId,
  ariaLabel,
  className,
  items,
  onSelect,
}: SubnavProps<Id>) {
  return (
    <nav className={["ui-subnav", className].filter(Boolean).join(" ")} aria-label={ariaLabel}>
      {items.map((item) => (
        <button
          className={activeId === item.id ? "ui-subnav-item ui-subnav-item-active" : "ui-subnav-item"}
          data-action-kind="control"
          key={item.id}
          onClick={() => onSelect(item.id)}
          type="button"
        >
          {item.icon}
          <span>{item.label}</span>
        </button>
      ))}
    </nav>
  );
}
