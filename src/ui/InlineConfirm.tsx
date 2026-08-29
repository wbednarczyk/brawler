import type { ReactNode } from "react";
import { ActionButton } from "./ActionButton";
import type { Verb } from "../shared/verbs";

export type InlineConfirmProps = {
  cancelLabel?: string;
  children: ReactNode;
  confirmLabel?: string;
  disabled?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
  /** Fix-C guardrail 2 (sol F4a R1 finding 2): the dictionary verb the
   * Confirm affordance carries in the per-screen action inventory — most
   * `InlineConfirm` call sites guard a removal, so `remove` is the default;
   * pass a different verb (e.g. `restore`-shaped custom copy) when the
   * confirmed action is something else. */
  verb?: Verb;
};

export function InlineConfirm({
  cancelLabel = "Cancel",
  children,
  confirmLabel = "Confirm",
  disabled,
  onCancel,
  onConfirm,
  verb = "remove",
}: InlineConfirmProps) {
  return (
    <div className="inline-confirm" role="group">
      <span>{children}</span>
      <ActionButton
        className="compact-button"
        disabled={disabled}
        onClick={onConfirm}
        variant="primary"
        verb={verb}
      >
        {confirmLabel}
      </ActionButton>
      <ActionButton className="compact-button" kind="control" onClick={onCancel}>
        {cancelLabel}
      </ActionButton>
    </div>
  );
}
