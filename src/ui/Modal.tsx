import { useEffect, useRef, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

// Centered overlay dialog. Dependency-free: backdrop click and Esc close it,
// focus moves into the dialog on open and is restored on close, and the dialog
// is marked up as an accessible modal. Renders nothing when closed.
//
// Rendered through a portal to <body> so it is never a DOM descendant of the
// surface that launched it (e.g. the feed detail rail). That keeps it free of
// any ancestor containment/overflow rules and any stacking-context surprises —
// the modal sizes to the viewport, not to the launching pane.

export type ModalProps = {
  open: boolean;
  onClose: () => void;
  title: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  ariaLabel?: string;
  className?: string;
  /** Element to focus on open, instead of the dialog container (F3c S1, plan
   * § Design 4). A descendant `autoFocus` must NOT be used for this — it
   * fires during commit, before this component's own effect runs, which is
   * exactly the race that made `previouslyFocused` capture the wrong node. */
  initialFocusRef?: RefObject<HTMLElement | null>;
};

const TABBABLE_SELECTOR =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

// Layout-geometry checks (`offsetParent`, `getClientRects`) are unusable here:
// jsdom (the vitest/browser test harness) never computes layout, so every
// element would read as invisible. Computed style DOES reflect inline/CSS
// `display`/`visibility` in jsdom, so that is the filter instead.
function isVisible(element: HTMLElement): boolean {
  if (element.hidden) return false;
  const style = getComputedStyle(element);
  return style.display !== "none" && style.visibility !== "hidden";
}

export function Modal({ open, onClose, title, children, footer, ariaLabel, className, initialFocusRef }: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previouslyFocused = useRef<HTMLElement | null>(null);

  // Keep the latest onClose without making it an effect dependency. Otherwise an
  // unstable onClose (a fresh arrow on each render of the consumer) re-runs the
  // focus-on-open effect on every render, stealing focus back to the dialog —
  // which makes typing in a field inside the modal lose focus after each letter.
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!open) return;

    // First statement: with no descendant `autoFocus` left in this tree, this
    // is always the true invoker (F3c S1 — previously an `autoFocus` child
    // moved focus during commit, before this effect ran, so this line
    // captured the WRONG node; see Modal.test.tsx).
    previouslyFocused.current = document.activeElement as HTMLElement | null;
    (initialFocusRef?.current ?? dialogRef.current)?.focus();

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;
      const dialog = dialogRef.current;
      if (!dialog) return;
      const tabbables = Array.from(dialog.querySelectorAll<HTMLElement>(TABBABLE_SELECTOR)).filter(isVisible);
      if (tabbables.length === 0) return;
      const first = tabbables[0];
      const last = tabbables[tabbables.length - 1];
      if (event.shiftKey) {
        if (document.activeElement === first || !dialog.contains(document.activeElement)) {
          event.preventDefault();
          last.focus();
        }
      } else if (document.activeElement === last || !dialog.contains(document.activeElement)) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      if (previouslyFocused.current?.isConnected) {
        previouslyFocused.current.focus();
      }
    };
  }, [open, initialFocusRef]);

  if (!open) return null;

  return createPortal(
    <div
      className="ui-modal-overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className={["ui-modal", className].filter(Boolean).join(" ")}
        role="dialog"
        aria-modal="true"
        aria-label={ariaLabel}
        ref={dialogRef}
        tabIndex={-1}
      >
        <div className="ui-modal-header">
          <h3>{title}</h3>
          <button className="ui-modal-close" type="button" aria-label="Close dialog" onClick={onClose}>
            ×
          </button>
        </div>
        <div className="ui-modal-body">{children}</div>
        {footer ? <div className="ui-modal-footer">{footer}</div> : null}
      </div>
    </div>,
    document.body,
  );
}
