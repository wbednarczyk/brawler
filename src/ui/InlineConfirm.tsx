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
  /** The dictionary verb the Confirm affordance carries in the action
   * inventory (e.g. `remove`); without one it is classified as a control. */
  verb?: Verb;
};

export function InlineConfirm({
  cancelLabel = "Cancel",
  children,
  confirmLabel = "Confirm",
  disabled,
  onCancel,
  onConfirm,
  verb,
}: InlineConfirmProps) {
  return (
    <div className="inline-confirm" role="group">
      <span>{children}</span>
      {verb ? (
        <ActionButton className="compact-button" disabled={disabled} onClick={onConfirm} variant="primary" verb={verb}>
          {confirmLabel}
        </ActionButton>
      ) : (
        <ActionButton className="compact-button" disabled={disabled} kind="control" onClick={onConfirm} variant="primary">
          {confirmLabel}
        </ActionButton>
      )}
      <ActionButton className="compact-button" kind="control" onClick={onCancel}>
        {cancelLabel}
      </ActionButton>
    </div>
  );
}
