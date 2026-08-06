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

// Feedback policy (ADR 0076 Decision 5, narrowed by ADR 0097): a toast is
// TRANSIENT feedback for a direct user action — the undo-vs-confirm contract
// (a reversible destroy runs immediately and offers `Cofnij` here instead of a
// blocking native dialog), an import applied, a refresh finished. Bottom-left
// queue, auto-dismiss after 6s (paused while hovered), at most 3 stacked
// toasts, each a `role="status"` live region. Ambient/system attention NEVER
// renders here — it lives in the Today stream + the sidebar badge (ADR 0097);
// the production consumer allowlist in `toastConsumers.test.ts` guards the
// class. Strings arrive already translated — the primitive is locale-agnostic
// so it can mount above LocaleContext.

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

export type ToastProviderProps = {
  children: ReactNode;
};

export function ToastProvider({ children }: ToastProviderProps) {
  const [toasts, setToasts] = useState<ToastRecord[]>([]);

  const dismiss = useCallback((id: string) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const show = useCallback((toast: ToastInput) => {
    const id = `toast-${(toastCounter += 1)}`;
    // MAX_STACK caps the queue: newest last, oldest dropped.
    setToasts((current) => [...current, { ...toast, id }].slice(-MAX_STACK));
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
    if (paused) {
      return;
    }
    const timer = window.setTimeout(() => onDismissRef.current(toast.id), duration);
    return () => window.clearTimeout(timer);
    // Re-arm the timer whenever hover state flips (leave restarts the window).
  }, [toast.id, duration, paused]);

  const toneClass = toast.tone && toast.tone !== "neutral" ? ` ui-toast-${toast.tone}` : "";

  function handleAction() {
    toast.onAction?.();
    onDismissRef.current(toast.id);
  }

  return (
    <li
      className={`ui-toast${toneClass}`}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      {/* `role="status"` is not an ARIA-in-HTML-allowed role on `<li>` (axe
          `aria-allowed-role`/`list`), so the live-region role sits on this
          inner wrapper instead of the list item. `ui-toast-live` is
          `display: contents` so it stays visually transparent — the li keeps
          doing the actual flex layout of the message/action row. Polite by
          design: a toast is ambient feedback for the user's own action. */}
      <div className="ui-toast-live" role="status">
        <span className="ui-toast-message">{toast.message}</span>
        {toast.actionLabel ? (
          <button className="ui-toast-action" type="button" onClick={handleAction}>
            {toast.actionLabel}
          </button>
        ) : null}
      </div>
    </li>
  );
}
