import { expect, type Locator } from "@playwright/test";

// Paint-assertion helpers (G11, dogfooding #5/#10/#11): a `data-*` attribute
// or a DOM position proves nothing about what the user actually sees. #11 set
// `data-document-highlighted="true"` for 4 seconds with no CSS rule painting
// it — the browser spec asserted the attribute and stayed green while the row
// never visibly changed. These helpers assert the rendered computed style
// instead. See docs/testing.md § Frontend test responsibilities and
// docs/ui-authoring.md § Enforcement.
//
// Usage: capture the element's style with `captureMarkStyle` BEFORE the
// marking event (the click/navigation that is supposed to highlight it),
// trigger the event, then pass that snapshot to `expectVisiblyMarked` — it
// re-captures the style and fails unless at least one of
// background/outline/box-shadow actually changed on that SAME element. A
// before/after comparison on one element is used rather than an
// unmarked-sibling comparison: a sibling can legitimately differ for
// unrelated reasons (zebra striping, hover, row parity), which let the old
// sibling-diff form pass without the element itself ever having changed.
//
//   const before = await captureMarkStyle(row);
//   await triggerTheHighlight();
//   await expectVisiblyMarked(row, before);

/** Computed style fields a "visibly marked" row is expected to change. */
export interface MarkStyle {
  background: string;
  outline: string;
  boxShadow: string;
}

/** Snapshots the computed background/outline/box-shadow of `locator`. */
export async function captureMarkStyle(locator: Locator): Promise<MarkStyle> {
  return locator.evaluate((el) => {
    const style = getComputedStyle(el);
    return {
      background: style.backgroundColor,
      outline: `${style.outlineWidth} ${style.outlineStyle} ${style.outlineColor}`,
      boxShadow: style.boxShadow,
    };
  });
}

/**
 * Asserts `locator` is visibly marked relative to its OWN state before the
 * marking event: a computed-style difference (background-color, outline, or
 * box-shadow) against the `before` snapshot from `captureMarkStyle`, and the
 * element is actually in the viewport. A `data-*-highlighted`/`-selected`
 * attribute alone is not proof of a visible mark.
 */
export async function expectVisiblyMarked(locator: Locator, before: MarkStyle): Promise<void> {
  await expect(locator).toBeVisible();
  await expect(locator).toBeInViewport();

  const after = await captureMarkStyle(locator);

  const differs =
    after.background !== before.background ||
    after.outline !== before.outline ||
    after.boxShadow !== before.boxShadow;

  expect(
    differs,
    "Expected a visible paint difference (background-color/outline/box-shadow) vs. this element's " +
      "own state before the marking event — computed style is identical. A " +
      "data-*-highlighted/-selected attribute alone is not proof of a visible mark (dogfooding #11).",
  ).toBe(true);
}

/**
 * Asserts `locator` (a sticky column cell) stays opaque and on top while its
 * scrolling ancestor is scrolled horizontally: computed `position: sticky`,
 * an opaque background (alpha 1), and a z-index at or above its scrolling
 * sibling cells' — so scrolled-away content cannot show through or on top of
 * it (dogfooding #5: a sticky first column that wasn't opaque). The caller
 * scrolls the container to `scrollLeft > 0` before calling this.
 */
export async function expectOpaqueSticky(locator: Locator): Promise<void> {
  const result = await locator.evaluate((el) => {
    let scrollAncestor: HTMLElement | null = el.parentElement;
    while (scrollAncestor && scrollAncestor.scrollWidth <= scrollAncestor.clientWidth) {
      scrollAncestor = scrollAncestor.parentElement;
    }
    const scrollLeft = scrollAncestor?.scrollLeft ?? 0;

    const siblingCells = el.parentElement ? Array.from(el.parentElement.children).filter((c) => c !== el) : [];
    const maxSiblingZIndex = siblingCells.reduce((max, cell) => {
      const z = getComputedStyle(cell).zIndex;
      const n = z === "auto" ? 0 : Number(z);
      return Number.isFinite(n) ? Math.max(max, n) : max;
    }, 0);

    const style = getComputedStyle(el);
    const alphaMatch = style.backgroundColor.match(/rgba?\([^)]*,\s*([\d.]+)\)/);
    const alpha = alphaMatch ? Number(alphaMatch[1]) : style.backgroundColor === "transparent" ? 0 : 1;

    return {
      scrollLeft,
      position: style.position,
      alpha,
      zIndex: style.zIndex === "auto" ? 0 : Number(style.zIndex),
      maxSiblingZIndex,
    };
  });

  expect(
    result.scrollLeft,
    "expectOpaqueSticky: scroll the container to scrollLeft > 0 before asserting stickiness",
  ).toBeGreaterThan(0);
  expect(result.position, "expected computed position: sticky").toBe("sticky");
  expect(
    result.alpha,
    `expected an opaque background (alpha 1), got ${result.alpha} — a translucent sticky column lets scrolled content show through (dogfooding #5)`,
  ).toBe(1);
  expect(
    result.zIndex,
    `sticky z-index (${result.zIndex}) must be >= the scrolling sibling cells' z-index (${result.maxSiblingZIndex})`,
  ).toBeGreaterThanOrEqual(result.maxSiblingZIndex);
}

/**
 * Asserts a row action's right edge sits inside the scroller's visible width
 * (`clientWidth`, not `scrollWidth`) — an action outside that bound is
 * clipped or hidden under the scrollbar even though it "exists" in the DOM
 * (dogfooding #10: a row action under the scrollbar).
 */
export async function expectInsideScroller(actionLocator: Locator, scrollerLocator: Locator): Promise<void> {
  const [actionBox, scrollerBox, clientWidth] = await Promise.all([
    actionLocator.boundingBox(),
    scrollerLocator.boundingBox(),
    scrollerLocator.evaluate((el) => el.clientWidth),
  ]);
  if (!actionBox || !scrollerBox) {
    throw new Error("expectInsideScroller: the action or scroller locator is not visible/attached");
  }

  const rightEdge = actionBox.x - scrollerBox.x + actionBox.width;
  expect(
    rightEdge,
    `Action's right edge (${rightEdge.toFixed(1)}px) must be within the scroller's visible width ` +
      `(clientWidth ${clientWidth}px) — outside this bound it is clipped or hidden under the ` +
      "scrollbar (dogfooding #10).",
  ).toBeLessThanOrEqual(clientWidth);
}
