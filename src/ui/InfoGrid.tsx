import type { ReactNode } from "react";

export type InfoGridItem = {
  label: ReactNode;
  value: ReactNode;
  valueAriaLabel?: string;
};

export type InfoGridProps = {
  ariaLabel?: string;
  className?: string;
  items: InfoGridItem[];
};

export function InfoGrid({ ariaLabel, className, items }: InfoGridProps) {
  return (
    <dl
      aria-label={ariaLabel}
      className={["ui-info-grid", className].filter(Boolean).join(" ")}
    >
      {items.map((item, index) => (
        <div key={infoGridItemKey(item, index)}>
          <dt>{item.label}</dt>
          <dd aria-label={item.valueAriaLabel}>{item.value}</dd>
        </div>
      ))}
    </dl>
  );
}

function infoGridItemKey(item: InfoGridItem, index: number) {
  return typeof item.label === "string" ? item.label : index;
}
