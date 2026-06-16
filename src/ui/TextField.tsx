import { forwardRef, type InputHTMLAttributes, type ReactNode } from "react";

// App-owned single-line text input, the text-input counterpart to SelectField.
// Consistent border, padding, focus ring, placeholder, and disabled treatment
// behind one boundary so inputs stop looking like raw browser controls. Pass a
// `label` to render the labelled grid layout; omit it for a bare styled input
// (e.g. inside a flex row that already supplies its own label/aria). Forwards a
// ref to the underlying <input> so callers that need imperative focus/blur (e.g.
// registry-lookup forms) can use the primitive instead of a raw element.

export type TextFieldProps = Omit<InputHTMLAttributes<HTMLInputElement>, "className"> & {
  label?: ReactNode;
  className?: string;
  inputClassName?: string;
};

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(function TextField(
  { label, className, inputClassName, ...props },
  ref,
) {
  const input = (
    <input ref={ref} className={["ui-text-input", inputClassName].filter(Boolean).join(" ")} {...props} />
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
