import { useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Modal } from "./Modal";

describe("Modal", () => {
  it("renders nothing when closed", () => {
    const { container } = render(
      <Modal open={false} onClose={() => {}} title="Title">
        body
      </Modal>,
    );
    expect(container).toBeEmptyDOMElement();
  });

  it("renders a labelled dialog with title, body and footer when open", () => {
    render(
      <Modal open onClose={() => {}} title="Review" ariaLabel="Review dialog" footer={<button>OK</button>}>
        <p>body content</p>
      </Modal>,
    );
    const dialog = screen.getByRole("dialog", { name: "Review dialog" });
    expect(dialog).toBeInTheDocument();
    expect(screen.getByText("body content")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "OK" })).toBeInTheDocument();
  });

  it("closes on Escape and on the close button", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(
      <Modal open onClose={onClose} title="Review">
        body
      </Modal>,
    );

    await user.click(screen.getByRole("button", { name: "Close dialog" }));
    expect(onClose).toHaveBeenCalledTimes(1);

    await user.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("keeps focus in an input while typing, even with an unstable onClose", async () => {
    // Regression: a fresh onClose arrow on each render must not re-run the
    // focus-on-open effect and steal focus back to the dialog after each letter.
    const user = userEvent.setup();
    function Harness() {
      const [value, setValue] = useState("");
      return (
        <Modal open onClose={() => {}} title="Name">
          <input aria-label="name" value={value} onChange={(e) => setValue(e.target.value)} />
        </Modal>
      );
    }
    render(<Harness />);
    const input = screen.getByLabelText("name");
    await user.click(input);
    await user.keyboard("hello");
    expect(input).toHaveValue("hello");
    expect(input).toHaveFocus();
  });

  // F3c S1 (plan § Design 4): the invoker is captured as the FIRST statement
  // of the opening effect, so it is the exact node focus restores to on
  // close — the bug this fixes (a descendant `autoFocus` grabbing focus
  // during React's commit, before ANY effect — including this one — runs,
  // so the captured node was wrong) is closed by REMOVING every descendant
  // `autoFocus` (this S1 slice: `ToolHostConfirmModal`'s Stay button, now
  // `initialFocusRef`; the palette input stays a known S2 follow-up) rather
  // than by reordering statements here — no effect can out-race React's own
  // commit-phase autoFocus handling.
  it("restores focus to the invoker on close", async () => {
    const user = userEvent.setup();
    function Harness() {
      const [open, setOpen] = useState(false);
      return (
        <div>
          <button type="button" onClick={() => setOpen(true)}>
            Invoker
          </button>
          {open ? (
            <Modal open onClose={() => setOpen(false)} title="Dialog" ariaLabel="Dialog">
              <p>Body</p>
            </Modal>
          ) : null}
        </div>
      );
    }
    render(<Harness />);
    const invoker = screen.getByRole("button", { name: "Invoker" });
    await user.click(invoker);

    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(invoker).toHaveFocus();
  });

  it("focuses initialFocusRef when given, instead of the dialog container", () => {
    function Harness() {
      const confirmRef = useRef<HTMLButtonElement>(null);
      return (
        <Modal
          open
          onClose={() => {}}
          title="Dialog"
          ariaLabel="Dialog"
          initialFocusRef={confirmRef}
          footer={
            <button type="button" ref={confirmRef}>
              Confirm
            </button>
          }
        >
          <p>Body</p>
        </Modal>
      );
    }
    render(<Harness />);
    expect(screen.getByRole("button", { name: "Confirm" })).toHaveFocus();
  });

  it("falls back to the dialog container when no initialFocusRef is given", () => {
    render(
      <Modal open onClose={() => {}} title="Dialog" ariaLabel="Dialog">
        <p>Body</p>
      </Modal>,
    );
    expect(screen.getByRole("dialog")).toHaveFocus();
  });

  it("skips the restore when the previously-focused node is no longer connected", () => {
    function Harness() {
      const [open, setOpen] = useState(true);
      const [showInvoker, setShowInvoker] = useState(true);
      return (
        <div>
          {showInvoker ? <button type="button">Invoker</button> : null}
          <Modal open={open} onClose={() => {}} title="Dialog" ariaLabel="Dialog">
            <button
              type="button"
              onClick={() => {
                // The invoker is removed from the DOM WHILE the dialog is still open.
                setShowInvoker(false);
                setOpen(false);
              }}
            >
              Remove invoker and close
            </button>
          </Modal>
        </div>
      );
    }
    const { container } = render(<Harness />);
    screen.getByRole("button", { name: "Invoker" }).focus();
    fireEvent.click(screen.getByRole("button", { name: "Remove invoker and close" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    // Nothing throws, and no removed node is left as the active element.
    expect(container.querySelector("button")).toBeNull();
    expect(document.activeElement === document.body || document.activeElement === null).toBe(true);
  });

  // The dialog's own × close button is the first DOM tabbable (it sits in
  // the header, before the body/footer), so it — not a body button — is the
  // real wrap target.
  it("Tab wraps forward from the last tabbable to the first (the × close button)", () => {
    render(
      <Modal open onClose={() => {}} title="Dialog" ariaLabel="Dialog" footer={<button type="button">Last</button>}>
        <button type="button">Middle</button>
      </Modal>,
    );
    const last = screen.getByRole("button", { name: "Last" });
    const close = screen.getByRole("button", { name: "Close dialog" });
    last.focus();
    fireEvent.keyDown(last, { key: "Tab" });
    expect(close).toHaveFocus();
  });

  it("Shift+Tab wraps backward from the first tabbable (the × close button) to the last", () => {
    render(
      <Modal open onClose={() => {}} title="Dialog" ariaLabel="Dialog" footer={<button type="button">Last</button>}>
        <button type="button">Middle</button>
      </Modal>,
    );
    const last = screen.getByRole("button", { name: "Last" });
    const close = screen.getByRole("button", { name: "Close dialog" });
    close.focus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(last).toHaveFocus();
  });

  it("containment skips what a browser would never Tab to: hidden inputs, hidden/inert subtrees, disabled fieldsets, closed details, negative tabindex — but not aria-hidden", () => {
    render(
      <Modal
        open
        onClose={() => {}}
        title="Dialog"
        ariaLabel="Dialog"
        footer={
          <>
            <button type="button">Visible last</button>
            <input type="hidden" />
            <div hidden>
              <button type="button">Under hidden</button>
            </div>
            <fieldset disabled>
              <legend>
                <button type="button">In legend</button>
              </legend>
              <button type="button">Under disabled fieldset</button>
            </fieldset>
            <details>
              <summary>Summary</summary>
              <button type="button">Under closed details</button>
            </details>
            <div aria-hidden="true">
              <button type="button">Under aria-hidden</button>
            </div>
            <div style={{ display: "none" }}>
              <button type="button">Under display none</button>
            </div>
            <button type="button" tabIndex={-1}>
              Negative tabindex
            </button>
          </>
        }
      >
        <button type="button">Middle</button>
      </Modal>,
    );
    const close = screen.getByRole("button", { name: "Close dialog" });
    // The aria-hidden button IS a browser tab stop (aria-hidden hides from
    // the accessibility tree, not from Tab), so it is the real last tabbable;
    // the legend's button is reachable inside a disabled fieldset.
    const realLast = screen.getByRole("button", { name: "Under aria-hidden", hidden: true });
    realLast.focus();
    fireEvent.keyDown(realLast, { key: "Tab" });
    expect(close).toHaveFocus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(realLast).toHaveFocus();
  });

  it("a <summary> is a real tab stop and can be the dialog's last one; its closed details' content is not", () => {
    render(
      <Modal
        open
        onClose={() => {}}
        title="Dialog"
        ariaLabel="Dialog"
        footer={
          <details>
            <summary>Summary last</summary>
            <button type="button">Under closed details</button>
          </details>
        }
      >
        <button type="button">Middle</button>
      </Modal>,
    );
    const summary = screen.getByText("Summary last");
    const close = screen.getByRole("button", { name: "Close dialog" });
    summary.focus();
    fireEvent.keyDown(summary, { key: "Tab" });
    expect(close).toHaveFocus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(summary).toHaveFocus();
  });

  it("a disabled fieldset's controls never form the containment boundary", () => {
    render(
      <Modal
        open
        onClose={() => {}}
        title="Dialog"
        ariaLabel="Dialog"
        footer={
          <>
            <button type="button">Real last</button>
            <fieldset disabled>
              <button type="button">Disabled by fieldset</button>
            </fieldset>
          </>
        }
      >
        <button type="button">Middle</button>
      </Modal>,
    );
    const realLast = screen.getByRole("button", { name: "Real last" });
    const close = screen.getByRole("button", { name: "Close dialog" });
    close.focus();
    fireEvent.keyDown(close, { key: "Tab", shiftKey: true });
    expect(realLast).toHaveFocus();
  });
});
