import { X } from "lucide-react";
import type { MouseEventHandler } from "react";

export type ClearButtonProps = {
  label: string;
  onClick: () => void;
  onMouseDown?: MouseEventHandler<HTMLButtonElement>;
  title?: string;
};

export function ClearButton({ label, onClick, onMouseDown, title }: ClearButtonProps) {
  return (
    <button
      aria-label={label}
      className="field-clear-button"
      onClick={onClick}
      onMouseDown={onMouseDown ?? ((event) => event.preventDefault())}
      title={title ?? label}
      type="button"
    >
      <X size={13} />
    </button>
  );
}
