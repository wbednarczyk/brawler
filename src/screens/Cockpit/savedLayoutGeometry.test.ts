import { describe, it, expect } from "vitest";
import type { SerializedDockview } from "dockview";

import { sanitizeGeometry } from "./DockLayout";

// GUARDRAIL (ADR 0045 harvest, ADR 0053 decision 3A) — the class this file owns:
//
//   Removing a cockpit pane kind must NEVER leave a ghost pane in a user's
//   saved layout.
//
// A saved `cockpit_layouts` row stores dockview geometry that names panels by id
// (`follow:<PinnedKind>`). `normalizePinned` and the `openGlobals` filter already
// drop unknown pinned/global kinds when rebuilding panel STATE, but the geometry
// replay (`DockLayout.restore`) must sanitize the stored tree too: `DockPanel`
// renders `null` when no content is registered for a panel id, so an unfiltered
// removed kind would not crash — it would leave an EMPTY TILE WITH A TAB
// occupying grid space in the user's real dashboard.
//
// Found live: the owner's database held `dashboard:company_gpw_acp` with
// `"views":["follow:review"]` after ADR 0084 removed the `review` pane kind.
//
// If you are here because you just removed a pane kind: you do not need to touch
// this file. It is kind-agnostic on purpose — it asserts that ANY unknown view id
// is stripped and the surviving tree stays valid. Do NOT "fix" a ghost pane by
// bumping DOCKVIEW_VERSION; that discards every user's saved geometry to solve
// one removed kind.

/** A leaf group holding `views`, in dockview's serialized shape. */
function leaf(id: string, views: string[], size = 100) {
  return { type: "leaf" as const, size, data: { id, views, activeView: views[0] } };
}

function branch(children: unknown[], size = 200) {
  return { type: "branch" as const, size, data: children };
}

function panelsMap(ids: string[]) {
  return Object.fromEntries(ids.map((id) => [id, { id, contentComponent: "default", title: id }]));
}

// The owner's real saved dashboard shape (`dashboard:company_gpw_acp`): four
// company panels, one of which (`follow:review`) is a kind that no longer exists.
function ownerLayout(): SerializedDockview {
  return {
    grid: {
      root: branch([
        leaf("group-1", ["follow:fundamentals"]),
        branch([
          leaf("group-2", ["follow:review"]),
          leaf("group-3", ["follow:coverage"]),
        ]),
        leaf("group-4", ["follow:companyFeed"]),
      ]),
      width: 1600,
      height: 900,
      orientation: "HORIZONTAL",
    },
    panels: panelsMap([
      "follow:fundamentals",
      "follow:review",
      "follow:coverage",
      "follow:companyFeed",
    ]),
    activeGroup: "group-2",
  } as unknown as SerializedDockview;
}

/** Every view id still referenced anywhere in a sanitized geometry. */
function viewIdsIn(layout: SerializedDockview): string[] {
  const found: string[] = [];
  const walk = (node: unknown): void => {
    const n = node as { type?: string; data?: unknown };
    if (n.type === "leaf") {
      found.push(...((n.data as { views: string[] }).views ?? []));
      return;
    }
    if (n.type === "branch") (n.data as unknown[]).forEach(walk);
  };
  walk((layout as unknown as { grid: { root: unknown } }).grid.root);
  return found;
}

/** Leaves carrying no views at all — a ghost pane is exactly this. */
function emptyLeafCount(layout: SerializedDockview): number {
  let count = 0;
  const walk = (node: unknown): void => {
    const n = node as { type?: string; data?: unknown };
    if (n.type === "leaf") {
      if (((n.data as { views: string[] }).views ?? []).length === 0) count += 1;
      return;
    }
    if (n.type === "branch") (n.data as unknown[]).forEach(walk);
  };
  walk((layout as unknown as { grid: { root: unknown } }).grid.root);
  return count;
}

describe("saved cockpit layout geometry — no ghost panes after a pane kind is removed", () => {
  // The currently-registered panel ids. `follow:review` is deliberately absent:
  // it is the removed kind (ADR 0084).
  const known = new Set(["follow:fundamentals", "follow:coverage", "follow:companyFeed"]);

  it("strips a removed pane kind from the owner's real saved dashboard geometry", () => {
    const sanitized = sanitizeGeometry(ownerLayout(), known);
    expect(sanitized).not.toBeNull();

    // The removed kind is gone from the tree AND from the panels map.
    expect(viewIdsIn(sanitized!)).not.toContain("follow:review");
    expect(Object.keys(sanitized!.panels)).not.toContain("follow:review");
  });

  it("keeps every surviving pane", () => {
    const sanitized = sanitizeGeometry(ownerLayout(), known)!;
    const views = viewIdsIn(sanitized);
    for (const id of ["follow:fundamentals", "follow:coverage", "follow:companyFeed"]) {
      expect(views).toContain(id);
      expect(Object.keys(sanitized.panels)).toContain(id);
    }
  });

  it("leaves no empty pane behind (the ghost-pane regression)", () => {
    const sanitized = sanitizeGeometry(ownerLayout(), known)!;
    expect(emptyLeafCount(sanitized)).toBe(0);
  });

  it("collapses a branch left with a single child rather than leaving it malformed", () => {
    // The owner's inner branch held [review, coverage]; dropping review leaves one
    // child, which must collapse into the parent — never a one-child branch and
    // never a dangling empty branch.
    const sanitized = sanitizeGeometry(ownerLayout(), known)!;
    const oneChildBranches: unknown[] = [];
    const walk = (node: unknown): void => {
      const n = node as { type?: string; data?: unknown };
      if (n.type !== "branch") return;
      const children = n.data as unknown[];
      if (children.length <= 1) oneChildBranches.push(n);
      children.forEach(walk);
    };
    walk((sanitized as unknown as { grid: { root: unknown } }).grid.root);
    expect(oneChildBranches).toHaveLength(0);
  });

  it("repoints activeGroup when it named the removed pane's group", () => {
    const sanitized = sanitizeGeometry(ownerLayout(), known)!;
    const groupIds: string[] = [];
    const walk = (node: unknown): void => {
      const n = node as { type?: string; data?: unknown };
      if (n.type === "leaf") {
        groupIds.push((n.data as { id: string }).id);
        return;
      }
      if (n.type === "branch") (n.data as unknown[]).forEach(walk);
    };
    walk((sanitized as unknown as { grid: { root: unknown } }).grid.root);
    // group-2 held only the removed pane, so it is gone; activeGroup must name a
    // group that still exists (or be absent), never a dangling id.
    expect(groupIds).not.toContain("group-2");
    const active = (sanitized as unknown as { activeGroup?: string }).activeGroup;
    if (active !== undefined) expect(groupIds).toContain(active);
  });

  it("preserves the sanitized branch's total size when a sibling is dropped", () => {
    const sanitized = sanitizeGeometry(ownerLayout(), known)!;
    const root = (sanitized as unknown as { grid: { root: { data: unknown[] } } }).grid.root;
    const total = (root.data as { size?: number }[]).reduce((sum, child) => sum + (child.size ?? 0), 0);
    // Original root children summed to 100 + 200 + 100 = 400; the inner branch
    // collapsed but its share must not evaporate.
    expect(total).toBe(400);
  });

  it("returns null when nothing survives, so the caller rebuilds the default", () => {
    const allRemoved = sanitizeGeometry(ownerLayout(), new Set<string>());
    expect(allRemoved).toBeNull();
  });

  it("returns the geometry unchanged when every view id is still known", () => {
    const intact = new Set([
      "follow:fundamentals",
      "follow:review",
      "follow:coverage",
      "follow:companyFeed",
    ]);
    const sanitized = sanitizeGeometry(ownerLayout(), intact)!;
    expect(viewIdsIn(sanitized).sort()).toEqual(viewIdsIn(ownerLayout()).sort());
    expect(emptyLeafCount(sanitized)).toBe(0);
  });

  it("tolerates a malformed geometry instead of throwing", () => {
    expect(sanitizeGeometry(null as unknown as SerializedDockview, known)).toBeNull();
    expect(sanitizeGeometry({} as SerializedDockview, known)).toBeNull();
    expect(
      sanitizeGeometry({ grid: { root: { type: "leaf" } } } as unknown as SerializedDockview, known),
    ).toBeNull();
  });
});
