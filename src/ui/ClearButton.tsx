import { X } from "lucide-react";
import type { MouseEventHandler } from "react";
import { ActionButton } from "./ActionButton";

export type ClearButtonProps = {
  label: string;
  onClick: () => void;
  onMouseDown?: MouseEventHandler<HTMLButtonElement>;
  title?: string;
};

// Fix-C guardrail 2 (sol F4a R1 finding 2): a field's clear affordance is a
// filter reset, not a command — renders through `ActionButton` with
// `kind="control"` so it carries `data-action-kind` like every other dynamic
// button (the per-screen action-inventory contract, `src/test/uxContracts.tsx`
// `collectActionInventory`, would otherwise see it as `"unclassified"`).
export function ClearButton({ label, onClick, onMouseDown, title }: ClearButtonProps) {
  return (
    <ActionButton
      aria-label={label}
      className="field-clear-button"
      kind="control"
      onClick={onClick}
      onMouseDown={onMouseDown ?? ((event) => event.preventDefault())}
      title={title ?? label}
    >
      <X size={13} />
    </ActionButton>
  );
}
