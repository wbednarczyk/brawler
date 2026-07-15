import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

import { ClearButton } from "./ClearButton";

// Feedback policy (ADR 0076 Decision 5, extended by ADR 0068 Decision 1). The
// Toast surface backs the undo-vs-confirm contract: a reversible destroy runs
// immediately and offers a `Cofnij` action here instead of a blocking native
// dialog. Bottom-left queue, auto-dismiss after 6s (paused while hovered), at
// most 3 stacked transient toasts, each toast a `role="status"` live region.
// Strings arrive already translated — the primitive is locale-agnostic so it
// can mount above LocaleContext.
//
// ADR 0068 adds a `persistent` variant for attention events (fired alerts,
// signals worth a look): no auto-dismiss, an explicit dismiss control, and an
// optional click-through action to jump to the evidence. Persistent toasts
// use `role="alert"` (assertive) instead of `role="status"` (polite) — they
// represent something the user is meant to act on, not ambient async
// feedback, so they should interrupt an in-progress screen-reader announcement
// rather than wait politely. `dismissLabel` defaults to the English literal
// "Dismiss" like other primitive defaults (see `InlineConfirm`'s
// `cancelLabel`); callers pass a translated `text("Dismiss")` for the real UI.

const AUTO_DISMISS_MS = 6000;
const MAX_STACK = 3;

export type ToastTone = "neutral" | "positive" | "caution" | "negative";

export type ToastInput = {
  message: string;
  tone?: ToastTone;
  actionLabel?: string;
  onAction?: () => void;
  /** Override the auto-dismiss window (ms). Defaults to 6000. */
  durationMs?: number;
  /**
   * Attention-event variant (ADR 0068): no auto-dismiss, an explicit dismiss
   * button, `role="alert"` instead of `role="status"`, and exempt from
   * MAX_STACK eviction by transient toast churn.
   */
  persistent?: boolean;
  /** Accessible label + visible title for the persistent dismiss button. Defaults to "Dismiss". */
  dismissLabel?: string;
  /**
   * Fired when the toast is dismissed via its own explicit dismiss control
   * (ADR 0068 T4) — lets a persistent attention toast sync the dismissal back
   * to its domain store (e.g. `dismissAttentionEvent`). Not called on
   * `onAction` click-through (that path already leaves the evidence handler
   * to decide what "resolved" means) or on auto-dismiss (transient toasts
   * only, which carry no domain state to sync).
   */
  onDismiss?: () => void;
};

type ToastRecord = ToastInput & { id: string };

export type ToastApi = {
  show: (toast: ToastInput) => string;
  dismiss: (id: string) => void;
};

const ToastContext = createContext<ToastApi | null>(null);

export function useToast(): ToastApi {
  const api = useContext(ToastContext);
  if (!api) {
    throw new Error("useToast must be used within a <ToastProvider>");
  }
  return api;
}

let toastCounter = 0;

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<ToastRecord[]>([]);

  const dismiss = useCallback((id: string) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const show = useCallback((toast: ToastInput) => {
    const id = `toast-${(toastCounter += 1)}`;
    setToasts((current) => {
      const next = [...current, { ...toast, id }];
      // MAX_STACK caps the TRANSIENT queue only (newest last, oldest
      // dropped). Persistent toasts (ADR 0068 attention events) are exempt:
      // they represent something the user must still act on, so a burst of
      // transient async feedback must never silently evict them. Filter
      // `next` (not the two sublists concatenated) to preserve original
      // insertion order in the rendered stack.
      const persistentIds = new Set(next.filter((t) => t.persistent).map((t) => t.id));
      const keptTransientIds = new Set(
        next
          .filter((t) => !t.persistent)
          .slice(-MAX_STACK)
          .map((t) => t.id),
      );
      return next.filter((t) => persistentIds.has(t.id) || keptTransientIds.has(t.id));
    });
    return id;
  }, []);

  const api = useMemo<ToastApi>(() => ({ show, dismiss }), [show, dismiss]);

  return (
    <ToastContext.Provider value={api}>
      {children}
      <ToastViewport toasts={toasts} onDismiss={dismiss} />
    </ToastContext.Provider>
  );
}

function ToastViewport({
  toasts,
  onDismiss,
}: {
  toasts: ToastRecord[];
  onDismiss: (id: string) => void;
}) {
  if (toasts.length === 0) {
    return null;
  }
  return (
    <ol className="ui-toast-viewport">
      {toasts.map((toast) => (
        <ToastItem key={toast.id} toast={toast} onDismiss={onDismiss} />
      ))}
    </ol>
  );
}

function ToastItem({
  toast,
  onDismiss,
}: {
  toast: ToastRecord;
  onDismiss: (id: string) => void;
}) {
  const [paused, setPaused] = useState(false);
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  const duration = toast.durationMs ?? AUTO_DISMISS_MS;

  useEffect(() => {
    // Persistent toasts (ADR 0068) never auto-dismiss — only the explicit
    // dismiss button or the click-through action removes them.
    if (paused || toast.persistent) {
      return;
    }
    const timer = window.setTimeout(() => onDismissRef.current(toast.id), duration);
    return () => window.clearTimeout(timer);
    // Re-arm the timer whenever hover state flips (leave restarts the window).
  }, [toast.id, duration, paused, toast.persistent]);

  const toneClass = toast.tone && toast.tone !== "neutral" ? ` ui-toast-${toast.tone}` : "";
  const persistentClass = toast.persistent ? " ui-toast-persistent" : "";

  function handleAction() {
    toast.onAction?.();
    onDismissRef.current(toast.id);
  }

  return (
    <li
      className={`ui-toast${toneClass}${persistentClass}`}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      {/* `role="alert"` is not an ARIA-in-HTML-allowed role on `<li>` (axe
          `aria-allowed-role`/`list`), so the live-region role sits on this
          inner wrapper instead of the list item. `ui-toast-live` is
          `display: contents` so it stays visually transparent — the li keeps
          doing the actual flex layout of the message/action/dismiss row.
          Persistent attention toasts (ADR 0068) are assertive (role="alert"):
          they represent something the user must act on, so they interrupt
          rather than politely queue behind other announcements. Transient
          toasts stay role="status" (polite ambient feedback). */}
      <div className="ui-toast-live" role={toast.persistent ? "alert" : "status"}>
        <span className="ui-toast-message">{toast.message}</span>
        {toast.actionLabel ? (
          <button className="ui-toast-action" type="button" onClick={handleAction}>
            {toast.actionLabel}
          </button>
        ) : null}
        {toast.persistent ? (
          <ClearButton
            label={toast.dismissLabel ?? "Dismiss"}
            onClick={() => {
              toast.onDismiss?.();
              onDismissRef.current(toast.id);
            }}
          />
        ) : null}
      </div>
    </li>
  );
}
