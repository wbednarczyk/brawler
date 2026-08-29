import { expect } from "vitest";
import type { LocaleCode } from "../shared/locale";
import { localeTag } from "../shared/format/financialValue";

// F4a S1 shared guardrail harnesses (docs/plans/frontend-v2-f4a.md § Shared
// guardrails): generic, screen-agnostic assertions a per-screen contract test
// composes over a rendered root. None of these infer intent from markup shape
// — they read the explicit primitive-level markers (`data-action-kind`,
// `data-ux-primary-action`, `data-empty-kind`) the F4a primitives emit
// (`ActionButton`, `Button`, `EmptyState`).

export type ActionInventoryEntry = { name: string; kind: string };

// The accessible name a screen-test author would read off the row: an
// explicit `aria-label` first (icon-only actions), falling back to the
// rendered text content. Not a full ARIA accessible-name computation (no
// aria-labelledby/title chaining) — the F4a screens don't need it, and a
// heavier computation would hide a missing label rather than flag it.
function accessibleName(node: Element): string {
  const ariaLabel = node.getAttribute("aria-label")?.trim();
  if (ariaLabel) return ariaLabel;
  return (node.textContent ?? "").trim();
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
