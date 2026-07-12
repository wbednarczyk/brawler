import { forwardRef, type InputHTMLAttributes, type ReactNode } from "react";

// App-owned date input, the `<input type="date">` counterpart to TextField.
// A raw native date input renders unstyled browser chrome (a light box off the
// dark design system — the v0.52 decision-journal composer flagged exactly this);
// this primitive gives it the shared `.ui-text-input` box (token border, padding,
// focus ring) plus `.ui-date-input` polish for the native calendar indicator,
// while the native picker keeps following the theme's inherited `color-scheme`.
// Pass a `label` for the labelled grid layout; omit it for a bare styled input
// (e.g. inside a wrapper that supplies its own label/aria — `NotebookDateField`).
// Forwards a ref to the underlying <input> like the other field primitives.

export type DateFieldProps = Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "type"> & {
  label?: ReactNode;
  className?: string;
  inputClassName?: string;
};

export const DateField = forwardRef<HTMLInputElement, DateFieldProps>(function DateField(
  { label, className, inputClassName, ...props },
  ref,
) {
  const input = (
    <input
      ref={ref}
      className={["ui-text-input", "ui-date-input", inputClassName].filter(Boolean).join(" ")}
      type="date"
      {...props}
    />
  );

  if (label === undefined) {
    return className ? <span className={className}>{input}</span> : input;
  }

  return (
    <label className={["ui-text-field", className].filter(Boolean).join(" ")}>
      {label}
      {input}
    </label>
  );
});
