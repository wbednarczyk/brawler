import { describe, expect, it } from "vitest";
import { appShortcutReferenceItems, resolveAppShortcutReferenceItems } from "./shortcuts";

// F4c S2 (ADR 0108 amendment, sol re-review item C): `app.openNotebooks` /
// `notebook.editSelected` / `notebook.saveCurrent` retire outright (no no-op
// stub) — this pins that a persisted `shortcutBindings` entry for one of
// them (left over from an older settings row) is silently ignored: the
// resolver only maps over the CURRENT static registry
// (`appShortcutReferenceItems`), so an unknown id neither crashes nor
// renders a ghost row in Settings → Keyboard shortcuts (ShortcutSettings.tsx
// iterates the resolved list, never `Object.keys(shortcutBindings)`).
// F4c S3 (contract § Decisions #4, sol R2 amendment): Ctrl+4 moves from the
// retired Notebooks-global screen (F4c S2) to Research.
describe("app.openResearch — default binding", () => {
  it("defaults to Ctrl+4", () => {
    const item = appShortcutReferenceItems.find((entry) => entry.id === "app.openResearch");
    expect(item?.defaultBinding).toEqual({ ctrlKey: true, key: "4" });
  });
});

describe("resolveAppShortcutReferenceItems — unknown persisted ids", () => {
  it("ignores a persisted binding for a retired shortcut id", () => {
    const shortcutBindings = {
      "app.openNotebooks": { key: "4", ctrlKey: true, disabled: false },
      "notebook.editSelected": { key: "E", ctrlKey: true, disabled: false },
      "notebook.saveCurrent": { key: "S", ctrlKey: true, disabled: false },
    };

    const resolved = resolveAppShortcutReferenceItems(shortcutBindings);

    expect(resolved).toHaveLength(appShortcutReferenceItems.length);
    expect(resolved.some((item) => (item.id as string) === "app.openNotebooks")).toBe(false);
    expect(resolved.some((item) => (item.id as string) === "notebook.editSelected")).toBe(false);
    expect(resolved.some((item) => (item.id as string) === "notebook.saveCurrent")).toBe(false);
  });
});
