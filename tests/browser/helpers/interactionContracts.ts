import { expect, type Locator, type Page } from "@playwright/test";

// Scoped discoverability / interaction-hierarchy contracts (ADR 0081 Q4).
// Every helper here takes an EXPLICIT surface/action locator supplied by the
// caller — never a whole-page scan — because a multi-pane workspace
// legitimately has many concurrent actions; only a caller who knows which
// decision surface is under test can scope the check correctly. None of
// these helpers infer which information "looks important": the primary
// action is a primitive-level marker the caller sets explicitly
// (`data-ux-primary-action="true"` on a `Button`, ADR 0081 Q4), never a CSS
// class or heuristic guess.

/**
 * Asserts a decision surface renders exactly `max` primary actions, marked
 * with `data-ux-primary-action="true"` (see `src/ui/Button.tsx`) — so a
 * missing marker (metadata dropped, contract not yet applied) reddens just
 * as loudly as a doubled one. `max` is normally 1; pass `max: 0` for a
 * surface documented to have no single primary (e.g. a quiet/empty state).
 *
 * A `max` above 1 is a deliberate multi-primary exemption and requires a
 * non-empty `reason` — pair it with a matching entry in the surface's
 * experience contract (docs/plans/EXPERIENCE-CONTRACT-TEMPLATE.md § 6). An
 * exempted surface is checked as a ceiling (up to `max`) rather than exact,
 * since a genuinely multi-primary surface's count can vary.
 */
export async function expectPrimaryActionCount(
  surface: Locator,
  { max, reason }: { max: number; reason?: string },
): Promise<void> {
  if (max > 1 && !reason?.trim()) {
    throw new Error(
      "expectPrimaryActionCount: a max > 1 exemption requires a non-empty `reason` " +
        "documenting the matching experience-contract entry (ADR 0081 Q4 § 6).",
    );
  }
  const primaries = surface.locator('[data-ux-primary-action="true"]');
  const count = await primaries.count();
  const message =
    `Expected ${reason ? `at most ${max}` : `exactly ${max}`} primary action(s) marked ` +
    `data-ux-primary-action="true" inside the surface${reason ? ` (exempt: ${reason})` : ""}, found ${count}.`;
  if (reason) {
    expect(count, message).toBeLessThanOrEqual(max);
  } else {
    expect(count, message).toBe(max);
  }
}

/**
 * Asserts `action` is already inside `scrollOwner`'s current (unscrolled)
 * visible box — i.e. the primary action for a decision surface is reachable
 * without the user having to scroll to find it.
 */
export async function expectActionBeforeScroll(action: Locator, scrollOwner: Locator): Promise<void> {
  // Settle before measuring: a surface that just re-rendered (e.g. returning
  // from another screen) can momentarily yield a null boundingBox even though
  // the action renders visible a frame later — that race is not the contract
  // under test (flaked under I/O load, 2026-07-14). The contract itself (the
  // action sits inside the UNSCROLLED viewport) is asserted below, unchanged.
  await expect(action).toBeVisible();
  // toBeVisible alone is not enough: a React re-render can swap the DOM node
  // BETWEEN the visibility check and the box read, so boundingBox() hits a
  // momentarily-detached node and yields null (recurred under full-suite CPU
  // load, 2026-07-18, two different projects). Poll until both boxes read
  // non-null — the geometric contract asserted below stays unchanged.
  let ownerBox: Awaited<ReturnType<Locator["boundingBox"]>> = null;
  let actionBox: Awaited<ReturnType<Locator["boundingBox"]>> = null;
  await expect
    .poll(
      async () => {
        [ownerBox, actionBox] = await Promise.all([scrollOwner.boundingBox(), action.boundingBox()]);
        return ownerBox !== null && actionBox !== null;
      },
      {
        message:
          "expectActionBeforeScroll: scrollOwner/action never yielded a stable bounding box (not visible/rendered)",
      },
    )
    .toBe(true);
  const owner = ownerBox!;
  const box = actionBox!;
  const withinScrollport = box.y >= owner.y && box.y + box.height <= owner.y + owner.height;
  expect(
    withinScrollport,
    `Primary action must be visible inside its scrollport before any scroll ` +
      `(action y=${box.y.toFixed(0)}..${(box.y + box.height).toFixed(0)}, ` +
      `scrollport y=${owner.y.toFixed(0)}..${(owner.y + owner.height).toFixed(0)}).`,
  ).toBe(true);
}

/**
 * Asserts that pressing Tab from `locators[0]` visits the rest of `locators`
 * in the given order — the declared Tab sequence for a decision surface.
 */
export async function expectFocusOrder(page: Page, locators: Locator[]): Promise<void> {
  if (locators.length === 0) return;
  await locators[0].focus();
  await expect(locators[0], "expectFocusOrder: first locator did not receive focus").toBeFocused();
  for (let i = 1; i < locators.length; i++) {
    await page.keyboard.press("Tab");
    await expect(locators[i], `expectFocusOrder: focus step ${i} did not land on the declared locator`).toBeFocused();
  }
}

/**
 * Asserts every primitive icon-only button (`data-ui-button-variant="icon"`)
 * inside `surface` carries an accessible name (aria-label, title, or visible
 * text). Scoped, explicit alternative to a whole-page axe scan for reviewing
 * one decision surface's icon actions; axe remains the general
 * accessible-name authority (`expectNoA11yViolations`).
 */
export async function expectNamedIconActions(surface: Locator): Promise<void> {
  const iconButtons = surface.locator('[data-ui-button-variant="icon"]');
  const count = await iconButtons.count();
  const unnamed: string[] = [];
  for (let i = 0; i < count; i++) {
    const button = iconButtons.nth(i);
    const name = await button.evaluate((el) => {
      const ariaLabel = el.getAttribute("aria-label");
      const title = el.getAttribute("title");
      const text = el.textContent?.trim();
      return (ariaLabel || title || text || "").trim();
    });
    if (!name) {
      const outerHtml = await button.evaluate((el) => el.outerHTML.slice(0, 160));
      unnamed.push(outerHtml);
    }
  }
  expect(unnamed, `Icon-only actions missing an accessible name/title:\n${unnamed.join("\n")}`).toEqual([]);
}

/** Asserts the contracted next step remains visible (success must not hide it). */
export async function expectNextStepVisible(locator: Locator): Promise<void> {
  await expect(locator).toBeVisible();
}

/**
 * Asserts `root` renders at most `max` VISIBLE filled elements — a `Button`
 * styled `variant="primary"` (`data-ui-button-variant="primary"`, see
 * `src/ui/Button.tsx`) — at rest (, sol a).
 * Counts only visible ones: a fold that keeps a primary-styled button
 * mounted-but-hidden behind a disclosure (e.g. Alerts' S-tier composer fold)
 * must not read as a second filled element. This is the "one filled element
 * at rest" contract (ADR 0104 dec. 1) checked at the VISUAL/variant level —
 * distinct from `expectPrimaryActionCount`, which checks the semantic
 * `data-ux-primary-action` marker; both should normally agree, and a
 * mismatch between them is itself the class of bug this helper exists to
 * catch (a screen where the styled-filled button and the marked-primary
 * button drift apart, e.g. Alerts before ).
 */
export async function expectFilledAtRest(root: Locator, { max = 1 }: { max?: number } = {}): Promise<void> {
  const filled = root.locator('[data-ui-button-variant="primary"]');
  const count = await filled.count();
  const visibleCount = (
    await Promise.all(Array.from({ length: count }, (_, i) => filled.nth(i).isVisible()))
  ).filter(Boolean).length;
  expect(
    visibleCount,
    `Expected at most ${max} visible filled (variant="primary") element(s) at rest, found ${visibleCount}.`,
  ).toBeLessThanOrEqual(max);
}

/**
 * Asserts `action`'s right edge stays inside `scroller`'s CLIENT box — the
 * content box a scroller actually renders into, excluding a rendered
 * scrollbar's own track — not merely inside its border-box bounding
 * rectangle (dogfooding #10: a right-aligned row action sat flush against
 * `.activity-panel`'s own scrollbar with no reserved gutter, clipping it).
 * `clientWidth` is what excludes the scrollbar track; `boundingBox().width`
 * would not. The formula: `actionBox.right <= scrollerBox.left +
 * scroller.clientWidth − 1`.
 */
export async function expectActionInsideScroller(action: Locator, scroller: Locator): Promise<void> {
  await expect(action).toBeVisible();
  let scrollerBox: Awaited<ReturnType<Locator["boundingBox"]>> = null;
  let actionBox: Awaited<ReturnType<Locator["boundingBox"]>> = null;
  let clientWidth = 0;
  await expect
    .poll(
      async () => {
        [scrollerBox, actionBox, clientWidth] = await Promise.all([
          scroller.boundingBox(),
          action.boundingBox(),
          scroller.evaluate((el) => el.clientWidth),
        ]);
        return scrollerBox !== null && actionBox !== null;
      },
      {
        message:
          "expectActionInsideScroller: scroller/action never yielded a stable bounding box (not visible/rendered)",
      },
    )
    .toBe(true);
  const scrollerRect = scrollerBox!;
  const actionRect = actionBox!;
  const clientRight = scrollerRect.x + clientWidth;
  const actionRight = actionRect.x + actionRect.width;
  expect(
    actionRight,
    `Row action's right edge (${actionRight.toFixed(0)}) must stay inside the scroller's client box ` +
      `(right edge ${clientRight.toFixed(0)} = boundingBox x ${scrollerRect.x.toFixed(0)} + clientWidth ${clientWidth}) ` +
      `— a scrollbar occupying the last ${(scrollerRect.width - clientWidth).toFixed(0)}px must not clip it.`,
  ).toBeLessThanOrEqual(clientRight - 1);
}

/**
 * Owner guardrail (2026-09-07): text never touches a table cell's edge. For
 * every `th`/`td` under `table` the ink of its TEXT NODES (a Range per node,
 * so stretched wrappers like a `width: 100%` button don't count) stays at
 * least `minInset` px inside the cell's edges — left/right for horizontal
 * text, top/bottom for a vertical writing mode. Visually hidden text is
 * skipped; an ellipsized cell (content wider than its box) checks only its
 * leading edge, `text-overflow` already keeps the ellipsis inside the padding.
 */
export async function expectTableCellTextInset(table: Locator, minInset = 6): Promise<void> {
  const offenders = await table.evaluate((root, min) => {
    const bad: string[] = [];
    for (const cell of Array.from(root.querySelectorAll<HTMLElement>("th, td"))) {
      const cellStyle = getComputedStyle(cell);
      if (cellStyle.display === "none" || cellStyle.visibility === "hidden") continue;
      const walker = document.createTreeWalker(cell, NodeFilter.SHOW_TEXT);
      let ink: DOMRect | null = null;
      let vertical = false;
      let ellipsized = cell.scrollWidth > cell.clientWidth + 1;
      for (let node = walker.nextNode(); node; node = walker.nextNode()) {
        if (!node.textContent?.trim()) continue;
        const parent = node.parentElement as HTMLElement;
        if (parent.closest(".visually-hidden")) continue;
        const style = getComputedStyle(parent);
        if (style.writingMode.startsWith("vertical")) vertical = true;
        for (let el: HTMLElement | null = parent; el && el !== cell; el = el.parentElement) {
          if (el.scrollWidth > el.clientWidth + 1 && getComputedStyle(el).overflowX !== "visible") ellipsized = true;
        }
        const range = document.createRange();
        range.selectNodeContents(node);
        const rect = range.getBoundingClientRect();
        if (rect.width === 0 && rect.height === 0) continue;
        ink = ink
          ? new DOMRect(
              Math.min(ink.left, rect.left),
              Math.min(ink.top, rect.top),
              Math.max(ink.right, rect.right) - Math.min(ink.left, rect.left),
              Math.max(ink.bottom, rect.bottom) - Math.min(ink.top, rect.top),
            )
          : rect;
      }
      if (!ink) continue;
      const box = cell.getBoundingClientRect();
      const lead = vertical ? ink.top - box.top : ink.left - box.left;
      const trail = vertical ? box.bottom - ink.bottom : box.right - ink.right;
      if (lead < min || (!ellipsized && trail < min)) {
        bad.push(
          `${cell.tagName.toLowerCase()}.${cell.className || "-"} "${cell.textContent?.trim().slice(0, 24)}" lead=${lead.toFixed(1)} trail=${trail.toFixed(1)}`,
        );
      }
    }
    return bad;
  }, minInset);
  expect(offenders, `cells whose text touches an edge (< ${minInset}px inset):\n${offenders.join("\n")}`).toEqual([]);
}
