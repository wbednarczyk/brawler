import type { ReactNode } from "react";
import { Button } from "./Button";

export type InlineConfirmProps = {
  cancelLabel?: string;
  children: ReactNode;
  confirmLabel?: string;
  disabled?: boolean;
  onCancel: () => void;
  onConfirm: () => void;
};

export function InlineConfirm({
  cancelLabel = "Cancel",
  children,
  confirmLabel = "Confirm",
  disabled,
  onCancel,
  onConfirm,
}: InlineConfirmProps) {
  return (
    <div className="inline-confirm" role="group">
      <span>{children}</span>
      <Button className="compact-button" disabled={disabled} onClick={onConfirm} variant="primary">
        {confirmLabel}
      </Button>
      <Button className="compact-button" onClick={onCancel}>
        {cancelLabel}
      </Button>
    </div>
  );
}
