import type { ReactNode, SelectHTMLAttributes } from "react";

type FieldRowProps = {
  children: ReactNode;
  className?: string;
};

type SelectFieldProps = SelectHTMLAttributes<HTMLSelectElement> & {
  label: ReactNode;
};

export function FieldRow({ children, className }: FieldRowProps) {
  return <div className={["ui-field-row", className].filter(Boolean).join(" ")}>{children}</div>;
}

export function SelectField({ children, label, ...props }: SelectFieldProps) {
  return (
    <label className="ui-select-field">
      {label}
      <select {...props}>{children}</select>
    </label>
  );
}
