import { expect, type Locator } from "@playwright/test";

// Paint-assertion helpers (G11, dogfooding #5/#10/#11): a `data-*` attribute
// or a DOM position proves nothing about what the user actually sees. #11 set
// `data-document-highlighted="true"` for 4 seconds with no CSS rule painting
// it — the browser spec asserted the attribute and stayed green while the row
// never visibly changed. These helpers assert the rendered computed style
// instead. See docs/testing.md § Frontend test responsibilities and
// docs/ui-authoring.md § Enforcement.

/**
 * Asserts `locator` is visibly marked relative to an unmarked sibling/neutral
 * row: a computed-style difference (background-color, outline, or
 * box-shadow) and the element is actually in the viewport. A
 * `data-*-highlighted`/`-selected` attribute alone is not proof of a visible
 * mark. The reference row is a DOM sibling when one exists (a group of one —
 * e.g. a single document under its period — has none), otherwise another
 * element elsewhere in the page sharing the same first CSS class (the same
 * kind of row, rendered unmarked).
 */
export async function expectVisiblyMarked(locator: Locator): Promise<void> {
  await expect(locator).toBeVisible();
  await expect(locator).toBeInViewport();

  const diff = await locator.evaluate((el) => {
    let neutral = (el.previousElementSibling ?? el.nextElementSibling) as Element | null;
    if (!neutral) {
      const firstClass = el.classList[0];
      if (firstClass) {
        for (const candidate of document.getElementsByClassName(firstClass)) {
          if (candidate !== el) {
            neutral = candidate;
            break;
          }
        }
      }
    }
    if (!neutral) return { hasNeutral: false as const };
    const a = getComputedStyle(el);
    const b = getComputedStyle(neutral);
    return {
      hasNeutral: true as const,
      background: [a.backgroundColor, b.backgroundColor] as [string, string],
      outline: [
        `${a.outlineWidth} ${a.outlineStyle} ${a.outlineColor}`,
        `${b.outlineWidth} ${b.outlineStyle} ${b.outlineColor}`,
      ] as [string, string],
      boxShadow: [a.boxShadow, b.boxShadow] as [string, string],
    };
  });

  if (!diff.hasNeutral) {
    throw new Error(
      "expectVisiblyMarked: no sibling or same-class row elsewhere on the page to compare against",
    );
  }

  const differs =
    diff.background[0] !== diff.background[1] ||
    diff.outline[0] !== diff.outline[1] ||
    diff.boxShadow[0] !== diff.boxShadow[1];

  expect(
    differs,
    "Expected a visible paint difference (background-color/outline/box-shadow) vs. the unmarked " +
      "sibling row — computed style is identical. A data-*-highlighted/-selected attribute alone " +
      "is not proof of a visible mark (dogfooding #11).",
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
