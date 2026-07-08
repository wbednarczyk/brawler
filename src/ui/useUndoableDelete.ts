import { useCallback } from "react";

import { useToast, type ToastTone } from "./Toast";

// Reversible-destroy orchestration (ADR 0076 Decision 5). The delete runs
// immediately (no blocking dialog); on success a toast offers `Cofnij`, which
// re-creates the entity through the caller-supplied restore path. Callers own
// the local-state updates (onPerformed / onRestored) so the hook stays generic
// across entities. Only wire this for entities a create/update API can faithfully
// re-create — cascading or irreversible actions use InlineConfirm instead.

export type UndoableDeleteConfig = {
  /** Runs the actual delete. Resolves when the backend has removed the entity. */
  perform: () => Promise<unknown>;
  /** Re-creates the entity via an existing create/update API. */
  restore: () => Promise<unknown>;
  /** Toast body, already translated. */
  message: string;
  /** Undo action label, already translated (e.g. "Cofnij"). */
  undoLabel: string;
  /** Local-state update after the delete resolves (optimistic UI removal). */
  onPerformed?: () => void;
  /** Local-state update after a successful restore. */
  onRestored?: () => void;
  /** Surface a delete- or restore-path failure. */
  onError?: (error: unknown) => void;
  tone?: ToastTone;
};

export function useUndoableDelete() {
  const { show } = useToast();

  return useCallback(
    (config: UndoableDeleteConfig) => {
      config
        .perform()
        .then(() => {
          config.onPerformed?.();
          show({
            message: config.message,
            tone: config.tone,
            actionLabel: config.undoLabel,
            onAction: () => {
              config
                .restore()
                .then(() => config.onRestored?.())
                .catch((error) => config.onError?.(error));
            },
          });
        })
        .catch((error) => config.onError?.(error));
    },
    [show],
  );
}
