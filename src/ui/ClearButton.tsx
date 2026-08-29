import { X } from "lucide-react";
import type { MouseEventHandler } from "react";

export type ClearButtonProps = {
  label: string;
  onClick: () => void;
  onMouseDown?: MouseEventHandler<HTMLButtonElement>;
  title?: string;
};

// A field's clear affordance is a filter reset (`data-action-kind="control"`
// for the action-inventory guard); it keeps its own compact styling rather
// than a `Button` variant.
export function ClearButton({ label, onClick, onMouseDown, title }: ClearButtonProps) {
  return (
    <button
      aria-label={label}
      className="field-clear-button"
      data-action-kind="control"
      onClick={onClick}
      onMouseDown={onMouseDown ?? ((event) => event.preventDefault())}
      title={title ?? label}
      type="button"
    >
      <X size={13} />
    </button>
  );
}
