import { expect } from "vitest";
import type { LocaleCode } from "../shared/locale";
import { localeTag } from "../shared/format/financialValue";

// F4a S1 shared guardrail harnesses (docs/plans/frontend-v2-f4a.md § Shared
// guardrails): generic, screen-agnostic assertions a per-screen contract test
// composes over a rendered root. None of these infer intent from markup shape
// — they read the explicit primitive-level markers (`data-action-kind`,
// `data-ux-primary-action`, `data-empty-kind`) the F4a primitives emit
// (`ActionButton`, `Button`, `EmptyState`).

import { computeAccessibleName } from "dom-accessibility-api";

export type ActionInventoryEntry = { name: string; kind: string };

// The real ARIA accessible name (what a screen reader announces and what
// `getByRole(..., { name })` matches).
function accessibleName(node: Element): string {
  return computeAccessibleName(node).trim();
}

/**
 * Every `button`/`[role=button]` inside `root`, sorted by accessible name
 * (locale-collated — Polish diacritics sort differently from English) then by
 * kind. `kind` comes from `data-action-kind` (`ActionButton`/`Button`'s
 * marker); a button carrying neither is `"unclassified"` — the F4a contract's
 * "anything else rendered as a button is a defect" row.
 */
export function collectActionInventory(root: HTMLElement, locale: LocaleCode): ActionInventoryEntry[] {
  const nodes = root.querySelectorAll('button, [role="button"]');
  const entries: ActionInventoryEntry[] = Array.from(nodes).map((node) => ({
    name: accessibleName(node),
    kind: node.getAttribute("data-action-kind") ?? "unclassified",
  }));
  const collator = localeTag(locale);
  return entries.sort(
    (a, b) => a.name.localeCompare(b.name, collator) || a.kind.localeCompare(b.kind, collator),
  );
}

/**
 * Asserts `root` carries exactly `expected` primary actions
 * (`data-ux-primary-action="true"`, ADR 0081 Q4) — default 1 ("one primary at
 * rest"); pass 0 for a named state documented to have none (an empty state
 * whose invitation action IS the primary still counts as 1).
 */
export function expectSinglePrimary(root: HTMLElement, expected = 1): void {
  const primaries = root.querySelectorAll('[data-ux-primary-action="true"]');
  expect(primaries.length).toBe(expected);
}

/** The `data-empty-kind` of every `EmptyState` rendered inside `root`. */
export function collectEmptyStates(root: HTMLElement): string[] {
  return Array.from(root.querySelectorAll("[data-empty-kind]")).map(
    (node) => node.getAttribute("data-empty-kind") ?? "",
  );
}

/**
 * F4b S1 (decision 5): a demoted button loses `data-ux-primary-action` AND
 * `variant="primary"` TOGETHER — the browser helpers (`interactionContracts.ts`)
 * count the two markers independently, so nothing there catches them landing
 * on two different elements. Asserts every `data-ux-primary-action="true"`
 * element also carries `data-ui-button-variant="primary"`, and vice versa.
 */
export function expectPrimaryMarkerMatchesVariant(root: HTMLElement): void {
  const marked = Array.from(root.querySelectorAll('[data-ux-primary-action="true"]'));
  for (const node of marked) {
    expect(node.getAttribute("data-ui-button-variant")).toBe("primary");
  }
  const primaryVariant = Array.from(root.querySelectorAll('[data-ui-button-variant="primary"]'));
  for (const node of primaryVariant) {
    expect(node.getAttribute("data-ux-primary-action")).toBe("true");
  }
}

/** Sol R2: a consumer guard for `ExpandableRow` — the row renders a real
 * `<button>`, so its summary must stay phrasing content. Call from the
 * screen contract test of every ExpandableRow consumer. */
export function expectPhrasingOnlyExpandableRows(root: HTMLElement): void {
  const rows = Array.from(root.querySelectorAll<HTMLElement>("button.expandable-row"));
  if (rows.length === 0) {
    throw new Error("expectPhrasingOnlyExpandableRows: no rendered ExpandableRow found — the guard would pass vacuously");
  }
  for (const row of rows) {
    const offender = row.querySelector("ul, ol, li, p, div, h1, h2, h3, h4, h5, h6, table");
    if (offender) {
      throw new Error(
        `ExpandableRow summary contains non-phrasing <${offender.tagName.toLowerCase()}> — keep row summaries span-based`,
      );
    }
  }
}
