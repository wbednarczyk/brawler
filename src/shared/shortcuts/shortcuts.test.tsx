import { useState } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  formatShortcutBinding,
  isEditableShortcutTarget,
  shortcutMatchesEvent,
  useKeyboardShortcuts,
} from "./index";
import { appShortcutReferenceItems, createAppShortcutDefinitions, type AppShortcutActionMap } from "../../app/shortcuts";

function ShortcutHarness({ onRun }: { onRun: () => void }) {
  useKeyboardShortcuts([
    {
      id: "test.shortcut",
      label: "Test shortcut",
      scope: "app",
      binding: {
        ctrlKey: true,
        key: "K",
      },
      action: onRun,
    },
  ]);

  return (
    <div>
      <button type="button">Plain button</button>
      <input aria-label="Search field" />
      <select aria-label="Filter select">
        <option>All</option>
      </select>
      <label>
        Select row
        <input type="checkbox" />
      </label>
      <div contentEditable aria-label="Note editor" />
    </div>
  );
}

describe("keyboard shortcuts", () => {
  it("matches bindings and formats them for display", () => {
    const event = new KeyboardEvent("keydown", {
      key: "k",
      code: "KeyK",
      ctrlKey: true,
    });
    const shiftedDigitEvent = new KeyboardEvent("keydown", {
      key: "!",
      code: "Digit1",
      shiftKey: true,
    });

    expect(shortcutMatchesEvent({ key: "K", ctrlKey: true }, event)).toBe(true);
    expect(shortcutMatchesEvent({ key: "1", shiftKey: true }, shiftedDigitEvent)).toBe(true);
    expect(shortcutMatchesEvent({ key: "K", altKey: true }, event)).toBe(false);
    expect(formatShortcutBinding({ key: "K", ctrlKey: true })).toBe("Ctrl+K");
  });

  it("detects editable and selection targets that should suppress shortcuts", () => {
    render(<ShortcutHarness onRun={() => undefined} />);

    expect(isEditableShortcutTarget(screen.getByLabelText("Search field"))).toBe(true);
    expect(isEditableShortcutTarget(screen.getByLabelText("Filter select"))).toBe(true);
    expect(isEditableShortcutTarget(screen.getByLabelText("Select row"))).toBe(true);
    expect(isEditableShortcutTarget(screen.getByLabelText("Note editor"))).toBe(true);
    expect(isEditableShortcutTarget(screen.getByRole("button", { name: "Plain button" }))).toBe(false);
  });

  it("registers shortcuts and suppresses them while interacting with fields", () => {
    const onRun = vi.fn();

    render(<ShortcutHarness onRun={onRun} />);

    fireEvent.keyDown(document, { key: "K", code: "KeyK", ctrlKey: true });
    expect(onRun).toHaveBeenCalledTimes(1);

    fireEvent.keyDown(screen.getByLabelText("Search field"), { key: "K", code: "KeyK", ctrlKey: true });
    fireEvent.keyDown(screen.getByLabelText("Filter select"), { key: "K", code: "KeyK", ctrlKey: true });
    fireEvent.keyDown(screen.getByLabelText("Select row"), { key: "K", code: "KeyK", ctrlKey: true });
    fireEvent.keyDown(screen.getByLabelText("Note editor"), { key: "K", code: "KeyK", ctrlKey: true });

    expect(onRun).toHaveBeenCalledTimes(1);
  });
});

// F3d S2 (#133, plan § D4 "one-modal shortcut policy" — harvest of a class
// F3c missed): the global capture-phase dispatcher (`useKeyboardShortcuts`)
// must suppress EVERY registered app shortcut while any `[aria-modal="true"]`
// dialog is open — not a per-shortcut check (`app.focusWorkshop` carried the
// only one before this). No `preventDefault` on the early return: the
// dialog's own bubble-phase Escape and native editing keys stay untouched.

function AppShortcutsHarness({ fired }: { fired: Record<string, number> }) {
  const [modalOpen, setModalOpen] = useState(false);
  const actions = Object.fromEntries(
    appShortcutReferenceItems.map((item) => [
      item.id,
      () => {
        fired[item.id] = (fired[item.id] ?? 0) + 1;
      },
    ]),
  ) as AppShortcutActionMap;
  const definitions = createAppShortcutDefinitions(actions, {});
  useKeyboardShortcuts(definitions);

  return (
    <div>
      <button onClick={() => setModalOpen(true)}>open modal</button>
      {modalOpen ? <div aria-modal="true">dialog</div> : null}
    </div>
  );
}

describe("one-modal shortcut policy (plan § D4)", () => {
  it("suppresses every registered app shortcut while a dialog is open; fires normally without one", () => {
    const fired: Record<string, number> = {};
    render(<AppShortcutsHarness fired={fired} />);

    for (const item of appShortcutReferenceItems) {
      fireEvent.keyDown(document, { ...item.defaultBinding });
    }
    const firedWithoutModal = { ...fired };
    expect(Object.keys(firedWithoutModal).length).toBeGreaterThan(0);

    // Open the modal, reset the tally, replay every binding.
    fireEvent.click(document.querySelector("button")!);
    for (const key of Object.keys(fired)) delete fired[key];

    for (const item of appShortcutReferenceItems) {
      fireEvent.keyDown(document, { ...item.defaultBinding });
    }
    expect(fired).toEqual({});
  });

  it("does not call preventDefault while a dialog is open (native editing/Escape stay live)", () => {
    render(<AppShortcutsHarness fired={{}} />);
    fireEvent.click(document.querySelector("button")!);

    const event = new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true, cancelable: true });
    document.dispatchEvent(event);
    expect(event.defaultPrevented).toBe(false);
  });
});
