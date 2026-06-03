import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import {
  formatShortcutBinding,
  isEditableShortcutTarget,
  shortcutMatchesEvent,
  useKeyboardShortcuts,
} from "./index";

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
