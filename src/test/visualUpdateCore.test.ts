import { existsSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { CATALOG } from "../../tests/browser/visual/catalog.core.mjs";
import {
  allExpectedCells,
  cellFileName,
  diffSnapshots,
  resolveScreen,
} from "../../scripts/ux/visual-update-core.mjs";

describe("visual-update-core", () => {
  it("resolves every catalog id to its owning spec", () => {
    for (const entry of CATALOG) {
      const resolved = resolveScreen(entry.screen);
      expect(resolved.spec).toBe(entry.spec);
      expect(resolved.cells.length).toBeGreaterThan(0);
      expect(resolved.cells.every((cell) => cell.screen === entry.screen)).toBe(true);
    }
  });

  it("throws listing valid ids for an unknown screen", () => {
    expect(() => resolveScreen("does-not-exist")).toThrowError(/does-not-exist/);
    try {
      resolveScreen("does-not-exist");
      expect.unreachable();
    } catch (err) {
      const message = (err as Error).message;
      for (const entry of CATALOG) expect(message).toContain(entry.screen);
    }
  });

  it("maps dark S/M/L and light M cells to baseline PNGs that exist on disk", () => {
    const cases: Array<{ tier: "S" | "M" | "L"; theme: "dark" | "light" }> = [
      { tier: "S", theme: "dark" },
      { tier: "M", theme: "dark" },
      { tier: "L", theme: "dark" },
      { tier: "M", theme: "light" },
    ];
    for (const { tier, theme } of cases) {
      const path = cellFileName({ screen: "basic-info", state: "default", tier, theme });
      expect(existsSync(path)).toBe(true);
    }
  });

  it("every catalog cell (86 today) has an existing baseline file", () => {
    const cells = allExpectedCells();
    expect(cells.length).toBe(86);
    for (const cell of cells) {
      expect(existsSync(cellFileName(cell))).toBe(true);
    }
  });

  it("diffSnapshots flags added, removed, and sibling drift, and passes clean target-only changes", () => {
    const pre = { "/a.png": "hash-a", "/b.png": "hash-b", "/c.png": "hash-c" };
    const dirty = { "/a.png": "hash-a-changed", "/b.png": "hash-b", "/d.png": "hash-d" };
    const dirtyDiff = diffSnapshots(pre, dirty, ["/b.png"]);
    expect(dirtyDiff.added).toEqual(["/d.png"]);
    expect(dirtyDiff.removed).toEqual(["/c.png"]);
    expect(dirtyDiff.changedSiblings).toEqual(["/a.png"]);
    expect(dirtyDiff.missingTarget).toEqual([]);

    const post = { "/a.png": "hash-a", "/b.png": "hash-b-changed", "/c.png": "hash-c" };
    const cleanDiff = diffSnapshots(pre, post, ["/b.png"]);
    expect(cleanDiff.added).toEqual([]);
    expect(cleanDiff.removed).toEqual([]);
    expect(cleanDiff.changedSiblings).toEqual([]);
    expect(cleanDiff.missingTarget).toEqual([]);
  });

  it("diffSnapshots reports a target cell the run failed to regenerate", () => {
    const pre = { "/a.png": "hash-a" };
    const post = { "/a.png": "hash-a" };
    const diff = diffSnapshots(pre, post, ["/a.png", "/target-missing.png"]);
    expect(diff.missingTarget).toEqual(["/target-missing.png"]);
    // A missing target is NOT also a sibling "removal" — one failure, one slot.
    expect(diff.removed).toEqual([]);
  });

  it("diffSnapshots allows a target to (re)appear — rm-first workflow — but flags an added sibling", () => {
    const pre = { "/sibling.png": "hash-s" };
    const post = { "/sibling.png": "hash-s", "/target.png": "hash-t", "/stray.png": "hash-x" };
    const diff = diffSnapshots(pre, post, ["/target.png"]);
    expect(diff.added).toEqual(["/stray.png"]);
    expect(diff.missingTarget).toEqual([]);
  });
});
