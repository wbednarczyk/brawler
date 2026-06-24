import { useEffect, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";

// Distraction-free, full-screen "Focus" surface (ADR 0054): a Zen-mode overlay
// for deep reading (a long report-over-report diff) or long-form writing (a
// note/thesis), invoked from anywhere. Modeled on Modal but full-viewport with
// minimal chrome. Esc closes and restores focus; renders nothing when closed.

export type FocusOverlayProps = {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  /** Optional leading eyebrow above the title (e.g. the company ticker). */
  eyebrow?: ReactNode;
  /** Header actions rendered before the close button (e.g. a Save button). */
  actions?: ReactNode;
  ariaLabel?: string;
  className?: string;
};

export function FocusOverlay({
  open,
  onClose,
  title,
  children,
  eyebrow,
  actions,
  ariaLabel,
  className,
}: FocusOverlayProps) {
  const surfaceRef = useRef<HTMLDivElement>(null);
  const previouslyFocused = useRef<HTMLElement | null>(null);

  // Keep the latest onClose without making it an effect dependency, so an
  // unstable arrow from the consumer never re-runs focus-on-open and steals
  // focus mid-typing (the Modal lesson).
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!open) return undefined;

    previouslyFocused.current = document.activeElement as HTMLElement | null;
    surfaceRef.current?.focus();

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.stopPropagation();
        onCloseRef.current();
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previouslyFocused.current?.focus?.();
    };
  }, [open]);

  if (!open) return null;

  return createPortal(
    <div
      className={["ui-focus-overlay", className].filter(Boolean).join(" ")}
      role="dialog"
      aria-modal="true"
      aria-label={ariaLabel}
      ref={surfaceRef}
      tabIndex={-1}
    >
      <div className="ui-focus-overlay-header">
        <div className="ui-focus-overlay-heading">
          {eyebrow ? <span className="eyebrow">{eyebrow}</span> : null}
          <h2>{title}</h2>
        </div>
        <div className="ui-focus-overlay-actions">
          {actions}
          <button
            className="ui-focus-overlay-close"
            type="button"
            aria-label="Exit focus mode"
            onClick={onClose}
          >
            ×
          </button>
        </div>
      </div>
      <div className="ui-focus-overlay-body">{children}</div>
    </div>,
    document.body,
  );
}
