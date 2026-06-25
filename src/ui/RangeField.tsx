import { type InputHTMLAttributes, type ReactNode } from "react";

// App-owned range slider (ADR 0037 primitive-first). Codifies the visual-first
// configuration preference (docs/ui-authoring.md): a labeled native
// `<input type="range">` styled to the night-neon tokens. Pair it with a number
// field bound to the same state for the exact value (two-way), per that rule.
export type RangeFieldProps = Omit<InputHTMLAttributes<HTMLInputElement>, "type"> & {
  label?: ReactNode;
  className?: string;
};

export function RangeField({ label, className, ...props }: RangeFieldProps) {
  return (
    <label className={["ui-range-field", className].filter(Boolean).join(" ")}>
      {label}
      <input type="range" {...props} />
    </label>
  );
}
