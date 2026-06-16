import { forwardRef, type ReactNode, type TextareaHTMLAttributes } from "react";

// App-owned multi-line text input, the textarea counterpart to TextField. Consistent border,
// padding, focus ring, placeholder, and disabled treatment behind one boundary. Pass a `label`
// to render the labelled layout; omit it for a bare styled textarea inside a row that supplies
// its own label/aria. Forwards a ref to the underlying <textarea> for imperative focus/blur.

export type TextareaFieldProps = Omit<TextareaHTMLAttributes<HTMLTextAreaElement>, "className"> & {
  label?: ReactNode;
  className?: string;
  textareaClassName?: string;
};

export const TextareaField = forwardRef<HTMLTextAreaElement, TextareaFieldProps>(function TextareaField(
  { label, className, textareaClassName, ...props },
  ref,
) {
  const textarea = (
    <textarea
      ref={ref}
      className={["ui-text-input", "ui-textarea-input", textareaClassName].filter(Boolean).join(" ")}
      {...props}
    />
  );

  if (label === undefined) {
    return className ? <span className={className}>{textarea}</span> : textarea;
  }

  return (
    <label className={["ui-text-field", className].filter(Boolean).join(" ")}>
      {label}
      {textarea}
    </label>
  );
});
