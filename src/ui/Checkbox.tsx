import { type InputHTMLAttributes, type ReactNode } from "react";

// App-owned checkbox: a native checkbox paired with its label behind one
// boundary, so plain checkbox controls share consistent box/label spacing.
// Pass `label` for the text/content after the box and `className` to keep a
// site-specific layout class on the wrapping <label>. All other props
// (`checked`, `onChange`, `disabled`, `aria-label`, …) flow to the <input>.
//
// This is for PLAIN checkboxes. A toggle SWITCH (track + thumb, role="switch")
// or a selectable list row that happens to contain a checkbox are different
// shapes — keep those bespoke (they are documented intentional natives).

export type CheckboxProps = Omit<InputHTMLAttributes<HTMLInputElement>, "type"> & {
  label?: ReactNode;
  className?: string;
};

export function Checkbox({ label, className, ...props }: CheckboxProps) {
  return (
    <label className={["ui-checkbox", className].filter(Boolean).join(" ")}>
      <input type="checkbox" {...props} />
      {label}
    </label>
  );
}
