import { useEffect, useRef, type ReactNode } from "react";
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
};

export function Modal({ open, onClose, title, children, footer, ariaLabel, className }: ModalProps) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const previouslyFocused = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!open) return;

    previouslyFocused.current = document.activeElement as HTMLElement | null;
    dialogRef.current?.focus();

    function onKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.stopPropagation();
        onClose();
      }
    }

    document.addEventListener("keydown", onKeyDown);
    return () => {
      document.removeEventListener("keydown", onKeyDown);
      previouslyFocused.current?.focus?.();
    };
  }, [open, onClose]);

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
