import { forwardRef, type ReactNode, type SelectHTMLAttributes } from "react";

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

// Forwards a ref to the underlying <select> for imperative focus.
export const SelectField = forwardRef<HTMLSelectElement, SelectFieldProps>(function SelectField(
  { children, label, ...props },
  ref,
) {
  return (
    <label className="ui-select-field">
      {label}
      <select ref={ref} {...props}>
        {children}
      </select>
    </label>
  );
});
